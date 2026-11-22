use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, read_to_string};
#[cfg(feature = "reader-lm-api")]
use std::io::Read;
use std::io::{Seek, Write};
use std::path::PathBuf;
use uuid::Uuid;

use crate::acquisition::Acquisition;
use crate::extractor_config::{
    ExtractorStates, Html2TextPythonConfig, HtmlToMarkdownGoConfig, MarkdownifyConfig,
    PercollateConfig, TrafilaturaConfig, TurndownConfig, DEFAULT_SKIP_TAGS,
};
use crate::runner::{ExtractionStatus, Runner};
use crate::runtime_config::ExecutionPolicy;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Score {
    pub id: String,
    pub url: String,
    pub acquisition: Acquisition,
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
    pub reduction_percent: Option<f64>,
    pub time_micros: u64,
    pub wall_time_micros: u64,
    pub status: ExtractionStatus,
    pub error: String,
    pub output_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractorPreview {
    pub name: String,
    pub extractor_key: String,
    pub output: String,
    pub output_size: usize,
    pub reduction_percent: Option<f64>,
    pub time_micros: u64,
    pub wall_time_micros: u64,
    pub status: ExtractionStatus,
    pub error: String,
}

impl Score {
    pub fn new(
        policy: &ExecutionPolicy,
        url: String,
        html: String,
        acquisition: Acquisition,
        states: &ExtractorStates,
        store: &ScoreStore,
    ) -> Result<Self, String> {
        let id = Uuid::now_v7().to_string();
        let timestamp = chrono_lite_now();
        let html_size = html.len();
        let parsed_url = url::Url::parse(&url).map_err(|error| error.to_string())?;
        let out_dir = store.create_run_dir(&id)?;
        let mut guard = OwnedRunDir::new(out_dir.clone());
        let source_html_file = out_dir.join("source.html");
        fs::write(&source_html_file, &html).map_err(|error| error.to_string())?;

        let mut runner = Runner::new(out_dir.clone(), html, policy.clone());
        let states = states.clone();

        for extractor_name in enabled_extractors(&states) {
            runner.run_isolated(extractor_name, extractor_name, parsed_url.as_str(), &states);
        }

        let outcomes = runner.into_stats();
        let extractor_results: Vec<ExtractorResult> = outcomes
            .iter()
            .map(|stat| ExtractorResult {
                name: stat.name.clone(),
                extractor_key: stat.name.clone(),
                output_size: stat.output_size,
                reduction_percent: stat
                    .status
                    .is_success()
                    .then(|| crate::metrics::reduction_percent(stat.output_size, html_size)),
                time_micros: stat.time_micros,
                wall_time_micros: stat.wall_time_micros,
                status: stat.status,
                error: stat.error.clone(),
                output_file: out_dir
                    .join(format!("{}.txt", stat.name))
                    .display()
                    .to_string(),
            })
            .collect();

        let result_record = Self {
            id,
            url,
            acquisition,
            html_size,
            timestamp,
            source_html_file: source_html_file.display().to_string(),
            settings_snapshot: states,
            grades: BTreeMap::new(),
            extractor_results,
        };
        guard.persist();
        Ok(result_record)
    }
}

fn enabled_extractors(states: &ExtractorStates) -> Vec<&str> {
    states
        .states
        .iter()
        .filter_map(|(name, state)| state.enabled.then_some(name.as_str()))
        .collect()
}

#[expect(
    clippy::too_many_lines,
    reason = "Exhaustive native extractor dispatch retains local per-extractor configurations; future structural splitting must preserve individual feature gates and output equivalence"
)]
pub fn run_named_extractor(
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
                    .map(|result| result.text)
                    .map_err(|error| format!("readability failed: {error:?}"))
            });
        }
        #[cfg(feature = "llm_readability")]
        "llm_readability" => {
            let parsed_url = parsed_url.clone();
            runner.run(output_name, move |html| {
                let mut html = std::io::Cursor::new(html.as_bytes());
                llm_readability::extractor::extract(&mut html, &parsed_url)
                    .map(|result| result.text)
                    .map_err(|error| format!("llm_readability failed: {error:?}"))
            });
        }
        #[cfg(feature = "html2text")]
        "html2text" => {
            let cfg = states
                .states
                .get("html2text")
                .map_or_default(|s| s.config.html2text.clone());
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
                    .map_err(|error| format!("html2text failed: {error}"))
            });
        }
        #[cfg(feature = "htmd")]
        "htmd" => {
            let cfg = states
                .states
                .get("htmd")
                .map_or_default(|s| s.config.htmd.clone());
            let legacy_skip_tags = states
                .states
                .get("htmd")
                .map_or_default(|s| s.config.skip_tags.clone());
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
                    builder = builder
                        .skip_tags(skip_tags.iter().map(std::string::String::as_str).collect());
                }
                builder
                    .build()
                    .convert(html)
                    .map_err(|error| format!("htmd failed: {error}"))
            });
        }
        #[cfg(feature = "html2md-rs")]
        "html2md-rs" => {
            let cfg = states
                .states
                .get("html2md-rs")
                .map_or_default(|s| s.config.html2md_rs.clone());
            let legacy_skip_tags = states
                .states
                .get("html2md-rs")
                .map_or_default(|s| s.config.skip_tags.clone());
            let tags = if cfg.ignore_tags.is_empty() {
                legacy_skip_tags
            } else {
                cfg.ignore_tags
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
                .map_err(|error| format!("html2md-rs failed: {error}"))
            });
        }
        #[cfg(feature = "nanohtml2text")]
        "nanohtml2text" => {
            runner.run(output_name, |html| Ok(nanohtml2text::html2text(html)));
        }
        #[cfg(feature = "mdka")]
        "mdka" => {
            let cfg = states
                .states
                .get("mdka")
                .map_or_default(|s| s.config.mdka.clone());
            runner.run(output_name, move |html| {
                Ok(mdka::html_to_markdown_with(
                    html,
                    &cfg.into_conversion_options()?,
                ))
            });
        }
        #[cfg(feature = "readable-readability")]
        "readable-readability" => {
            let cfg = states
                .states
                .get("readable-readability")
                .map_or_default(|s| s.config.readable_readability.clone());
            runner.run(output_name, move |html| {
                let mut parser = readable_readability::Readability::new();
                parser.strip_unlikelys(cfg.strip_unlikelys);
                parser.weight_classes(cfg.weight_classes);
                parser.clean_conditionally(cfg.clean_conditionally);
                parser.clean_attributes(cfg.clean_attributes);
                let (node, _) = parser.parse(html);
                Ok(node.text_contents())
            });
        }
        #[cfg(feature = "dom_smoothie")]
        "dom_smoothie" => {
            let cfg = states
                .states
                .get("dom_smoothie")
                .map_or_default(|s| s.config.dom_smoothie.clone());
            runner.run(output_name, move |html| {
                let dom_cfg = cfg;
                let mut parser =
                    dom_smoothie::Readability::new(html, None, Some(dom_cfg.into_config()))
                        .map_err(|error| format!("dom_smoothie setup failed: {error:?}"))?;
                parser
                    .parse()
                    .map(|result| result.text_content.to_string())
                    .map_err(|error| format!("dom_smoothie parse failed: {error:?}"))
            });
        }
        #[cfg(feature = "boilerpipe")]
        "boilerpipe" => {
            runner.run(output_name, |html| {
                Ok(boilerpipe::parse_document(html).content().to_string())
            });
        }
        #[cfg(feature = "august")]
        "august" => {
            let cfg = states
                .states
                .get("august")
                .map_or(usize::MAX, |s| s.config.augus_max_width);
            runner.run(output_name, move |html| Ok(august::convert(html, cfg)));
        }
        #[cfg(feature = "fast_html2md")]
        "fast_html2md" => {
            let cfg = states
                .states
                .get("fast_html2md")
                .map_or_default(|s| s.config.fast_html2md.clone());
            runner.run(output_name, move |html| {
                if cfg.preserve_spans {
                    Ok(fast_html2md::parse_html_extended(html, cfg.commonmark))
                } else {
                    Ok(fast_html2md::parse_html(html, cfg.commonmark))
                }
            });
        }
        #[cfg(feature = "html2md")]
        "html2md" => {
            let cfg = states
                .states
                .get("html2md")
                .map_or_default(|s| s.config.html2md.clone());
            runner.run(output_name, move |html| {
                if cfg.preserve_spans {
                    Ok(html2md::parse_html_extended(html))
                } else {
                    Ok(html2md::parse_html(html))
                }
            });
        }
        #[cfg(feature = "mdream")]
        "mdream" => {
            let cfg = states
                .states
                .get("mdream")
                .map_or_default(|s| s.config.mdream.clone());
            runner.run(output_name, move |html| {
                use mdream::{
                    html_to_markdown,
                    types::{
                        CleanConfig, ExtractionConfig, FilterConfig, FrontmatterConfig,
                        HtmlToMarkdownOptions, IsolateMainConfig, PluginConfig, TailwindConfig,
                    },
                };
                let mut opts = HtmlToMarkdownOptions::default();
                if !cfg.origin.is_empty() {
                    opts.origin = Some(cfg.origin.clone());
                }
                if cfg.clean_urls
                    || cfg.clean_fragments
                    || cfg.clean_empty_links
                    || cfg.clean_redundant_links
                    || cfg.clean_self_link_headings
                    || cfg.clean_empty_images
                    || cfg.clean_empty_link_text
                {
                    opts.clean = Some(CleanConfig {
                        urls: cfg.clean_urls,
                        fragments: cfg.clean_fragments,
                        empty_links: cfg.clean_empty_links,
                        redundant_links: cfg.clean_redundant_links,
                        self_link_headings: cfg.clean_self_link_headings,
                        empty_images: cfg.clean_empty_images,
                        empty_link_text: cfg.clean_empty_link_text,
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
                        });
                    }
                    if cfg.isolate_main {
                        plugins.isolate_main = Some(IsolateMainConfig);
                    }
                    if cfg.frontmatter {
                        plugins.frontmatter = Some(FrontmatterConfig {
                            additional_fields: None,
                            meta_fields: (!cfg.frontmatter_meta_fields.is_empty())
                                .then_some(cfg.frontmatter_meta_fields.clone()),
                        });
                    }
                    if cfg.tailwind {
                        plugins.tailwind = Some(TailwindConfig);
                    }
                    if !cfg.extraction_selectors.is_empty() {
                        plugins.extraction = Some(ExtractionConfig {
                            selectors: cfg.extraction_selectors,
                        });
                    }
                    opts.plugins = Some(plugins);
                }
                Ok(html_to_markdown(html, opts))
            });
        }
        _ => {
            let extractor_key = extractor_key.to_string();
            let states = states.clone();
            let parsed_url = parsed_url.clone();
            let policy = runner.policy().clone();
            runner.run(output_name, move |html| {
                run_cli_extractor(&policy, &extractor_key, html, &states, &parsed_url)
            });
        }
    }
}

