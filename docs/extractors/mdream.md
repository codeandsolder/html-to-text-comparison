# Mdream Extractor: In-Depth Analysis

Mdream is a high-performance Rust crate that converts HTML to Markdown, designed specifically for LLM applications with a focus on token efficiency. It claims to be up to 37x faster than Turndown and produces up to 2x fewer tokens than competing solutions. This analysis examines how mdream works within the html-to-text-comparison benchmark, covering all configuration options, element handling, and edge cases.

## Table of Contents

1. [Overview and Architecture](#overview-and-architecture)
2. [Configuration in the Benchmark](#configuration-in-the-benchmark)
3. [Clean URLs Feature](#clean-urls-feature)
4. [Minimal Mode (Filter Plugin)](#minimal-mode-filter-plugin)
5. [Isolate Main Content Detection](#isolate-main-content-detection)
6. [Frontmatter Extraction](#frontmatter-extraction)
7. [Tailwind CSS Processing](#tailwind-css-processing)
8. [Plugin System Architecture](#plugin-system-architecture)
9. [Element Handling](#element-handling)
10. [Output Format](#output-format)
11. [Edge Cases](#edge-cases)
12. [Sample HTML Demonstrations](#sample-html-demonstrations)

## Overview and Architecture

Mdream is a zero-dependency HTML-to-Markdown converter written in Rust. The crate is available on crates.io and provides both a synchronous conversion function and a streaming processor for large documents. The core design philosophy centers on producing minimal, token-efficient Markdown output suitable for LLM consumption while maintaining high performance through native Rust implementation.

The architecture consists of a custom HTML parser that processes HTML into ElementNode and TextNode structures with parent-child relationships, followed by a conversion layer that transforms these nodes into Markdown. The plugin system allows post-processing transformations including content filtering, main content isolation, frontmatter extraction, and Tailwind CSS class conversion.

The primary API entry point is the `html_to_markdown` function, which accepts an HTML string and an `HTMLToMarkdownOptions` struct:

```rust
pub fn html_to_markdown(html: &str, options: HTMLToMarkdownOptions) -> String
```

The options struct contains four main fields:

```rust
pub struct HTMLToMarkdownOptions {
    pub origin: Option<String>,      // Base URL for resolving relative links
    pub clean_urls: bool,            // Enable URL cleaning (default: true in benchmark)
    pub clean: Option<CleanConfig>,  // Detailed cleanup options
    pub plugins: Option<PluginConfig>, // Plugin configuration
}
```

## Configuration in the Benchmark

In the html-to-text-comparison benchmark, mdream is configured through the `MdreamConfig` struct defined in `extractor_config.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MdreamConfig {
    pub minimal: bool,        // Enable minimal preset
    pub isolate_main: bool,   // Extract main content area
    pub frontmatter: bool,   // Extract metadata from <head>
    pub clean_urls: bool,     // Clean tracking parameters from URLs
    pub tailwind: bool,       // Convert Tailwind classes to Markdown
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
```

The benchmark sets `clean_urls: true` by default, which enables URL cleaning through the `CleanConfig` struct. When any of the plugin-based features (minimal, isolate_main, frontmatter, tailwind) are enabled, the benchmark constructs a `PluginConfig` that chains multiple plugins together.

The conversion code in `scores.rs` (lines 293-341) shows how these options are translated into mdream's API:

```rust
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
```

This code reveals that when `minimal` is enabled, the filter plugin automatically excludes four HTML elements: nav, footer, aside, and form. The other plugins are enabled individually based on their respective configuration flags.

## Clean URLs Feature

The URL cleaning functionality in mdream is controlled through the `CleanConfig` struct, which provides granular control over various URL cleanup operations. When `clean_urls` is set to true in the benchmark configuration, mdream enables the URL cleaning by setting `urls: true` in the CleanConfig.

The CleanConfig struct includes the following options:

```rust
pub struct CleanConfig {
    pub urls: bool,                  // Strip tracking query parameters
    pub fragments: bool,              // Strip invalid fragment-only links
    pub empty_links: bool,           // Convert meaningless links to text
    pub blank_lines: bool,           // Collapse excessive blank lines
    pub redundant_links: bool,       // Remove links where text equals URL
    pub self_link_headings: bool,    // Remove self-referencing heading anchors
    pub empty_images: bool,          // Remove images without alt text
    pub empty_link_text: bool,       // Remove links that produce no visible text
}
```

The URL cleaning specifically targets common tracking parameters that are commonly added to URLs for marketing purposes. These include parameters like utm_source, utm_medium, utm_campaign, utm_content, utm_term, fbclid, gclid, ref, source, and various other tracking identifiers. When a URL contains these parameters, they are stripped while preserving the rest of the URL path and query parameters that are meaningful.

For example, a URL like `https://example.com/page?utm_source=twitter&fbclid=abc123&id=42` would be cleaned to `https://example.com/page?id=42`. This significantly reduces token count when URLs appear in the Markdown output, which is particularly valuable for LLM applications where every token costs money and affects context window usage.

The benchmark enables this feature by default with `clean_urls: true`, which is a sensible default for LLM use cases. The redundant_links option is also valuable because it converts link-style URLs like `[https://example.com](https://example.com)` to plain URLs like `https://example.com`, eliminating unnecessary Markdown syntax when the link text matches the URL.

## Minimal Mode (Filter Plugin)

The minimal mode in mdream is implemented through the Filter plugin, which excludes specific HTML elements from the conversion process. When `minimal: true` is set in the benchmark configuration, mdream automatically applies the following exclusion rules:

```rust
FilterConfig {
    exclude: Some(vec![
        "nav".to_string(),
        "footer".to_string(),
        "aside".to_string(),
        "form".to_string(),
    ]),
    ..Default::default()
}
```

This filter configuration excludes four types of elements that typically contain navigation, footer, sidebar, and form content rather than main article content. The filter plugin supports more sophisticated selection through its configuration options:

```rust
pub struct FilterConfig {
    pub include: Option<Vec<String>>,      // Elements to include (all others excluded)
    pub exclude: Option<Vec<String>>,       // Elements to exclude
    pub process_children: Option<bool>,     // Whether to process children of matched elements
}
```

The include option allows whitelisting specific elements, which is useful when you only want to extract content from certain parts of the page. For example, `include: ["article", "main"]` would only process content within article and main elements.

The exclude option accepts CSS selectors in addition to tag names, so you can exclude specific elements by ID or class. For instance, `exclude: ["nav", "#sidebar", ".advertisement", "aside"]` would exclude the nav element, an element with id="sidebar", any elements with class="advertisement", and all aside elements.

The `process_children` option controls what happens when a matched element is found. When set to true (the default), the children of excluded elements are still processed. When set to false, excluding an element also excludes all its children from processing.

Additionally, when the filter plugin is active, mdream automatically excludes elements that have inline styles positioning them absolutely or in fixed positions. This handles cases where elements are positioned off-screen or as overlays:

```html
<div style="position: absolute; top: -9999px;">Hidden content</div>
<div style="position: fixed;">Fixed overlay</div>
```

These elements are automatically filtered out because they typically represent UI overlays, modals, or hidden content that is not part of the main article text.

In the benchmark context, the minimal mode provides a quick way to remove common non-content elements without having to manually configure each filter. It is particularly useful for blog posts, articles, and documentation pages where navigation and footer content should be excluded.

## Isolate Main Content Detection

The isolate_main feature attempts to identify and extract only the main content area of a web page, filtering out navigation, headers, sidebars, and other peripheral content. This is implemented through the `IsolateMainConfig` struct, which is an empty configuration struct that triggers the built-in main content detection algorithm.

```rust
pub struct IsolateMainConfig {}
```

The detection algorithm uses a priority-based approach to find the main content:

1. **Primary Method**: If an explicit `<main>` element exists within 5 depth levels from the document root, use its content exclusively. The algorithm checks up to 5 levels deep to handle cases where main is nested within other container elements.

2. **Fallback Method**: If no main element is found, the algorithm searches for content between the first header tag (h1 through h6) and the first `<footer>` element. This handles pages that use semantic HTML differently but still have a clear article structure with headings.

3. **Header Exclusion**: When using the fallback method, headings inside `<header>` elements are skipped during the detection process. This prevents the algorithm from picking up navigation-related headings that are not part of the main article content.

4. **Head Preservation**: The `<head>` section is always passed through to the conversion process, even when isolate_main is active. This ensures that frontmatter extraction still works correctly, as the metadata is typically located in the head section.

This algorithm is relatively simple compared to more sophisticated readability algorithms like Mozilla Readability, but it works well for pages that follow common semantic HTML patterns. For more complex pages, combining isolate_main with a readability library like @mozilla/readability before passing to mdream provides better results.

The isolate_main feature is particularly useful when processing full page HTML that includes navigation, headers, and footers, and you want to extract just the article or main content portion. It reduces noise in the output and produces cleaner Markdown for LLM consumption.

## Frontmatter Extraction

The frontmatter plugin extracts metadata from the HTML `<head>` section and generates YAML frontmatter at the beginning of the Markdown output. This is particularly valuable for LLM applications that can use the metadata for context, classification, or routing decisions.

```rust
pub struct FrontmatterConfig {
    pub additional_fields: Option<Vec<(String, String)>>,  // Static fields to add
    pub meta_fields: Option<Vec<String>>,                // Additional meta tag names to extract
}
```

By default, the frontmatter plugin extracts the following fields from meta tags andog tags:

- title
- description
- keywords
- author
- date
- og:title
- og:description
- twitter:title
- twitter:description

The plugin searches for these fields in both standard meta tags and Open Graph / Twitter Card meta tags. For example:

```html
<head>
    <title>My Article Title</title>
    <meta name="description" content="A brief description of the article">
    <meta name="author" content="John Doe">
    <meta name="date" content="2024-01-15">
    <meta property="og:title" content="My Article Title">
    <meta property="og:description" content="Open Graph description">
    <meta name="twitter:title" content="Twitter title">
</head>
```

When frontmatter extraction is enabled, this HTML would produce:

```yaml
---
title: My Article Title
description: A brief description of the article
author: John Doe
date: 2024-01-15
og:title: My Article Title
og:description: Open Graph description
twitter:title: Twitter title
---

# (rest of converted content follows)
```

The additional_fields option allows adding static metadata that is not extracted from the HTML. This is useful for adding source information, processing timestamps, or other application-specific metadata:

```rust
FrontmatterConfig {
    additional_fields: Some(vec![
        ("source".to_string(), "https://example.com".to_string()),
        ("processed_at".to_string(), "2024-01-15".to_string()),
    ]),
    ..Default::default()
}
```

The meta_fields option allows specifying additional meta tag names to extract beyond the defaults. For example, to also extract the robots meta tag and viewport meta tag:

```rust
FrontmatterConfig {
    meta_fields: Some(vec!["robots".to_string(), "viewport".to_string()]),
    ..Default::default()
}
```

The frontmatter feature is especially valuable for LLM applications that need to understand the context of the content before processing the main body. The title, description, and author fields provide quick context, while og: and twitter: fields can be useful for social media preview information.

## Tailwind CSS Processing

The Tailwind plugin converts Tailwind CSS utility classes to semantic Markdown formatting. This is particularly useful for sites built with Tailwind CSS that use utility classes for styling rather than semantic HTML elements.

```rust
pub struct TailwindConfig {}
```

The plugin recognizes the following Tailwind classes and converts them to corresponding Markdown:

| Tailwind Class | Markdown Output |
|----------------|-----------------|
| font-bold | **bold** |
| font-semibold | **bold** |
| font-medium | **bold** |
| font-extrabold | **bold** |
| font-black | **bold** |
| italic | *italic* |
| font-italic | *italic* |
| line-through | ~~strikethrough~~ |
| hidden | Content removed |
| invisible | Content removed |
| absolute | Content removed |
| fixed | Content removed |
| sticky | Content removed |

The plugin supports responsive breakpoint prefixes (sm:, md:, lg:, xl:, 2xl:) with mobile-first resolution. This means that if an element has both a default class and a responsive variant, the plugin will consider the breakpoints and apply the appropriate formatting.

For example, an element with `class="text-lg sm:text-xl md:text-2xl"` would have the styling based on the current viewport, but since mdream does not have viewport information, it typically applies the base class or the most specific class found.

The font-weight classes are particularly useful because they indicate semantic emphasis in Tailwind-styled content. A paragraph with `class="font-bold"` is semantically similar to strong text in HTML, so converting it to **bold** in Markdown preserves the semantic meaning.

The removal of hidden, invisible, absolute, fixed, and sticky elements complements the filter plugin's behavior. Elements with these classes are typically used for UI overlays, modal dialogs, or off-screen content that is not part of the main content flow.

The Tailwind plugin is less sophisticated than a full CSS parser but provides a useful heuristic for content that uses heavy Tailwind styling. It works best for content where developers have used semantic-appropriate Tailwind classes rather than purely presentational ones.

## Plugin System Architecture

The mdream plugin system allows chaining multiple transformations together through the `PluginConfig` struct:

```rust
pub struct PluginConfig {
    pub filter: Option<FilterConfig>,           // Element filtering
    pub isolate_main: Option<IsolateMainConfig>, // Main content isolation
    pub frontmatter: Option<FrontmatterConfig>, // Metadata extraction
    pub tailwind: Option<TailwindConfig>,       // Tailwind class conversion
    pub extraction: Option<ExtractionConfig>,   // Custom element extraction
    pub tag_overrides: Option<Vec<(String, TagOverrideConfig)>>, // Tag rendering override
}
```

The plugins are applied in a specific order that ensures correct behavior:

1. **Filter** runs first, removing elements before conversion. This means excluded elements do not appear in the output at all.

2. **IsolateMain** runs after filtering, identifying the main content region from the filtered HTML. This operates on the DOM after filter exclusions have been applied.

3. **Tailwind** runs during conversion, transforming Tailwind classes to Markdown as elements are processed. This requires access to element class attributes.

4. **Frontmatter** runs last, extracting metadata from the head section. The head section is preserved even when isolate_main is active, so frontmatter extraction works correctly.

The extraction plugin provides a callback-based mechanism for extracting specific elements during conversion. This is useful for analytics, logging, or collecting specific content elements:

```rust
pub struct ExtractionConfig {
    // Map of CSS selector to callback function
}
```

The extraction callbacks receive the matched element with its accumulated text content and attributes:

```rust
interface ExtractedElement {
    selector: string      // The CSS selector that matched
    tagName: string       // HTML tag name
    textContent: string   // Accumulated text content
    attributes: Record<string, string> // Element attributes
}
```

The tag_overrides plugin allows customizing how specific HTML tags are rendered:

```rust
pub struct TagOverrideConfig {
    pub enter: Option<String>,   // String to insert when entering tag
    pub exit: Option<String>,    // String to insert when exiting tag
    pub spacing: Option<Vec<usize>>, // [newlines_before, newlines_after]
    pub is_inline: Option<bool>,     // Treat as inline element
    pub is_self_closing: Option<bool>, // Element is self-closing
    pub collapses_inner_whitespace: Option<bool>, // Collapse whitespace inside
    pub alias: Option<String>,   // Alias to another tag's handler
}
```

This allows complete customization of tag rendering behavior. For example, you could make custom elements render as blockquotes, callouts, or special LLM-friendly formatting.

## Element Handling

This section details how mdream handles specific HTML elements and produces their Markdown equivalents.

### Headings

Mdream converts h1 through h6 elements to ATX-style Markdown headings with # through ###### prefixes. The heading level is preserved exactly, so an h2 becomes ## heading, an h3 becomes ### heading, and so on.

When the clean option `self_link_headings` is enabled, self-referencing heading anchors are removed. For example:

```html
<h2 id="intro">Introduction</h2>
```

Becomes:

```markdown
## Introduction
```

Instead of:

```markdown
## [Introduction](#intro)
```

The heading text content is preserved exactly, including any inline formatting that may be present within the heading element.

### Links

Links are converted to standard Markdown link syntax: `[link text](url)`. Relative URLs are resolved against the origin if provided in the options.

When `clean.urls` is enabled, tracking parameters are stripped from URLs. The `clean.empty_links` option converts meaningless links (like `#`, `javascript:void(0)`, or empty hrefs) to plain text instead of link syntax.

The `clean.redundant_links` option handles a common pattern where the link text is identical to the URL:

```markdown
[https://example.com](https://example.com)
```

Becomes:

```markdown
https://example.com
```

This significantly reduces token count when URLs are used as link text, which is common in content management systems and generated content.

The `clean.empty_link_text` option removes links that would produce no visible text, such as `[](url)` or links containing only whitespace or images without alt text.

### Images

Images are converted to Markdown image syntax: `![alt text](image-url)`. The alt attribute is preserved if present.

When `clean.empty_images` is enabled, images without alt text are removed entirely. This is useful for filtering out tracking pixels and decorative images that add no semantic value:

```html
<img src="tracking.gif">
```

This would be completely removed from the output rather than producing an empty image tag.

Images with relative URLs are resolved against the origin if provided. This ensures that image references in the Markdown work correctly when the Markdown is served from a different location than the original HTML.

### Code Blocks

Mdream supports both fenced and unfenced code blocks. For code blocks with language specification:

```html
<pre><code class="language-javascript">
function hello() {
  console.log("Hello");
}
</code></pre>
```

This produces:

````markdown
```javascript
function hello() {
  console.log("Hello");
}
```
````

Inline code is wrapped in backticks: `<code>inline code</code>` becomes `` `inline code` ``.

The crate handles the pre and code elements appropriately, preserving whitespace and line breaks within code blocks. Language detection relies on the class attribute containing language- prefixes, which is the standard convention.

### Tables

HTML tables are converted to Markdown table syntax. The implementation supports standard table structures with thead, tbody, tr, th, and td elements.

```html
<table>
    <thead>
        <tr>
            <th>Header 1</th>
            <th>Header 2</th>
        </tr>
    </thead>
    <tbody>
        <tr>
            <td>Cell 1</td>
            <td>Cell 2</td>
        </tr>
    </tbody>
</table>
```

Becomes:

```markdown
| Header 1 | Header 2 |
|----------|----------|
| Cell 1   | Cell 2   |
```

The table column alignment is determined by the presence of align attributes, though mdream is relatively simple in this regard and may not preserve all alignment information.

### Lists

Ordered and unordered lists are supported. Unordered lists use hyphen (-) by default in the output. Ordered lists use numeric prefixes with periods:

```html
<ul>
    <li>Item 1</li>
    <li>Item 2</li>
</ul>
<ol>
    <li>First</li>
    <li>Second</li>
</ol>
```

Becomes:

```markdown
- Item 1
- Item 2

1. First
2. Second
```

Nested lists are supported and properly indented. List items can contain paragraphs, and mdream handles the spacing appropriately between list content and following paragraphs.

### Blockquotes

Blockquote elements produce Markdown blockquotes with > prefixes:

```html
<blockquote>
    <p>This is a quote.</p>
</blockquote>
```

Becomes:

```markdown
> This is a quote.
```

Nested blockquotes are supported, with additional > characters for each level of nesting. Blockquotes can contain other block elements like headings, lists, and code blocks.

### Emphasis

Inline formatting is handled through the standard HTML elements:

- `<strong>` and `<b>` produce **bold** text
- `<em>` and `<i>` produce *italic* text
- `<del>` and `<s>` produce ~~strikethrough~~ text

When the Tailwind plugin is enabled, corresponding Tailwind classes also produce these formatting effects as described in the Tailwind section.

The emphasis elements can be nested, so `**<em>bold and italic</em>**` produces the expected nested formatting.

## Output Format

Mdream produces GitHub Flavored Markdown (GFM) compatible output. The default output format is clean Markdown without additional wrapper elements.

The output follows these general characteristics:

1. **Line endings**: Unix-style line endings (LF) are used consistently.

2. **Whitespace**: Multiple consecutive blank lines are collapsed to a maximum of two. When `clean.blank_lines` is enabled, sequences of three or more blank lines become two blank lines.

3. **Fenced code blocks**: Code blocks use triple backticks with optional language identifier.

4. **Link format**: Links use standard Markdown link syntax with the URL in parentheses.

5. **Image format**: Images use standard Markdown image syntax with alt text and URL.

6. **Frontmatter**: When frontmatter extraction is enabled, YAML frontmatter appears at the very beginning of the output, separated by --- delimiters.

7. **Heading format**: ATX-style headings (# through ######) are used without closing # characters.

The output is optimized for token efficiency, which means some Markdown features that add tokens without semantic value are omitted or simplified. This design choice directly supports the LLM use case that mdream targets.

## Edge Cases

This section describes how mdream handles various edge cases and potentially problematic HTML inputs.

### Malformed HTML

Mdream uses a custom HTML parser that is designed to be lenient with malformed HTML. The parser will attempt to produce reasonable output even when the HTML is not well-formed. However, extremely malformed HTML may produce unexpected results. Testing with representative samples is recommended when working with HTML from diverse sources.

### Script and Style Content

Script and style elements are not included in the Markdown output by default. The parser processes these elements but their content does not appear in the output. This is appropriate since script content is typically JavaScript code and style content is CSS, neither of which is relevant for Markdown text content.

### Nested Elements

Deeply nested elements are handled correctly, with the parser maintaining proper parent-child relationships. However, extremely deep nesting (hundreds of levels) may cause performance issues or stack overflow in some cases.

### Empty Elements

Empty elements like `<br>`, `<hr>`, and `<img>` are handled appropriately. `<br>` produces a line break, `<hr>` produces a horizontal rule `---`, and empty `<img>` tags may be removed if `clean.empty_images` is enabled.

### Unicode and Special Characters

Unicode characters are preserved in the output. Special characters that have meaning in Markdown (like *, #, [, ], (, ), etc.) are properly escaped when they appear in contexts where they would be interpreted as Markdown syntax.

### Very Long Documents

Mdream has a streaming API (`streamHtmlToMarkdown`) that can handle very large HTML documents without loading the entire document into memory. This is useful for processing large pages or HTML files that are several megabytes in size.

### Relative URLs Without Origin

When no origin is specified and relative URLs are encountered, they are preserved as-is in the output. This may result in broken links in the Markdown if the Markdown is moved to a different location, but it does not cause conversion errors.

### Forms and Interactive Elements

Form elements (input, textarea, select, button) are filtered out when minimal mode is enabled. In default mode, these elements may appear in the output as plain text or be handled according to their specific element type. Form elements are generally not meaningful in Markdown output since Markdown cannot represent form inputs.

### SVG and Canvas

SVG elements are not converted to Markdown. Their content is skipped, though text content within SVG elements may be extracted if it appears as text nodes. Canvas elements have no HTML content to convert and are effectively skipped.

## Sample HTML Demonstrations

The following examples demonstrate the effect of each configuration option on the Markdown output.

### Clean URLs Example

Input HTML:

```html
<html>
<body>
<p>Visit <a href="https://example.com/page?utm_source=newsletter&fbclid=abc123">this link</a> for more info.</p>
</body>
</html>
```

With `clean_urls: true` (default):

```markdown
Visit [this link](https://example.com/page) for more info.
```

The tracking parameters `utm_source` and `fbclid` are removed from the URL.

### Minimal Mode (Filter) Example

Input HTML:

```html
<html>
<body>
<nav>
    <ul>
        <li><a href="/">Home</a></li>
        <li><a href="/about">About</a></li>
    </ul>
</nav>
<main>
    <article>
        <h1>Main Article</h1>
        <p>This is the main content of the page.</p>
    </article>
</main>
<aside>
    <p>Sidebar content</p>
</aside>
<footer>
    <p>Footer information</p>
</footer>
</body>
</html>
```

With `minimal: true`:

```markdown
# Main Article

This is the main content of the page.
```

The nav, aside, and footer elements are completely removed from the output. Only the main article content remains.

### Isolate Main Example

Input HTML:

```html
<html>
<body>
<header>
    <h1>Site Title</h1>
    <nav>Navigation content</nav>
</header>
<main>
    <article>
        <h1>Article Title</h1>
        <p>Article content goes here.</p>
    </article>
</main>
<footer>
    <p>Copyright info</p>
</footer>
</body>
</html>
```

With `isolate_main: true`:

```markdown
# Article Title

Article content goes here.
```

The header and footer are excluded, and only the content within the main element is preserved.

### Frontmatter Example

Input HTML:

```html
<html>
<head>
    <title>My Article</title>
    <meta name="description" content="A great article about something">
    <meta name="author" content="Jane Smith">
    <meta property="og:title" content="Social Title">
</head>
<body>
    <p>Article content...</p>
</body>
</html>
```

With `frontmatter: true`:

```yaml
---
title: My Article
description: A great article about something
author: Jane Smith
og:title: Social Title
---

Article content...
```

The metadata from the head section appears as YAML frontmatter at the top of the output.

### Tailwind Example

Input HTML:

```html
<html>
<body>
<p class="font-bold">This text is bold.</p>
<p class="font-medium">This is medium weight.</p>
<p class="italic">This text is italic.</p>
<p class="line-through">This text is crossed out.</p>
<div class="hidden">This is hidden</div>
</body>
</html>
```

With `tailwind: true`:

```markdown
**This text is bold.**

**This is medium weight.**

*This text is italic.*

~~This text is crossed out.~~
```

The Tailwind classes are converted to Markdown formatting. The hidden element is completely removed.

### Combined Options Example

Input HTML:

```html
<html>
<head>
    <title>Blog Post</title>
    <meta name="description" content="An interesting blog post">
</head>
<body>
    <nav>
        <a href="/">Home</a>
        <a href="/about">About</a>
    </nav>
    <header>
        <h1>My Blog</h1>
    </header>
    <main>
        <article>
            <h1>Understanding Rust</h1>
            <p class="font-bold">Rust is a systems programming language.</p>
            <p class="italic">It focuses on safety and performance.</p>
        </article>
    </main>
    <footer>
        <p>Copyright 2024</p>
    </footer>
</body>
</html>
```

With `minimal: true`, `frontmatter: true`, `tailwind: true`, and `clean_urls: true`:

```yaml
---
title: Blog Post
description: An interesting blog post
---

# Understanding Rust

**Rust is a systems programming language.**

*It focuses on safety and performance.*
```

All options work together: frontmatter is extracted, navigation and footer are filtered out, Tailwind classes are converted to Markdown, and URLs would be cleaned if present.

## Conclusion

Mdream is a well-designed HTML-to-Markdown converter that prioritizes token efficiency and performance, making it particularly suitable for LLM applications. The benchmark configuration enables URL cleaning by default, which is a sensible choice for reducing token count by removing tracking parameters.

The configuration options provide flexibility for different use cases. The minimal preset offers a quick way to get clean output from full web pages, while individual options allow fine-grained control. The plugin system is extensible, with support for custom tag overrides and extraction callbacks.

The element handling covers all common HTML elements appropriately, producing clean GitHub Flavored Markdown output. The edge case handling is reasonable, with special attention to empty elements, tracking parameters, and content that should be excluded.

For LLM applications, mdream's focus on token efficiency directly addresses cost concerns. The combination of URL cleaning, redundant link removal, and minimal output format produces fewer tokens than many competing converters while maintaining readability and semantic accuracy.
