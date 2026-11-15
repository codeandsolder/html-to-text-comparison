use serde::{Deserialize, Serialize};
use std::io::Write;

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
#[expect(
    clippy::struct_excessive_bools,
    reason = "These independently configurable flags map directly to the MdreamConfig extractor settings exposed through serde"
)]
pub struct MdreamConfig {
    pub minimal: bool,
    pub isolate_main: bool,
    pub frontmatter: bool,
    pub clean_urls: bool,
    pub tailwind: bool,
    pub origin: String,
    pub clean_fragments: bool,
    pub clean_empty_links: bool,
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
#[expect(
    clippy::struct_excessive_bools,
    reason = "These independently configurable flags map directly to the TrafilaturaConfig extractor settings exposed through serde"
)]
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
#[expect(
    clippy::struct_excessive_bools,
    reason = "These independently configurable flags map directly to the Html2TextPythonConfig extractor settings exposed through serde"
)]
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
#[expect(
    clippy::struct_excessive_bools,
    reason = "These independently configurable flags map directly to the MarkdownifyConfig extractor settings exposed through serde"
)]
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
#[expect(
    clippy::struct_excessive_bools,
    reason = "These independently configurable flags map directly to the Html2TextConfig extractor settings exposed through serde"
)]
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
    pub drop_interactive_shell: Option<bool>,
    pub emphasis_from_style: Option<bool>,
    pub backslash_hard_breaks: Option<bool>,
}

