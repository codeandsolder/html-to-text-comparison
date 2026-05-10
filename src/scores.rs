use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, read_to_string};
use std::path::PathBuf;
use uuid::Uuid;

use crate::extractor_config::{
    ExtractorStates, Html2TextPythonConfig, HtmlToMarkdownGoConfig, LightpandaConfig,
    MarkdownifyConfig, PercollateConfig, TrafilaturaConfig, TurndownConfig, DEFAULT_SKIP_TAGS,
};
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
                    .min_wrap_width(cfg.min_wrap_width.max(1))
                    .raw_mode(cfg.raw_mode);
                if cfg.no_link_wrapping {
                    render = render.no_link_wrapping();
                }
                if cfg.link_footnotes {
                    render = render.link_footnotes(true);
                }
                if cfg.no_table_borders {
                    render = render.no_table_borders();
                }
                if cfg.pad_block_width {
                    render = render.pad_block_width();
                }
                if cfg.allow_width_overflow {
                    render = render.allow_width_overflow();
                }
                if cfg.decorate {
                    render = render.do_decorate();
                }
                render = render.unicode_strikeout(cfg.unicode_strikeout);
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
                options.hr_style = match cfg.hr_style.as_str() {
                    "dashes" => htmd::options::HrStyle::Dashes,
                    "underscores" => htmd::options::HrStyle::Underscores,
                    _ => htmd::options::HrStyle::Asterisks,
                };
                options.br_style = match cfg.br_style.as_str() {
                    "backslash" => htmd::options::BrStyle::Backslash,
                    _ => htmd::options::BrStyle::TwoSpaces,
                };
                options.link_style = match cfg.link_style.as_str() {
                    "referenced" => htmd::options::LinkStyle::Referenced,
                    "inlined_prefer_autolinks" => htmd::options::LinkStyle::InlinedPreferAutolinks,
                    _ => htmd::options::LinkStyle::Inlined,
                };
                options.link_reference_style = match cfg.link_reference_style.as_str() {
                    "collapsed" => htmd::options::LinkReferenceStyle::Collapsed,
                    "shortcut" => htmd::options::LinkReferenceStyle::Shortcut,
                    _ => htmd::options::LinkReferenceStyle::Full,
                };
                options.code_block_style = match cfg.code_block_style.as_str() {
                    "indented" => htmd::options::CodeBlockStyle::Indented,
                    _ => htmd::options::CodeBlockStyle::Fenced,
                };
                options.code_block_fence = match cfg.code_block_fence.as_str() {
                    "tildes" => htmd::options::CodeBlockFence::Tildes,
                    _ => htmd::options::CodeBlockFence::Backticks,
                };
                options.bullet_list_marker = match cfg.bullet_list_marker.as_str() {
                    "-" => htmd::options::BulletListMarker::Dash,
                    _ => htmd::options::BulletListMarker::Asterisk,
                };
                options.ul_bullet_spacing = cfg.ul_bullet_spacing;
                options.ol_number_spacing = cfg.ol_number_spacing;
                options.preformatted_code = cfg.preformatted_code;
                options.translation_mode = match cfg.translation_mode.as_str() {
                    "faithful" => htmd::options::TranslationMode::Faithful,
                    _ => htmd::options::TranslationMode::Pure,
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
                parser.clean_attributes(cfg.clean_attributes);
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
            let cfg = states
                .states
                .get("mdream")
                .map(|s| s.config.mdream.clone())
                .unwrap_or_default();
            runner.run(output_name, move |html| {
                use mdream::{
                    html_to_markdown,
                    types::{
                        CleanConfig, ExtractionConfig, FilterConfig, FrontmatterConfig,
                        HTMLToMarkdownOptions, IsolateMainConfig, PluginConfig, TailwindConfig,
                    },
                };
                let mut opts = HTMLToMarkdownOptions::default();
                if !cfg.origin.is_empty() {
                    opts.origin = Some(cfg.origin.clone());
                }
                opts.clean_urls = cfg.clean_urls;
                if cfg.clean_urls
                    || cfg.clean_fragments
                    || cfg.clean_empty_links
                    || cfg.clean_blank_lines
                    || cfg.clean_redundant_links
                    || cfg.clean_self_link_headings
                    || cfg.clean_empty_images
                    || cfg.clean_empty_link_text
                {
                    opts.clean = Some(CleanConfig {
                        urls: cfg.clean_urls,
                        fragments: cfg.clean_fragments,
                        empty_links: cfg.clean_empty_links,
                        blank_lines: cfg.clean_blank_lines,
                        redundant_links: cfg.clean_redundant_links,
                        self_link_headings: cfg.clean_self_link_headings,
                        empty_images: cfg.clean_empty_images,
                        empty_link_text: cfg.clean_empty_link_text,
                        ..Default::default()
                    });
                }
                if cfg.minimal
                    || cfg.isolate_main
                    || cfg.frontmatter
                    || cfg.tailwind
                    || !cfg.filter_include.is_empty()
                    || !cfg.filter_exclude.is_empty()
                    || cfg.filter_process_children
                    || !cfg.frontmatter_meta_fields.is_empty()
                    || !cfg.extraction_selectors.is_empty()
                {
                    let mut plugins = PluginConfig::default();
                    if cfg.minimal
                        || !cfg.filter_include.is_empty()
                        || !cfg.filter_exclude.is_empty()
                        || cfg.filter_process_children
                    {
                        plugins.filter = Some(FilterConfig {
                            include: (!cfg.filter_include.is_empty())
                                .then_some(cfg.filter_include.clone()),
                            exclude: if !cfg.filter_exclude.is_empty() {
                                Some(cfg.filter_exclude.clone())
                            } else if cfg.minimal {
                                Some(vec![
                                    "nav".to_string(),
                                    "footer".to_string(),
                                    "aside".to_string(),
                                    "form".to_string(),
                                ])
                            } else {
                                None
                            },
                            process_children: cfg.filter_process_children.then_some(true),
                            ..Default::default()
                        });
                    }
                    if cfg.isolate_main {
                        plugins.isolate_main = Some(IsolateMainConfig::default());
                    }
                    if cfg.frontmatter {
                        plugins.frontmatter = Some(FrontmatterConfig {
                            additional_fields: None,
                            meta_fields: (!cfg.frontmatter_meta_fields.is_empty())
                                .then_some(cfg.frontmatter_meta_fields.clone()),
                        });
                    }
                    if cfg.tailwind {
                        plugins.tailwind = Some(TailwindConfig::default());
                    }
                    if !cfg.extraction_selectors.is_empty() {
                        plugins.extraction = Some(ExtractionConfig {
                            selectors: cfg.extraction_selectors.clone(),
                        });
                    }
                    opts.plugins = Some(plugins);
                }
                html_to_markdown(html, opts)
            });
        }
        _ => {
            let extractor_key = extractor_key.to_string();
            let states = states.clone();
            let parsed_url = parsed_url.clone();
            runner.run(output_name, move |html| {
                run_cli_extractor(&extractor_key, html, &states, &parsed_url)
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
        "turndown" => run_turndown(
            html,
            &states
                .states
                .get("turndown")
                .map(|s| s.config.turndown.clone())
                .unwrap_or_default(),
            &["noscript"]
                .into_iter()
                .chain(DEFAULT_SKIP_TAGS.iter().copied())
                .collect::<Vec<_>>(),
        ),
        "percollate" => run_percollate(
            html,
            &states
                .states
                .get("percollate")
                .map(|s| s.config.percollate.clone())
                .unwrap_or_default(),
        ),
        "trafilatura" => run_trafilatura(
            html,
            &states
                .states
                .get("trafilatura")
                .map(|s| s.config.trafilatura.clone())
                .unwrap_or_default(),
        ),
        "html2text-py" => run_html2text_py(
            html,
            &states
                .states
                .get("html2text-py")
                .map(|s| s.config.html2text_py.clone())
                .unwrap_or_default(),
        ),
        "markdownify" => run_markdownify(
            html,
            &states
                .states
                .get("markdownify")
                .map(|s| s.config.markdownify.clone())
                .unwrap_or_default(),
        ),
        "lightpanda" => run_lightpanda(
            parsed_url,
            &states
                .states
                .get("lightpanda")
                .map(|s| s.config.lightpanda.clone())
                .unwrap_or_default(),
        ),
        "webclaw" => run_webclaw(html, states),
        "e2m" => run_e2m(html, states),
        "html-to-markdown-go" => run_html_to_markdown_go(
            html,
            parsed_url,
            &states
                .states
                .get("html-to-markdown-go")
                .map(|s| s.config.html_to_markdown_go.clone())
                .unwrap_or_default(),
        ),
        _ => String::new(),
    }
}

fn run_turndown(html: &str, cfg: &TurndownConfig, remove_tags: &[&str]) -> String {
    let tmp = std::env::temp_dir().join(format!("turndown_{}.html", uuid::Uuid::new_v4()));
    let _ = std::fs::write(&tmp, html);
    let options = serde_json::json!({
        "headingStyle": cfg.heading_style,
        "hr": cfg.hr,
        "bulletListMarker": cfg.bullet_list_marker,
        "codeBlockStyle": cfg.code_block_style,
        "fence": cfg.fence,
        "emDelimiter": cfg.em_delimiter,
        "strongDelimiter": cfg.strong_delimiter,
        "linkStyle": cfg.link_style,
        "linkReferenceStyle": cfg.link_reference_style,
        "preformattedCode": cfg.preformatted_code,
    });
    let remove_tags_json = serde_json::to_string(remove_tags).unwrap_or_else(|_| "[]".to_string());
    let node_code = r#"const fs = require('fs'); const td = require('/home/jan/git/turndown'); const options = JSON.parse(process.argv[2]); const removeTags = JSON.parse(process.argv[3]); const svc = new td(options); for (const tag of removeTags) svc.remove(tag); const html = fs.readFileSync(process.argv[1], 'utf8'); process.stdout.write(svc.turndown(html))"#;
    let out = std::process::Command::new("node")
        .args([
            "-e",
            node_code,
            tmp.to_str().unwrap(),
            &options.to_string(),
            &remove_tags_json,
        ])
        .output();

    let _ = std::fs::remove_file(&tmp);
    match out {
        Ok(o) => {
            if !o.status.success() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return format!(
                    "[ERROR] turndown failed (exit {}): {}\n",
                    o.status,
                    stderr.trim()
                );
            }
            let stdout = String::from_utf8_lossy(&o.stdout);
            if stdout.is_empty() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return format!(
                    "[ERROR] turndown returned empty output. stderr: {}\n",
                    stderr.trim()
                );
            }
            stdout.to_string()
        }
        Err(e) => format!("[ERROR] node failed: {}\n", e),
    }
}

