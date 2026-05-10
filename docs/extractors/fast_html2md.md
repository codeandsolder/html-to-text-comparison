# fast_html2md - Deep Analysis

## Overview

**fast_html2md** is a Rust crate that converts HTML to Markdown. It is marketed as "the fastest Rust library for transforming HTML into Markdown" and is actively used in production at [Spider](https://spider.cloud).

| Property | Value |
|----------|-------|
| Crate Name | `fast_html2md` |
| Version in Benchmark | 0.0.62 |
| License | MIT |
| Repository | [spider-rs/html2md](https://github.com/spider-rs/html2md) |
| Downloads | 188,562+ all-time |
| Stars | 71 |

---

## How It Is Called in the Benchmark

The benchmark invokes `fast_html2md` via the `parse_html` function:

**Location:** `src/scores.rs` (line 288-290)

```rust
#[cfg(feature = "fast_html2md")]
"fast_html2md" => {
    runner.run(output_name, |html| fast_html2md::parse_html(html, false));
}
```

### Function Signature

```rust
pub fn parse_html(html: &str, commonmark: bool) -> String
```

- **First argument (`html`)**: The source HTML as a string
- **Second argument (`commonmark`)**: A boolean flag to adjust markdown output to CommonMark spec. In the benchmark, it's set to `false`, meaning it uses non-standard markdown output.

The crate is compiled with default features, which means the `rewriter` feature is enabled (using `lol_html` for parsing).

---

## Why Is It Called "fast"? Performance Characteristics

The "fast" designation comes from several architectural decisions:

### 1. Two Parsing Backends

The crate offers two parsing approaches via feature flags:

| Feature | Parser Used | Purpose |
|---------|-------------|---------|
| `rewriter` (default) | `lol_html` | High performance, streaming-capable HTML rewriting |
| `scraper` | `html5ever` + `scraper` | Alternative approach using the DOM tree model |

The benchmark uses `parse_html` which comes from the `scraper` feature, but the default `rewrite_html` uses the rewriter backend.

### 2. Performance Benchmarks

From the benchmark results:

| URL | Time (microseconds) | Peak Memory (bytes) | Memory as % of HTML | Output Size (bytes) | % Reduction |
|-----|-------------------|-------------------|-------------------|-------------------|------------|
| example.com (1.2KB) | 56 | 3,260 | 259.55% | 229 | 81.77% |
| mozilla/readability (351KB) | 3,749 | 8,707 | 2.50% | 16,111 | 95.37% |

**Key observations:**
- Very low memory footprint: 2.50% of HTML size on large documents
- Fast execution: 3,749 microseconds (3.7ms) for 351KB HTML
- Lower output reduction than readability extractors but preserves more content

### 3. Optimized String Processing

The library includes optimized functions for markdown character handling:
- `replace_markdown_chars()` - Bulk byte scanning
- `replace_markdown_chars_opt()` - Returns `None` to avoid allocation when no changes needed
- `contains_markdown_chars()` - Fast path to skip escaping when not needed

### 4. Streaming Support

The `stream` feature enables async streaming for large HTML documents:
```rust
rewrite_html_streaming(html, false).await
rewrite_html_stream(stream, false).await
```

---

## How It Converts HTML to Markdown

### Architecture

The conversion happens through a handler pattern:

1. **HTML Parsing**: Uses html5ever (when `scraper` feature is enabled) to build a DOM tree
2. **Tree Walking**: Recursively walks the DOM tree
3. **Handler Dispatch**: Each HTML tag type is handled by a specialized `TagHandler`
4. **Output Building**: A `StructuredPrinter` accumulates the markdown output

### Handler System

```rust
pub trait TagHandler {
    fn handle(&mut self, tag: &Handle, printer: &mut StructuredPrinter);
    fn after_handle(&mut self, printer: &mut StructuredPrinter);
    fn skip_descendants(&self) -> bool;
}
```

---

## Element Handling

### Headings (`<h1>` to `<h6>`)

**Output Format:** ATX-style headers with `#` prefix

```rust
match self.header_type.as_ref() {
    "h1" => printer.append_str("# "),
    "h2" => printer.append_str("## "),
    "h3" => printer.append_str("### "),
    "h4" => printer.append_str("#### "),
    "h5" => printer.append_str("##### "),
    "h6" => printer.append_str("###### "),
    _ => (),
}
```

**Example:**
```html
<h1>Title</h1>
<h2>Subtitle</h2>
```

**Output:**
```markdown
# Title

## Subtitle
```

**Notes:**
- No setext-style (underline) headings
- Adds newline after each heading

---

### Links (`<a>`)

**Output Format:** Standard Markdown link `[text](url)`

**Key features:**
1. **URL decoding**: Uses `percent_decode_str` to decode URLs
2. **Relative URL resolution**: Converts relative URLs to absolute using base URL if provided
3. **URL sanitization**: Wraps URLs containing spaces or control characters in `<>` brackets
4. **Position tracking**: Uses `start_pos` to insert brackets around existing text

**Example:**
```html
<a href="/docs">Documentation</a>
<a href="https://example.com/page">External</a>
```

**Output:**
```markdown
[Documentation](/docs)
[External](https://example.com/page)
```

---

### Images (`<img>`)

**Output Format:** Two modes depending on attributes:

1. **Markdown native** (default):
   ```markdown
   ![alt](url "title")
   ```

2. **Inline HTML** (when commonmark=true with geometry attributes):
   ```html
   <img alt="text" src="url" title="title" height="100" width="200" align="center" />
   ```

**Key features:**
- Extracts: `src`, `alt`, `title`, `height`, `width`, `align`
- URL encoding: Replaces spaces with `%20` for URLs with spaces
- Relative URL resolution for paths starting with `/`
- Block mode detection via `display: block` style

**Example:**
```html
<img src="photo.jpg" alt="A sunset" title="Beautiful sunset">
```

**Output:**
```markdown
![A sunset](photo.jpg "Beautiful sunset")
```

---

### Code Blocks (`<pre>`, `<code>`)

**Output Format:** Fenced code blocks with triple backticks

```rust
match self.code_type.as_ref() {
    "pre" => {
        printer.append_str("\n```\n");
        // content
        printer.append_str("\n```\n");
    }
    "code" => printer.append_str("`"),
    _ => (),
}
```

**Example:**
```html
<pre><code>function hello() {
    console.log("Hello");
}</code></pre>
```

**Output:**
```markdown
```

function hello() {
    console.log("Hello");
}
```
```

**Notes:**
- Adds newlines before and after code blocks
- Inline code uses single backticks
- Language specification is NOT included (no syntax highlighting)

---

### Tables (`<table>`, `<tr>`, `<td>`, `<th>`)

**Output Format:** Standard Markdown tables with alignment padding

**Key features:**
1. **Auto column width detection**: Calculates max content width per column
2. **Header row detection**: First row is treated as header
3. **Separator row**: Adds `|---|` separator after header
4. **Cell padding**: Centers text with spaces
5. **Limit**: Processes up to 1000 rows (TABLE_LIMIT)

**Example:**
```html
<table>
    <tr><th>Name</th><th>Age</th></tr>
    <tr><td>Alice</td><td>30</td></tr>
    <tr><td>Bob</td><td>25</td></tr>