impl MdkaConfig {
    pub fn into_conversion_options(self) -> Result<mdka::options::ConversionOptions, String> {
        let mode: mdka::options::ConversionMode = if self.mode.is_empty() {
            mdka::options::ConversionMode::Balanced
        } else {
            self.mode.parse()?
        };
        let mut options = mdka::options::ConversionOptions::for_mode(mode);
        if let Some(value) = self.preserve_ids {
            options.preserve_ids = value;
        }
        if let Some(value) = self.drop_interactive_shell {
            options.drop_interactive_shell = value;
        }
        if let Some(value) = self.emphasis_from_style {
            options.emphasis_from_style = value;
        }
        if let Some(value) = self.backslash_hard_breaks {
            options.backslash_hard_breaks = value;
        }
        Ok(options)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "These independently configurable flags map directly to the ReadableReadabilityConfig extractor settings exposed through serde"
)]
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
    pub fn into_config(self) -> dom_smoothie::Config {
        let text_mode = match self.text_mode.as_str() {
            "raw" => dom_smoothie::TextMode::Raw,
            "formatted" => dom_smoothie::TextMode::Formatted,
            _ => dom_smoothie::TextMode::Markdown,
        };
        let candidate_select_mode = match self.candidate_select_mode.as_str() {
            "dom_smoothie" => dom_smoothie::CandidateSelectMode::DomSmoothie,
            _ => dom_smoothie::CandidateSelectMode::Readability,
        };
        dom_smoothie::Config {
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
        }
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
    /// Hard process deadline; None uses a backend-specific default.
    pub timeout_ms: Option<u64>,
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
    pub webclaw: WebclawConfig,
    pub e2m: E2mConfig,
    pub html_to_markdown_go: HtmlToMarkdownGoConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractorState {
    pub enabled: bool,
    pub config: ExtractorConfig,
}

pub fn default_timeout_ms(name: &str) -> u64 {
    match name {
        "e2m" => 120_000,
        "webclaw" => 60_000,
        "percollate" | "trafilatura" | "markdownify" | "html2text-py" | "resiliparse" => 45_000,
        "turndown" | "html-to-markdown-go" | "webforai" => 30_000,
        _ => 10_000,
    }
}

impl ExtractorState {
    pub fn timeout(&self, name: &str) -> std::time::Duration {
        std::time::Duration::from_millis(
            self.config
                .timeout_ms
                .unwrap_or_else(|| default_timeout_ms(name)),
        )
    }
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
                if let Ok(loaded) = serde_json::from_str::<Self>(&s) {
                    let mut merged = defaults;
                    for (name, mut state) in loaded.states {
                        if let Some(current) = merged.states.get_mut(&name) {
                            if state.config.timeout_ms.is_none() {
                                state.config.timeout_ms = Some(default_timeout_ms(&name));
                            }
                            *current = state;
                        }
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
        let serialized = serde_json::to_vec_pretty(self).map_err(|error| error.to_string())?;
        let parent = path.parent().ok_or("states path has no parent directory")?;
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let mut temporary =
            tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;

        temporary
            .write_all(&serialized)
            .map_err(|error| error.to_string())?;
        temporary.persist(path).map_err(|error| error.to_string())?;
        Ok(())
    }
}

impl Default for ExtractorStates {
    #[expect(
        clippy::too_many_lines,
        reason = "Defines the independent defaults for each supported extractor; centralized schema inventory must remain exhaustive and auditable"
    )]
    fn default() -> Self {
        let mut states = std::collections::HashMap::new();
        let all_extractors = [
            "august",
            "boilerpipe",
            "dom_smoothie",
            "defuddle",
            "e2m",
            "fast_html2md",
            "html2md",
            "html2md-rs",
            "html2text",
            "html2text-py",
            "markdownify",
            "mozilla-readability",
            "html-to-markdown-go",
            "htmd",
            "llm_readability",
            "mdka",
            "mdream",
            "nanohtml2text",
            "node-html-markdown",
            "percollate",
            "readability",
            "readable-readability",
            "resiliparse",
            "trafilatura",
            "turndown",
            "webclaw",
            "webforai",
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
                    skip_tags: DEFAULT_SKIP_TAGS
                        .iter()
                        .map(std::string::ToString::to_string)
                        .collect(),
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
                    dom_smoothie: DomSmoothieConfig::default(),
                    ..Default::default()
                },
                "html2md-rs" => ExtractorConfig {
                    skip_tags: DEFAULT_SKIP_TAGS
                        .iter()
                        .map(std::string::ToString::to_string)
                        .collect(),
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
                    config: ExtractorConfig {
                        timeout_ms: Some(default_timeout_ms(name)),
                        ..cfg
                    },
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
    fn extractor_timeouts_match_backend_costs() {
        let states = super::ExtractorStates::default();
        assert_eq!(states.states["e2m"].timeout("e2m").as_millis(), 120_000);
        assert_eq!(
            states.states["trafilatura"]
                .timeout("trafilatura")
                .as_millis(),
            45_000
        );
        assert_eq!(
            states.states["html2text"].timeout("html2text").as_millis(),
            10_000
        );
    }

    #[test]
    fn mdka_config_preserves_mode_defaults_when_optional_flags_are_unset() -> Result<(), String> {
        let options = MdkaConfig {
            mode: "minimal".to_string(),
            ..Default::default()
        }
        .into_conversion_options()?;

        assert!(!options.preserve_ids);
        assert!(options.drop_interactive_shell);
        assert!(!options.emphasis_from_style);
        assert!(!options.backslash_hard_breaks);
        Ok(())
    }

    #[test]
    fn mdka_config_applies_optional_overrides() -> Result<(), String> {
        let options = MdkaConfig {
            mode: "balanced".to_string(),
            preserve_ids: Some(false),
            drop_interactive_shell: Some(true),
            emphasis_from_style: Some(true),
            backslash_hard_breaks: Some(true),
        }
        .into_conversion_options()?;

        assert!(!options.preserve_ids);
        assert!(options.drop_interactive_shell);
        assert!(options.emphasis_from_style);
        assert!(options.backslash_hard_breaks);
        assert!(MdkaConfig {
            mode: "strict".to_string(),
            ..Default::default()
        }
        .into_conversion_options()
        .is_err());
        Ok(())
    }
}