fn normalize_percollate_marker(value: &str, allowed: &[char]) -> Option<char> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut chars = trimmed.chars();
    let first = chars.next()?;
    if !allowed.contains(&first) {
        return None;
    }
    if chars.all(|ch| ch == first) {
        Some(first)
    } else {
        None
    }
}

fn build_percollate_args(input_path: &std::path::Path, cfg: &PercollateConfig) -> Vec<String> {
    let mut args = vec![
        "/home/jan/git/percollate/cli.js".to_string(),
        "md".to_string(),
        "-o".to_string(),
        "-".to_string(),
    ];
    if cfg.inline_images {
        args.push("--inline".to_string());
    }
    args.push(if cfg.hyphenate {
        "--hyphenate".to_string()
    } else {
        "--no-hyphenate".to_string()
    });
    args.push(format!(
        "--md.fences={}",
        if cfg.fences { "true" } else { "false" }
    ));
    if let Some(fence) = normalize_percollate_marker(&cfg.fence, &['`', '~']) {
        args.push(format!("--md.fence={fence}"));
    }
    if let Some(emphasis) = normalize_percollate_marker(&cfg.emphasis, &['_', '*']) {
        args.push(format!("--md.emphasis={emphasis}"));
    }
    if let Some(strong) = normalize_percollate_marker(&cfg.strong, &['_', '*']) {
        args.push(format!("--md.strong={strong}"));
    }
    args.push(format!(
        "--md.resourceLink={}",
        if cfg.resource_link { "true" } else { "false" }
    ));
    if let Some(rule) = normalize_percollate_marker(&cfg.rule, &['-', '*', '_']) {
        args.push(format!("--md.rule={rule}"));
    }
    args.push(input_path.to_string_lossy().to_string());
    args
}

