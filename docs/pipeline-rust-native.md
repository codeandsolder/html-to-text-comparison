# Rust-Native HTML-to-Text Extractors

This document details the 13 Rust-native HTML-to-text extractors used in this comparison framework, including their configuration options, processing pipelines, and edge cases.

---

## readability

- **Source**: [`readability`](https://crates.io/crates/readability) crate (Mozilla's Readability algorithm port)
- **Config**: None. This extractor uses the default configuration with no user-configurable options.
- **Pipeline**:
  1. Wraps the input HTML string in a `std::io::Cursor` to create a readable stream
  2. Calls `readability::extractor::extract(&mut html, &parsed_url)` where `parsed_url` is the URL of the page being processed
  3. The Readability algorithm analyzes the DOM to identify the main content area by:
     - Scoring elements based on class/id names, tag types, and content density
     - Removing likely noise elements (navigation, ads, sidebars)
     - Finding the element with the highest content score
  4. Extracts the `.text` field from the result, which contains the cleaned content as plain text
  5. Uses `.unwrap()` on the result - will panic if extraction fails
- **Edge cases**:
  - **Panics**: The extractor uses `.unwrap()` on the result, so malformed HTML or parsing failures will cause a panic. No fallback to empty string.
  - **Empty output**: Returns empty string if no main content is detected (handled by the algorithm gracefully in most cases)
  - **URL dependency**: Requires a valid parsed URL for context-aware extraction (used for relative link resolution and domain-specific heuristics)
  - **No config**: Cannot customize behavior - uses Mozilla's default heuristics exactly

---

## llm_readability

- **Source**: [`llm_readability`](https://crates.io/crates/llm_readability) crate (LLM-optimized version of Readability)
- **Config**: None. Uses default configuration.
- **Pipeline**:
  1. Same as readability: wraps HTML in `std::io::Cursor`
  2. Calls `llm_readability::extractor::extract(&mut html, &parsed_url)`
  3. This is a fork of the Readability algorithm optimized for LLM consumption:
     - Preserves more structural information beneficial for AI models
     - May retain more context that standard Readability strips
     - Still uses URL for content scoring heuristics
  4. Extracts the `.text` field from the result
  5. Uses `.unwrap()` - same panic behavior as readability
- **Edge cases**:
  - **Panics**: Identical to readability - `.unwrap()` will panic on extraction failure
  - **Empty output**: Returns empty if no main content found
  - **URL dependency**: Requires parsed URL for extraction
  - **LLM optimization**: May produce longer output than standard readability as it preserves more content
  - **No config**: Cannot customize extraction behavior

---

## html2text

- **Source**: [`html2text`](https://crates.io/crates/html2text) crate
- **Config**: `Html2TextConfig` struct with three fields:
  - `max_wrap_width`: Maximum character width for text wrapping (default: 1000, min enforced to 1)
  - `raw_mode`: Boolean to enable raw mode (preserves more formatting)
  - `no_link_wrapping`: Boolean to disable link wrapping
- **Pipeline**:
  1. Retrieves config from `states.states.get("html2text")`, defaults to empty config if not found
  2. Wraps HTML in `std::io::Cursor`
  3. Creates a plain text renderer: `html2text::config::plain().max_wrap_width(width).raw_mode(cfg.raw_mode)`
  4. If `cfg.no_link_wrapping` is true, calls `.no_link_wrapping()` to disable wrapping of links
  5. Calls `render.string_from_read(&mut html, width)` to perform conversion
  6. Uses `.unwrap_or_default()` - returns empty string on failure instead of panicking
- **Edge cases**:
  - **Graceful fallback**: Unlike most extractors, this uses `.unwrap_or_default()` so failures return empty string rather than panicking
  - **Width handling**: Enforces minimum width of 1 with `width.max(1)` to prevent zero-width errors
  - **Raw mode**: When enabled, preserves more HTML formatting (useful for preserving complex layouts)
  - **Link wrapping**: Can disable wrapping of long URLs which helps with readability in plain text

---

## htmd

- **Source**: [`htmd`](https://crates.io/crates/htmd) crate (HTML to Markdown)
- **Config**: `HtmdConfig` struct with two fields:
  - `skip_tags`: Vector of tag names to exclude from output (default: nav, script, style, header, footer, img, svg, iframe)
  - `heading_style`: String - "setex" or "atx" (default: "atx")
- **Pipeline**:
  1. Retrieves config from state, handles legacy `skip_tags` in global config (prefers extractor-specific config)
  2. Creates `htmd::options::Options::default()`
  3. Sets heading style: `HeadingStyle::Setex` for "setex", `HeadingStyle::Atx` for anything else
  4. Builds converter: `htmd::HtmlToMarkdown::builder().options(options)`
  5. If skip_tags provided, adds them via `.skip_tags()` (converts to `&str` slice)
  6. Calls `.build().convert(html)` and uses `.unwrap_or_default()`
- **Edge cases**:
  - **Heading style**: "setex" uses underline style (`Heading 1\n==========`), "atx" uses hash prefix (`# Heading 1`)
  - **Legacy config**: Prioritizes `cfg.skip_tags` over global `skip_tags`, allows migration path
  - **Empty fallback**: Returns empty string on conversion failure via `.unwrap_or_default()`
  - **Tag filtering**: Skip tags are case-sensitive string matches, removes entire elements

---

## html2md-rs

- **Source**: [`html2md-rs`](https://crates.io/crates/html2md-rs) crate
- **Config**: `Html2MdRsConfig` struct with one field:
  - `ignore_tags`: Vector of tag names to ignore (default: nav, script, style, header, footer, img, svg, iframe)
- **Pipeline**:
  1. Retrieves config, handles legacy skip_tags from global config (same pattern as htmd)
  2. Creates `ToMdConfig` with `ignore_rendering` containing converted tag set
  3. Converts ignore_tags to `NodeType` via `NodeType::from_tag_str(tag.as_str())`
  4. Calls `safe_from_html_to_md_with_config(html.to_string(), &ToMdConfig {...})`
  5. Uses `.unwrap_or_default()` for graceful failure handling
- **Edge cases**:
  - **Safe function**: Uses `safe_from_html_to_md_with_config` instead of unsafe version, provides error handling
  - **NodeType conversion**: Converts string tag names to NodeType enum - may fail silently for unknown tags
  - **Legacy support**: Same legacy config handling as htmd
  - **Empty fallback**: Returns empty string on failure

---

## mdka

- **Source**: [`mdka`](https://crates.io/crates/mdka) crate
- **Config**: `MdkaConfig` struct with two fields:
  - `mode`: Conversion mode - "strict", "minimal", "semantic", "preserve", or "balanced" (default: "balanced")
  - `drop_interactive_shell`: Boolean to drop interactive shell elements (default: false)
- **Pipeline**:
  1. Retrieves config from state with `.unwrap_or_default()` for missing config
  2. Converts config to mdka's internal `ConversionOptions` via `cfg.clone().into_conversion_options()`
  3. Mode mapping:
     - "strict" -> `ConversionMode::Strict`
     - "minimal" -> `ConversionMode::Minimal`
     - "semantic" -> `ConversionMode::Semantic`
     - "preserve" -> `ConversionMode::Preserve`
     - anything else -> `ConversionMode::Balanced`
  4. Calls `mdka::html_to_markdown_with(html, &options)`
- **Edge cases**:
  - **Mode differences**: 
    - "strict" follows Markdown spec strictly
    - "minimal" strips most formatting
    - "semantic" uses semantic HTML mapping
    - "preserve" keeps as much original as possible
    - "balanced" is the default compromise
  - **Interactive shell**: When `drop_interactive_shell` is true, removes elements like `<input>`, `<button>`
  - **No fallback**: Uses direct call without unwrap - may propagate errors differently

---

## readable-readability

- **Source**: [`readable-readability`](https://crates.io/crates/readable-readability) crate
- **Config**: `ReadableReadabilityConfig` struct with three fields:
  - `strip_unlikelys`: Boolean to strip unlikely candidates (default: true)
  - `weight_classes`: Boolean to weight class names in scoring (default: true)
  - `clean_conditionally`: Boolean to enable conditional cleaning (default: false)
- **Pipeline**:
  1. Creates new parser: `readable_readability::Readability::new()`
  2. Configures parser via chain of setter methods:
     - `.strip_unlikelys(cfg.strip_unlikelys)` - removes elements that look like navigation/ad based on class/id patterns
     - `.weight_classes(cfg.weight_classes)` - uses class names as positive/negative signals
     - `.clean_conditionally(cfg.clean_conditionally)` - enables conditional content removal
  3. Calls `.parse(&html)` which returns `(node, metadata)` tuple
  4. Extracts text via `.text_contents()` method on the node
- **Edge cases**:
  - **Three boolean tunables**: More configurable than standard readability
  - **Strip unlikelys**: When true, removes elements with class/id patterns like "comment", "social", "share", "ad"
  - **Weight classes**: When true, positive classes like "content", "article" boost score; negative ones reduce it
  - **Clean conditionally**: Expensive operation that removes low-value content based on density calculations
  - **Returns tuple**: `.parse()` returns (node, _) - extracts text from first element only

---

## dom_smoothie

- **Source**: [`dom_smoothie`](https://crates.io/crates/dom_smoothie) crate
- **Config**: `DomSmoothieConfig` struct with two fields:
  - `max_elements_to_parse`: Optional usize for parsing limit (default: unbounded/usize::MAX)
  - `text_mode`: String - "raw", "formatted", or "markdown" (default: "markdown")
- **Pipeline**:
  1. Retrieves config, clones it for the closure
  2. Converts config to dom_smoothie internal `Config` via `.into_config()`:
     - Maps `text_mode` string: "raw" -> `TextMode::Raw`, "formatted" -> `TextMode::Formatted`, anything else (including "markdown") -> `TextMode::Markdown`
     - Sets `max_elements_to_parse` (defaults to `usize::MAX` if None)
  3. Creates parser: `dom_smoothie::Readability::new(html, None, dom_cfg.into_config())`
     - Second parameter (options) is None
     - Third parameter is the converted config
  4. Calls `.parse().unwrap()` - will panic on parse failure
  5. Extracts `.text_content.to_string()` from result
- **Edge cases**:
  - **Panics on parse**: Uses `.parse().unwrap()` so parse failures cause panic
  - **Text mode differences**:
    - "raw" - minimal processing, just extracts text
    - "formatted" - applies some formatting
    - "markdown" - outputs Markdown (default)
  - **Max elements**: Limits DOM traversal depth - can improve performance on huge pages but may miss content
  - **None options**: Second parameter to `new()` is None - uses default behavior for that field

---

## boilerpipe

- **Source**: [`boilerpipe`](https://crates.io/crates/boilerpipe) crate (Java boilerpipe port)
- **Config**: None. Uses default configuration.
- **Pipeline**:
  1. Simple one-liner: `boilerpipe::parse_document(html).content().to_string()`
  2. Calls `parse_document()` which creates a boilerpipe document
  3. Extracts `.content()` which is the main content extractor result
  4. Converts to String
- **Edge cases**:
  - **No config**: No customization available
  - **Algorithm**: Uses the classic boilerpipe algorithm - similar to Readability but with different heuristics
  - **Simple pipeline**: Most straightforward conversion - no unwrap, no options
  - **Content extraction**: Focuses specifically on extracting main content, strips boilerplate

---

## august

- **Source**: [`august`](https://crates.io/crates/august) crate
- **Config**: `augus_max_width` (usize) - maximum line width for text wrapping (default: usize::MAX for unlimited)
- **Pipeline**:
  1. Retrieves config with fallback to `usize::MAX`: `states.states.get("august").map(|s| s.config.augus_max_width).unwrap_or(usize::MAX)`
  2. Calls directly: `august::convert(html, cfg)` - no wrapping in Cursor needed
  3. The convert function takes HTML string and max_width parameter
- **Edge cases**:
  - **Width parameter**: When `usize::MAX`, no wrapping occurs (unlimited width)
  - **Simple API**: Direct function call, no builder pattern
  - **Default behavior**: With max_width = usize::MAX, outputs as single long lines
  - **Performance**: Likely fast due to simple API

---

## fast_html2md

- **Source**: [`fast_html2md`](https://crates.io/crates/fast_html2md) crate
- **Config**: None. Uses default configuration.
- **Pipeline**:
  1. Simple call: `fast_html2md::parse_html(html, false)`
  2. Second parameter (false) is likely a boolean option - probably for strict mode or similar
  3. Returns Markdown directly
- **Edge cases**:
  - **No config**: No customization available
  - **Boolean parameter**: Second arg is hardcoded to false - unknown effect without crate docs
  - **Fast**: Name suggests optimization for speed

---

## html2md

- **Source**: [`html2md`](https://crates.io/crates/html2md) crate
- **Config**: None. Uses default configuration.
- **Pipeline**:
  1. Simplest of all: `html2md::parse_html(html)`
  2. Direct function call, no options
  3. Returns Markdown string
- **Edge cases**:
  - **Minimal API**: No config, no options, single function call
  - **Default behavior**: Uses whatever default the crate provides
  - **No error handling visible**: Direct call without unwrap - behavior on error unknown

---

## mdream

- **Source**: [`mdream`](https://crates.io/crates/mdream) crate (modern HTML to Markdown)
- **Config**: `MdreamConfig` struct (defined in `src/extractor_config.rs` lines 52-70) with five boolean fields:
  - `minimal` (default: false) - Enable minimal output mode
  - `isolate_main` (default: false) - Isolate main content only
  - `frontmatter` (default: false) - Extract frontmatter metadata
  - `clean_urls` (default: true) - Clean/normalize URLs in links
  - `tailwind` (default: false) - Enable Tailwind CSS processing
- **Pipeline**:
  1. Imports mdream modules: `html_to_markdown`, `types::{HTMLToMarkdownOptions, CleanConfig, PluginConfig, FilterConfig, IsolateMainConfig, FrontmatterConfig, TailwindConfig}`
  2. Retrieves mdream config from state, defaults if not found
  3. Creates `HTMLToMarkdownOptions::default()`
  4. **URL cleaning**: If `cfg.clean_urls` is true:
     - Sets `opts.clean_urls = true`
     - Creates `CleanConfig` with `urls: true` and other defaults
     - Attaches to `opts.clean = Some(CleanConfig {...})`
  5. **Plugin configuration**: If any of minimal/isolate_main/frontmatter/tailwind are true:
     - Creates `PluginConfig::default()`
     - If `minimal`: Sets `plugins.filter` with `FilterConfig` excluding: nav, footer, aside, form
     - If `isolate_main`: Sets `plugins.isolate_main = Some(IsolateMainConfig::default())`
     - If `frontmatter`: Sets `plugins.frontmatter = Some(FrontmatterConfig::default())`
     - If `tailwind`: Sets `plugins.tailwind = Some(TailwindConfig::default())`
     - Attaches plugins: `opts.plugins = Some(plugins)`
  6. Calls `html_to_markdown(html, opts)` - direct function call
- **Edge cases**:
  - **Most complex config**: mdream has the most sophisticated configuration system
  - **Plugin system**: Uses plugin architecture for extensibility
  - **Clean URLs**: Enabled by default (unlike most others)
  - **Minimal mode**: Filters out common non-content elements (nav, footer, aside, form)
  - **Isolate main**: Attempts to find and extract only the main content area
  - **Frontmatter**: Extracts metadata as YAML frontmatter in output
  - **Tailwind**: Processes Tailwind CSS utility classes in HTML
  - **No fallback**: Direct call without explicit unwrap - check crate docs for error behavior
  - **Boolean-heavy**: All plugin options are boolean flags - enables/disables features

---

## Configuration Summary Table

| Extractor | Config Struct | Key Options | Default Behavior |
|-----------|---------------|-------------|------------------|
| readability | none | N/A | Mozilla Readability algorithm |
| llm_readability | none | N/A | LLM-optimized Readability |
| html2text | Html2TextConfig | max_wrap_width, raw_mode, no_link_wrapping | Wrapped plain text |
| htmd | HtmdConfig | skip_tags, heading_style | Markdown with ATX headings |
| html2md-rs | Html2MdRsConfig | ignore_tags | Markdown |
| mdka | MdkaConfig | mode (strict/minimal/semantic/preserve/balanced), drop_interactive_shell | Markdown with mode selection |
| readable-readability | ReadableReadabilityConfig | strip_unlikelys, weight_classes, clean_conditionally | Configurable Readability |
| dom_smoothie | DomSmoothieConfig | max_elements_to_parse, text_mode | Readability with text mode |
| boilerpipe | none | N/A | Main content extraction |
| august | augus_max_width (usize) | max_width | Plain text with optional wrapping |
| fast_html2md | none | N/A | Fast Markdown conversion |
| html2md | none | N/A | Simple Markdown conversion |
| mdream | MdreamConfig | minimal, isolate_main, frontmatter, clean_urls, tailwind | Full-featured Markdown with plugins |

---

## Error Handling Patterns

- **Panics on failure**: readability, llm_readability, dom_smoothie (use `.unwrap()`)
- **Graceful fallback**: html2text, htmd, html2md-rs (use `.unwrap_or_default()`)
- **Direct calls**: mdka, august, fast_html2md, html2md, mdream (no visible error handling in wrapper)
- **Boilerpipe**: Simple call without visible error handling

## URL Dependencies

- Requires parsed URL: readability, llm_readability
- No URL dependency: All others