</table>
```

**Output:**
```markdown
|  Name  | Age |
|--------|-----|
| Alice  | 30  |
| Bob    | 25  |
```

**Notes:**
- Alignment detection is commented out in source code
- Uses `walk()` internally to process cell content
- Calls `clean_markdown()` on cell content

---

### Lists (`<ul>`, `<ol>`, `<li>`, `<menu>`)

**Output Format:**
- Unordered: `* ` prefix
- Ordered: `1. `, `2. `, etc.

**Key features:**
1. **Nesting support**: Uses parent chain to detect nesting depth
2. **Siblings tracking**: Tracks sibling count for ordered list numbering
3. **Paragraph handling**: Indents subsequent paragraphs within list items with spaces
4. **Nested list preservation**: Extra newline for non-nested lists only

**Example:**
```html
<ul>
    <li>First item</li>
    <li>Second item</li>
</ul>
<ol>
    <li>First numbered</li>
    <li>Second numbered</li>
</ol>
```

**Output:**
```markdown
* First item
* Second item

1. First numbered
2. Second numbered
```

---

### Blockquotes (`<blockquote>`, `<q>`, `<cite>`)

**Output Format:** `> ` prefix on each line

**Algorithm:**
1. Insert `> ` after opening newline
2. Replace all internal newlines with `\n> `

**Example:**
```html
<blockquote>
    This is a quote.
    It spans multiple lines.