fn run_percollate(html: &str, cfg: &PercollateConfig) -> String {
    let tmp = std::env::temp_dir().join(format!("percollate_in_{}.html", uuid::Uuid::new_v4()));
    let _ = std::fs::write(&tmp, html);
    let args = build_percollate_args(&tmp, cfg);
    let out = std::process::Command::new("node").args(&args).output();

    let _ = std::fs::remove_file(&tmp);
    match out {
        Ok(o) => {
            if !o.status.success() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return format!(
                    "[ERROR] percollate failed (exit {}): {}\n",
                    o.status,
                    stderr.trim()
                );
            }
            let stdout = String::from_utf8_lossy(&o.stdout);
            if stdout.is_empty() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return format!(
                    "[ERROR] percollate returned empty output. stderr: {}\n",
                    stderr.trim()
                );
            }
            stdout.to_string()
        }
        Err(e) => format!("[ERROR] percollate failed: {}\n", e),
    }
}

fn run_trafilatura(html: &str, cfg: &TrafilaturaConfig) -> String {
    let tmp = std::env::temp_dir().join(format!("trafilatura_{}.html", uuid::Uuid::new_v4()));
    let _ = std::fs::write(&tmp, html);
    let cfg_json = serde_json::to_string(cfg).unwrap();
    let script = r#"import json, sys, trafilatura; cfg = json.loads(sys.argv[2]); html = open(sys.argv[1]).read(); result = trafilatura.extract(html, output_format='markdown', favor_precision=cfg['favor_precision'], favor_recall=cfg['favor_recall'], include_comments=cfg['include_comments'], include_tables=cfg['include_tables'], include_images=cfg['include_images'], include_formatting=cfg['include_formatting'], include_links=cfg['include_links'], deduplicate=cfg['deduplicate'], with_metadata=cfg['with_metadata']); print(result if result else '', end='')"#;
    let out = std::process::Command::new("uv")
        .args(["run", "--", "python3", "-c", script])
        .arg(tmp.to_str().unwrap())
        .arg(cfg_json)
        .output();

    let _ = std::fs::remove_file(&tmp);
    match out {
        Ok(o) => {
            if !o.status.success() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return format!(
                    "[ERROR] trafilatura failed (exit {}): {}\n",
                    o.status,
                    stderr.trim()
                );
            }
            let stdout = String::from_utf8_lossy(&o.stdout);
            if stdout.is_empty() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return format!(
                    "[ERROR] trafilatura returned empty output. stderr: {}\n",
                    stderr.trim()
                );
            }
            stdout.to_string()
        }
        Err(e) => format!("[ERROR] uv failed: {}\n", e),
    }
}

