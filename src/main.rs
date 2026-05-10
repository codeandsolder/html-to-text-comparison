mod extractor_config;
mod runner;
mod scores;
mod web;

use extractor_config::ExtractorStates;

#[allow(unused_imports)]
use scores::run_cli_extractor;

use std::path::PathBuf;
use std::process::exit;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 && args[1] == "--server" {
        let port: u16 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(3000);
        let data_dir: PathBuf = args
            .get(3)
            .map(|s| PathBuf::from(s))
            .unwrap_or_else(|| "data".into());
        if let Err(e) = web::start_server(port, data_dir).await {
            eprintln!("Server error: {}", e);
            exit(1);
        }
    } else if args.len() > 1 {
        let input = args[1..].join(" ");
        let current_dir = std::env::current_dir().unwrap();
        let out_dir = current_dir.join("out");

        let (html, url) =
            if input.ends_with(".html") || input.ends_with(".htm") || input.ends_with(".xml") {
                let html = std::fs::read_to_string(&input).expect("Failed to read file");
                let url = url::Url::parse("file:///local/input.html").unwrap();
                (html, url)
            } else {
                let url = url::Url::parse(&input).expect("Invalid URL");
                let response = ureq::get(url.as_str()).call().expect("Failed to fetch URL");
                let mut s = String::new();
                response.into_reader().read_to_string(&mut s).unwrap();
                (s, url)
            };

        if out_dir.exists() {
            std::fs::remove_dir_all(&out_dir).unwrap();
        }
        std::fs::create_dir_all(&out_dir).unwrap();

        let html_file = out_dir.join("html.html");
        std::fs::write(&html_file, &html).unwrap();
        println!("HTML Size (bytes): {}", html.len());

        let mut runner = runner::Runner::new(out_dir, html);

        #[cfg(feature = "readability")]
        {
            runner.run("readability", |html| {
                let mut html = std::io::Cursor::new(html.as_bytes());
                readability::extractor::extract(&mut html, &url)
                    .unwrap()
                    .text
            });
        }

        #[cfg(feature = "llm_readability")]
        {
            runner.run("llm_readability", |html| {
                let mut html = std::io::Cursor::new(html.as_bytes());
                llm_readability::extractor::extract(&mut html, &url)
                    .unwrap()
                    .text
            });
        }

        #[cfg(feature = "html2text")]
        {
            runner.run("html2text", |html| {
                let mut html = std::io::Cursor::new(html.as_bytes());
                html2text::from_read(&mut html, 1000).unwrap_or_default()
            });
        }

        #[cfg(feature = "htmd")]
        {
            static IGNORE_TAGS: &[&str] = &[
                "nav", "script", "style", "header", "footer", "img", "svg", "iframe",
            ];
            runner.run("htmd", |html| {
                htmd::HtmlToMarkdown::builder()
                    .skip_tags(IGNORE_TAGS.to_vec())
                    .build()
                    .convert(html)
                    .unwrap_or_default()
            });
        }

        #[cfg(feature = "html2md-rs")]
        {
            use html2md_rs::structs::{NodeType, ToMdConfig};
            use html2md_rs::to_md::safe_from_html_to_md_with_config;
            static IGNORE_TAGS: &[&str] = &[
                "nav", "script", "style", "header", "footer", "img", "svg", "iframe",
            ];
            runner.run("html2md-rs", |html| {
                safe_from_html_to_md_with_config(
                    html.to_string(),
                    &ToMdConfig {
                        ignore_rendering: IGNORE_TAGS
                            .iter()
                            .map(|tag| NodeType::from_tag_str(*tag))
                            .collect(),
                    },
                )
                .unwrap_or_default()
            });
        }

        #[cfg(feature = "nanohtml2text")]
        {
            runner.run("nanohtml2text", |html| nanohtml2text::html2text(html));
        }

        #[cfg(feature = "readable-readability")]
        {
            runner.run("readable-readability", |html| {
                let mut parser = readable_readability::Readability::new();
                parser.base_url(url.clone());
                let (node, _metadata) = parser.parse(&html);
                node.text_contents()
            });
        }

        #[cfg(feature = "mdka")]
        {
            runner.run("mdka", |html| mdka::html_to_markdown(html));
        }

        #[cfg(feature = "boilerpipe")]
        {
            runner.run("boilerpipe", |html| {
                boilerpipe::parse_document(&html).content().to_string()
            });
        }

        #[cfg(feature = "august")]
        {
            runner.run("august", |html| august::convert(html, usize::MAX));
        }

        #[cfg(feature = "fast_html2md")]
        {
            runner.run("fast_html2md", |html| fast_html2md::parse_html(html, false));
        }

        #[cfg(feature = "dom_smoothie")]
        {
            runner.run("dom_smoothie", |html| {
                dom_smoothie::Readability::new(html, None, None)
                    .unwrap()
                    .parse()
                    .unwrap()
                    .text_content
                    .to_string()
            });
        }

        #[cfg(feature = "html2md")]
        {
            runner.run("html2md", |html| html2md::parse_html(html));
        }

        #[cfg(feature = "mdream")]
        {
            use mdream::types::HTMLToMarkdownOptions;
            runner.run("mdream", |html| mdream::html_to_markdown(html, HTMLToMarkdownOptions::default()));
        }

{
            use scores::run_cli_extractor;
            let placeholder = url::Url::parse("https://placeholder.example.com").unwrap();
            runner.run("turndown", |html| {
                run_cli_extractor("turndown", html, &ExtractorStates::default(), &placeholder)
            });
            runner.run("percollate", |html| {
                run_cli_extractor("percollate", html, &ExtractorStates::default(), &placeholder)
            });
            runner.run("trafilatura", |html| {
                run_cli_extractor("trafilatura", html, &ExtractorStates::default(), &placeholder)
            });
            runner.run("html2text-py", |html| {
                run_cli_extractor("html2text-py", html, &ExtractorStates::default(), &placeholder)
            });
            runner.run("lightpanda", |_html| {
                run_cli_extractor("lightpanda", "", &ExtractorStates::default(), &url)
            });
            runner.run("webclaw", |html| {
                run_cli_extractor("webclaw", html, &ExtractorStates::default(), &placeholder)
            });
            runner.run("e2m", |html| {
                run_cli_extractor("e2m", html, &ExtractorStates::default(), &placeholder)
            });
            runner.run("html-to-markdown-go", |html| {
                run_cli_extractor("html-to-markdown-go", html, &ExtractorStates::default(), &url)
            });
        }

        #[cfg(feature = "reader-lm-api")]
        {
            let jina_api_key =
                std::env::var("JINA_API_KEY").expect("Must set JINA_API_KEY environment variable");
            runner.run("reader-lm-api", |_html| {
                let response = ureq::get(&format!("https://r.jina.ai/{}", url))
                    .set("authorization", &format!("Bearer {}", jina_api_key))
                    .call()
                    .expect("Failed to fetch URL");
                let mut s = String::new();
                response.into_reader().read_to_string(&mut s).unwrap();
                s
            });
        }

        println!("{}", runner.into_table());
        println!("Remember to check the output files to make sure they have parsed the information you expect!");
    } else {
        eprintln!("Usage:");
        eprintln!("  {} <url_or_file>     Run CLI extraction", args[0]);
        eprintln!("  {} --server [port]   Start web server", args[0]);
        eprintln!("");
        eprintln!("Web server mode: {} --server 3000", args[0]);
    }
}
