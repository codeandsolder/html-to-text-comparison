use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Html2TextConfig {
    pub max_wrap_width: usize,
    pub raw_mode: bool,
    pub no_link_wrapping: bool,
}

impl Default for Html2TextConfig {
    fn default() -> Self {
        Self {
            max_wrap_width: 1000,
            raw_mode: false,
            no_link_wrapping: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HtmdConfig {
    pub skip_tags: Vec<String>,
    pub heading_style: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Html2MdRsConfig {
    pub ignore_tags: Vec<String>,
}

pub const DEFAULT_SKIP_TAGS: &[&str] = &[
    "nav", "script", "style", "header", "footer", "img", "svg", "iframe",
];

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MdkaConfig {
    pub mode: String,
    pub drop_interactive_shell: bool,
}

impl MdkaConfig {
    pub fn into_conversion_options(self) -> mdka::options::ConversionOptions {
        let mode = match self.mode.as_str() {
            "strict" => mdka::options::ConversionMode::Strict,
            "minimal" => mdka::options::ConversionMode::Minimal,
            "semantic" => mdka::options::ConversionMode::Semantic,
            "preserve" => mdka::options::ConversionMode::Preserve,
            _ => mdka::options::ConversionMode::Balanced,
        };
        mdka::options::ConversionOptions::for_mode(mode)
            .drop_interactive_shell(self.drop_interactive_shell)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReadableReadabilityConfig {
    pub strip_unlikelys: bool,
    pub weight_classes: bool,
    pub clean_conditionally: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DomSmoothieConfig {
    pub max_elements_to_parse: Option<usize>,
    pub text_mode: String,
}

impl DomSmoothieConfig {
    pub fn into_config(self) -> Option<dom_smoothie::Config> {
        let text_mode = match self.text_mode.as_str() {
            "raw" => dom_smoothie::TextMode::Raw,
            "formatted" => dom_smoothie::TextMode::Formatted,
            "markdown" | _ => dom_smoothie::TextMode::Markdown,
        };
        Some(dom_smoothie::Config {
            max_elements_to_parse: self.max_elements_to_parse.unwrap_or(usize::MAX),
            text_mode,
            ..Default::default()
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtractorConfig {
    pub skip_tags: Vec<String>,
    pub html2text: Html2TextConfig,
    pub htmd: HtmdConfig,
    pub html2md_rs: Html2MdRsConfig,
    pub mdka: MdkaConfig,
    pub readable_readability: ReadableReadabilityConfig,
    pub dom_smoothie: DomSmoothieConfig,
    pub augus_max_width: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractorState {
    pub enabled: bool,
    pub config: ExtractorConfig,
}

impl Default for ExtractorState {
    fn default() -> Self {
        Self {
            enabled: true,
            config: ExtractorConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractorStates {
    pub states: std::collections::HashMap<String, ExtractorState>,
}

impl ExtractorStates {
    pub fn load(path: &std::path::PathBuf) -> Self {
        let defaults = Self::default();
        match std::fs::read_to_string(path) {
            Ok(s) => {
                if let Ok(loaded) = serde_json::from_str::<ExtractorStates>(&s) {
                    let mut merged = defaults;
                    for (name, state) in loaded.states {
                        merged.states.insert(name, state);
                    }
                    merged
                } else {
                    defaults
                }
            }
            Err(_) => defaults,
        }
    }

    pub fn save(&self, path: &std::path::PathBuf) -> Result<(), String> {
        let s = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, s).map_err(|e| e.to_string())
    }
}

impl Default for ExtractorStates {
    fn default() -> Self {
        let mut states = std::collections::HashMap::new();
        let all_extractors = [
            "august",
            "boilerpipe",
            "dom_smoothie",
            "fast_html2md",
            "htmd",
            "html2md",
            "html2md-rs",
            "html2text",
            "llm_readability",
            "mdka",
            "nanohtml2text",
            "readability",
            "readable-readability",
        ];
        for name in all_extractors {
            let cfg = match name {
                "html2text" => ExtractorConfig {
                    html2text: Html2TextConfig {
                        max_wrap_width: 1000,
                        raw_mode: false,
                        no_link_wrapping: false,
                    },
                    ..Default::default()
                },
                "htmd" => ExtractorConfig {
                    skip_tags: DEFAULT_SKIP_TAGS.iter().map(|s| s.to_string()).collect(),
                    htmd: HtmdConfig {
                        skip_tags: vec![
                            "nav".to_string(),
                            "script".to_string(),
                            "style".to_string(),
                            "header".to_string(),
                            "footer".to_string(),
                            "img".to_string(),
                            "svg".to_string(),
                            "iframe".to_string(),
                        ],
                        heading_style: "atx".to_string(),
                    },
                    ..Default::default()
                },
                "mdka" => ExtractorConfig {
                    mdka: MdkaConfig {
                        mode: "balanced".to_string(),
                        drop_interactive_shell: false,
                    },
                    ..Default::default()
                },
                "readable-readability" => ExtractorConfig {
                    readable_readability: ReadableReadabilityConfig {
                        strip_unlikelys: true,
                        weight_classes: true,
                        clean_conditionally: false,
                    },
                    ..Default::default()
                },
                "dom_smoothie" => ExtractorConfig {
                    dom_smoothie: DomSmoothieConfig {
                        max_elements_to_parse: None,
                        text_mode: "markdown".to_string(),
                    },
                    ..Default::default()
                },
                "html2md-rs" => ExtractorConfig {
                    skip_tags: DEFAULT_SKIP_TAGS.iter().map(|s| s.to_string()).collect(),
                    html2md_rs: Html2MdRsConfig {
                        ignore_tags: vec![
                            "nav".to_string(),
                            "script".to_string(),
                            "style".to_string(),
                            "header".to_string(),
                            "footer".to_string(),
                            "img".to_string(),
                            "svg".to_string(),
                            "iframe".to_string(),
                        ],
                    },
                    ..Default::default()
                },
                "august" => ExtractorConfig {
                    augus_max_width: usize::MAX,
                    ..Default::default()
                },
                _ => ExtractorConfig::default(),
            };
            states.insert(
                name.to_string(),
                ExtractorState {
                    enabled: true,
                    config: cfg,
                },
            );
        }
        Self { states }
    }
}
