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
            runner.run(output_name, |html| {
                let mut html = std::io::Cursor::new(html.as_bytes());
                html2text::from_read(&mut html, 1000).unwrap_or_default()
            });
        }
        #[cfg(feature = "htmd")]
        "htmd" => {
            let cfg = states
                .states
                .get("htmd")
                .map(|s| s.config.htmd.clone())
                .unwrap_or_default();
            let global_skip_tags = states
                .states
                .get("htmd")
                .map(|s| s.config.skip_tags.clone())
                .unwrap_or_default();
            runner.run(output_name, move |html| {
                htmd::HtmlToMarkdown::builder()
                    .skip_tags(if global_skip_tags.is_empty() {
                        cfg.skip_tags.iter().map(|s| s.as_str()).collect()
                    } else {
                        global_skip_tags.iter().map(|s| s.as_str()).collect()
                    })
                    .build()
                    .convert(html)
                    .unwrap_or_default()
            });
        }
        #[cfg(feature = "html2md-rs")]
        "html2md-rs" => {
            let cfg = states
                .states
                .get("html2md-rs")
                .map(|s| s.config.html2md_rs.clone())
                .unwrap_or_default();
            let global_skip_tags = states
                .states
                .get("html2md-rs")
                .map(|s| s.config.skip_tags.clone())
                .unwrap_or_default();
            runner.run(output_name, move |html| {
                use html2md_rs::structs::{NodeType, ToMdConfig};
                use html2md_rs::to_md::safe_from_html_to_md_with_config;
                let tags = if global_skip_tags.is_empty() {
                    cfg.ignore_tags.clone()
                } else {
                    global_skip_tags.clone()
                };
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
        _ => {}
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
