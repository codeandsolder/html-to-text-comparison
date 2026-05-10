# webclaw Extractor Analysis

## Overview

**webclaw** is a high-performance web content extraction tool written in Rust, designed specifically for LLM consumption. The benchmark uses webclaw version **0.5.8** (binary at `/home/jan/git/webclaw/webclaw_bin` or `/tmp/webclaw-v0.5.8-x86_64-unknown-linux-gnu/webclaw`).

> **Note:** The binary at `/tmp/html2markdown` is **NOT webclaw** — it is `html-to-markdown` v2.5.1 by Johannes Kaufmann, a separate Go tool with different capabilities. The benchmark correctly uses the actual webclaw binary for extraction.

## Architecture

webclaw is built as a Rust workspace with focused crates:

```
webclaw/
  crates/
    webclaw-core/     # Pure extraction engine. WASM-safe. Zero network deps.
    webclaw-fetch/    # HTTP client with TLS fingerprinting
    webclaw-llm/      # LLM provider chain
    webclaw-pdf/      # PDF text extraction
    webclaw-cli/      # CLI binary
```

The core extraction engine (`webclaw-core`) takes raw HTML as a `&str` and returns structured output — no network calls, no I/O. This design makes it WASM-compatible.

## Extraction Algorithm

webclaw uses a **readability-style multi-signal content detection** approach:

### 1. Readability Scoring (`extractor.rs`)
- **Text density scoring**: Elements with higher text-to-HTML ratios score higher
- **Semantic tag bonuses**: `<article>` and `<main>` receive +50 points, content-class/ID elements receive +25 points
- **Link density penalty**: Elements with >50% links get 0.1x score multiplier; >30% links get 0.5x
- **Scoring minimum**: 50 characters of text required to be considered

### 2. Noise Filtering (`noise.rs`)
Removes common boilerplate patterns:
- Tags: `<script>`, `<style>`, `<noscript>`, `<iframe>`
- ARIA roles: `navigation`, `complementary`, `contentinfo`
- Class/ID patterns: `nav`, `menu`, `sidebar`, `footer`, `header`, `ad`, `social`
- **Tailwind-safe**: Recognizes and ignores Tailwind utility classes (e.g., `flex`, `p-4`, `bg-white`)

### 3. Data Island Extraction (`data_island.rs`)

Handles JavaScript-heavy SPAs by extracting embedded JSON from HTML script tags. Falls back when DOM word count < 30.

**JSON extraction pattern** (lines 41-68): Walks all `script[type='application/json']` blocks and recursively extracts text content from nested JSON structures.

**Contentful rich text** (lines 128-134): Handles the `{ "nodeType": "document", "content": [...] }` pattern used by Contentful CMS rich text format.

**CMS entry pattern**: heading + description/title/body pairs — common in headless CMS systems.

**Quote/testimonial pattern**: `quote` / `quoteText` + `author`/`position` — extracts testimonials and pull quotes.

**Recovery flow** (`lib.rs` lines 179-185): Only fires if DOM word count < 500, deduplicates against existing markdown, appends recovered content.

### 4. Domain-Specific Heuristics (`domain.rs`)
Auto-detects site type and adapts strategy accordingly. Detection order:
1. **URL patterns first**: GitHub, docs sites (readme.io, readthedocs.io, gitbook.io), forums, social, ecommerce
2. **DOM fallback**: `<article>` tag or `og:type=article` or Schema.org Article → Article, else Generic

| Type | URL Patterns | DOM Signals |
|------|-------------|-------------|
| `article` | — | `<article>`, `og:type=article`, JSON-LD Article |
| `documentation` | readme.io, readthedocs.io, gitbook.io | — |
| `github` | github.com, github.io | — |
| `forum` | stackoverflow.com, discourse.com, phpbb | — |
| `ecommerce` | shopify.com, bigcommerce.com, woocommerce | — |
| `social` | twitter.com, x.com, facebook.com, linkedin.com, reddit.com, instagram.com | — |
| `generic` | (fallback) | Everything else |

## Configuration (WebclawConfig)

Defined in `/home/jan/git/html-to-text-comparison/src/extractor_config.rs`:

