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
        let (extractor, input_start) = if args[1] == "--extractor" {
            let name = args.get(2).ok_or("--extractor requires a name")?;
            (Some(name.as_str()), 3)
        } else {
            (None, 1)
        };
        if args.len() <= input_start {
            return Err("missing HTML file or URL".into());
        }
        let input = args[input_start..].join(" ");
        let current_dir = std::env::current_dir()?;
        let out_dir = current_dir.join("out");

        let (html, url) = if std::path::Path::new(&input).is_file() {
            let html = std::fs::read_to_string(&input)?;
            let canonical = std::fs::canonicalize(&input)?;
            let url = url::Url::from_file_path(canonical)
                .map_err(|()| std::io::Error::other("Cannot represent input file as URL"))?;
            (html, url)
        } else {
            let url = url::Url::parse(&input)?;
            let response = ureq::get(url.as_str()).call()?;
            let mut html = String::new();
            response.into_reader().read_to_string(&mut html)?;
            (html, url)
        };

        if out_dir.exists() {
            std::fs::remove_dir_all(&out_dir)?;
        }
        std::fs::create_dir_all(&out_dir)?;

        let html_file = out_dir.join("html.html");
        std::fs::write(&html_file, &html)?;
        println!("HTML Size (bytes): {}", html.len());

        let mut runner = runner::Runner::new(out_dir, html);
        let states = ExtractorStates::default();
        let mut names: Vec<_> = if let Some(extractor) = extractor {
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
        eprintln!("  {} --server [port]   Start web server", args[0]);
        eprintln!();
        eprintln!("Web server mode: {} --server 3000", args[0]);
    }
    Ok(())
}