pub fn compare_single_extractor_settings(
    policy: &ExecutionPolicy,
    url: &str,
    html: &str,
    extractor: &str,
    baseline_config: crate::extractor_config::ExtractorConfig,
    candidate_config: crate::extractor_config::ExtractorConfig,
    score_store: &ScoreStore,
) -> Result<Score, String> {
    let baseline_states = single_extractor_state(extractor, baseline_config);
    let candidate_states = single_extractor_state(extractor, candidate_config);

    let id = Uuid::now_v7().to_string();
    let timestamp = chrono_lite_now();
    let html_size = html.len();
    let parsed_url = url::Url::parse(url).map_err(|error| error.to_string())?;
    let out_dir = score_store.create_run_dir(&id)?;
    let mut guard = OwnedRunDir::new(out_dir.clone());
    let source_html_file = out_dir.join("source.html");
    fs::write(&source_html_file, html).map_err(|error| error.to_string())?;

    let mut runner = Runner::new(out_dir.clone(), html.to_string(), policy.clone());
    let baseline_name = format!("{extractor} baseline");
    let candidate_name = format!("{extractor} current settings");
    runner.run_isolated(
        extractor,
        &baseline_name,
        parsed_url.as_str(),
        &baseline_states,
    );
    runner.run_isolated(
        extractor,
        &candidate_name,
        parsed_url.as_str(),
        &candidate_states,
    );

    let extractor_results = runner
        .into_stats()
        .into_iter()
        .map(|stat| ExtractorResult {
            name: stat.name.clone(),
            extractor_key: extractor.to_string(),
            output_size: stat.output_size,
            reduction_percent: stat
                .status
                .is_success()
                .then(|| crate::metrics::reduction_percent(stat.output_size, html_size)),
            time_micros: stat.time_micros,
            wall_time_micros: stat.wall_time_micros,
            status: stat.status,
            error: stat.error,
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
        acquisition: Acquisition::SavedSnapshot,
        html_size,
        timestamp,
        source_html_file: source_html_file.display().to_string(),
        settings_snapshot,
        grades: BTreeMap::new(),
        extractor_results,
    };
    score_store.save(&score)?;
    guard.persist();
    score_store.prune()?;
    Ok(score)
}

pub fn preview_single_extractor_settings(
    policy: &ExecutionPolicy,
    url: &str,
    html: &str,
    extractor: &str,
    config: crate::extractor_config::ExtractorConfig,
) -> Result<ExtractorPreview, String> {
    let states = single_extractor_state(extractor, config);
    let parsed_url = url::Url::parse(url).map_err(|error| error.to_string())?;
    let html_size = html.len();
    let temporary = tempfile::tempdir().map_err(|error| error.to_string())?;
    let out_dir = temporary.path().to_path_buf();
    std::fs::write(out_dir.join("source.html"), html)
        .map_err(|error| format!("writing temporary preview snapshot: {error}"))?;
    let mut runner = Runner::new(out_dir.clone(), html.to_string(), policy.clone());
    runner.run_isolated(extractor, extractor, parsed_url.as_str(), &states);

    let stat = runner.into_stats().into_iter().next().unwrap_or_default();
    let output = std::fs::read_to_string(out_dir.join(format!("{extractor}.txt")))
        .map_err(|error| error.to_string())?;

    Ok(ExtractorPreview {
        name: format!("{extractor} current settings"),
        extractor_key: extractor.to_string(),
        output_size: stat.output_size,
        reduction_percent: stat
            .status
            .is_success()
            .then(|| crate::metrics::reduction_percent(stat.output_size, html_size)),
        time_micros: stat.time_micros,
        wall_time_micros: stat.wall_time_micros,
        status: stat.status,
        error: stat.error,
        output,
    })
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
    policy: &ExecutionPolicy,
    url: &str,
    html: &str,
    acquisition: Acquisition,
    states: &ExtractorStates,
    score_store: &ScoreStore,
) -> Result<Score, String> {
    let score = Score::new(
        policy,
        url.to_string(),
        html.to_string(),
        acquisition,
        states,
        score_store,
    )?;
    if let Err(error) = score_store.save(&score) {
        let _ = score_store.remove_artifacts(&score.id);
        return Err(error);
    }
    score_store.prune()?;
    Ok(score)
}

fn chrono_lite_now() -> String {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => duration.as_secs().to_string(),
        Err(before_epoch) => format!("-{}", before_epoch.duration().as_secs()),
    }
}