fn run_html2text_py(html: &str, cfg: &Html2TextPythonConfig) -> String {
    let tmp = std::env::temp_dir().join(format!("h2t_py_{}.html", uuid::Uuid::new_v4()));
    let _ = std::fs::write(&tmp, html);
    let cfg_json = serde_json::to_string(cfg).unwrap();
    let script = r#"from html2text import HTML2Text; import json, sys; cfg = json.loads(sys.argv[2]); h = HTML2Text(); h.ignore_links = cfg['ignore_links']; h.ignore_images = cfg['ignore_images']; h.ignore_emphasis = cfg['ignore_emphasis']; h.body_width = cfg['body_width']; h.unicode_snob = cfg['unicode_snob']; h.escape_snob = cfg['escape_snob']; h.inline_links = cfg['inline_links']; h.google_doc = cfg['google_doc']; h.dash_unordered_list = cfg['dash_unordered_list']; html = open(sys.argv[1]).read(); print(h.handle(html), end='')"#;
    let out = std::process::Command::new("uv")
        .args(["run", "--", "python3", "-c", script])
        .arg(tmp.to_str().unwrap())
        .arg(cfg_json)
        .output();

    let _ = std::fs::remove_file(&tmp);
    match out {
        Ok(o) => {
            if !o.status.success() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return format!(
                    "[ERROR] html2text.py failed (exit {}): {}\n",
                    o.status,
                    stderr.trim()
                );
            }
            let stdout = String::from_utf8_lossy(&o.stdout);
            if stdout.is_empty() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return format!(
                    "[ERROR] html2text.py returned empty output. stderr: {}\n",
                    stderr.trim()
                );
            }
            stdout.to_string()
        }
        Err(e) => format!("[ERROR] uv failed: {}\n", e),
    }
}

fn build_markdownify_config_json(cfg: &MarkdownifyConfig) -> Result<String, String> {
    fn normalize_markdownify_symbolic(value: &str, mappings: &[(&str, &str)]) -> String {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return String::new();
        }
        for (symbolic, resolved) in mappings {
            if trimmed.eq_ignore_ascii_case(symbolic) {
                return (*resolved).to_string();
            }
        }
        trimmed.to_string()
    }

    let strip = cfg
        .strip
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let convert = cfg
        .convert
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    if !strip.is_empty() && !convert.is_empty() {
        return Err("markdownify options 'strip' and 'convert' are mutually exclusive".to_string());
    }

    let heading_style = normalize_markdownify_symbolic(
        &cfg.heading_style,
        &[
            ("ATX", "atx"),
            ("ATX_CLOSED", "atx_closed"),
            ("SETEXT", "underlined"),
            ("UNDERLINED", "underlined"),
        ],
    );
    let strong_em_symbol = normalize_markdownify_symbolic(
        &cfg.strong_em_symbol,
        &[("ASTERISK", "*"), ("UNDERSCORE", "_")],
    );
    let newline_style = normalize_markdownify_symbolic(
        &cfg.newline_style,
        &[("SPACES", "spaces"), ("BACKSLASH", "backslash")],
    );
    let strip_document = normalize_markdownify_symbolic(
        &cfg.strip_document,
        &[
            ("STRIP", "strip"),
            ("LSTRIP", "lstrip"),
            ("RSTRIP", "rstrip"),
        ],
    );
    let strip_pre = normalize_markdownify_symbolic(
        &cfg.strip_pre,
        &[("STRIP", "strip"), ("STRIP_ONE", "strip_one")],
    );

    serde_json::to_string(&serde_json::json!({
        "strip": strip,
        "convert": convert,
        "autolinks": cfg.autolinks,
        "default_title": cfg.default_title,
        "heading_style": heading_style,
        "bullets": cfg.bullets,
        "strong_em_symbol": strong_em_symbol,
        "sub_symbol": cfg.sub_symbol,
        "sup_symbol": cfg.sup_symbol,
        "newline_style": newline_style,
        "code_language": cfg.code_language,
        "escape_asterisks": cfg.escape_asterisks,
        "escape_underscores": cfg.escape_underscores,
        "escape_misc": cfg.escape_misc,
        "keep_inline_images_in": cfg
            .keep_inline_images_in
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>(),
        "table_infer_header": cfg.table_infer_header,
        "wrap": cfg.wrap,
        "wrap_width": cfg.wrap_width,
        "strip_document": (!strip_document.is_empty()).then_some(strip_document),
        "strip_pre": (!strip_pre.is_empty()).then_some(strip_pre),
        "bs4_options": (!cfg.bs4_parser.trim().is_empty())
            .then_some(serde_json::Value::String(cfg.bs4_parser.trim().to_string())),
    }))
    .map_err(|error| error.to_string())
}

