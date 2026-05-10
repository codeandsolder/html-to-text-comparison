use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
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
#[serde(default)]
pub struct PercollateConfig {
    pub inline_images: bool,
    pub hyphenate: bool,
    pub fences: bool,
    pub fence: String,
    pub emphasis: String,
    pub strong: String,
    pub resource_link: bool,
    pub rule: String,
}

impl Default for PercollateConfig {
    fn default() -> Self {
        Self {
            inline_images: false,
            hyphenate: true,
            fences: true,
            fence: "`".repeat(3),
            emphasis: "_".to_string(),
            strong: "_".to_string(),
            resource_link: true,
            rule: "-".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MdreamConfig {
    pub minimal: bool,
    pub isolate_main: bool,
    pub frontmatter: bool,
    pub clean_urls: bool,
    pub tailwind: bool,
    pub origin: String,
    pub clean_fragments: bool,
    pub clean_empty_links: bool,
    pub clean_blank_lines: bool,
    pub clean_redundant_links: bool,
    pub clean_self_link_headings: bool,
    pub clean_empty_images: bool,
    pub clean_empty_link_text: bool,
    pub filter_include: Vec<String>,
    pub filter_exclude: Vec<String>,
    pub filter_process_children: bool,
    pub frontmatter_meta_fields: Vec<String>,
    pub extraction_selectors: Vec<String>,
}

impl Default for MdreamConfig {
    fn default() -> Self {
        Self {
            minimal: false,
            isolate_main: false,
            frontmatter: false,
            clean_urls: true,
            tailwind: false,
            origin: String::new(),
            clean_fragments: false,
            clean_empty_links: false,
            clean_blank_lines: false,
            clean_redundant_links: false,
            clean_self_link_headings: false,
            clean_empty_images: false,
            clean_empty_link_text: false,
            filter_include: Vec::new(),
            filter_exclude: Vec::new(),
            filter_process_children: false,
            frontmatter_meta_fields: Vec::new(),
            extraction_selectors: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
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
#[serde(default)]
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
#[serde(default)]
pub struct MarkdownifyConfig {
    pub strip: Vec<String>,
    pub convert: Vec<String>,
    pub autolinks: bool,
    pub default_title: bool,
    pub heading_style: String,
    pub bullets: String,
    pub strong_em_symbol: String,
    pub sub_symbol: String,
    pub sup_symbol: String,
    pub newline_style: String,
    pub code_language: String,
    pub escape_asterisks: bool,
    pub escape_underscores: bool,
    pub escape_misc: bool,
    pub keep_inline_images_in: Vec<String>,
    pub table_infer_header: bool,
    pub wrap: bool,
    pub wrap_width: Option<usize>,
    pub strip_document: String,
    pub strip_pre: String,
    pub bs4_parser: String,
}

impl Default for MarkdownifyConfig {
    fn default() -> Self {
        Self {
            strip: Vec::new(),
            convert: Vec::new(),
            autolinks: true,
            default_title: false,
            heading_style: "UNDERLINED".to_string(),
            bullets: "*+-".to_string(),
            strong_em_symbol: "ASTERISK".to_string(),
            sub_symbol: String::new(),
            sup_symbol: String::new(),
            newline_style: "SPACES".to_string(),
            code_language: String::new(),
            escape_asterisks: true,
            escape_underscores: true,
            escape_misc: false,
            keep_inline_images_in: Vec::new(),
            table_infer_header: false,
            wrap: false,
            wrap_width: Some(80),
            strip_document: "STRIP".to_string(),
            strip_pre: "STRIP".to_string(),
            bs4_parser: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
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
#[serde(default)]
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
#[serde(default)]
pub struct E2mConfig {
    pub engine: String,
    pub langs: Vec<String>,
    pub skip_headers_and_footers: bool,
    pub include_image_link_in_text: bool,
}

impl Default for E2mConfig {
    fn default() -> Self {
        Self {
            engine: "unstructured".to_string(),
            langs: vec!["en".to_string(), "zh".to_string()],
            skip_headers_and_footers: true,
            include_image_link_in_text: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HtmlToMarkdownGoConfig {
    pub domain: String,
    pub plugins: Vec<String>,
    pub include_selector: String,
    pub exclude_selector: String,
}

impl Default for HtmlToMarkdownGoConfig {
    fn default() -> Self {
        Self {
            domain: String::new(),
            plugins: vec!["commonmark".to_string()],
            include_selector: String::new(),
            exclude_selector: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Html2TextConfig {
    pub max_wrap_width: usize,
    pub min_wrap_width: usize,
    pub raw_mode: bool,
    pub no_link_wrapping: bool,
    pub link_footnotes: bool,
    pub no_table_borders: bool,
    pub unicode_strikeout: bool,
    pub decorate: bool,
    pub pad_block_width: bool,
    pub allow_width_overflow: bool,
}

impl Default for Html2TextConfig {
    fn default() -> Self {
        Self {
            max_wrap_width: 1000,
            min_wrap_width: 3,
            raw_mode: false,
            no_link_wrapping: false,
            link_footnotes: false,
            no_table_borders: false,
            unicode_strikeout: true,
            decorate: false,
            pad_block_width: false,
            allow_width_overflow: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HtmdConfig {
    pub skip_tags: Vec<String>,
    pub heading_style: String,
    pub hr_style: String,
    pub br_style: String,
    pub link_style: String,
    pub link_reference_style: String,
    pub code_block_style: String,
    pub code_block_fence: String,
    pub bullet_list_marker: String,
    pub ul_bullet_spacing: u8,
    pub ol_number_spacing: u8,
    pub preformatted_code: bool,
    pub translation_mode: String,
}

impl Default for HtmdConfig {
    fn default() -> Self {
        Self {
            skip_tags: Vec::new(),
            heading_style: "atx".to_string(),
            hr_style: "asterisks".to_string(),
            br_style: "two_spaces".to_string(),
            link_style: "inlined".to_string(),
            link_reference_style: "full".to_string(),
            code_block_style: "fenced".to_string(),
            code_block_fence: "backticks".to_string(),
            bullet_list_marker: "*".to_string(),
            ul_bullet_spacing: 3,
            ol_number_spacing: 2,
            preformatted_code: false,
            translation_mode: "pure".to_string(),
        }
    }
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
    pub preserve_ids: Option<bool>,
    pub preserve_classes: Option<bool>,
    pub preserve_data_attrs: Option<bool>,
    pub preserve_aria_attrs: Option<bool>,
    pub preserve_unknown_attrs: Option<bool>,
    pub drop_presentation_attrs: Option<bool>,
    pub drop_interactive_shell: Option<bool>,
    pub unwrap_unknown_wrappers: Option<bool>,
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
        let mut options = mdka::options::ConversionOptions::for_mode(mode);
        if let Some(v) = self.preserve_ids {
            options.preserve_ids = v;
        }
        if let Some(v) = self.preserve_classes {
            options.preserve_classes = v;
        }
        if let Some(v) = self.preserve_data_attrs {
            options.preserve_data_attrs = v;
        }
        if let Some(v) = self.preserve_aria_attrs {
            options.preserve_aria_attrs = v;
        }
        if let Some(v) = self.preserve_unknown_attrs {
            options.preserve_unknown_attrs = v;
        }
        if let Some(v) = self.drop_presentation_attrs {
            options.drop_presentation_attrs = v;
        }
        if let Some(v) = self.drop_interactive_shell {
            options.drop_interactive_shell = v;
        }
        if let Some(v) = self.unwrap_unknown_wrappers {
            options.unwrap_unknown_wrappers = v;
        }
        options
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ReadableReadabilityConfig {
    pub strip_unlikelys: bool,
    pub weight_classes: bool,
    pub clean_conditionally: bool,
    pub clean_attributes: bool,
}

impl Default for ReadableReadabilityConfig {
    fn default() -> Self {
        Self {
            strip_unlikelys: true,
            weight_classes: true,
            clean_conditionally: true,
            clean_attributes: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DomSmoothieConfig {
    pub max_elements_to_parse: Option<usize>,
    pub text_mode: String,
    pub keep_classes: bool,
    pub classes_to_preserve: Vec<String>,
    pub disable_json_ld: bool,
    pub n_top_candidates: usize,
    pub char_threshold: usize,
    pub min_score_to_adjust: f32,
    pub candidate_select_mode: String,
}

impl DomSmoothieConfig {
    pub fn into_config(self) -> Option<dom_smoothie::Config> {
        let text_mode = match self.text_mode.as_str() {
            "raw" => dom_smoothie::TextMode::Raw,
            "formatted" => dom_smoothie::TextMode::Formatted,
            "markdown" | _ => dom_smoothie::TextMode::Markdown,
        };
        let candidate_select_mode = match self.candidate_select_mode.as_str() {
            "dom_smoothie" => dom_smoothie::CandidateSelectMode::DomSmoothie,
            "readability" | _ => dom_smoothie::CandidateSelectMode::Readability,
        };
        Some(dom_smoothie::Config {
            keep_classes: self.keep_classes,
            classes_to_preserve: self.classes_to_preserve,
            max_elements_to_parse: self.max_elements_to_parse.unwrap_or(0),
            disable_json_ld: self.disable_json_ld,
            n_top_candidates: self.n_top_candidates,
            char_threshold: self.char_threshold,
            min_score_to_adjust: self.min_score_to_adjust,
            candidate_select_mode,
            text_mode,
            ..Default::default()
        })
    }
}

impl Default for DomSmoothieConfig {
    fn default() -> Self {
        Self {
            max_elements_to_parse: None,
            text_mode: "markdown".to_string(),
            keep_classes: false,
            classes_to_preserve: Vec::new(),
            disable_json_ld: false,
            n_top_candidates: 5,
            char_threshold: 500,
            min_score_to_adjust: 5.0,
            candidate_select_mode: "readability".to_string(),
        }
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
    pub markdownify: MarkdownifyConfig,
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
            "markdownify",
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
                        ..Default::default()
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
                        ..Default::default()
                    },
                    ..Default::default()
                },
                "mdka" => ExtractorConfig {
                    mdka: MdkaConfig {
                        mode: "balanced".to_string(),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                "readable-readability" => ExtractorConfig {
                    readable_readability: ReadableReadabilityConfig {
                        strip_unlikelys: true,
                        weight_classes: true,
                        clean_conditionally: false,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                "dom_smoothie" => ExtractorConfig {
                    dom_smoothie: DomSmoothieConfig {
                        ..Default::default()
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
                "markdownify" => ExtractorConfig {
                    markdownify: MarkdownifyConfig::default(),
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

#[cfg(test)]
mod tests {
    use super::MdkaConfig;

    #[test]
    fn mdka_config_preserves_mode_defaults_when_optional_flags_are_unset() {
        let options = MdkaConfig {
            mode: "minimal".to_string(),
            ..Default::default()
        }
        .into_conversion_options();

        assert!(!options.preserve_ids);
        assert!(!options.preserve_classes);
        assert!(!options.preserve_data_attrs);
        assert!(!options.preserve_aria_attrs);
        assert!(!options.preserve_unknown_attrs);
        assert!(options.drop_presentation_attrs);
        assert!(options.drop_interactive_shell);
        assert!(options.unwrap_unknown_wrappers);
    }

    #[test]
    fn mdka_config_applies_optional_overrides() {
        let options = MdkaConfig {
            mode: "balanced".to_string(),
            preserve_ids: Some(false),
            preserve_classes: Some(true),
            preserve_data_attrs: Some(true),
            preserve_aria_attrs: Some(false),
            preserve_unknown_attrs: Some(true),
            drop_presentation_attrs: Some(false),
            drop_interactive_shell: Some(true),
            unwrap_unknown_wrappers: Some(true),
        }
        .into_conversion_options();

        assert!(!options.preserve_ids);
        assert!(options.preserve_classes);
        assert!(options.preserve_data_attrs);
        assert!(!options.preserve_aria_attrs);
        assert!(options.preserve_unknown_attrs);
        assert!(!options.drop_presentation_attrs);
        assert!(options.drop_interactive_shell);
        assert!(options.unwrap_unknown_wrappers);
    }
}