pub fn run_cli_extractor(
    policy: &ExecutionPolicy,
    extractor_key: &str,
    html: &str,
    states: &ExtractorStates,
    parsed_url: &url::Url,
) -> Result<String, String> {
    match extractor_key {
        "turndown" => run_turndown(
            policy,
            html,
            &states
                .states
                .get("turndown")
                .map_or_default(|s| s.config.turndown.clone()),
            &std::iter::once("noscript")
                .chain(DEFAULT_SKIP_TAGS.iter().copied())
                .collect::<Vec<_>>(),
        ),
        "percollate" => run_percollate(
            policy,
            html,
            &states
                .states
                .get("percollate")
                .map_or_default(|s| s.config.percollate.clone()),
        ),
        "resiliparse" => run_resiliparse(
            policy,
            html,
            &states
                .states
                .get("resiliparse")
                .map_or_default(|s| s.config.resiliparse.clone()),
        ),
        "trafilatura" => run_trafilatura(
            policy,
            html,
            &states
                .states
                .get("trafilatura")
                .map_or_default(|s| s.config.trafilatura.clone()),
        ),
        "html2text-py" => run_html2text_py(
            policy,
            html,
            &states
                .states
                .get("html2text-py")
                .map_or_default(|s| s.config.html2text_py.clone()),
        ),
        "mozilla-readability" => run_mozilla_readability(
            policy,
            html,
            &states
                .states
                .get("mozilla-readability")
                .map_or_default(|s| s.config.mozilla_readability.clone()),
        ),
        "defuddle" | "node-html-markdown" | "webforai" => {
            let node_config = if extractor_key == "node-html-markdown" {
                serde_json::to_value(
                    states
                        .states
                        .get(extractor_key)
                        .map_or_default(|s| s.config.node_html_markdown.clone()),
                )
                .map_err(|error| error.to_string())?
            } else {
                serde_json::json!({})
            };
            run_node_snapshot_parser(policy, extractor_key, html, parsed_url, &node_config)
        }
        "markdownify" => run_markdownify(
            policy,
            html,
            &states
                .states
                .get("markdownify")
                .map_or_default(|s| s.config.markdownify.clone()),
        ),
        "webclaw" => run_webclaw(policy, html, states),
        "e2m" => run_e2m(policy, html, states),
        "html-to-markdown-go" => run_html_to_markdown_go(
            policy,
            html,
            parsed_url,
            &states
                .states
                .get("html-to-markdown-go")
                .map_or_default(|s| s.config.html_to_markdown_go.clone()),
        ),
        #[cfg(feature = "reader-lm-api")]
        "reader-lm-api" => {
            let key = std::env::var("JINA_API_KEY")
                .map_err(|error| format!("reader-lm-api missing API key: {error}"))?;
            // Jina accepts the acquired HTML. The original URL is only a
            // base for relative links; sending it alone would re-fetch the site.
            let response = ureq::post("https://r.jina.ai/")
                .header("authorization", &format!("Bearer {key}"))
                .send_json(serde_json::json!({
                    "html": html,
                    "url": parsed_url.as_str()
                }))
                .map_err(|error| format!("reader-lm-api request failed: {error}"))?;
            let limit = policy.max_output_bytes;
            let mut body = Vec::new();
            response
                .into_body()
                .into_reader()
                .take(u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1))
                .read_to_end(&mut body)
                .map_err(|error| format!("reader-lm-api read failed: {error}"))?;
            if body.len() > limit {
                return Err(format!("reader-lm-api output exceeds {limit} byte limit"));
            }
            String::from_utf8(body)
                .map_err(|error| format!("reader-lm-api returned invalid UTF-8: {error}"))
        }
        _ => Err(format!("unknown extractor: {extractor_key}")),
    }
}

// Keep external converter descendants in their isolated worker's process
// group; the worker's parent owns the actual extractor deadline.
trait BoundedOutput {
    fn bounded_output(&mut self, max_output_bytes: usize) -> Result<std::process::Output, String>;
}

impl BoundedOutput for std::process::Command {
    fn bounded_output(&mut self, max_output_bytes: usize) -> Result<std::process::Output, String> {
        crate::worker::run_bounded_command(
            self,
            std::time::Duration::from_mins(15),
            max_output_bytes,
        )
    }
}

fn command_output_text(
    output: Result<std::process::Output, String>,
    tool: &str,
) -> Result<String, String> {
    let output = output.map_err(|error| format!("{tool} launch failed: {error}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        return Err(format!(
            "{tool} exited {}: {}",
            output.status,
            stderr.trim()
        ));
    }
    if !stderr.trim().is_empty() {
        eprintln!("[{tool}] {}", stderr.trim());
    }
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    if text.is_empty() {
        return Err(format!(
            "{tool} returned empty output. stderr: {}",
            stderr.trim()
        ));
    }
    Ok(text)
}

