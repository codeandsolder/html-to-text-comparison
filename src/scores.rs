use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, read_to_string};
use std::path::PathBuf;
use uuid::Uuid;

use crate::extractor_config::ExtractorStates;
use crate::runner::Runner;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Score {
    pub id: String,
    pub url: String,
    pub html_size: usize,
    pub timestamp: String,
    #[serde(default)]
    pub source_html_file: String,
    #[serde(default)]
    pub settings_snapshot: ExtractorStates,
    #[serde(default)]
    pub grades: BTreeMap<String, u8>,
    pub extractor_results: Vec<ExtractorResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractorResult {
    pub name: String,
    #[serde(default)]
    pub extractor_key: String,
    pub output_size: usize,
    pub reduction_percent: f64,
    pub time_micros: u64,
    pub panicked: bool,
    pub output_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractorPreview {
    pub name: String,
    pub extractor_key: String,
    pub output: String,
    pub output_size: usize,
    pub reduction_percent: f64,
    pub time_micros: u64,
    pub panicked: bool,
}

impl Score {
    pub fn new(url: String, html: String, states: &ExtractorStates) -> Self {
        let id = Uuid::new_v4().to_string();
        let timestamp = chrono_lite_now();
        let html_size = html.len();
        let out_dir = PathBuf::from("/tmp/html-extract").join(&id);
        let _ = fs::create_dir_all(&out_dir);
        let source_html_file = out_dir.join("source.html");
        let _ = fs::write(&source_html_file, &html);
        let parsed_url = url::Url::parse(&url).expect("run_extraction requires a valid URL");

        let mut runner = Runner::new(out_dir, html);
        let states = states.clone();

        for extractor_name in enabled_extractors(&states) {
            run_named_extractor(
                &mut runner,
                &states,
                &parsed_url,
                extractor_name,
                extractor_name,
            );
        }

        let stats = runner.into_stats();
        let out_dir = PathBuf::from("/tmp/html-extract").join(&id);
        let extractor_results: Vec<ExtractorResult> = stats
            .iter()
            .map(|stat| ExtractorResult {
                name: stat.name.clone(),
                extractor_key: stat.name.clone(),
                output_size: stat.output_size,
                reduction_percent: 100.0 - (stat.output_size as f64 / html_size as f64) * 100.0,
                time_micros: stat.time_micros,
                panicked: stat.panicked,
                output_file: out_dir
                    .join(format!("{}.txt", stat.name))
                    .display()
                    .to_string(),
            })
            .collect();

        Self {
            id,
            url,
            html_size,
            timestamp,
            source_html_file: source_html_file.display().to_string(),
            settings_snapshot: states,
            grades: BTreeMap::new(),
            extractor_results,
        }
    }
}

fn enabled_extractors(states: &ExtractorStates) -> Vec<&str> {
    states
        .states
        .iter()
        .filter_map(|(name, state)| state.enabled.then_some(name.as_str()))
        .collect()
}

fn run_named_extractor(
    runner: &mut Runner,
    states: &ExtractorStates,
    parsed_url: &url::Url,
    extractor_key: &str,
    output_name: &str,
) {
    match extractor_key {
        #[cfg(feature = "readability")]
        "readability" => {
            let parsed_url = parsed_url.clone();
            runner.run(output_name, move |html| {
                let mut html = std::io::Cursor::new(html.as_bytes());
                readability::extractor::extract(&mut html, &parsed_url)
                    .unwrap()
                    .text
            });
        }
        #[cfg(feature = "llm_readability")]
        "llm_readability" => {
            let parsed_url = parsed_url.clone();
            runner.run(output_name, move |html| {
                let mut html = std::io::Cursor::new(html.as_bytes());
                llm_readability::extractor::extract(&mut html, &parsed_url)
                    .unwrap()
                    .text
            });
        }
        #[cfg(feature = "html2text")]
        "html2text" => {
            let cfg = states
                .states
                .get("html2text")
                .map(|s| s.config.html2text.clone())
                .unwrap_or_default();
            runner.run(output_name, move |html| {
                let mut html = std::io::Cursor::new(html.as_bytes());
                let width = cfg.max_wrap_width.max(1);
                let mut render = html2text::config::plain()
                    .max_wrap_width(width)
                    .raw_mode(cfg.raw_mode);
                if cfg.no_link_wrapping {
                    render = render.no_link_wrapping();
                }
                render
                    .string_from_read(&mut html, width)
                    .unwrap_or_default()
            });
        }
        #[cfg(feature = "htmd")]
        "htmd" => {
            let cfg = states
                .states
                .get("htmd")
                .map(|s| s.config.htmd.clone())
                .unwrap_or_default();
            let legacy_skip_tags = states
                .states
                .get("htmd")
                .map(|s| s.config.skip_tags.clone())
                .unwrap_or_default();
            let skip_tags = if cfg.skip_tags.is_empty() {
                legacy_skip_tags
            } else {
                cfg.skip_tags.clone()
            };
            runner.run(output_name, move |html| {
                let mut options = htmd::options::Options::default();
                options.heading_style = match cfg.heading_style.as_str() {
                    "setex" => htmd::options::HeadingStyle::Setex,
                    _ => htmd::options::HeadingStyle::Atx,
                };
                let mut builder = htmd::HtmlToMarkdown::builder().options(options);
                if !skip_tags.is_empty() {
                    builder = builder.skip_tags(skip_tags.iter().map(|s| s.as_str()).collect());
                }
                builder.build().convert(html).unwrap_or_default()
            });
        }
        #[cfg(feature = "html2md-rs")]
        "html2md-rs" => {
            let cfg = states
                .states
                .get("html2md-rs")
                .map(|s| s.config.html2md_rs.clone())
                .unwrap_or_default();
            let legacy_skip_tags = states
                .states
                .get("html2md-rs")
                .map(|s| s.config.skip_tags.clone())
                .unwrap_or_default();
            let tags = if cfg.ignore_tags.is_empty() {
                legacy_skip_tags
            } else {
                cfg.ignore_tags.clone()
            };
            runner.run(output_name, move |html| {
                use html2md_rs::structs::{NodeType, ToMdConfig};
                use html2md_rs::to_md::safe_from_html_to_md_with_config;
                safe_from_html_to_md_with_config(
                    html.to_string(),
                    &ToMdConfig {
                        ignore_rendering: tags
                            .iter()
                            .map(|tag| NodeType::from_tag_str(tag.as_str()))
                            .collect(),
                    },
                )
                .unwrap_or_default()
            });
        }
        #[cfg(feature = "nanohtml2text")]
        "nanohtml2text" => {
            runner.run(output_name, |html| nanohtml2text::html2text(html));
        }
        #[cfg(feature = "mdka")]
        "mdka" => {
            let cfg = states
                .states
                .get("mdka")
                .map(|s| s.config.mdka.clone())
                .unwrap_or_default();
            runner.run(output_name, move |html| {
                mdka::html_to_markdown_with(html, &cfg.clone().into_conversion_options())
            });
        }
        #[cfg(feature = "readable-readability")]
        "readable-readability" => {
            let cfg = states
                .states
                .get("readable-readability")
                .map(|s| s.config.readable_readability.clone())
                .unwrap_or_default();
            runner.run(output_name, move |html| {
                let mut parser = readable_readability::Readability::new();
                parser.strip_unlikelys(cfg.strip_unlikelys);
                parser.weight_classes(cfg.weight_classes);
                parser.clean_conditionally(cfg.clean_conditionally);
                let (node, _) = parser.parse(&html);
                node.text_contents()
            });
        }
        #[cfg(feature = "dom_smoothie")]
        "dom_smoothie" => {
            let cfg = states
                .states
                .get("dom_smoothie")
                .map(|s| s.config.dom_smoothie.clone())
                .unwrap_or_default();
            runner.run(output_name, move |html| {
                let dom_cfg = cfg.clone();
                dom_smoothie::Readability::new(html, None, dom_cfg.into_config())
                    .unwrap()
                    .parse()
                    .unwrap()
                    .text_content
                    .to_string()
            });
        }
        #[cfg(feature = "boilerpipe")]
        "boilerpipe" => {
            runner.run(output_name, |html| {
                boilerpipe::parse_document(html).content().to_string()
            });
        }
        #[cfg(feature = "august")]
        "august" => {
            let cfg = states
                .states
                .get("august")
                .map(|s| s.config.augus_max_width)
                .unwrap_or(usize::MAX);
            runner.run(output_name, move |html| august::convert(html, cfg));
        }
        #[cfg(feature = "fast_html2md")]
        "fast_html2md" => {
            runner.run(output_name, |html| fast_html2md::parse_html(html, false));
        }
        #[cfg(feature = "html2md")]
        "html2md" => {
            runner.run(output_name, |html| html2md::parse_html(html));
        }
        #[cfg(feature = "mdream")]
        "mdream" => {
            runner.run(output_name, |html| {
                use mdream::{html_to_markdown, types::{HTMLToMarkdownOptions, CleanConfig, PluginConfig, FilterConfig, IsolateMainConfig, FrontmatterConfig, TailwindConfig}};
                let cfg = states
                    .states
                    .get("mdream")
                    .map(|s| s.config.mdream.clone())
                    .unwrap_or_default();
                let mut opts = HTMLToMarkdownOptions::default();
                opts.clean_urls = cfg.clean_urls;
                if cfg.clean_urls {
                    opts.clean = Some(CleanConfig {
                        urls: true,
                        ..Default::default()
                    });
                }
                if cfg.minimal || cfg.isolate_main || cfg.frontmatter || cfg.tailwind {
                    let mut plugins = PluginConfig::default();
                    if cfg.minimal {
                        plugins.filter = Some(FilterConfig {
                            exclude: Some(vec![
                                "nav".to_string(),
                                "footer".to_string(),
                                "aside".to_string(),
                                "form".to_string(),
                            ]),
                            ..Default::default()
                        });
                    }
                    if cfg.isolate_main {
                        plugins.isolate_main = Some(IsolateMainConfig::default());
                    }
                    if cfg.frontmatter {
                        plugins.frontmatter = Some(FrontmatterConfig::default());
                    }
                    if cfg.tailwind {
                        plugins.tailwind = Some(TailwindConfig::default());
                    }
                    opts.plugins = Some(plugins);
                }
                html_to_markdown(html, opts)
            });
        }
        _ => {
            let extractor_key = extractor_key.to_string();
            runner.run(output_name, move |html| {
                run_cli_extractor(&extractor_key, html, states, parsed_url)
            });
        }
    }
}

pub fn compare_single_extractor_settings(
    url: &str,
    html: &str,
    extractor: &str,
    baseline_config: crate::extractor_config::ExtractorConfig,
    candidate_config: crate::extractor_config::ExtractorConfig,
    score_store: &ScoreStore,
) -> Score {
    let baseline_states = single_extractor_state(extractor, baseline_config.clone());
    let candidate_states = single_extractor_state(extractor, candidate_config.clone());

    let id = Uuid::new_v4().to_string();
    let timestamp = chrono_lite_now();
    let html_size = html.len();
    let out_dir = PathBuf::from("/tmp/html-extract").join(&id);
    let _ = fs::create_dir_all(&out_dir);
    let source_html_file = out_dir.join("source.html");
    let _ = fs::write(&source_html_file, html);
    let parsed_url =
        url::Url::parse(url).expect("compare_single_extractor_settings requires a valid URL");

    let mut runner = Runner::new(out_dir.clone(), html.to_string());
    let baseline_name = format!("{extractor} baseline");
    let candidate_name = format!("{extractor} current settings");
    run_named_extractor(
        &mut runner,
        &baseline_states,
        &parsed_url,
        extractor,
        &baseline_name,
    );
    run_named_extractor(
        &mut runner,
        &candidate_states,
        &parsed_url,
        extractor,
        &candidate_name,
    );

    let extractor_results = runner
        .into_stats()
        .into_iter()
        .map(|stat| ExtractorResult {
            name: stat.name.clone(),
            extractor_key: extractor.to_string(),
            output_size: stat.output_size,
            reduction_percent: 100.0 - (stat.output_size as f64 / html_size as f64) * 100.0,
            time_micros: stat.time_micros,
            panicked: stat.panicked,
            output_file: out_dir
                .join(format!("{}.txt", stat.name))
                .display()
                .to_string(),
        })
        .collect();

    let mut settings_snapshot = candidate_states;
    if let Some(state) = settings_snapshot.states.get_mut(extractor) {
        state.enabled = true;
    }

    let score = Score {
        id,
        url: url.to_string(),
        html_size,
        timestamp,
        source_html_file: source_html_file.display().to_string(),
        settings_snapshot,
        grades: BTreeMap::new(),
        extractor_results,
    };
    let _ = score_store.save(&score);
    score
}

pub fn preview_single_extractor_settings(
    url: &str,
    html: &str,
    extractor: &str,
    config: crate::extractor_config::ExtractorConfig,
) -> ExtractorPreview {
    let states = single_extractor_state(extractor, config);
    let parsed_url =
        url::Url::parse(url).expect("preview_single_extractor_settings requires a valid URL");
    let html_size = html.len();
    let out_dir = PathBuf::from("/tmp/html-extract-preview").join(Uuid::new_v4().to_string());
    let _ = fs::create_dir_all(&out_dir);

    let mut runner = Runner::new(out_dir.clone(), html.to_string());
    run_named_extractor(&mut runner, &states, &parsed_url, extractor, extractor);

    let stat = runner.into_stats().into_iter().next().unwrap_or_default();
    let output =
        std::fs::read_to_string(out_dir.join(format!("{extractor}.txt"))).unwrap_or_default();
    let _ = fs::remove_dir_all(&out_dir);

    ExtractorPreview {
        name: format!("{extractor} current settings"),
        extractor_key: extractor.to_string(),
        output_size: stat.output_size,
        reduction_percent: 100.0 - (stat.output_size as f64 / html_size as f64) * 100.0,
        time_micros: stat.time_micros,
        panicked: stat.panicked,
        output,
    }
}

fn single_extractor_state(
    extractor: &str,
    config: crate::extractor_config::ExtractorConfig,
) -> ExtractorStates {
    let mut states = ExtractorStates::default();
    for state in states.states.values_mut() {
        state.enabled = false;
    }
    let entry = states.states.entry(extractor.to_string()).or_default();
    entry.enabled = true;
    entry.config = config;
    states
}

pub fn run_extraction(
    url: &str,
    html: &str,
    states: &ExtractorStates,
    score_store: &ScoreStore,
) -> Score {
    let score = Score::new(url.to_string(), html.to_string(), states);
    let _ = score_store.save(&score);
    score
}

fn chrono_lite_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("{}", now)
}

pub(crate) fn run_cli_extractor(
    extractor_key: &str,
    html: &str,
    states: &ExtractorStates,
    parsed_url: &url::Url,
) -> String {
    match extractor_key {
        "turndown" => run_turndown(html),
        "percollate" => run_percollate(html),
        "trafilatura" => run_trafilatura(html),
        "html2text-py" => run_html2text_py(html),
        "lightpanda" => run_lightpanda(parsed_url),
        "webclaw" => run_webclaw(html, states),
        "e2m" => run_e2m(html, states),
        "html-to-markdown-go" => run_html_to_markdown_go(html, parsed_url),
        _ => String::new(),
    }
}

fn run_turndown(html: &str) -> String {
    let tmp = std::env::temp_dir().join(format!("turndown_{}.html", uuid::Uuid::new_v4()));
    let _ = std::fs::write(&tmp, html);
    let node_code = r#"const fs = require('fs'); const td = require('/home/jan/git/turndown'); const svc = new td(); const html = fs.readFileSync(process.argv[1], 'utf8'); process.stdout.write(svc.turndown(html))"#;
    let out = std::process::Command::new("node")
        .args(["-e", node_code, tmp.to_str().unwrap()])
        .output();

    let _ = std::fs::remove_file(&tmp);
    match out {
        Ok(o) => {
            if !o.status.success() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return format!("[ERROR] turndown failed (exit {}): {}\n", o.status, stderr.trim());
            }
            let stdout = String::from_utf8_lossy(&o.stdout);
            if stdout.is_empty() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return format!("[ERROR] turndown returned empty output. stderr: {}\n", stderr.trim());
            }
            stdout.to_string()
        }
        Err(e) => format!("[ERROR] node failed: {}\n", e),
    }
}