fn run_markdownify(html: &str, cfg: &MarkdownifyConfig) -> String {
    let tmp = std::env::temp_dir().join(format!("markdownify_{}.html", uuid::Uuid::new_v4()));
    let _ = std::fs::write(&tmp, html);
    let cfg_json = match build_markdownify_config_json(cfg) {
        Ok(cfg_json) => cfg_json,
        Err(error) => return format!("[ERROR] markdownify config invalid: {}\n", error),
    };
    let script = r#"from markdownify import markdownify as md; import json, sys; cfg = json.loads(sys.argv[2]); kwargs = {'autolinks': cfg['autolinks'], 'default_title': cfg['default_title'], 'heading_style': cfg['heading_style'], 'bullets': cfg['bullets'], 'strong_em_symbol': cfg['strong_em_symbol'], 'sub_symbol': cfg['sub_symbol'], 'sup_symbol': cfg['sup_symbol'], 'newline_style': cfg['newline_style'], 'code_language': cfg['code_language'], 'escape_asterisks': cfg['escape_asterisks'], 'escape_underscores': cfg['escape_underscores'], 'escape_misc': cfg['escape_misc'], 'keep_inline_images_in': cfg['keep_inline_images_in'], 'table_infer_header': cfg['table_infer_header'], 'wrap': cfg['wrap'], 'wrap_width': cfg['wrap_width']}; html = open(sys.argv[1]).read(); strip = cfg['strip']; convert = cfg['convert']; strip_document = cfg['strip_document']; strip_pre = cfg['strip_pre']; bs4_options = cfg['bs4_options']; kwargs['strip'] = strip if strip else None; kwargs['convert'] = convert if convert else None; kwargs['strip_document'] = strip_document; kwargs['strip_pre'] = strip_pre; kwargs['bs4_options'] = bs4_options; kwargs = {key: value for key, value in kwargs.items() if value is not None}; print(md(html, **kwargs), end='')"#;
    let out = std::process::Command::new("uv")
        .args([
            "run",
            "--with",
            "markdownify",
            "--",
            "python3",
            "-c",
            script,
        ])
        .arg(tmp.to_str().unwrap())
        .arg(cfg_json)
        .output();

    let _ = std::fs::remove_file(&tmp);
    match out {
        Ok(o) => {
            if !o.status.success() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return format!(
                    "[ERROR] markdownify failed (exit {}): {}\n",
                    o.status,
                    stderr.trim()
                );
            }
            let stdout = String::from_utf8_lossy(&o.stdout);
            if stdout.is_empty() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return format!(
                    "[ERROR] markdownify returned empty output. stderr: {}\n",
                    stderr.trim()
                );
            }
            stdout.to_string()
        }
        Err(e) => format!("[ERROR] uv failed: {}\n", e),
    }
}

fn build_lightpanda_args(parsed_url: &url::Url, cfg: &LightpandaConfig) -> Vec<String> {
    let mut args = vec![
        "exec".to_string(),
        "lightpanda".to_string(),
        "lightpanda".to_string(),
        "fetch".to_string(),
        "--dump".to_string(),
        "markdown".to_string(),
    ];
    if !cfg.wait_until.is_empty() {
        args.push("--wait-until".to_string());
        args.push(cfg.wait_until.clone());
    }
    if cfg.wait_ms > 0 {
        args.push("--wait-ms".to_string());
        args.push(cfg.wait_ms.to_string());
    }
    args.push(parsed_url.to_string());
    args
}

fn run_lightpanda(parsed_url: &url::Url, cfg: &LightpandaConfig) -> String {
    let args = build_lightpanda_args(parsed_url, cfg);
    let out = std::process::Command::new("docker").args(&args).output();

    match out {
        Ok(o) => {
            if !o.status.success() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return format!(
                    "[ERROR] lightpanda docker exec failed (exit {}): {}\n",
                    o.status,
                    stderr.trim()
                );
            }
            let stdout = String::from_utf8_lossy(&o.stdout);
            if stdout.is_empty() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return format!(
                    "[ERROR] lightpanda returned empty output. stderr: {}\n",
                    stderr.trim()
                );
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
    args.push(if cfg.format.is_empty() {
        "markdown".to_string()
    } else {
        cfg.format.clone()
    });
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
                return format!(
                    "[ERROR] webclaw failed (exit {}): {}\n",
                    o.status,
                    stderr.trim()
                );
            }
            let stdout = String::from_utf8_lossy(&o.stdout);
            if stdout.is_empty() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return format!(
                    "[ERROR] webclaw returned empty output. stderr: {}\n",
                    stderr.trim()
                );
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
    let engine = if cfg.engine.is_empty() {
        "unstructured"
    } else {
        &cfg.engine
    };
    let work_dir = std::env::temp_dir().join(format!("e2m_{}", uuid::Uuid::new_v4()));
    let _ = std::fs::create_dir_all(&work_dir);
    let tmp = work_dir.join("input.html");
    let _ = std::fs::write(&tmp, html);
    let langs = serde_json::to_string(&cfg.langs).unwrap();
    let script = r#"
import json
import sys
from pathlib import Path

from wisup_e2m import HtmlParser

work_dir = Path(sys.argv[1]).resolve()
input_path = Path(sys.argv[2]).resolve()
langs = json.loads(sys.argv[4])
skip_headers_and_footers = sys.argv[5] == "true"
include_image_link_in_text = sys.argv[6] == "true"

parser = HtmlParser(engine=sys.argv[3], langs=langs)
elements = parser.unstructured_parse_func(
    filename=str(input_path),
    encoding="utf-8",
    languages=parser.config.langs,
    skip_headers_and_footers=skip_headers_and_footers,
    include_metadata=True,
)

sanitized = []
missing_image_paths = 0
for element in elements:
    if getattr(element, "category", None) == "Image":
        metadata = getattr(element, "metadata", None)
        image_path = getattr(metadata, "image_path", None) if metadata is not None else None
        if not image_path:
            missing_image_paths += 1
            if getattr(element, "text", None):
                element.category = "Text"
            else:
                continue
    sanitized.append(element)

if missing_image_paths:
    print(
        f"[e2m] skipped {missing_image_paths} image element(s) without image_path",
        file=sys.stderr,
    )

result = parser._prepare_unstructured_data_to_e2m_parsed_data(
    sanitized,
    add_title_marker=True,
    include_image_link_in_text=include_image_link_in_text,
    work_dir=str(work_dir),
    image_dir=str(work_dir / "figures"),
    relative_path=True,
)
print(result.text, end="")
"#;
    let out = std::process::Command::new("uv")
        .args([
            "run",
            "--",
            "python3",
            "-c",
            script,
            work_dir.to_str().unwrap(),
            tmp.to_str().unwrap(),
            engine,
            &langs,
            if cfg.skip_headers_and_footers {
                "true"
            } else {
                "false"
            },
            if cfg.include_image_link_in_text {
                "true"
            } else {
                "false"
            },
        ])
        .output();

    let _ = std::fs::remove_file(&tmp);
    let _ = std::fs::remove_dir_all(&work_dir);
    match out {
        Ok(o) => {
            if !o.status.success() {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return format!(
                    "[ERROR] e2m failed (exit {}): {}\n",
                    o.status,
                    stderr.trim()
                );
            }
            let stdout = String::from_utf8_lossy(&o.stdout);
            let stderr = String::from_utf8_lossy(&o.stderr);
            if !stderr.trim().is_empty() {
                eprintln!("[e2m] {}", stderr.trim());
            }
            if stdout.is_empty() {
                return format!(
                    "[ERROR] e2m returned empty output. stderr: {}\n",
                    stderr.trim()
                );
            }
            stdout.to_string()
        }
        Err(e) => format!("[ERROR] uv failed: {}\n", e),
    }
}