```rust
pub struct WebclawConfig {
    pub only_main_content: bool,  // default: false
    pub include_css: String,       // default: ""
    pub exclude_css: String,       // default: ""
    pub format: String,            // default: "markdown"
}
```

The `run_webclaw` function in `scores.rs` (line 882-937) translates this config into CLI arguments:
- `--only-main-content` when enabled
- `--include <selector>` when `include_css` is non-empty
- `--exclude <selector>` when `exclude_css` is non-empty
- `-f <format>` always (defaults to "markdown")

## Output Formats

webclaw supports 5 output formats specified via `-f/--format`:

### `markdown` (default)
Clean markdown with resolved URLs and preserved formatting.

**Characteristics:**
- Headings: `#` through `######`
- Bold: `**text**`
- Italic: `*text*`
- Links: `[text](url)`
- Images: `![alt](src)`
- Code blocks: fenced with language (e.g., ` ```rust `)
- Tables: GFM pipe syntax
- Blockquotes: `>`
- Horizontal rules: `---`
- Lists: `-` for unordered, `1.` for ordered

**Example output:**
```markdown
# Main Article Title

This is a **bold** and *italic* paragraph with a [link](https://example.com).

*This is a bold and italic paragraph with a link.*

## Section Heading

Another paragraph with `inline code`.

  ![Example image](/image.png)

> This is a blockquote with important info.

- List item one
- List item two

1. Numbered one
2. Numbered two

```rust
fn main() { println!("Hello"); }
```

| Header A | Header B |
| --- | --- |
| Cell 1 | Cell 2 |

---

End of content.
```

### `text`
Plain text with no markdown formatting.

**Characteristics:**
- No markdown syntax preserved
- Links embedded inline: `text (https://example.com)`
- Images show alt text only
- Tables use tab separators
- Code blocks use double-backticks (no language)
- Blockquotes prefixed with `>`
- Horizontal rules preserved as `---`

**Example output:**
```
Main Article Title

This is a bold and italic paragraph with a link.

Section Heading

Another paragraph with inline code.

  Example image

> This is a blockquote with important info.

- List item one
- List item two

1. Numbered one
2. Numbered two

``rust
fn main() { println!("Hello"); }
``

Header A	Header B
Cell 1	Cell 2

---

End of content.
```

### `llm`
LLM-optimized output with 9-step optimization pipeline:
1. Image stripping
2. Emphasis removal
3. Link deduplication
4. Stat block merging
5. Whitespace collapse
6. Navigation link filtering
7. Action link removal
8. Redundant content elimination
9. Metadata header inclusion

**Characteristics:**
- Metadata header with title, word count, language
- Inline emphasis converted to plain text
- Links collected at end (deduplicated)
- Images stripped from inline text
- Reduced whitespace
- ~67% token reduction vs raw HTML

**Example output:**
```
> Title: Test Page
> Word count: 86

# Main Article Title

This is bold and italic paragraph with a link.

## Section Heading

Another paragraph with `inline code`.

> This is a blockquote with important info.

- List item one
- List item two

1. Numbered one
2. Numbered two

```rust
fn main() { println!("Hello"); }
```

| Header A | Header B |
| --- | --- |
| Cell 1 | Cell 2 | ---

End of content.

## Links
- link: https://example.com
```

### `json`
Full `ExtractionResult` structure as JSON including:
- `metadata`: title, description, author, published_date, language, url, site_name, image, favicon, word_count
- `content.markdown`: full markdown content
- `content.plain_text`: plain text version
- `content.links`: array of `{text, href}` objects
- `content.images`: array of `{alt, src}` objects
- `content.code_blocks`: array of `{language, code}` objects
- `domain_data`: including `domain_type`
- `structured_data`: JSON-LD extracted from script blocks

**Example output:**
```json
{
  "metadata": {
    "title": "Test Page",
    "description": null,
    "author": null,
    "published_date": null,
    "language": null,
    "url": null,
    "site_name": null,
    "image": null,
    "favicon": null,
    "word_count": 86
  },
  "content": {
    "markdown": "# Main Article Title\n\n...",
    "plain_text": "Main Article Title\n\n...",
    "links": [{"text": "link", "href": "https://example.com"}],
    "images": [{"alt": "Example image", "src": "/image.png"}],
    "code_blocks": [{"language": "rust", "code": "fn main() { println!(\"Hello\"); }"}]
  },
  "domain_data": {"domain_type": "article"},
  "structured_data": [
    {"@type": "Product", "name": "Example Widget", "offers": {"@type": "Offer", "price": "29.99", "priceCurrency": "USD"}},
    {"@type": "BreadcrumbList", "itemListElement": [{"@type": "ListItem", "position": 1, "name": "Home", "item": "https://example.com/"}, {"@type": "ListItem", "position": 2, "name": "Products", "item": "https://example.com/products"}]}
  ]
}
```

## Domain Type Detection (`domain.rs`)

webclaw auto-detects the site type to adapt its extraction strategy. The `DomainType` enum is defined in `/home/jan/git/webclaw/crates/webclaw-core/src/domain.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DomainType {
    Article,
    Documentation,
    GitHub,
    Forum,
    ECommerce,
    Social,
    Generic,
}
```

**Detection order** (lines 19-25):
1. **URL-based detection first** (`detect_from_url`): Checks URL patterns for GitHub, docs sites (readme.io, readthedocs.io, gitbook.io), forums, social platforms, ecommerce
2. **DOM fallback** (`detect_from_dom`): If URL detection fails, examines DOM:
   - `<article>` tag or `og:type=article` → `Article`
   - Schema.org `Article` in JSON-LD → `Article`
   - Otherwise → `Generic`

**Domain types and their indicators:**

| DomainType | URL Patterns | DOM Signals |
|------------|-------------|-------------|
| `GitHub` | `github.com`, `github.io` | — |
| `Documentation` | `readme.io`, `readthedocs.io`, `gitbook.io`, `sphinx`, `jekyll` | — |
| `Forum` | `stackoverflow.com`, `discourse.com`, `phpbb` | — |
| `Social` | `twitter.com`, `x.com`, `facebook.com`, `linkedin.com`, `reddit.com`, `instagram.com` | — |
| `ECommerce` | `shopify.com`, `bigcommerce.com`, `woocommerce` | — |
| `Article` | — | `<article>` tag, `og:type=article`, Schema.org Article |
| `Generic` | (fallback) | Everything else |

The detected `domain_type` is stored in `DomainData` and used to potentially adjust extraction strategy in future versions.

## Structured Data Extraction (`structured_data.rs`)

The `structured_data` field (`Vec<serde_json::Value>`) in `ExtractionResult` captures data from JavaScript-heavy pages and CMS systems. Three extractors run in `lib.rs` (lines 207-210):

```rust
let mut structured_data = structured_data::extract_json_ld(html);
structured_data.extend(structured_data::extract_next_data(html));
structured_data.extend(structured_data::extract_sveltekit(html));
```

### JSON-LD Extraction (`extract_json_ld`, lines 14-70)

Extracts Schema.org structured data from `<script type="application/ld+json">` tags:

```rust
pub fn extract_json_ld(html: &str) -> Vec<Value> {
    // Finds all script[type="application/ld+json"] blocks
    // Parses and returns as serde_json::Value
    // Common types: Product, Article, BreadcrumbList, Organization, FAQPage, etc.
}
```

**Example JSON-LD types commonly extracted:**
- `Product` — e-commerce product info with price, availability
- `Article`, `NewsArticle`, `BlogPosting` — editorial content metadata
- `BreadcrumbList` — navigation hierarchy
- `Organization` — site branding info
- `FAQPage` — Q&A content
- `Recipe` — cooking instructions
- `Video`, `Audio` — media metadata

### Next.js Data Extraction (`extract_next_data`, lines 79-123)

Extracts data from React/Next.js Single Page Applications via `window.__NEXT_DATA__` embedded JSON:

```rust
pub fn extract_next_data(html: &str) -> Vec<Value> {
    // Finds <script id="__NEXT_DATA__" type="application/json">
    // Extracts the pageProps object (actual page content)
    // Falls back to entire object if pageProps is missing/empty
}
```

This handles pages built with Next.js where content is hydrated from server-side props embedded in the initial HTML.

### SvelteKit Data Extraction (`extract_sveltekit`, lines 131-173)

Extracts data from SvelteKit applications via `kit.start()` calls:

```rust
pub fn extract_sveltekit(html: &str) -> Vec<Value> {
    // Finds patterns like: kit.start(app, element, { data: [...] })
    // Converts JS object literals to valid JSON (unquoted keys -> quoted)
    // Unwraps {"type":"data","data":{...}} wrappers
}
```

### Contentful CMS Extraction (`data_island.rs`)

Handles Contentful CMS rich text format embedded in pages:

```rust
// Contentful rich text node pattern (lines 126-160)
if let Some(node_type) = map.get("nodeType").and_then(|v| v.as_str()) {
    if nodeType == "document" { ... }  // Rich text root
    if nodeType == "paragraph" { ... } // Text blocks
    if nodeType == "text" { ... }      // Text with marks (bold, italic)
}
```

Also extracts:
- **CMS entry patterns**: heading + description/title/body pairs
- **Quote/testimonial patterns**: `quote` / `quoteText` + `author`/`position`

## Data Island Fallback (`lib.rs` lines 179-185)

Structured data extraction includes a **recovery mechanism** that fires when normal DOM extraction yields sparse content:

```rust
// Only fires if DOM word_count < 500
if let Some(island_md) = data_island::try_extract(&doc, meta.word_count, &content.markdown) {
    content.markdown.push_str("\n\n");
    content.markdown.push_str(&island_md);
}
```

The `try_extract` function in `data_island.rs` attempts to recover content from:
- JSON embedded in `<script type="application/json">` blocks
- React/Next.js `__NEXT_DATA__` payloads
- JSON-LD structured data
- Contentful rich text nodes

## Complete Output Structure

```rust
// From types.rs (lines 7-16)
pub struct ExtractionResult {
    pub metadata: Metadata,
    pub content: Content,
    pub domain_data: Option<DomainData>,
    pub structured_data: Vec<serde_json::Value>,
}

pub struct DomainData {
    pub domain_type: DomainType,  // Article, Documentation, GitHub, Forum, ECommerce, Social, Generic
}

pub struct Metadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub published_date: Option<String>,
    pub language: Option<String>,
    pub url: Option<String>,
    pub site_name: Option<String>,
    pub image: Option<String>,
    pub favicon: Option<String>,
    pub word_count: usize,
}
```

## Full Extraction Pipeline (`lib.rs` lines 73-218)

The complete orchestration flow:

```
1. YouTube fast path (if YouTube URL detected)
       ↓
2. Metadata extraction from <head> (metadata.rs)
       ↓
3. Main content extraction (extractor.rs)
       ↓  ┌─ Relax only_main_content if <30 words
4. Retry strategies (if sparse) ── Retry with body selector if <200 words
       ↓
5. Data island fallback (if word_count < 500)
       ↓  └─ Extracts JSON from script[type="application/json"]
           └─ Extracts __NEXT_DATA__, JSON-LD, Contentful
       ↓
6. QuickJS runtime (if quickjs feature enabled)
       ↓
7. Domain detection (domain.rs)
       ↓
8. Structured data collection:
       ├─ extract_json_ld() → JSON-LD blocks
       ├─ extract_next_data() → Next.js pageProps
       └─ extract_sveltekit() → SvelteKit data arrays
       ↓
9. Return ExtractionResult { metadata, content, domain_data, structured_data }
```

### `html`
Sanitized HTML output with only main content elements.

**Characteristics:**
- Preserves semantic HTML structure
- Removes scripts, styles, iframes
- Converts to simplified HTML
- Wraps in `<main>` container

**Example output:**
```html
<main>
  <h1>Main Article Title</h1>
  <p>This is a <strong>bold</strong> and <em>italic</em> paragraph with a <a href="https://example.com">link</a>.</p>
  ...
</main>
```

## Content Filtering Options

### `--only-main-content`
Extracts only `article`, `main`, or `[role="main"]` elements. Useful for extracting just the primary content area.

**Behavior:** Only extracts elements matching `<article>`, `<main>`, or `[role="main"]`. All other content is excluded.

### `--include <selectors>`
CSS selectors to include. Exclusive mode — only matched elements are returned.

**Behavior:** When specified, only content matching the selectors is extracted. Other content is ignored.

### `--exclude <selectors>`
CSS selectors to exclude. Removes matched elements from normal extraction.

**Behavior:** Content matching the selectors is removed before extraction. Useful for filtering ads, sidebars, related posts, etc.

**Example:**
```bash
webclaw URL --exclude "nav, footer, .sidebar, .ads, .related-posts"
```

## Element Handling

### Headings
- `h1` through `h6` converted to `#` through `######` markdown
- Properly nested within document structure

### Links
- Inline format: `[text](url)`
- Relative URLs preserved as-is (no automatic resolution unless `--domain` specified — note: not available in local file mode)
- In `llm` format: deduplicated and collected at end
- Link text preserved
- JavaScript links (`javascript:`) and action links filtered out in `llm` format

### Images
- Markdown format: `![alt](src)`
- Lazy-loading attributes supported: `data-src`, `data-lazy-src`, `data-original`
- `srcset` attributes: uses default `src` value
- Alt text preserved
- In `llm` format: stripped from inline text

### Code Blocks
- Fenced code blocks with language identifier: ` ```language `
- Inline code: `` `code` ``
- Languages preserved when detected (e.g., ` ```rust `)

### Tables
- GFM pipe syntax: `| Header A | Header B |`
- Alignment row: `| --- | --- |`
- Cells separated by pipes

### Lists
- Unordered: `-` marker
- Ordered: `1.` numbering
- Nested lists properly indented (2 spaces)
- **Edge case observed**: Nested list items may have spacing issues

### Blockquotes
- Prefix `>` on each line
- Nested blockquotes not distinguished
- Preserve text content

### Emphasis
- **Bold**: `**text**`
- *Italic*: `*text*`
- ~~Strikethrough~~: `~~text~~` (from `<del>` or `<s>`)
- In `llm` format: stripped/converted to plain text

### Horizontal Rules
- Rendered as `---`
- Preserved in all formats

## What Gets Stripped

### Always Removed
- `<script>` tags and content
- `<style>` tags and content
- `<noscript>` content
- `<iframe>` elements
- HTML comments

### Noise Patterns Filtered
- Navigation elements (via noise filter)
- Footer content
- Sidebar content
- Ad elements
- Cookie banners
- Social share buttons
- Modals and popups

### Tailwind-Safe
webclaw recognizes Tailwind utility classes and doesn't filter based on them. Classes like `flex`, `p-4`, `bg-white`, `text-blue-600` are ignored by the noise filter.

## Edge Cases

### Nested Lists
**Issue observed**: Nested list rendering may have spacing problems.
```markdown
- Top level item 2
  - Nested item A
  - Nested item B- Top level item 3   # Missing space before bullet
```

### Relative URLs
Relative URLs are preserved as-is. Use `--domain` flag (not available in local file mode) to resolve them to absolute URLs.

### React/Next.js SPAs
Data island extraction catches `window.__NEXT_DATA__` JSON payloads and JSON-LD structured data. Falls back when DOM word count < 30.

### Lazy-Loaded Images
Supports `data-src`, `data-lazy-src`, and `data-original` attributes as fallback sources.

### YouTube URLs
Detects YouTube video URLs and extracts structured metadata (title, channel, views, duration, description, transcript).

### Empty/Minimal Content
Data island fallback triggers when DOM word count < 30, attempting to extract from JSON payloads.

## Key Thresholds

From the implementation:
- **Scoring minimum**: 50 characters text length
- **Semantic bonus**: +50 for `<article>`/`<main>`, +25 for content class/ID
- **Link density**: >50% = 0.1x score, >30% = 0.5x
- **Data island fallback**: triggers when DOM word count < 30
- **Eyebrow text max**: 80 characters

## Performance

From webclaw benchmarks (on 100KB page):
- **Extraction time**: ~3.2ms
- **Speed comparison**: 3x faster than trafilatura, 3x faster than readability

## Sample HTML to Markdown

### Input HTML
```html
<!DOCTYPE html>
<html>
<head><title>Test Page</title></head>
<body>
<nav>Navigation content</nav>
<main>
  <h1>Main Article Title</h1>
  <p>This is a <strong>bold</strong> and <em>italic</em> paragraph 
     with a <a href="https://example.com">link</a>.</p>
  <h2>Section Heading</h2>
  <p>Another paragraph with <code>inline code</code>.</p>
  <img src="/image.png" alt="Example image" />
  <blockquote>This is a blockquote with important info.</blockquote>
  <ul>
    <li>List item one</li>
    <li>List item two</li>
  </ul>
  <ol>
    <li>Numbered one</li>
    <li>Numbered two</li>
  </ol>
  <pre><code class="language-rust">fn main() { println!("Hello"); }</code></pre>
  <table>
    <tr><th>Header A</th><th>Header B</th></tr>
    <tr><td>Cell 1</td><td>Cell 2</td></tr>
  </table>
  <hr>
  <p>End of content.</p>
</main>
<footer>Footer content</footer>
</body>
</html>
```

### Expected Markdown Output
```markdown
# Main Article Title


This is a **bold** and *italic* paragraph with a [link](https://example.com).


*This is a bold and italic paragraph with a link.*

## Section Heading


Another paragraph with `inline code`.


  ![Example image](/image.png)


> This is a blockquote with important info.


- List item one
- List item two


1. Numbered one
2. Numbered two


```rust
fn main() { println!("Hello"); }
```


| Header A | Header B |
| --- | --- |
| Cell 1 | Cell 2 |


---


End of content.
```

### Notes on Output
- **Duplicate text**: The paragraph content appears twice — once with formatting and once as italicized plain text (body text without inline markup)
- **Image spacing**: Images have 2-space indent before the markdown
- **Extra blank lines**: Multiple consecutive blank lines appear between elements
- **Code block language**: Language identifier preserved in fenced code block

## Comparison with html-to-markdown

The binary at `/tmp/html2markdown` is **not webclaw**. It's `html-to-markdown` v2.5.1 by Johannes Kaufmann, which:
- Is a Go library/tool for HTML to Markdown conversion
- Does NOT include content extraction (assumes you have HTML)
- Does NOT have readability-style scoring
- Does NOT have noise filtering
- Uses a plugin-based architecture (Commonmark, GFM tables, etc.)

webclaw is fundamentally different — it's a complete web scraping solution with content detection, noise filtering, and LLM optimization.

## CLI Reference

```bash
# Basic extraction from file
webclaw --file input.html -f markdown

# Only main content
webclaw --file input.html --only-main-content

# Include specific elements
webclaw --file input.html --include "article,.content"

# Exclude specific elements
webclaw --file input.html --exclude "nav,footer,.sidebar"

# All format options
webclaw --file input.html -f markdown    # Default, clean markdown
webclaw --file input.html -f text         # Plain text
webclaw --file input.html -f llm          # LLM-optimized
webclaw --file input.html -f json         # Full JSON
webclaw --file input.html -f html         # Sanitized HTML

# With metadata
webclaw --file input.html --metadata -f markdown

# URL mode (fetches and extracts)
webclaw https://example.com -f llm

# Batch mode
webclaw url1 url2 url3 -f markdown

# Crawl site
webclaw https://example.com --crawl --depth 2 --max-pages 50
```

## Benchmark Integration

In this benchmark (`scores.rs`, line 882-937):
- `run_webclaw` writes HTML to temp file
- Builds CLI args from `WebclawConfig`
- Executes via `std::process::Command`
- Returns stdout on success, error message on failure

```rust
fn run_webclaw(html: &str, states: &ExtractorStates) -> String {
    let cfg = states.states.get("webclaw")
        .map(|s| s.config.webclaw.clone()).unwrap_or_default();
    let tmp = std::env::temp_dir()
        .join(format!("webclaw_{}.html", uuid::Uuid::new_v4()));
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
    // ... handle output
}
```

## Summary

webclaw is a production-grade web extraction tool optimized for LLM consumption:
- **95.1% extraction accuracy** (vs 83.5% readability, 80.6% trafilatura)
- **3.2ms extraction speed** for 100KB pages
- **67% token reduction** vs raw HTML (in `llm` mode)
- **5 output formats** with CSS selector filtering
- **No browser required** — pure HTTP with TLS fingerprinting
- **Self-hosted, AGPL-3.0 licensed**, free to use

For this benchmark, webclaw provides high-quality content extraction with excellent noise filtering and multiple output formats suitable for comparing against other HTML-to-text tools.