fn run_percollate(html: &str) -> String {
    let tmp = std::env::temp_dir().join(format!("percollate_in_{}.html", uuid::Uuid::new_v4()));
    let _ = std::fs::write(&tmp, html);
    let out = std::process::Command::new("node")
        .args(["/home/jan/git/percollate/cli.js", "md", "-o", "-", tmp.to_str().unwrap()])
        .output();

    let _ = std::fs::remove_file(&tmp);
    match out {
        Ok(o) => {
            if !o.status.success() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return format!("[ERROR] percollate failed (exit {}): {}\n", o.status, stderr.trim());
            }
            let stdout = String::from_utf8_lossy(&o.stdout);
            if stdout.is_empty() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return format!("[ERROR] percollate returned empty output. stderr: {}\n", stderr.trim());
            }
            stdout.to_string()
        }
        Err(e) => format!("[ERROR] percollate failed: {}\n", e),
    }
}

fn run_trafilatura(html: &str) -> String {
    let tmp = std::env::temp_dir().join(format!("trafilatura_{}.html", uuid::Uuid::new_v4()));
    let _ = std::fs::write(&tmp, html);
    let out = std::process::Command::new("uv")
        .args(["run", "--", "python3", "-c", "import trafilatura; import sys; html=open(sys.argv[1]).read(); result=trafilatura.extract(html, output_format='markdown', include_links=True); print(result if result else '', end='')"])
        .arg(tmp.to_str().unwrap())
        .output();

    let _ = std::fs::remove_file(&tmp);
    match out {
        Ok(o) => {
            if !o.status.success() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return format!("[ERROR] trafilatura failed (exit {}): {}\n", o.status, stderr.trim());
            }
            let stdout = String::from_utf8_lossy(&o.stdout);
            if stdout.is_empty() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return format!("[ERROR] trafilatura returned empty output. stderr: {}\n", stderr.trim());
            }
            stdout.to_string()
        }
        Err(e) => format!("[ERROR] uv failed: {}\n", e),
    }
}

