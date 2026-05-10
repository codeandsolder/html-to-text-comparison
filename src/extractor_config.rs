use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurndownConfig {
    pub heading_style: String,
    pub hr: String,
    pub bullet_list_marker: String,
    pub code_block_style: String,
    pub fence: String,
    pub em_delimiter: String,
    pub strong_delimiter: String,
    pub link_style: String,
    pub link_reference_style: String,
    pub preformatted_code: bool,
}

impl Default for TurndownConfig {
    fn default() -> Self {
        Self {
            heading_style: "setext".to_string(),
            hr: "* * *".to_string(),
            bullet_list_marker: "*".to_string(),
            code_block_style: "indented".to_string(),
            fence: "```".to_string(),
            em_delimiter: "_".to_string(),
            strong_delimiter: "**".to_string(),
            link_style: "inlined".to_string(),
            link_reference_style: "full".to_string(),
            preformatted_code: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PercollateConfig {
    pub inline_images: bool,
    pub hyphenate: bool,
    pub fences: bool,
}

impl Default for PercollateConfig {
    fn default() -> Self {
        Self {
            inline_images: false,
            hyphenate: true,
            fences: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MdreamConfig {
    pub minimal: bool,
    pub isolate_main: bool,
    pub frontmatter: bool,
    pub clean_urls: bool,
    pub tailwind: bool,
}

impl Default for MdreamConfig {
    fn default() -> Self {
        Self {
            minimal: false,
            isolate_main: false,
            frontmatter: false,
            clean_urls: true,
            tailwind: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafilaturaConfig {
    pub favor_precision: bool,
    pub favor_recall: bool,
    pub include_comments: bool,
    pub include_tables: bool,
    pub include_images: bool,
    pub include_formatting: bool,
    pub include_links: bool,
    pub deduplicate: bool,
    pub with_metadata: bool,
}

impl Default for TrafilaturaConfig {
    fn default() -> Self {
        Self {
            favor_precision: false,
            favor_recall: false,
            include_comments: true,
            include_tables: true,
            include_images: false,
            include_formatting: false,
            include_links: false,
            deduplicate: true,
            with_metadata: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Html2TextPythonConfig {
    pub ignore_links: bool,
    pub ignore_images: bool,
    pub ignore_emphasis: bool,
    pub body_width: usize,
    pub unicode_snob: bool,
    pub escape_snob: bool,
    pub inline_links: bool,
    pub google_doc: bool,
    pub dash_unordered_list: bool,
}

impl Default for Html2TextPythonConfig {
    fn default() -> Self {
        Self {
            ignore_links: false,
            ignore_images: false,
            ignore_emphasis: false,
            body_width: 78,
            unicode_snob: false,
            escape_snob: false,
            inline_links: true,
            google_doc: false,
            dash_unordered_list: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightpandaConfig {
    pub strip_js: bool,
    pub strip_css: bool,
    pub strip_ui: bool,
    pub wait_until: String,
    pub wait_ms: u64,
}

impl Default for LightpandaConfig {
    fn default() -> Self {
        Self {
            strip_js: true,
            strip_css: true,
            strip_ui: false,
            wait_until: "done".to_string(),
            wait_ms: 5000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebclawConfig {
    pub only_main_content: bool,
    pub include_css: String,
    pub exclude_css: String,
    pub format: String,
}

impl Default for WebclawConfig {
    fn default() -> Self {
        Self {
            only_main_content: false,
            include_css: String::new(),
            exclude_css: String::new(),
            format: "markdown".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E2mConfig {
    pub engine: String,
}

impl Default for E2mConfig {
    fn default() -> Self {
        Self {
            engine: "unstructured".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HtmlToMarkdownGoConfig {
    pub domain: String,
    pub plugins: Vec<String>,
}

impl Default for HtmlToMarkdownGoConfig {
    fn default() -> Self {
        Self {
            domain: String::new(),
            plugins: vec!["commonmark".to_string()],
        }
    }
}

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
    pub turndown: TurndownConfig,
    pub percollate: PercollateConfig,
    pub mdream: MdreamConfig,
    pub trafilatura: TrafilaturaConfig,
    pub html2text_py: Html2TextPythonConfig,
    pub lightpanda: LightpandaConfig,
    pub webclaw: WebclawConfig,
    pub e2m: E2mConfig,
    pub html_to_markdown_go: HtmlToMarkdownGoConfig,
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
            "e2m",
            "fast_html2md",
            "html2md",
            "html2md-rs",
            "html2text",
            "html2text-py",
            "html-to-markdown-go",
            "htmd",
            "lightpanda",
            "llm_readability",
            "mdka",
            "mdream",
            "nanohtml2text",
            "percollate",
            "readability",
            "readable-readability",
            "trafilatura",
            "turndown",
            "webclaw",
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
                "turndown" => ExtractorConfig {
                    turndown: TurndownConfig::default(),
                    ..Default::default()
                },
                "percollate" => ExtractorConfig {
                    percollate: PercollateConfig::default(),
                    ..Default::default()
                },
                "mdream" => ExtractorConfig {
                    mdream: MdreamConfig::default(),
                    ..Default::default()
                },
                "trafilatura" => ExtractorConfig {
                    trafilatura: TrafilaturaConfig::default(),
                    ..Default::default()
                },
                "html2text-py" => ExtractorConfig {
                    html2text_py: Html2TextPythonConfig::default(),
                    ..Default::default()
                },
                "lightpanda" => ExtractorConfig {
                    lightpanda: LightpandaConfig::default(),
                    ..Default::default()
                },
                "webclaw" => ExtractorConfig {
                    webclaw: WebclawConfig::default(),
                    ..Default::default()
                },
                "e2m" => ExtractorConfig {
                    e2m: E2mConfig::default(),
                    ..Default::default()
                },
                "html-to-markdown-go" => ExtractorConfig {
                    html_to_markdown_go: HtmlToMarkdownGoConfig::default(),
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