fn run_turndown(
    policy: &ExecutionPolicy,
    html: &str,
    cfg: &TurndownConfig,
    remove_tags: &[&str],
) -> Result<String, String> {
    let tmp = std::env::temp_dir().join(format!("turndown_{}.html", uuid::Uuid::new_v4()));
    std::fs::write(&tmp, html)
        .map_err(|error| format!("writing temporary extractor input: {error}"))?;
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
    let node_code = r"const fs = require('fs'); const td = require('turndown'); const options = JSON.parse(process.argv[2]); const removeTags = JSON.parse(process.argv[3]); const svc = new td(options); for (const tag of removeTags) svc.remove(tag); const html = fs.readFileSync(process.argv[1], 'utf8'); process.stdout.write(svc.turndown(html))";
    let out = std::process::Command::new("node")
        .env("NODE_PATH", &policy.node_modules)
        .arg("-e")
        .arg(node_code)
        .arg(&tmp)
        .arg(options.to_string())
        .arg(remove_tags_json)
        .bounded_output(policy.max_output_bytes);

    let _ = std::fs::remove_file(&tmp);
    command_output_text(out, "turndown")
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

fn build_percollate_args(
    policy: &ExecutionPolicy,
    input_path: &std::path::Path,
    cfg: &PercollateConfig,
) -> Vec<String> {
    let mut args = vec![
        policy
            .node_modules
            .join("percollate/cli.js")
            .display()
            .to_string(),
        "md".to_string(),
        "--no-amp".to_string(),
        "-o".to_string(),
        "-".to_string(),
    ];
    // Never fetch image resources from the original site.
    // Percollate consumes the captured HTML snapshot only.
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

fn run_percollate(
    policy: &ExecutionPolicy,
    html: &str,
    cfg: &PercollateConfig,
) -> Result<String, String> {
    let tmp = std::env::temp_dir().join(format!("percollate_in_{}.html", uuid::Uuid::new_v4()));
    std::fs::write(&tmp, html)
        .map_err(|error| format!("writing temporary extractor input: {error}"))?;
    let args = build_percollate_args(policy, &tmp, cfg);
    let out = std::process::Command::new("node")
        .args(&args)
        .bounded_output(policy.max_output_bytes);

    let _ = std::fs::remove_file(&tmp);
    command_output_text(out, "percollate")
}

fn run_node_snapshot_parser(
    policy: &ExecutionPolicy,
    kind: &str,
    html: &str,
    url: &url::Url,
    config: &serde_json::Value,
) -> Result<String, String> {
    let input = tempfile::NamedTempFile::new().map_err(|error| error.to_string())?;
    std::fs::write(input.path(), html).map_err(|error| error.to_string())?;
    let script = policy.extractor_assets.join("snapshot-parsers.mjs");
    let output = std::process::Command::new("node")
        .arg(script)
        .arg(kind)
        .arg(input.path())
        .arg(url.as_str())
        .arg(config.to_string())
        .bounded_output(policy.max_output_bytes);
    command_output_text(output, kind)
}

/// The original Mozilla Readability JS implementation. The DOM is built
/// from the *acquired HTML snapshot*; no browser navigation or website fetch.
fn run_mozilla_readability(
    policy: &ExecutionPolicy,
    html: &str,
    cfg: &crate::extractor_config::MozillaReadabilityConfig,
) -> Result<String, String> {
    let input = tempfile::NamedTempFile::new().map_err(|error| error.to_string())?;
    std::fs::write(input.path(), html).map_err(|error| error.to_string())?;
    let cfg_json = serde_json::to_string(cfg).map_err(|error| error.to_string())?;
    let script = r#"const fs = require('fs');
const {parseHTML} = require('linkedom');
const {Readability} = require('@mozilla/readability');
const html = fs.readFileSync(process.argv[1], 'utf8');
const cfg = JSON.parse(process.argv[2]);
const {document} = parseHTML(html);
const opts = {
  maxElemsToParse: cfg.max_elems_to_parse,
  nbTopCandidates: cfg.nb_top_candidates,
  charThreshold: cfg.char_threshold,
  keepClasses: cfg.keep_classes,
  classesToPreserve: cfg.classes_to_preserve,
  disableJSONLD: cfg.disable_json_ld,
  linkDensityModifier: cfg.link_density_modifier
};
const result = new Readability(document, opts).parse();
if (!result) { console.error('No readable content'); process.exit(2); }
process.stdout.write(result.textContent || '');
"#;
    let out = std::process::Command::new("node")
        .env("NODE_PATH", &policy.node_modules)
        .args(["-e", script])
        .arg(input.path())
        .arg(cfg_json)
        .bounded_output(policy.max_output_bytes);
    command_output_text(out, "mozilla-readability")
}

fn python_requirements(policy: &ExecutionPolicy, name: &str) -> PathBuf {
    policy
        .extractor_assets
        .join("python")
        .join(name)
        .join("requirements.txt")
}

/// Independent Lexbor-based content extractor operating exclusively on
/// the acquired HTML, with no URL or network access.
fn run_resiliparse(
    policy: &ExecutionPolicy,
    html: &str,
    cfg: &crate::extractor_config::ResiliparseConfig,
) -> Result<String, String> {
    let input = tempfile::NamedTempFile::new().map_err(|error| error.to_string())?;
    std::fs::write(input.path(), html).map_err(|error| error.to_string())?;
    let cfg_json = serde_json::to_string(cfg).map_err(|error| error.to_string())?;
    let script = r#"import sys,json
from pathlib import Path
from resiliparse.extract.html2text import extract_plain_text
cfg=json.loads(sys.argv[2])
mode=cfg.pop('formatting_mode')
cfg['preserve_formatting']={'formatted':True,'plain':False,'minimal_html':'minimal_html'}[mode]
html=Path(sys.argv[1]).read_text(encoding='utf-8')
print(extract_plain_text(html, **cfg), end='')
"#;
    let out = std::process::Command::new("uv")
        .args(["run", "--no-project", "--with-requirements"])
        .arg(python_requirements(policy, "resiliparse"))
        .args(["--", "python3", "-c", script])
        .arg(input.path())
        .arg(cfg_json)
        .bounded_output(policy.max_output_bytes);
    command_output_text(out, "resiliparse")
}

fn run_trafilatura(
    policy: &ExecutionPolicy,
    html: &str,
    cfg: &TrafilaturaConfig,
) -> Result<String, String> {
    let tmp = std::env::temp_dir().join(format!("trafilatura_{}.html", uuid::Uuid::new_v4()));
    std::fs::write(&tmp, html)
        .map_err(|error| format!("writing temporary extractor input: {error}"))?;
    let cfg_json = serde_json::to_string(cfg)
        .map_err(|error| format!("serializing configuration: {error}"))?;
    let script = r"import json, sys, trafilatura; cfg = json.loads(sys.argv[2]); html = open(sys.argv[1]).read(); result = trafilatura.extract(html, output_format='markdown', favor_precision=cfg['favor_precision'], favor_recall=cfg['favor_recall'], include_comments=cfg['include_comments'], include_tables=cfg['include_tables'], include_images=cfg['include_images'], include_formatting=cfg['include_formatting'], include_links=cfg['include_links'], deduplicate=cfg['deduplicate'], with_metadata=cfg['with_metadata']); print(result if result else '', end='')";
    let out = std::process::Command::new("uv")
        .args(["run", "--no-project", "--with-requirements"])
        .arg(python_requirements(policy, "trafilatura"))
        .args(["--", "python3", "-c", script])
        .arg(&tmp)
        .arg(cfg_json)
        .bounded_output(policy.max_output_bytes);

    let _ = std::fs::remove_file(&tmp);
    command_output_text(out, "trafilatura")
}

fn run_html2text_py(
    policy: &ExecutionPolicy,
    html: &str,
    cfg: &Html2TextPythonConfig,
) -> Result<String, String> {
    let tmp = std::env::temp_dir().join(format!("h2t_py_{}.html", uuid::Uuid::new_v4()));
    std::fs::write(&tmp, html)
        .map_err(|error| format!("writing temporary extractor input: {error}"))?;
    let cfg_json = serde_json::to_string(cfg)
        .map_err(|error| format!("serializing configuration: {error}"))?;
    let script = r"from html2text import HTML2Text; import json, sys; cfg = json.loads(sys.argv[2]); h = HTML2Text(); h.ignore_links = cfg['ignore_links']; h.ignore_images = cfg['ignore_images']; h.ignore_emphasis = cfg['ignore_emphasis']; h.body_width = cfg['body_width']; h.unicode_snob = cfg['unicode_snob']; h.escape_snob = cfg['escape_snob']; h.inline_links = cfg['inline_links']; h.google_doc = cfg['google_doc']; h.dash_unordered_list = cfg['dash_unordered_list']; html = open(sys.argv[1]).read(); print(h.handle(html), end='')";
    let out = std::process::Command::new("uv")
        .args(["run", "--no-project", "--with-requirements"])
        .arg(python_requirements(policy, "html2text"))
        .args(["--", "python3", "-c", script])
        .arg(&tmp)
        .arg(cfg_json)
        .bounded_output(policy.max_output_bytes);

    let _ = std::fs::remove_file(&tmp);
    command_output_text(out, "html2text.py")
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

fn run_markdownify(
    policy: &ExecutionPolicy,
    html: &str,
    cfg: &MarkdownifyConfig,
) -> Result<String, String> {
    let tmp = std::env::temp_dir().join(format!("markdownify_{}.html", uuid::Uuid::new_v4()));
    std::fs::write(&tmp, html)
        .map_err(|error| format!("writing temporary extractor input: {error}"))?;
    let cfg_json = build_markdownify_config_json(cfg)
        .map_err(|error| format!("markdownify configuration invalid: {error}"))?;
    let script = r"from markdownify import markdownify as md; import json, sys; cfg = json.loads(sys.argv[2]); kwargs = {'autolinks': cfg['autolinks'], 'default_title': cfg['default_title'], 'heading_style': cfg['heading_style'], 'bullets': cfg['bullets'], 'strong_em_symbol': cfg['strong_em_symbol'], 'sub_symbol': cfg['sub_symbol'], 'sup_symbol': cfg['sup_symbol'], 'newline_style': cfg['newline_style'], 'code_language': cfg['code_language'], 'escape_asterisks': cfg['escape_asterisks'], 'escape_underscores': cfg['escape_underscores'], 'escape_misc': cfg['escape_misc'], 'keep_inline_images_in': cfg['keep_inline_images_in'], 'table_infer_header': cfg['table_infer_header'], 'wrap': cfg['wrap'], 'wrap_width': cfg['wrap_width']}; html = open(sys.argv[1]).read(); strip = cfg['strip']; convert = cfg['convert']; strip_document = cfg['strip_document']; strip_pre = cfg['strip_pre']; bs4_options = cfg['bs4_options']; kwargs['strip'] = strip if strip else None; kwargs['convert'] = convert if convert else None; kwargs['strip_document'] = strip_document; kwargs['strip_pre'] = strip_pre; kwargs['bs4_options'] = bs4_options; kwargs = {key: value for key, value in kwargs.items() if value is not None}; print(md(html, **kwargs), end='')";
    let out = std::process::Command::new("uv")
        .args(["run", "--no-project", "--with-requirements"])
        .arg(python_requirements(policy, "markdownify"))
        .args(["--", "python3", "-c", script])
        .arg(&tmp)
        .arg(cfg_json)
        .bounded_output(policy.max_output_bytes);

    let _ = std::fs::remove_file(&tmp);
    command_output_text(out, "markdownify")
}

fn run_webclaw(
    policy: &ExecutionPolicy,
    html: &str,
    states: &ExtractorStates,
) -> Result<String, String> {
    let cfg = states
        .states
        .get("webclaw")
        .map_or_default(|s| s.config.webclaw.clone());
    let tmp = std::env::temp_dir().join(format!("webclaw_{}.html", uuid::Uuid::new_v4()));
    std::fs::write(&tmp, html)
        .map_err(|error| format!("writing temporary extractor input: {error}"))?;
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
        cfg.format
    });
    let binary = &policy.webclaw_bin;
    let out = std::process::Command::new(binary)
        .args(&args)
        .bounded_output(policy.max_output_bytes);

    let _ = std::fs::remove_file(&tmp);
    command_output_text(out, "webclaw")
}

const E2M_SCRIPT: &str = r#"
import json
import sys
from pathlib import Path

import nltk
from wisup_e2m import HtmlParser

# wisup-e2m pins an unstructured release whose built-in NLTK bundle URL
# is no longer available. Use NLTK's supported package index instead.
for category, resource in (
    ("tokenizers", "punkt_tab"),
    ("taggers", "averaged_perceptron_tagger_eng"),
):
    try:
        nltk.data.find(f"{category}/{resource}")
    except LookupError:
        if not nltk.download(resource, quiet=True, raise_on_error=True):
            raise RuntimeError(f"Could not obtain NLTK resource {resource}")


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

fn run_e2m(
    policy: &ExecutionPolicy,
    html: &str,
    states: &ExtractorStates,
) -> Result<String, String> {
    let cfg = states
        .states
        .get("e2m")
        .map_or_default(|s| s.config.e2m.clone());
    let engine = if cfg.engine.is_empty() {
        "unstructured"
    } else {
        &cfg.engine
    };
    let work_dir = std::env::temp_dir().join(format!("e2m_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&work_dir)
        .map_err(|error| format!("creating e2m directory: {error}"))?;
    let tmp = work_dir.join("input.html");
    std::fs::write(&tmp, html)
        .map_err(|error| format!("writing temporary extractor input: {error}"))?;
    let langs = serde_json::to_string(&cfg.langs)
        .map_err(|error| format!("serializing e2m languages: {error}"))?;
    let script = E2M_SCRIPT;
    let out = std::process::Command::new("uv")
        .args([
            "run",
            "--no-project",
            "--python",
            "3.12",
            "--with-requirements",
        ])
        .arg(python_requirements(policy, "e2m"))
        .args(["--", "python3", "-c", script])
        .arg(&work_dir)
        .arg(&tmp)
        .args([
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
        .bounded_output(policy.max_output_bytes);

    let _ = std::fs::remove_file(&tmp);
    let _ = std::fs::remove_dir_all(&work_dir);
    command_output_text(out, "e2m")
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
        args.push(format!("--plugin-{plugin}"));
    }
    args
}

fn run_html_to_markdown_go(
    policy: &ExecutionPolicy,
    html: &str,
    parsed_url: &url::Url,
    cfg: &HtmlToMarkdownGoConfig,
) -> Result<String, String> {
    let args = build_html_to_markdown_go_args(parsed_url, cfg);
    let binary = &policy.html_to_markdown_go_bin;
    // Supply a seekable, anonymous file as stdin. Writing a large document
    // synchronously into a child's stdin pipe can deadlock if the child also
    // fills its stdout pipe before we start draining it.
    let mut input = tempfile::tempfile()
        .map_err(|error| format!("html-to-markdown-go temporary input: {error}"))?;
    input
        .write_all(html.as_bytes())
        .map_err(|error| format!("html-to-markdown-go temporary input: {error}"))?;
    input
        .rewind()
        .map_err(|error| format!("html-to-markdown-go rewind: {error}"))?;
    let out = std::process::Command::new(binary)
        .args(&args)
        .stdin(std::process::Stdio::from(input))
        .bounded_output(policy.max_output_bytes);
    command_output_text(out, "html-to-markdown-go")
}

/// Deletes incomplete runs; persist only after all metadata has been committed.
struct OwnedRunDir {
    path: PathBuf,
    committed: bool,
}

impl OwnedRunDir {
    const fn new(path: PathBuf) -> Self {
        Self {
            path,
            committed: false,
        }
    }

    const fn persist(&mut self) {
        self.committed = true;
    }
}

impl Drop for OwnedRunDir {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

#[derive(Clone)]
pub struct ScoreStore {
    dir: PathBuf,
    max_saved_runs: usize,
    metadata_lock: std::sync::Arc<std::sync::Mutex<()>>,
}

impl ScoreStore {
    pub fn with_retention(dir: PathBuf, max_saved_runs: usize) -> Result<Self, String> {
        if max_saved_runs == 0 {
            return Err("retention limit must be at least 1 saved run".to_string());
        }
        fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        Ok(Self {
            dir,
            max_saved_runs,
            metadata_lock: std::sync::Arc::new(std::sync::Mutex::new(())),
        })
    }

    fn lock_metadata(&self) -> Result<std::sync::MutexGuard<'_, ()>, String> {
        self.metadata_lock
            .lock()
            .map_err(|error| format!("score metadata lock poisoned: {error}"))
    }

    /// Retention is count-bounded and deterministic. Metadata and every artifact
    /// directory are removed together, even if the run used zero extractors.
    pub fn prune(&self) -> Result<(), String> {
        let _lock = self.lock_metadata()?;
        for score in self.list_unlocked()?.iter().skip(self.max_saved_runs) {
            self.delete_unlocked(&score.id)?;
        }
        // Completed runs without metadata can only be leftovers from an
        // interrupted process. Delay cleanup past the maximum supported
        // 15-minute extraction deadline to avoid deleting in-flight work.
        self.prune_orphans(std::time::Duration::from_hours(24))
    }

    fn prune_orphans(&self, minimum_age: std::time::Duration) -> Result<(), String> {
        let root = self.dir.join("artifacts");
        if !root.exists() {
            return Ok(());
        }
        for entry in fs::read_dir(root).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            if !entry
                .file_type()
                .map_err(|error| error.to_string())?
                .is_dir()
            {
                continue;
            }
            let name = entry.file_name();
            let Some(id) = name.to_str().and_then(|name| Uuid::parse_str(name).ok()) else {
                continue;
            };
            if self.dir.join(format!("{id}.json")).exists() {
                continue;
            }
            let age = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .map_err(|error| error.to_string())?
                .elapsed()
                .map_err(|error| error.to_string())?;
            if age >= minimum_age {
                fs::remove_dir_all(entry.path()).map_err(|error| error.to_string())?;
            }
        }
        Ok(())
    }

    pub fn artifact_dir(&self, id: &str) -> Result<PathBuf, String> {
        let parsed = Uuid::parse_str(id).map_err(|error| format!("invalid score ID: {error}"))?;
        Ok(self.dir.join("artifacts").join(parsed.to_string()))
    }

    pub fn output_path(&self, id: &str, name: &str) -> Result<PathBuf, String> {
        if name.is_empty()
            || !name
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == ' ')
        {
            return Err("invalid extractor output name".to_string());
        }
        Ok(self.artifact_dir(id)?.join(format!("{name}.txt")))
    }

    pub fn source_path(&self, id: &str) -> Result<PathBuf, String> {
        Ok(self.artifact_dir(id)?.join("source.html"))
    }

    fn create_run_dir(&self, id: &str) -> Result<PathBuf, String> {
        let path = self.artifact_dir(id)?;
        fs::create_dir_all(self.dir.join("artifacts")).map_err(|error| error.to_string())?;
        fs::create_dir(&path).map_err(|error| error.to_string())?;
        Ok(path)
    }

    pub fn remove_artifacts(&self, id: &str) -> Result<(), String> {
        let path = self.artifact_dir(id)?;
        if path.exists() {
            fs::remove_dir_all(path).map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    pub fn save(&self, score: &Score) -> Result<(), String> {
        let _lock = self.lock_metadata()?;
        self.save_unlocked(score)
    }

    fn save_unlocked(&self, score: &Score) -> Result<(), String> {
        let path = self.dir.join(format!(
            "{}.json",
            Uuid::parse_str(&score.id).map_err(|error| format!("invalid score ID: {error}"))?
        ));
        let content = serde_json::to_vec_pretty(score).map_err(|error| error.to_string())?;
        let mut temporary =
            tempfile::NamedTempFile::new_in(&self.dir).map_err(|error| error.to_string())?;
        temporary
            .write_all(&content)
            .map_err(|error| error.to_string())?;
        temporary.persist(path).map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn load(&self, id: &str) -> Result<Score, String> {
        let _lock = self.lock_metadata()?;
        self.load_unlocked(id)
    }

    fn load_unlocked(&self, id: &str) -> Result<Score, String> {
        let id = Uuid::parse_str(id).map_err(|error| format!("invalid score ID: {error}"))?;
        let path = self.dir.join(format!("{id}.json"));
        let s = read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&s).map_err(|e| e.to_string())
    }

    pub fn update_grade(&self, id: &str, extractor: &str, grade: u8) -> Result<Score, String> {
        let _lock = self.lock_metadata()?;
        let mut score = self.load_unlocked(id)?;
        if !score
            .extractor_results
            .iter()
            .any(|result| result.name == extractor)
        {
            return Err(format!("extractor not found in score: {extractor}"));
        }
        score.grades.insert(extractor.to_string(), grade);
        self.save_unlocked(&score)?;
        Ok(score)
    }

    pub fn list(&self) -> Result<Vec<Score>, String> {
        let _lock = self.lock_metadata()?;
        self.list_unlocked()
    }

    fn list_unlocked(&self) -> Result<Vec<Score>, String> {
        let mut scores = Vec::new();
        for entry in fs::read_dir(&self.dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                if let Ok(source) = read_to_string(&path) {
                    if let Ok(score) = serde_json::from_str::<Score>(&source) {
                        scores.push(score);
                    }
                }
            }
        }
        scores.sort_by(|a, b| b.timestamp.cmp(&a.timestamp).then_with(|| b.id.cmp(&a.id)));
        Ok(scores)
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        let _lock = self.lock_metadata()?;
        self.delete_unlocked(id)
    }

    fn delete_unlocked(&self, id: &str) -> Result<(), String> {
        let id = Uuid::parse_str(id).map_err(|error| format!("invalid score ID: {error}"))?;
        let path = self.dir.join(format!("{id}.json"));
        if !path.is_file() {
            return Err(format!("score not found: {id}"));
        }
        self.remove_artifacts(&id.to_string())?;
        fs::remove_file(path).map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extractor_config::{
        ExtractorConfig, HtmdConfig, Html2TextConfig, HtmlToMarkdownGoConfig, PercollateConfig,
    };

    fn test_store() -> Result<(ScoreStore, PathBuf), String> {
        let dir =
            std::env::temp_dir().join(format!("html-to-text-comparison-tests-{}", Uuid::new_v4()));
        Ok((ScoreStore::with_retention(dir.clone(), 100)?, dir))
    }

    #[test]
    fn deleting_empty_run_removes_source_and_metadata() -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let repository = ScoreStore::with_retention(root.path().join("scores"), 10)?;
        let mut states = ExtractorStates::default();
        for extractor in states.states.values_mut() {
            extractor.enabled = false;
        }

        let run = run_extraction(
            &ExecutionPolicy::default(),
            "https://example.com/article",
            "<h1>Saved exactly once</h1>",
            Acquisition::File,
            &states,
            &repository,
        )?;
        assert!(run.extractor_results.is_empty());
        assert_eq!(
            fs::read_to_string(repository.source_path(&run.id)?)?,
            "<h1>Saved exactly once</h1>"
        );
        assert_eq!(repository.list()?.len(), 1);
        assert!(repository.artifact_dir(&run.id)?.exists());
        repository.delete(&run.id)?;
        assert!(repository.list()?.is_empty());
        assert!(!repository.artifact_dir(&run.id)?.exists());
        assert!(repository.delete("../outside").is_err());
        Ok(())
    }

    #[test]
    fn recent_runs_survive_same_second_retention() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let store = ScoreStore::with_retention(temp.path().join("scores"), 2)?;
        let mut states = ExtractorStates::default();
        for entry in states.states.values_mut() {
            entry.enabled = false;
        }
        let mut first = run_extraction(
            &ExecutionPolicy::default(),
            "https://example.com/",
            "<h1>first</h1>",
            Acquisition::File,
            &states,
            &store,
        )?;
        let mut second = run_extraction(
            &ExecutionPolicy::default(),
            "https://example.com/",
            "<h1>second</h1>",
            Acquisition::File,
            &states,
            &store,
        )?;
        // Force the same timestamp instead of depending on two worker runs
        // completing before a wall-clock second boundary under CI load.
        assert!(Uuid::parse_str(&first.id)? < Uuid::parse_str(&second.id)?);
        first.timestamp = "same-second".to_string();
        second.timestamp = first.timestamp.clone();
        store.save(&first)?;
        store.save(&second)?;
        let limited = ScoreStore::with_retention(temp.path().join("scores"), 1)?;
        limited.prune()?;
        let saved = limited.list()?;
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].id, second.id);
        assert!(limited.load(&first.id).is_err());
        Ok(())
    }

    #[test]
    fn concurrent_grades_do_not_overwrite_each_other() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let repository = ScoreStore::with_retention(temp.path().join("scores"), 8)?;
        let mut states = ExtractorStates::default();
        for entry in states.states.values_mut() {
            entry.enabled = false;
        }
        let mut score = Score::new(
            &ExecutionPolicy::default(),
            "https://example.com/".to_string(),
            "<h1>fixture</h1>".to_string(),
            Acquisition::File,
            &states,
            &repository,
        )?;
        for name in ["alpha", "beta"] {
            score.extractor_results.push(ExtractorResult {
                name: name.to_string(),
                extractor_key: name.to_string(),
                output_size: 0,
                reduction_percent: None,
                time_micros: 0,
                wall_time_micros: 0,
                status: ExtractionStatus::NoContent,
                error: String::new(),
                output_file: String::new(),
            });
        }
        repository.save(&score)?;
        let gate = std::sync::Arc::new(std::sync::Barrier::new(3));
        let mut jobs = Vec::new();
        for (name, grade) in [("alpha", 3), ("beta", 7)] {
            let repository = repository.clone();
            let gate = gate.clone();
            let id = score.id.clone();
            jobs.push(std::thread::spawn(move || {
                gate.wait();
                repository.update_grade(&id, name, grade)
            }));
        }
        gate.wait();
        for job in jobs {
            let updated = job.join().map_err(|_| "grade thread panicked")?;
            assert!(updated.is_ok(), "failed concurrent grading: {updated:?}");
        }
        let after = repository.load(&score.id)?;
        assert_eq!(after.grades.get("alpha"), Some(&3));
        assert_eq!(after.grades.get("beta"), Some(&7));
        assert!(repository.update_grade(&score.id, "phantom", 1).is_err());
        Ok(())
    }

    #[test]
    fn retention_prunes_both_metadata_and_artifacts() -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let repository = ScoreStore::with_retention(root.path().join("scores"), 2)?;
        let mut states = ExtractorStates::default();
        for extractor in states.states.values_mut() {
            extractor.enabled = false;
        }

        let mut ids = Vec::new();
        for _ in 0..3 {
            let run = run_extraction(
                &ExecutionPolicy::default(),
                "https://example.com/",
                "one",
                Acquisition::File,
                &states,
                &repository,
            )?;
            ids.push(run.id);
        }
        let saved = repository.list()?;
        assert_eq!(saved.len(), 2);
        for id in ids {
            let preserved = saved.iter().any(|record| record.id == id);
            assert_eq!(preserved, repository.artifact_dir(&id)?.exists());
        }
        Ok(())
    }

    #[test]
    fn orphan_cleanup_preserves_live_runs_and_reaps_old_ones(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let repository = ScoreStore::with_retention(root.path().join("scores"), 10)?;
        let id = Uuid::new_v4().to_string();
        let path = repository.create_run_dir(&id)?;
        fs::write(path.join("source.html"), "uncommitted")?;

        repository.prune()?;
        assert!(path.exists(), "new in-flight run must not be removed");
        repository.prune_orphans(std::time::Duration::ZERO)?;
        assert!(!path.exists(), "stale orphan must be removed");
        Ok(())
    }

    #[test]
    fn compare_settings_applies_html2text_config() -> Result<(), Box<dyn std::error::Error>> {
        let (score_store, dir) = test_store()?;
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
            &ExecutionPolicy::default(),
            "https://example.com",
            html,
            "html2text",
            baseline,
            candidate,
            &score_store,
        )?;

        assert_eq!(score.extractor_results[0].name, "html2text baseline");
        assert_eq!(score.extractor_results[0].extractor_key, "html2text");
        assert_eq!(
            score.extractor_results[1].name,
            "html2text current settings"
        );
        assert_eq!(score.extractor_results[1].extractor_key, "html2text");

        let baseline_output = std::fs::read_to_string(&score.extractor_results[0].output_file)?;
        let candidate_output = std::fs::read_to_string(&score.extractor_results[1].output_file)?;

        assert_ne!(baseline_output, candidate_output);
        assert!(baseline_output.matches('\n').count() > candidate_output.matches('\n').count());

        let _ = std::fs::remove_dir_all(dir);
        Ok(())
    }

    #[test]
    fn compare_settings_applies_htmd_heading_style() -> Result<(), Box<dyn std::error::Error>> {
        let (score_store, dir) = test_store()?;
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
            &ExecutionPolicy::default(),
            "https://example.com",
            html,
            "htmd",
            baseline,
            candidate,
            &score_store,
        )?;

        let baseline_output = std::fs::read_to_string(&score.extractor_results[0].output_file)?;
        let candidate_output = std::fs::read_to_string(&score.extractor_results[1].output_file)?;

        assert!(baseline_output.contains("# Title"));
        assert!(candidate_output.contains("Title\n====="));
        assert_ne!(baseline_output, candidate_output);

        let _ = std::fs::remove_dir_all(dir);
        Ok(())
    }

    #[test]
    fn compare_settings_applies_turndown_heading_style() -> Result<(), Box<dyn std::error::Error>> {
        let (score_store, dir) = test_store()?;
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
            &ExecutionPolicy::default(),
            "https://example.com",
            html,
            "turndown",
            baseline,
            candidate,
            &score_store,
        )?;

        let baseline_output = std::fs::read_to_string(&score.extractor_results[0].output_file)?;
        let candidate_output = std::fs::read_to_string(&score.extractor_results[1].output_file)?;

        assert!(baseline_output.contains("# Title"));
        assert!(candidate_output.contains("Title\n====="));
        assert_ne!(baseline_output, candidate_output);

        let _ = std::fs::remove_dir_all(dir);
        Ok(())
    }

    #[test]
    fn turndown_removes_script_content() -> Result<(), Box<dyn std::error::Error>> {
        let output = run_turndown(&ExecutionPolicy::default(),
            "<html><body><article><p>Body</p></article><script>if(window.foo){bar()}</script></body></html>",
            &TurndownConfig::default(),
            &["script", "style", "noscript"],
        )?;

        assert!(output.contains("Body"));
        assert!(!output.contains("window.foo"));
        assert!(!output.contains("bar()"));
        Ok(())
    }

    #[test]
    fn compare_settings_applies_percollate_markdown_options(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (score_store, dir) = test_store()?;
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
            &ExecutionPolicy::default(),
            "https://example.com",
            html,
            "percollate",
            baseline,
            candidate,
            &score_store,
        )?;

        let baseline_output = std::fs::read_to_string(&score.extractor_results[0].output_file)?;
        let candidate_output = std::fs::read_to_string(&score.extractor_results[1].output_file)?;

        assert!(baseline_output.contains("```"));
        assert!(candidate_output.contains("~~~"));
        assert!(baseline_output.contains("\n---\n") || baseline_output.contains("\n- - -\n"));
        assert!(candidate_output.contains("\n***\n") || candidate_output.contains("\n* * *\n"));
        assert_ne!(baseline_output, candidate_output);

        let _ = std::fs::remove_dir_all(dir);
        Ok(())
    }

    #[test]
    fn compare_settings_applies_trafilatura_metadata_option(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (score_store, dir) = test_store()?;
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
            &ExecutionPolicy::default(),
            "https://example.com",
            html,
            "trafilatura",
            baseline,
            candidate,
            &score_store,
        )?;

        let baseline_output = std::fs::read_to_string(&score.extractor_results[0].output_file)?;
        let candidate_output = std::fs::read_to_string(&score.extractor_results[1].output_file)?;

        assert!(!baseline_output.contains("---\ntitle:"));
        assert!(candidate_output.contains("---\ntitle: Title"));
        assert_ne!(baseline_output, candidate_output);

        let _ = std::fs::remove_dir_all(dir);
        Ok(())
    }

    #[cfg(feature = "dom_smoothie")]
    #[test]
    fn compare_settings_applies_dom_smoothie_candidate_selection_mode(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (score_store, dir) = test_store()?;
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
            &ExecutionPolicy::default(),
            "https://example.com",
            html,
            "dom_smoothie",
            baseline,
            candidate,
            &score_store,
        )?;

        let baseline_output = std::fs::read_to_string(&score.extractor_results[0].output_file)?;
        let candidate_output = std::fs::read_to_string(&score.extractor_results[1].output_file)?;

        assert!(!baseline_output.contains("Alice was beginning"));
        assert!(baseline_output.contains("So she was considering"));
        assert!(candidate_output.contains("Alice was beginning"));
        assert!(candidate_output.contains("So she was considering"));
        assert_ne!(baseline_output, candidate_output);

        let _ = std::fs::remove_dir_all(dir);
        Ok(())
    }

    #[cfg(feature = "dom_smoothie")]
    #[test]
    fn preview_dom_smoothie_disable_json_ld_does_not_change_text_output(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let html = "<html><head><script type=\"application/ld+json\">{\"@context\":\"https://schema.org\",\"@type\":\"Article\",\"url\":\"https://example.com/from-json-ld\",\"headline\":\"JSON-LD headline\"}</script></head><body><article><p>Visible article body.</p></article></body></html>";
        let baseline = preview_single_extractor_settings(
            &ExecutionPolicy::default(),
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
        )?;
        let candidate = preview_single_extractor_settings(
            &ExecutionPolicy::default(),
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
        )?;

        assert_eq!(baseline.output, candidate.output);
        assert!(baseline.output.contains("Visible article body"));
        Ok(())
    }

    #[cfg(feature = "dom_smoothie")]
    #[test]
    fn preview_dom_smoothie_class_preservation_does_not_change_text_output(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let html = "<html><body><article><div class=\"keep-me drop-me\"><p>Visible article body.</p></div></article></body></html>";
        let baseline = preview_single_extractor_settings(
            &ExecutionPolicy::default(),
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
        )?;
        let candidate = preview_single_extractor_settings(
            &ExecutionPolicy::default(),
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
        )?;

        assert_eq!(baseline.output, candidate.output);
        assert!(baseline.output.contains("Visible article body"));
        Ok(())
    }

    #[test]
    fn e2m_handles_images_without_image_paths() -> Result<(), Box<dyn std::error::Error>> {
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
        let dir = tempfile::tempdir()?;
        fs::write(dir.path().join("source.html"), html)?;
        let mut runner = Runner::with_timeout(
            dir.path().to_path_buf(),
            html.to_string(),
            ExecutionPolicy::default(),
            std::time::Duration::from_secs(120),
        );
        runner.run_isolated("e2m", "e2m", "https://example.com", &states);
        let outcomes = runner.into_stats();
        assert_eq!(outcomes.len(), 1);
        assert_eq!(
            outcomes[0].status,
            ExtractionStatus::Success,
            "isolated e2m failed: {}",
            outcomes[0].error
        );
        let output = std::fs::read_to_string(dir.path().join("e2m.txt"))?;
        assert!(output.contains("Title"), "{output}");
        assert!(output.contains("Before"));
        assert!(output.contains("After"));
        Ok(())
    }

    #[test]
    fn two_native_extractors_share_one_persisted_snapshot() -> Result<(), Box<dyn std::error::Error>>
    {
        let directory = tempfile::tempdir()?;
        let html = "<html><body><h1>Shared source</h1><p>Same captured HTML for every converter.</p></body></html>";
        fs::write(directory.path().join("source.html"), html)?;
        let mut runner = Runner::new(
            directory.path().to_path_buf(),
            html.to_string(),
            ExecutionPolicy::default(),
        );
        let states = ExtractorStates::default();
        for key in ["nanohtml2text", "html2md"] {
            runner.run_isolated(key, key, "https://example.com/", &states);
        }
        let results = runner.into_stats();
        assert_eq!(results.len(), 2);
        for item in results {
            assert_eq!(
                item.status,
                ExtractionStatus::Success,
                "{}: {}",
                item.name,
                item.error
            );
        }
        assert_eq!(
            fs::read_to_string(directory.path().join("source.html"))?,
            html
        );
        assert!(fs::read_dir(directory.path())?
            .filter_map(Result::ok)
            .all(|entry| !entry.file_type().is_ok_and(|kind| kind.is_dir())));
        Ok(())
    }

    #[test]
    fn preview_single_extractor_returns_current_output() -> Result<(), Box<dyn std::error::Error>> {
        let html = "<html><body><p>alpha beta gamma delta epsilon zeta eta theta</p></body></html>";
        let preview = preview_single_extractor_settings(
            &ExecutionPolicy::default(),
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
        )?;

        assert_eq!(preview.extractor_key, "html2text");
        assert!(preview.output.contains("alpha beta"));
        assert!(preview.output.matches('\n').count() > 1);
        Ok(())
    }

    #[test]
    fn resiliparse_excludes_navigation_without_another_acquisition(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let html = include_str!("../fixtures/article-with-navigation.html");
        let output = run_resiliparse(
            &ExecutionPolicy::default(),
            html,
            &crate::extractor_config::ResiliparseConfig::default(),
        )?;
        for marker in [
            "CONTENT_SENTINEL_ALPHA",
            "CONTENT_SENTINEL_BETA",
            "CONTENT_SENTINEL_GAMMA",
        ] {
            assert!(output.contains(marker), "missing content marker: {marker}");
        }
        for marker in [
            "UNRELATED_NAVIGATION_HELP_CENTER",
            "UNRELATED_SIDEBAR_SPONSORED_LINKS",
            "UNRELATED_FOOTER_COPYRIGHT_NOTICE",
        ] {
            assert!(
                !output.contains(marker),
                "leaked boilerplate marker: {marker}"
            );
        }
        Ok(())
    }

    #[test]
    fn resiliparse_retains_technical_documentation_values() -> Result<(), Box<dyn std::error::Error>>
    {
        let html = include_str!("../fixtures/technical-reference.html");
        let output = run_resiliparse(
            &ExecutionPolicy::default(),
            html,
            &crate::extractor_config::ResiliparseConfig::default(),
        )?;
        for value in [
            "CONTENT_SENTINEL_REGISTER_TEXT",
            "CTRL_A",
            "0x18",
            "0x2C",
            "0x00ff_0000",
            "CONTENT_SENTINEL_END_DOC",
        ] {
            assert!(output.contains(value), "missing technical content: {value}");
        }
        assert!(!output.contains("IRRELEVANT_DOCS_NAVIGATION"));
        assert!(!output.contains("IRRELEVANT_DOCS_FOOTER"));
        Ok(())
    }

    #[test]
    fn mozilla_readability_extracts_captured_html() -> Result<(), Box<dyn std::error::Error>> {
        let body = "Readability operates on one HTML snapshot. ".repeat(24);
        let html = format!(
            "<html><head><title>Fixture</title></head><body><article><h1>Article title</h1><p>{body}</p></article><nav>Discard me</nav></body></html>"
        );
        let result = run_mozilla_readability(
            &ExecutionPolicy::default(),
            &html,
            &crate::extractor_config::MozillaReadabilityConfig::default(),
        )?;
        assert!(result.contains("Readability operates on one HTML snapshot"));
        assert!(!result.contains("Discard me"));
        Ok(())
    }

    #[test]
    fn build_percollate_args_respects_config() {
        let args = build_percollate_args(
            &ExecutionPolicy::default(),
            std::path::Path::new("/tmp/input.html"),
            &PercollateConfig {
                hyphenate: false,
                fences: false,
                fence: "~~~".to_string(),
                emphasis: "*".to_string(),
                strong: "**".to_string(),
                resource_link: false,
                rule: "***".to_string(),
            },
        );

        assert!(!args.iter().any(|arg| arg == "--inline"));
        assert!(args.iter().any(|arg| arg == "--no-amp"));
        assert!(args.iter().any(|arg| arg == "--no-hyphenate"));
        assert!(args.iter().any(|arg| arg == "--md.fences=false"));
        assert!(args.iter().any(|arg| arg == "--md.fence=~"));
        assert!(args.iter().any(|arg| arg == "--md.emphasis=*"));
        assert!(args.iter().any(|arg| arg == "--md.strong=*"));
        assert!(args.iter().any(|arg| arg == "--md.resourceLink=false"));
        assert!(args.iter().any(|arg| arg == "--md.rule=*"));
    }

    #[test]
    fn build_markdownify_config_json_rejects_conflicting_tag_filters(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Err(error) = build_markdownify_config_json(&MarkdownifyConfig {
            strip: vec!["a".to_string()],
            convert: vec!["p".to_string()],
            ..Default::default()
        }) else {
            return Err("conflicting tag filters unexpectedly accepted".into());
        };

        assert!(error.contains("mutually exclusive"));
        Ok(())
    }

    #[test]
    fn build_markdownify_config_json_normalizes_optional_fields(
    ) -> Result<(), Box<dyn std::error::Error>> {
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
        })?;
        let payload: serde_json::Value = serde_json::from_str(&payload)?;

        assert_eq!(payload["strip"], serde_json::json!(["a"]));
        assert_eq!(payload["heading_style"], serde_json::json!("underlined"));
        assert_eq!(payload["strong_em_symbol"], serde_json::json!("*"));
        assert_eq!(payload["newline_style"], serde_json::json!("backslash"));
        assert_eq!(payload["keep_inline_images_in"], serde_json::json!(["td"]));
        assert_eq!(payload["wrap_width"], serde_json::Value::Null);
        assert_eq!(payload["strip_document"], serde_json::Value::Null);
        assert_eq!(payload["strip_pre"], serde_json::json!("strip_one"));
        assert_eq!(payload["bs4_options"], serde_json::json!("lxml"));
        Ok(())
    }

    #[test]
    fn build_html_to_markdown_go_args_respects_domain_and_plugins(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let url = url::Url::parse("https://example.com/path")?;
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
        Ok(())
    }
}