fn run_html2text_py(html: &str) -> String {
    let tmp = std::env::temp_dir().join(format!("h2t_py_{}.html", uuid::Uuid::new_v4()));
    let _ = std::fs::write(&tmp, html);
    let out = std::process::Command::new("uv")
        .args(["run", "--", "python3", "-c", "from html2text import HTML2Text; import sys; h=HTML2Text(); h.ignore_links=False; h.ignore_images=False; h.body_width=78; html=open(sys.argv[1]).read(); print(h.handle(html), end='')"])
        .arg(tmp.to_str().unwrap())
        .output();

    let _ = std::fs::remove_file(&tmp);
    match out {
        Ok(o) => {
            if !o.status.success() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return format!("[ERROR] html2text.py failed (exit {}): {}\n", o.status, stderr.trim());
            }
            let stdout = String::from_utf8_lossy(&o.stdout);
            if stdout.is_empty() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return format!("[ERROR] html2text.py returned empty output. stderr: {}\n", stderr.trim());
            }
            stdout.to_string()
        }
        Err(e) => format!("[ERROR] uv failed: {}\n", e),
    }
}

fn run_lightpanda(parsed_url: &url::Url) -> String {
    let out = std::process::Command::new("docker")
        .args([
            "exec", "lightpanda", "lightpanda", "fetch",
            "--dump", "markdown",
            parsed_url.to_string().as_str(),
        ])
        .output();

    match out {
        Ok(o) => {
            if !o.status.success() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return format!("[ERROR] lightpanda docker exec failed (exit {}): {}\n", o.status, stderr.trim());
            }
            let stdout = String::from_utf8_lossy(&o.stdout);
            if stdout.is_empty() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return format!("[ERROR] lightpanda returned empty output. stderr: {}\n", stderr.trim());
            }
            stdout.to_string()
        }
        Err(e) => format!("[ERROR] docker exec lightpanda failed: {}\n", e),
    }
}

