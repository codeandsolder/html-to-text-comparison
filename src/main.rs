mod acquisition;
mod boilerplate;
mod extractor_config;
mod metrics;
mod runner;
mod runtime_config;
mod scores;
mod web;
mod worker;

use extractor_config::ExtractorStates;

use runtime_config::RuntimeConfig;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os();
    let program = args
        .next()
        .unwrap_or_else(|| "html-to-text-comparison".into());
    let program = program.to_string_lossy();
    let mut args = args.peekable();
    if args.peek().is_some_and(|arg| arg == "--extract-worker") {
        args.next();
        return crate::worker::worker_main(args).map_err(Into::into);
    }

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = if args.peek().is_some_and(|arg| arg == "--config") {
        args.next();
        let path = args
            .next()
            .filter(|path| !path.is_empty() && !path.as_encoded_bytes().starts_with(b"--"))
            .ok_or("--config requires a path")?;
        RuntimeConfig::load(Some(Path::new(&path)))?
    } else {
        RuntimeConfig::load(None)?
    };
    let arguments = args
        .map(|arg| {
            arg.into_string()
                .map_err(|_| "extraction CLI arguments must be valid UTF-8")
        })
        .collect::<Result<Vec<_>, _>>()?;

    if arguments.first().is_some_and(|arg| arg == "--server") {
        if arguments.len() != 1 {
            return Err("--server takes no positional arguments; use --config".into());
        }
        web::start_server(config.server, config.execution).await?;
    } else if !arguments.is_empty() {
        let CliArgs {
            extractor,
            mut method,
            input,
            base_url,
        } = parse_cli_args(&arguments)?;
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
        let current_dir = std::env::current_dir()?;
        let out_dir = current_dir.join("out");

        let (html, url) = if std::path::Path::new(&input).is_file() {
            if method != acquisition::Acquisition::Http {
                return Err("browser acquisition requires an HTTP(S) URL, not a file".into());
            }
            method = acquisition::Acquisition::File;
            let html =
                acquisition::read_html_file(Path::new(&input), config.execution.max_html_bytes)?;
            let canonical = std::fs::canonicalize(&input)?;
            let url = match base_url {
                Some(url) => url,
                None => url::Url::from_file_path(canonical)
                    .map_err(|()| std::io::Error::other("Cannot represent input file as URL"))?,
            };
            (html, url)
        } else {
            if base_url.is_some() {
                return Err("--base-url only applies to a local HTML snapshot".into());
            }
            let url = url::Url::parse(&input)?;
            let html = method.acquire(&url, &config.execution)?;
            (html, url)
        };

        if out_dir.exists() {
            std::fs::remove_dir_all(&out_dir)?;
        }
        std::fs::create_dir_all(&out_dir)?;

        let snapshot_path = out_dir.join("source.html");
        std::fs::write(&snapshot_path, &html)?;
        println!("Acquisition: {method:?}, HTML Size (bytes): {}", html.len());

        let mut runner = runner::Runner::new(out_dir, html, config.execution);
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
        eprintln!("  {} <url_or_file>     Run all extractors", program);
        eprintln!(
            "  {} --extractor <name> [--base-url URL] <url_or_file>     Run one extractor",
            program
        );
        eprintln!("  {} --acquire {{http|lightpanda|cloakbrowser}} [--extractor NAME] <url>  One acquisition", program);
        eprintln!("  {} [--config FILE] --server   Start web server", program);
        eprintln!();
        eprintln!("Prefix either mode with --config FILE to load explicit runtime settings.");
    }
    Ok(())
}

struct CliArgs {
    extractor: Option<String>,
    method: acquisition::Acquisition,
    input: String,
    base_url: Option<url::Url>,
}

fn parse_cli_args(arguments: &[String]) -> Result<CliArgs, Box<dyn std::error::Error>> {
    let mut extractor: Option<String> = None;
    let mut method = acquisition::Acquisition::Http;
    let mut base_url = None;
    let mut input = None;
    let mut words = arguments.iter();
    while let Some(word) = words.next() {
        match word.as_str() {
            "--extractor" => {
                extractor = Some(words.next().ok_or("--extractor requires a name")?.clone());
            }
            "--base-url" => {
                let parsed = url::Url::parse(words.next().ok_or("--base-url requires a URL")?)?;
                if !matches!(parsed.scheme(), "http" | "https") {
                    return Err("--base-url must use http or https".into());
                }
                base_url = Some(parsed);
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
    Ok(CliArgs {
        extractor,
        method,
        input,
        base_url,
    })
}