</blockquote>
```

**Output:**
```markdown
> This is a quote.
> It spans multiple lines.
```

---

### Emphasis (`<strong>`, `<b>`, `<em>`, `<i>`, `<del>`, `<s>`, `<u>`, `<ins>`)

**Output Format:**
| Tag | Markdown |
|-----|----------|
| `<strong>`, `<b>` | `**text**` |
| `<em>`, `<i>` | `*text*` |
| `<del>`, `<s>` | `~~text~~` |
| `<u>`, `<ins>` | `__text__` |

**Key features:**
1. **Whitespace trimming**: Only applies marks around non-whitespace content
2. **First/last non-space detection**: Finds actual content boundaries

**Example:**
```html
<strong>bold</strong> and <em>italic</em> and <del>strikethrough</del>
```

**Output:**
```markdown
**bold** and *italic* and ~~strikethrough~~
```

---

### Other Elements

| Element | Handling |
|---------|----------|
| `<div>`, `<section>`, `<header>`, `<footer>` | ContainerHandler - passes through content |
| `<p>`, `<br>`, `<hr>` | ParagraphHandler - adds newlines |
| `<details>`, `<summary>` | HtmlCherryPickHandler - preserves HTML |
| `<sub>`, `<sup>` | IdentityHandler - preserves as-is |
| `<iframe>` | IframeHandler - placeholder only |
| `<script>`, `<style>` | Completely ignored (skipped) |

---

## Configuration Options

The benchmark uses **no custom configuration** - all defaults are applied. However, the crate offers several configuration mechanisms:

### 1. CommonMark Mode

```rust
parse_html(html, true)  // CommonMark compliant output
parse_html(html, false) // Non-standard (default in benchmark)
```

### 2. Base URL for Relative Links

```rust
use url::Url;

let base_url = Url::parse("https://example.com").ok();
let html = parse_html_custom_with_url(html, &custom, false, &base_url);
```

### 3. Custom Tag Handlers

```rust
use std::collections::HashMap;
use std::boxed::Box;

// Define custom handler
struct MyHandler;
impl TagHandler for MyHandler {
    fn handle(&mut self, tag: &Handle, printer: &mut StructuredPrinter) { }
    fn after_handle(&mut self, printer: &mut StructuredPrinter) { }
}

struct MyHandlerFactory;
impl TagHandlerFactory for MyHandlerFactory {
    fn instantiate(&self) -> Box<dyn TagHandler> {
        Box::new(MyHandler)
    }
}

// Register custom handlers
let mut custom = HashMap::new();
custom.insert("custom-tag".to_string(), Box::new(MyHandlerFactory {}));

parse_html_custom(html, &custom, false);
```

### 4. Extended Mode (preserve span tags)

```rust
let md = parse_html_extended(html, false);
```

---

## Feature Flags

```toml
[dependencies]
fast_html2md = { version = "0.0.62", default-features = false, features = [
    "rewriter",     # Default: high performance using lol_html
    "scraper",      # Alternative: html5ever DOM parsing
    "stream",       # Async streaming support (requires rewriter)
] }
```

| Feature | Default | Description |
|---------|---------|-------------|
| `rewriter` | Yes | High-performance transformation using lol_html |
| `scraper` | No | Alternative DOM-based approach using html5ever |
| `stream` | No | Async streaming for large HTML documents |

---

## Edge Cases and Limitations

### 1. Missing Features

- **No language specification in code blocks**: ` ``` ` has no language tag
- **No task lists**: `<input type="checkbox">` not supported
- **No footnotes**: Reference-style links not supported
- **No definition lists**: `<dl>`, `<dt>`, `<dd>` not supported

### 2. Known Limitations