fn run_webclaw(html: &str, states: &ExtractorStates) -> String {
    let cfg = states
        .states
        .get("webclaw")
        .map(|s| s.config.webclaw.clone())
        .unwrap_or_default();
    let tmp = std::env::temp_dir().join(format!("webclaw_{}.html", uuid::Uuid::new_v4()));
    let _ = std::fs::write(&tmp, html);
    let mut args = vec!["--file".to_string(), tmp.to_string_lossy().to_string()];
    if cfg.only_main_content {
        args.push("--only-main-content".to_string());
    }
    if !cfg.include_css.is_empty() {
        args.push("--include".to_string());
        args.push(cfg.include_css.clone());
    }
    if !cfg.exclude_css.is_empty() {
        args.push("--exclude".to_string());
        args.push(cfg.exclude_css.clone());
    }
    args.push("-f".to_string());
    args.push(if cfg.format.is_empty() { "markdown".to_string() } else { cfg.format.clone() });
    let bin = std::path::Path::new("/home/jan/git/webclaw/webclaw_bin");
    let out = if bin.exists() {
        std::process::Command::new(bin).args(&args).output()
    } else {
        std::process::Command::new("webclaw").args(&args).output()
    };

    let _ = std::fs::remove_file(&tmp);
    match out {
        Ok(o) => {
            if !o.status.success() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return format!("[ERROR] webclaw failed (exit {}): {}\n", o.status, stderr.trim());
            }
            let stdout = String::from_utf8_lossy(&o.stdout);
            if stdout.is_empty() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return format!("[ERROR] webclaw returned empty output. stderr: {}\n", stderr.trim());
            }
            stdout.to_string()
        }
        Err(e) => format!("[ERROR] webclaw not found: {}\n", e),
    }
}

