mod acquisition;
mod boilerplate;
mod extractor_config;
mod metrics;
mod runner;
mod scores;
mod web;
mod worker;

use extractor_config::ExtractorStates;

use std::path::PathBuf;
use std::process::exit;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().nth(1).as_deref() == Some("--extract-worker") {
        return crate::worker::worker_main().map_err(Into::into);
    }

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 && args[1] == "--server" {
        let port: u16 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(3000);
        let data_dir: PathBuf = args.get(3).map_or_else(|| "data".into(), PathBuf::from);
        if let Err(e) = web::start_server(port, data_dir).await {
            eprintln!("Server error: {e}");
            exit(1);
        }
    } else if args.len() > 1 {
        let (extractor, mut method, input) = parse_cli_args(&args[1..])?;
        let current_dir = std::env::current_dir()?;
        let out_dir = current_dir.join("out");

        let (html, url) = if std::path::Path::new(&input).is_file() {
            if method != acquisition::Acquisition::Http {
                return Err("browser acquisition requires an HTTP(S) URL, not a file".into());
            }
            method = acquisition::Acquisition::File;
            let html = std::fs::read_to_string(&input)?;
            let canonical = std::fs::canonicalize(&input)?;
            let url = url::Url::from_file_path(canonical)
                .map_err(|()| std::io::Error::other("Cannot represent input file as URL"))?;
            (html, url)
        } else {
            let url = url::Url::parse(&input)?;
            let html = method.acquire(&url)?;
            (html, url)
        };

        if out_dir.exists() {
            std::fs::remove_dir_all(&out_dir)?;
        }
        std::fs::create_dir_all(&out_dir)?;

        let html_file = out_dir.join("html.html");
        std::fs::write(&html_file, &html)?;
        println!("Acquisition: {method:?}, HTML Size (bytes): {}", html.len());

        let mut runner = runner::Runner::new(out_dir, html);
        let states = ExtractorStates::default();
        let mut names: Vec<_> = if let Some(extractor) = extractor.as_deref() {
            let known = states.states.contains_key(extractor);
            #[cfg(feature = "reader-lm-api")]
            let known = known || extractor == "reader-lm-api";
            if !known {
                return Err(format!("unknown extractor: {extractor}").into());
            }
            vec![extractor.to_string()]
        } else {
            states.states.keys().cloned().collect()
        };
        names.sort();
        for name in names {
            runner.run_isolated(&name, &name, url.as_str(), &states);
        }
        #[cfg(feature = "reader-lm-api")]
        if extractor.is_none() {
            runner.run_isolated("reader-lm-api", "reader-lm-api", url.as_str(), &states);
        }

        println!("{}", runner.into_table());
        println!("Remember to check the output files to make sure they have parsed the information you expect!");
    } else {
        eprintln!("Usage:");
        eprintln!("  {} <url_or_file>     Run all extractors", args[0]);
        eprintln!(
            "  {} --extractor <name> <url_or_file>     Run one extractor",
            args[0]
        );
        eprintln!("  {} --acquire {{http|lightpanda|cloakbrowser}} [--extractor NAME] <url>  One acquisition", args[0]);
        eprintln!("  {} --server [port]   Start web server", args[0]);
        eprintln!();
        eprintln!("Web server mode: {} --server 3000", args[0]);
    }
    Ok(())
}

fn parse_cli_args(
    arguments: &[String],
) -> Result<(Option<String>, acquisition::Acquisition, String), Box<dyn std::error::Error>> {
    let mut extractor: Option<String> = None;
    let mut method = acquisition::Acquisition::Http;
    let mut input = None;
    let mut words = arguments.iter();
    while let Some(word) = words.next() {
        match word.as_str() {
            "--extractor" => {
                extractor = Some(words.next().ok_or("--extractor requires a name")?.clone());
            }
            "--acquire" => {
                method = match words
                    .next()
                    .ok_or("--acquire requires http, lightpanda, or cloakbrowser")?
                    .as_str()
                {
                    "http" => acquisition::Acquisition::Http,
                    "lightpanda" => acquisition::Acquisition::Lightpanda,
                    "cloakbrowser" => acquisition::Acquisition::Cloakbrowser,
                    other => return Err(format!("unsupported acquisition mode: {other}").into()),
                };
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown option: {other}").into())
            }
            other if input.is_none() => input = Some(other.to_string()),
            _ => return Err("only one page or file can be acquired per run".into()),
        }
    }
    let input = input.ok_or("missing HTML file or URL")?;
    Ok((extractor, method, input))
}