1. **Table alignment**: Alignment detection from `align` attribute is commented out
2. **Complex table nesting**: Tables inside tables not supported
3. **HTML in code blocks**: Pre-formatted text may not be fully preserved
4. **Entity decoding**: Only handles common entities (`&amp;`, `&lt;`, `&gt;`, etc.)

### 3. Whitespace Handling

- Multiple whitespaces collapsed to single space
- Leading/trailing whitespace trimmed within elements
- Excessive whitespace cleaned via `clean_markdown()`

### 4. Error Handling

```rust
match document_parser.from_utf8().read_from(&mut html.as_bytes()) {
    Ok(dom) => { /* process */ }
    _ => Default::default(),  // Returns empty string on parse failure
}
```

---

## Dependencies

```
auto_encoder ^0
futures-util ^0.3       (optional, stream feature)
spider-html5ever ^0.39  (optional, scraper feature)
lazy_static ^1
lol_html ^2             (optional, rewriter feature)
spider-markup5ever_rcdom ^0.39  (optional, scraper feature)
percent-encoding ^2
regex ^1
url ^2
```

---

## Sample Input/Output

### Sample Input HTML

```html
<!DOCTYPE html>
<html>
<head>
    <title>Sample Document</title>
</head>
<body>
    <h1>Main Title</h1>
    <p>This is a <strong>bold</strong> and <em>italic</em> paragraph.</p>
    
    <h2>Section: Links and Images</h2>
    <p>Visit <a href="https://example.com">Example</a> for more info.</p>
    <img src="photo.jpg" alt="A sample photo" title="Sample">
    
    <h2>Section: Code</h2>
    <pre><code>fn main() {
    println!("Hello, world!");
}</code></pre>
    
    <h2>Section: Lists</h2>
    <ul>
        <li>First item</li>
        <li>Second item</li>
    </ul>
    <ol>
        <li>Number one</li>
        <li>Number two</li>
    </ol>
    
    <h2>Section: Table</h2>
    <table>
        <tr><th>Name</th><th>Value</th></tr>
        <tr><td>A</td><td>100</td></tr>
        <tr><td>B</td><td>200</td></tr>
    </table>
    
    <blockquote>
        This is a blockquote.
        It has multiple lines.
    </blockquote>
    
    <p>Text with <del>strikethrough</del> and <u>underline</u>.</p>
</body>
</html>
```

### Expected Output

```markdown
# Main Title

This is a **bold** and *italic* paragraph.

## Section: Links and Images

Visit [Example](https://example.com) for more info.

![A sample photo](photo.jpg "Sample")

## Section: Code

```

fn main() {
    println!("Hello, world!");
}
```

## Section: Lists

* First item
* Second item

1. Number one
2. Number two

## Section: Table

| Name | Value |
|------|-------|
| A    | 100   |
| B    | 200   |

> This is a blockquote.
> It has multiple lines.

Text with ~~strikethrough~~ and __underline__.
```

---

## Comparison with Other Extractors

From the benchmark (example.com):

| Extractor | Time (us) | Memory (bytes) | Output Size | % Reduction |
|-----------|-----------|----------------|-------------|-------------|
| html2md-rs | 3 | 275 | 0 | 100.00% |
| html2text | 77 | 1,767 | 240 | 80.89% |
| htmd | 83 | 1,948 | 247 | 80.33% |
| **fast_html2md** | **56** | **3,260** | **229** | **81.77%** |
| mdka | 54 | 1,585 | 241 | 80.81% |

**Observations:**
- fast_html2md is mid-range in speed (56us)
- Memory usage is moderate (3,260 bytes)
- Output reduction is comparable to other converters
- Preserves more content than readability-style extractors

---

## Summary

`fast_html2md` is a focused, performance-oriented HTML-to-Markdown converter. It:

1. **Prioritizes speed** through optional lol_html-based rewriter backend
2. **Offers flexibility** with dual parsing approaches (rewriter vs scraper)
3. **Handles common elements** well: headings, links, images, code, tables, lists, quotes, emphasis
4. **Has minimal configuration** in the benchmark - uses defaults
5. **Suitable for** applications needing fast conversion with acceptable markdown quality

The lack of extensive configuration options in the benchmark suggests it works well out-of-the-box for general HTML-to-Markdown conversion use cases.