fn run_e2m(html: &str, states: &ExtractorStates) -> String {
    let cfg = states
        .states
        .get("e2m")
        .map(|s| s.config.e2m.clone())
        .unwrap_or_default();
    let engine = if cfg.engine.is_empty() { "unstructured" } else { &cfg.engine };
    let tmp = std::env::temp_dir().join(format!("e2m_{}.html", uuid::Uuid::new_v4()));
    let _ = std::fs::write(&tmp, html);
    let out = std::process::Command::new("uv")
        .args(["run", "--", "python3", "-c", &format!(
            "import sys; from wisup_e2m import HtmlParser; p=HtmlParser(engine='{}'); result=p.parse(text=open(sys.argv[1]).read(), include_image_link_in_text=False); print(result.text, end='')",
            engine
        ), tmp.to_str().unwrap()])
        .output();

    let _ = std::fs::remove_file(&tmp);
    match out {
        Ok(o) => {
            if !o.status.success() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return format!("[ERROR] e2m failed (exit {}): {}\n", o.status, stderr.trim());
            }
            let stdout = String::from_utf8_lossy(&o.stdout);
            if stdout.is_empty() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return format!("[ERROR] e2m returned empty output. stderr: {}\n", stderr.trim());
            }
            stdout.to_string()
        }
        Err(e) => format!("[ERROR] uv failed: {}\n", e),
    }
}