fn build_html_to_markdown_go_args(
    parsed_url: &url::Url,
    cfg: &HtmlToMarkdownGoConfig,
) -> Vec<String> {
    let domain = if cfg.domain.trim().is_empty() {
        parsed_url.origin().ascii_serialization()
    } else {
        cfg.domain.trim().to_string()
    };
    let mut args = vec![format!("--domain={}", domain)];
    if !cfg.include_selector.trim().is_empty() {
        args.push(format!(
            "--include-selector={}",
            cfg.include_selector.trim()
        ));
    }
    if !cfg.exclude_selector.trim().is_empty() {
        args.push(format!(
            "--exclude-selector={}",
            cfg.exclude_selector.trim()
        ));
    }
    for plugin in cfg
        .plugins
        .iter()
        .map(|plugin| plugin.trim())
        .filter(|plugin| !plugin.is_empty() && *plugin != "commonmark" && *plugin != "base")
    {
        args.push(format!("--plugin-{}", plugin));
    }
    args
}

fn run_html_to_markdown_go(
    html: &str,
    parsed_url: &url::Url,
    cfg: &HtmlToMarkdownGoConfig,
) -> String {
    let args = build_html_to_markdown_go_args(parsed_url, cfg);
    let out = std::process::Command::new("/tmp/html2markdown")
        .args(&args)
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
                        return format!(
                            "[ERROR] html-to-markdown-go failed (exit {}): {}\n",
                            o.status,
                            stderr.trim()
                        );
                    }
                    let stdout = String::from_utf8_lossy(&o.stdout);
                    if stdout.is_empty() {
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        return format!(
                            "[ERROR] html-to-markdown-go returned empty output. stderr: {}\n",
                            stderr.trim()
                        );
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
    use crate::extractor_config::{
        ExtractorConfig, HtmdConfig, Html2TextConfig, HtmlToMarkdownGoConfig, LightpandaConfig,
        PercollateConfig,
    };

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
                ..Default::default()
            },
            ..Default::default()
        };
        let candidate = ExtractorConfig {
            html2text: Html2TextConfig {
                max_wrap_width: 120,
                ..Default::default()
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

        assert_eq!(score.extractor_results[0].name, "html2text baseline");
        assert_eq!(score.extractor_results[0].extractor_key, "html2text");
        assert_eq!(
            score.extractor_results[1].name,
            "html2text current settings"
        );
        assert_eq!(score.extractor_results[1].extractor_key, "html2text");

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
                ..Default::default()
            },
            ..Default::default()
        };
        let candidate = ExtractorConfig {
            htmd: HtmdConfig {
                skip_tags: Vec::new(),
                heading_style: "setex".to_string(),
                ..Default::default()
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
    fn compare_settings_applies_turndown_heading_style() {
        let (store, dir) = test_store();
        let html = "<html><body><h1>Title</h1><p>Body</p></body></html>";
        let baseline = ExtractorConfig {
            turndown: TurndownConfig {
                heading_style: "atx".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        let candidate = ExtractorConfig {
            turndown: TurndownConfig {
                heading_style: "setext".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        let score = compare_single_extractor_settings(
            "https://example.com",
            html,
            "turndown",
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
    fn turndown_removes_script_content() {
        let output = run_turndown(
            "<html><body><article><p>Body</p></article><script>if(window.foo){bar()}</script></body></html>",
            &TurndownConfig::default(),
            &["script", "style", "noscript"],
        );

        assert!(output.contains("Body"));
        assert!(!output.contains("window.foo"));
        assert!(!output.contains("bar()"));
    }

    #[test]
    fn compare_settings_applies_percollate_markdown_options() {
        let (store, dir) = test_store();
        let html = "<html><body><pre><code>code</code></pre><hr><p>Body</p></body></html>";
        let baseline = ExtractorConfig {
            percollate: PercollateConfig {
                fence: "`".to_string(),
                rule: "-".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        let candidate = ExtractorConfig {
            percollate: PercollateConfig {
                fence: "~".to_string(),
                rule: "*".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        let score = compare_single_extractor_settings(
            "https://example.com",
            html,
            "percollate",
            baseline,
            candidate,
            &store,
        );

        let baseline_output =
            std::fs::read_to_string(&score.extractor_results[0].output_file).unwrap();
        let candidate_output =
            std::fs::read_to_string(&score.extractor_results[1].output_file).unwrap();

        assert!(baseline_output.contains("```"));
        assert!(candidate_output.contains("~~~"));
        assert!(baseline_output.contains("\n---\n") || baseline_output.contains("\n- - -\n"));
        assert!(candidate_output.contains("\n***\n") || candidate_output.contains("\n* * *\n"));
        assert_ne!(baseline_output, candidate_output);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn compare_settings_applies_trafilatura_metadata_option() {
        let (store, dir) = test_store();
        let html = "<html><head><title>Title</title></head><body><article><p>Body</p></article></body></html>";
        let baseline = ExtractorConfig {
            trafilatura: TrafilaturaConfig {
                with_metadata: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let candidate = ExtractorConfig {
            trafilatura: TrafilaturaConfig {
                with_metadata: true,
                ..Default::default()
            },
            ..Default::default()
        };

        let score = compare_single_extractor_settings(
            "https://example.com",
            html,
            "trafilatura",
            baseline,
            candidate,
            &store,
        );

        let baseline_output =
            std::fs::read_to_string(&score.extractor_results[0].output_file).unwrap();
        let candidate_output =
            std::fs::read_to_string(&score.extractor_results[1].output_file).unwrap();

        assert!(!baseline_output.contains("---\ntitle:"));
        assert!(candidate_output.contains("---\ntitle: Title"));
        assert_ne!(baseline_output, candidate_output);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(feature = "dom_smoothie")]
    #[test]
    fn compare_settings_applies_dom_smoothie_candidate_selection_mode() {
        let (store, dir) = test_store();
        let html = "<html><body><main><div><div><div><div><p>Alice was beginning to get very tired of sitting by her sister on the bank.</p></div></div></div><div><div><div><p>So she was considering in her own mind whether the pleasure of making a daisy-chain would be worth the trouble of getting up and picking the daisies.</p></div></div></div></main></body></html>";
        let baseline = ExtractorConfig {
            dom_smoothie: crate::extractor_config::DomSmoothieConfig {
                candidate_select_mode: "readability".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        let candidate = ExtractorConfig {
            dom_smoothie: crate::extractor_config::DomSmoothieConfig {
                candidate_select_mode: "dom_smoothie".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        let score = compare_single_extractor_settings(
            "https://example.com",
            html,
            "dom_smoothie",
            baseline,
            candidate,
            &store,
        );

        let baseline_output =
            std::fs::read_to_string(&score.extractor_results[0].output_file).unwrap();
        let candidate_output =
            std::fs::read_to_string(&score.extractor_results[1].output_file).unwrap();

        assert!(!baseline_output.contains("Alice was beginning"));
        assert!(baseline_output.contains("So she was considering"));
        assert!(candidate_output.contains("Alice was beginning"));
        assert!(candidate_output.contains("So she was considering"));
        assert_ne!(baseline_output, candidate_output);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(feature = "dom_smoothie")]
    #[test]
    fn preview_dom_smoothie_disable_json_ld_does_not_change_text_output() {
        let html = "<html><head><script type=\"application/ld+json\">{\"@context\":\"https://schema.org\",\"@type\":\"Article\",\"url\":\"https://example.com/from-json-ld\",\"headline\":\"JSON-LD headline\"}</script></head><body><article><p>Visible article body.</p></article></body></html>";
        let baseline = preview_single_extractor_settings(
            "https://example.com",
            html,
            "dom_smoothie",
            ExtractorConfig {
                dom_smoothie: crate::extractor_config::DomSmoothieConfig {
                    disable_json_ld: false,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let candidate = preview_single_extractor_settings(
            "https://example.com",
            html,
            "dom_smoothie",
            ExtractorConfig {
                dom_smoothie: crate::extractor_config::DomSmoothieConfig {
                    disable_json_ld: true,
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        assert_eq!(baseline.output, candidate.output);
        assert!(baseline.output.contains("Visible article body"));
    }

    #[cfg(feature = "dom_smoothie")]
    #[test]
    fn preview_dom_smoothie_class_preservation_does_not_change_text_output() {
        let html = "<html><body><article><div class=\"keep-me drop-me\"><p>Visible article body.</p></div></article></body></html>";
        let baseline = preview_single_extractor_settings(
            "https://example.com",
            html,
            "dom_smoothie",
            ExtractorConfig {
                dom_smoothie: crate::extractor_config::DomSmoothieConfig {
                    keep_classes: false,
                    classes_to_preserve: Vec::new(),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let candidate = preview_single_extractor_settings(
            "https://example.com",
            html,
            "dom_smoothie",
            ExtractorConfig {
                dom_smoothie: crate::extractor_config::DomSmoothieConfig {
                    keep_classes: true,
                    classes_to_preserve: vec!["keep-me".to_string()],
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        assert_eq!(baseline.output, candidate.output);
        assert!(baseline.output.contains("Visible article body"));
    }

    #[test]
    fn e2m_handles_images_without_image_paths() {
        let html = "<html><body><h1>Title</h1><p>Before</p><img src=\"https://example.com/a.png\" alt=\"A\"><p>After</p></body></html>";
        let states = single_extractor_state(
            "e2m",
            ExtractorConfig {
                e2m: crate::extractor_config::E2mConfig {
                    include_image_link_in_text: true,
                    langs: vec!["en".to_string()],
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let output = run_e2m(html, &states);

        assert!(!output.starts_with("[ERROR]"));
        assert!(output.contains("Title"));
        assert!(output.contains("Before"));
        assert!(output.contains("After"));
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
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        assert_eq!(preview.extractor_key, "html2text");
        assert!(preview.output.contains("alpha beta"));
        assert!(preview.output.matches('\n').count() > 1);
    }

    #[test]
    fn build_percollate_args_respects_config() {
        let args = build_percollate_args(
            std::path::Path::new("/tmp/input.html"),
            &PercollateConfig {
                inline_images: true,
                hyphenate: false,
                fences: false,
                fence: "~~~".to_string(),
                emphasis: "*".to_string(),
                strong: "**".to_string(),
                resource_link: false,
                rule: "***".to_string(),
            },
        );

        assert!(args.iter().any(|arg| arg == "--inline"));
        assert!(args.iter().any(|arg| arg == "--no-hyphenate"));
        assert!(args.iter().any(|arg| arg == "--md.fences=false"));
        assert!(args.iter().any(|arg| arg == "--md.fence=~"));
        assert!(args.iter().any(|arg| arg == "--md.emphasis=*"));
        assert!(args.iter().any(|arg| arg == "--md.strong=*"));
        assert!(args.iter().any(|arg| arg == "--md.resourceLink=false"));
        assert!(args.iter().any(|arg| arg == "--md.rule=*"));
    }

    #[test]
    fn build_markdownify_config_json_rejects_conflicting_tag_filters() {
        let error = build_markdownify_config_json(&MarkdownifyConfig {
            strip: vec!["a".to_string()],
            convert: vec!["p".to_string()],
            ..Default::default()
        })
        .unwrap_err();

        assert!(error.contains("mutually exclusive"));
    }

    #[test]
    fn build_markdownify_config_json_normalizes_optional_fields() {
        let payload = build_markdownify_config_json(&MarkdownifyConfig {
            strip: vec![" a ".to_string(), String::new()],
            heading_style: "SETEXT".to_string(),
            strong_em_symbol: "ASTERISK".to_string(),
            newline_style: "BACKSLASH".to_string(),
            keep_inline_images_in: vec![" td ".to_string()],
            bs4_parser: "lxml".to_string(),
            wrap_width: None,
            strip_document: String::new(),
            strip_pre: "STRIP_ONE".to_string(),
            ..Default::default()
        })
        .unwrap();
        let payload: serde_json::Value = serde_json::from_str(&payload).unwrap();

        assert_eq!(payload["strip"], serde_json::json!(["a"]));
        assert_eq!(payload["heading_style"], serde_json::json!("underlined"));
        assert_eq!(payload["strong_em_symbol"], serde_json::json!("*"));
        assert_eq!(payload["newline_style"], serde_json::json!("backslash"));
        assert_eq!(payload["keep_inline_images_in"], serde_json::json!(["td"]));
        assert_eq!(payload["wrap_width"], serde_json::Value::Null);
        assert_eq!(payload["strip_document"], serde_json::Value::Null);
        assert_eq!(payload["strip_pre"], serde_json::json!("strip_one"));
        assert_eq!(payload["bs4_options"], serde_json::json!("lxml"));
    }

    #[test]
    fn build_lightpanda_args_respects_wait_config() {
        let url = url::Url::parse("https://example.com/article").unwrap();
        let args = build_lightpanda_args(
            &url,
            &LightpandaConfig {
                wait_until: "networkidle".to_string(),
                wait_ms: 2500,
                ..Default::default()
            },
        );

        assert!(args
            .windows(2)
            .any(|window| window == ["--wait-until", "networkidle"]));
        assert!(args
            .windows(2)
            .any(|window| window == ["--wait-ms", "2500"]));
        assert_eq!(args.last().unwrap(), "https://example.com/article");
    }

    #[test]
    fn build_html_to_markdown_go_args_respects_domain_and_plugins() {
        let url = url::Url::parse("https://example.com/path").unwrap();
        let args = build_html_to_markdown_go_args(
            &url,
            &HtmlToMarkdownGoConfig {
                domain: "https://docs.example.com".to_string(),
                plugins: vec![
                    "commonmark".to_string(),
                    "table".to_string(),
                    " strikethrough ".to_string(),
                ],
                include_selector: "article".to_string(),
                exclude_selector: ".ads".to_string(),
            },
        );

        assert_eq!(args[0], "--domain=https://docs.example.com");
        assert!(args.iter().any(|arg| arg == "--include-selector=article"));
        assert!(args.iter().any(|arg| arg == "--exclude-selector=.ads"));
        assert!(args.iter().any(|arg| arg == "--plugin-table"));
        assert!(args.iter().any(|arg| arg == "--plugin-strikethrough"));
        assert!(!args.iter().any(|arg| arg == "--plugin-commonmark"));
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