fn run_html_to_markdown_go(html: &str, parsed_url: &url::Url) -> String {
    let domain = parsed_url.origin().ascii_serialization();
    let out = std::process::Command::new("/tmp/html2markdown")
        .arg(format!("--domain={}", domain))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();

    match out {
        Ok(mut child) => {
            use std::io::Write;
            if let Some(ref mut stdin) = child.stdin {
                stdin.write_all(html.as_bytes()).ok();
            }
            let result = child.wait_with_output();

            match result {
                Ok(o) => {
                    if !o.status.success() {
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        return format!("[ERROR] html-to-markdown-go failed (exit {}): {}\n", o.status, stderr.trim());
                    }
                    let stdout = String::from_utf8_lossy(&o.stdout);
                    if stdout.is_empty() {
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        return format!("[ERROR] html-to-markdown-go returned empty output. stderr: {}\n", stderr.trim());
                    }
                    stdout.to_string()
                }
                Err(e) => format!("[ERROR] html-to-markdown-go wait failed: {}\n", e),
            }
        }
        Err(e) => format!("[ERROR] html2markdown spawn failed: {}\n", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extractor_config::{ExtractorConfig, HtmdConfig, Html2TextConfig};

    fn test_store() -> (ScoreStore, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("html-to-text-comparison-tests-{}", Uuid::new_v4()));
        (ScoreStore::new(dir.clone()), dir)
    }

    #[test]
    fn compare_settings_applies_html2text_config() {
        let (store, dir) = test_store();
        let html = "<html><body><p>alpha beta gamma delta epsilon zeta eta theta</p></body></html>";
        let baseline = ExtractorConfig {
            html2text: Html2TextConfig {
                max_wrap_width: 12,
                raw_mode: false,
                no_link_wrapping: false,
            },
            ..Default::default()
        };
        let candidate = ExtractorConfig {
            html2text: Html2TextConfig {
                max_wrap_width: 120,
                raw_mode: false,
                no_link_wrapping: false,
            },
            ..Default::default()
        };

        let score = compare_single_extractor_settings(
            "https://example.com",
            html,
            "html2text",
            baseline,
            candidate,
            &store,
        );

        let baseline_output =
            std::fs::read_to_string(&score.extractor_results[0].output_file).unwrap();
        let candidate_output =
            std::fs::read_to_string(&score.extractor_results[1].output_file).unwrap();

        assert_ne!(baseline_output, candidate_output);
        assert!(baseline_output.matches('\n').count() > candidate_output.matches('\n').count());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn compare_settings_applies_htmd_heading_style() {
        let (store, dir) = test_store();
        let html = "<html><body><h1>Title</h1><p>Body</p></body></html>";
        let baseline = ExtractorConfig {
            htmd: HtmdConfig {
                skip_tags: Vec::new(),
                heading_style: "atx".to_string(),
            },
            ..Default::default()
        };
        let candidate = ExtractorConfig {
            htmd: HtmdConfig {
                skip_tags: Vec::new(),
                heading_style: "setex".to_string(),
            },
            ..Default::default()
        };

        let score = compare_single_extractor_settings(
            "https://example.com",
            html,
            "htmd",
            baseline,
            candidate,
            &store,
        );

        let baseline_output =
            std::fs::read_to_string(&score.extractor_results[0].output_file).unwrap();
        let candidate_output =
            std::fs::read_to_string(&score.extractor_results[1].output_file).unwrap();

        assert!(baseline_output.contains("# Title"));
        assert!(candidate_output.contains("Title\n====="));
        assert_ne!(baseline_output, candidate_output);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn preview_single_extractor_returns_current_output() {
        let html = "<html><body><p>alpha beta gamma delta epsilon zeta eta theta</p></body></html>";
        let preview = preview_single_extractor_settings(
            "https://example.com",
            html,
            "html2text",
            ExtractorConfig {
                html2text: Html2TextConfig {
                    max_wrap_width: 12,
                    raw_mode: false,
                    no_link_wrapping: false,
                },
                ..Default::default()
            },
        );

        assert_eq!(preview.extractor_key, "html2text");
        assert!(preview.output.contains("alpha beta"));
        assert!(preview.output.matches('\n').count() > 1);
    }
}

#[derive(Clone)]
pub struct ScoreStore {
    dir: PathBuf,
}

impl ScoreStore {
    pub fn new(dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(&dir);
        Self { dir }
    }

    pub fn save(&self, score: &Score) -> Result<(), String> {
        let path = self.dir.join(format!("{}.json", score.id));
        let s = serde_json::to_string_pretty(score).map_err(|e| e.to_string())?;
        fs::write(path, s).map_err(|e| e.to_string())
    }

    pub fn load(&self, id: &str) -> Result<Score, String> {
        let path = self.dir.join(format!("{}.json", id));
        let s = read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&s).map_err(|e| e.to_string())
    }

    pub fn update_grade(&self, id: &str, extractor: &str, grade: u8) -> Result<Score, String> {
        let mut score = self.load(id)?;
        score.grades.insert(extractor.to_string(), grade);
        self.save(&score)?;
        Ok(score)
    }

    pub fn list(&self) -> Result<Vec<Score>, String> {
        let mut scores = Vec::new();
        for entry in fs::read_dir(&self.dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                match read_to_string(&path) {
                    Ok(s) => {
                        if let Ok(score) = serde_json::from_str::<Score>(&s) {
                            scores.push(score);
                        }
                    }
                    Err(_) => continue,
                }
            }
        }
        scores.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(scores)
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        let path = self.dir.join(format!("{}.json", id));
        fs::remove_file(path).map_err(|e| e.to_string())
    }
}
