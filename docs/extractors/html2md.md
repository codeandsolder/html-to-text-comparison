# html2md Extractor Analysis

## Overview

The `html2md` extractor is a Rust crate that converts HTML documents to Markdown format. It is used in this benchmark via the simple function call `html2md::parse_html(html)`.

### Basic Information

| Attribute | Value |
|-----------|-------|
| **Crate Name** | html2md |
| **Version** | 0.2.15 |
| **Repository** | https://gitlab.com/Kanedias/html2md (GitHub mirror: https://github.com/Kaned1as/html2md) |
| **License** | GPL-3.0+ |
| **Author** | Oleg "Kanedias" Chernovskiy |
| **Downloads** | 575,093 total (136,424 in last 90 days) |
| **Reverse Dependencies** | 75 crates |

### Dependencies

The crate relies on these key libraries:

- **html5ever** (^0.27.0): Servo engine HTML parsing library - converts HTML input to DOM
- **markup5ever_rcdom** (^0.3.0): Provides the RcDom (reference-counted DOM) data structure
- **regex** (^1.4.2): PCRE support for whitespace cleanup and markdown escaping
- **percent-encoding** (^2.1.0): URL decoding for hyperlink href attributes
- **lazy_static** (^1.4.0): Static regex initialization

---

## Architecture

### Handler-Based Design

The conversion uses a **handler-based architecture** built around two core traits:

```rust
pub trait TagHandlerFactory {
    fn instantiate(&self) -> Box<dyn TagHandler>;
}

pub trait TagHandler {
    fn handle(&mut self, tag: &Handle, printer: &mut StructuredPrinter);
    fn after_handle(&mut self, printer: &mut StructuredPrinter);
    fn skip_descendants(&self) -> bool { false }
}
```

Each HTML element type has a dedicated handler that:
1. **`handle()`**: Called before processing children - emits opening Markdown syntax
2. **`after_handle()`**: Called after all children processed - emits closing syntax

### StructuredPrinter Context

The `StructuredPrinter` struct maintains conversion state:

```rust
pub struct StructuredPrinter {
    /// Chain of parents leading to upmost <html> tag
    pub parent_chain: Vec<String>,
    /// Siblings of currently processed tag in order
    pub siblings: HashMap<usize, Vec<String>>,
    /// Resulting markdown document
    pub data: String,
}
```

This context enables:
- Detection of parent elements (e.g., inside `<pre>`, inside `<code>`)
- List nesting level tracking
- Proper newline handling between siblings

### Entry Points

```rust
// Main conversion function - used in benchmark
pub fn parse_html(html: &str) -> String

// Custom variant - allows passing custom tag handlers
pub fn parse_html_custom(html: &str, custom: &HashMap<String, Box<dyn TagHandlerFactory>>) -> String

// Extended variant - preserves <span> elements intact
pub fn parse_html_extended(html: &str) -> String
```

The benchmark uses `parse_html(html)` with no configuration options.

---

## HTML Element Conversion Details

### Headings

**Handler**: `HeaderHandler` in `src/headers.rs`

The crate implements two heading styles:

| HTML | Markdown Output | Style |
|------|----------------|-------|
| `<h1>` | `Title\n==========\n` | Setext underline |
| `<h2>` | `Subtitle\n----------\n` | Setext underline |
| `<h3>` | `### Title ###` | ATX with closing |
| `<h4>` | `#### Title ####` | ATX with closing |
| `<h5>` | `##### Title #####` | ATX with closing |
| `<h6>` | `###### Title ######` | ATX with closing |

**Implementation notes**:
- Double newline inserted before headings
- H1/H2 use Setext-style (equals/dash underlines)
- H3-H6 use ATX-style with optional closing `###`
- Headers are not nested with other content

### Links (Anchors)

**Handler**: `AnchorHandler` in `src/anchors.rs`

Output format: `[link text](url)`

**Processing**:
1. Extract `href` attribute, percent-decode UTF-8
2. If URL contains whitespace, wrap in angle brackets: `<url>`
3. Wrap link text in `[...]` and append `(url)`
4. Named anchors (`<a name="...">`) emit HTML as-is via `IdentityHandler`

**Code snippet**:
```rust
let link = percent_decode_str(link).decode_utf8().unwrap_or_default();
if link.contains(|c: char| c.is_ascii_whitespace()) {
    format!("<{}>", link)
} else {
    link.to_string()
}
// Output: [text](url) or [text](<url with spaces>)
```

### Images

**Handler**: `ImgHandler` in `src/images.rs`

Output format: `![alt](url "title")`

**Processing**:
1. Extract `src`, `alt`, `title` attributes
2. URL-encode spaces using `utf8_percent_encode`
3. If element has `width`, `height`, or `align` attributes: emit as inline HTML
4. Otherwise: standard Markdown image syntax
5. Block-level images (detected via `display: block` in src) get extra newlines

**Code snippet**:
```rust
if height.is_some() || width.is_some() || align.is_some() {
    // emit inline HTML to preserve attributes
} else {
    format!("![{}]({}{})", alt, url, title)
}
```

### Code Blocks and Inline Code

**Handler**: `CodeHandler` in `src/codes.rs`

**Code blocks** (`<pre>`):
```
```rust
code content
```
```

- Language extracted from `<code class="language-rust">`
- Fence with triple backticks
- Extra newlines for block separation

**Inline code** (`<code>`, `<samp>`):
- Wrapped in single backticks: `` `code` ``

**Implementation notes**:
- Language detection via `class` attribute prefix `language-`
- If no language class, empty fence (just `````)
- Preformatted text inside `<pre>` skips markdown escaping

### Tables

**Handler**: `TableHandler` in `src/tables.rs`

Full Markdown table support with:

1. **Header row detection**: First `<tr>` is header
2. **Column width calculation**: Auto-detected from content
3. **Alignment detection**: From `<th align="...">` attribute
   - `left` -> `:--`
   - `center` -> `:--:`
   - `right` -> `--:`
   - default -> `---`
4. **Cell padding**: Center-aligned text within column width

**Example**:
```html
<table>
<tr><th>Name</th><th>Age</th></tr>
<tr><td>Alice</td><td>30</td></tr>
</table>
```

```markdown
|  Name  | Age |
|:-------|----:|
| Alice  |  30 |
```

**Implementation note**: Uses `skip_descendants=true` to process entire table internally rather than walking children normally.

### Lists

**Handler**: `ListHandler` and `ListItemHandler` in `src/lists.rs`

**Unordered lists**:
- Marker: `* ` (asterisk-space)
- Supports nested lists

**Ordered lists**:
- Marker: `1. `, `2. `, etc. (number-dot-space)
- Auto-incrementing numbers

**Implementation**:
```rust
match self.list_type.as_ref() {
    "ul" | "menu" => printer.append_str("* "),
    "ol" => printer.append_str(&(order.to_string() + ". ")),
    _ => {}
}
```

**List item processing**:
- Subsequent paragraphs in list items indented with spaces (1-2 spaces)
- Leading whitespace trimmed from list item content
- Nested list detection via parent chain

### Blockquotes

**Handler**: `QuoteHandler` in `src/quotes.rs`

Output: Lines prefixed with `> `

Processing:
1. Insert `> ` at start of quote
2. Replace all newlines within quote with `> ` prefix
3. Extra newlines after quote block

**Example**:
```html
<blockquote>
<p>Quote text</p>
<p>More text</p>
</blockquote>
```

```markdown
> Quote text
> More text
```

### Emphasis (Styles)

**Handler**: `StyleHandler` in `src/styles.rs`

| HTML Tag | Markdown | Notes |
|----------|----------|-------|
| `<b>`, `<strong>` | `**text**` | Bold |
| `<i>`, `<em>` | `*text*` | Italic |
| `<s>`, `<del>` | `~~text~~` | Strikethrough |
| `<u>`, `<ins>` | `__text__` | Underline |

**Implementation details**:
- Wraps non-whitespace text with markers
- Finds first/last non-space character positions
- Only applies if actual text content exists

### Paragraphs and Breaks

**Handler**: `ParagraphHandler` in `src/paragraphs.rs`

| HTML | Markdown |
|------|----------|
| `<p>` | Double newline (paragraph break) |
| `<br>` | Two spaces + newline `  \n` |
| `<hr>` | `---` |

### Container Elements

**Handler**: `ContainerHandler`

Elements like `<div>`, `<section>`, `<header>`, `<footer>` pass through transparently (no Markdown output, just process children).

### Special Elements

| Element | Handling |
|---------|----------|
| `<script>` | Ignored (no handler) |
| `<style>` | Ignored |
| `<details>`, `<summary>` | HTML as-is (via `HtmlCherryPickHandler`) |
| `<iframe>` | HTML as-is (via `IframeHandler`) |
| Comments | Ignored |
| `<sub>`, `<sup>` | HTML as-is (via `IdentityHandler`) |

---

## Post-Processing (Markdown Cleanup)

The `clean_markdown()` function applies regex-based cleanup:

```rust
// Remove empty lines with trailing spaces
EMPTY_LINE_PATTERN: Regex = Regex::new("(?m)^ +$")

// Collapse excessive newlines (>3 becomes 2)
EXCESSIVE_NEWLINE_PATTERN: Regex = Regex::new("\\n{3,}")

// Trim trailing single spaces
TRAILING_SPACE_PATTERN: Regex = Regex::new("(?m)(\\S) $")

// Trim leading newlines
LEADING_NEWLINES_PATTERN: Regex = Regex::new("^\\n+")

// Trim trailing whitespace
LAST_WHITESPACE_PATTERN: Regex::new("\\s+$")
```

### Markdown Escaping

The `escape_markdown()` function escapes special characters:

1. **Always escapes**: `<`, `>`, `*`, `_`, `~`, `\`
   - Pattern: `MARKDOWN_MIDDLE_KEYCHARS = Regex::new(r"[<>*\_~]")`
   - Replacement: `\$0` (backslash escape)

2. **At line start**: Escapes list markers and headers
   - Pattern: `MARKDOWN_STARTONLY_KEYCHARS = Regex::new(r"(^|\n) *$")`
   - Replaces `*`, `-`, `+`, `>`, `=` at line start with escaped versions

This prevents Markdown interpretation of HTML text content.

---

## Configuration Options

**The benchmark does not use any configuration** - it calls the simple `parse_html()` function with default behavior.

However, the crate does offer extensibility:

### Custom Tag Handlers

```rust
pub fn parse_html_custom(
    html: &str,
    custom: &HashMap<String, Box<dyn TagHandlerFactory>>
) -> String
```

Users can provide custom handlers for specific tags:

```rust
struct MyHandler;
impl TagHandlerFactory for MyHandler {
    fn instantiate(&self) -> Box<dyn TagHandler> {
        Box::new(MyTagHandler)
    }
}
```

### Extended Output

```rust
pub fn parse_html_extended(html: &str) -> String
```

Preserves `<span>` elements intact (normally stripped by Markdown parsers).

### No Built-in Configuration

There is no API to configure:
- Heading style (ATX vs Setext)
- List markers (`*` vs `-` vs `+`)
- Code fence style (backticks vs tildes)
- Link rendering (inline vs reference)
- Table formatting options

The crate implements a single, opinionated conversion style.

---

## Output Style Summary

| Aspect | Style |
|--------|-------|
| **Headings** | h1/h2: Setext (`===`/`---`), h3-h6: ATX with closing |
| **Lists** | Unordered: `*`, Ordered: `1.`, `2.` |
| **Code blocks** | Triple backticks with language |
| **Code inline** | Single backticks |
| **Emphasis** | `**bold**`, `*italic*`, `~~strike~~`, `__underline__` |
| **Links** | Inline `[text](url)` |
| **Images** | Inline `![alt](url "title")` |
| **Tables** | GFM with aligned columns |
| **Blockquotes** | `> ` prefix |
| **Horizontal rules** | `---` |

---

## Edge Cases and Limitations

### Handled Edge Cases

1. **Preformatted text**: Inside `<pre>`, text is not escaped for Markdown
2. **Percent-encoded URLs**: URLs are decoded for cleaner output
3. **URLs with spaces**: Wrapped in angle brackets `<url>`
4. **Named anchors**: Emit as HTML (no Markdown equivalent)
5. **Images with dimensions**: Fall back to inline HTML
6. **Nested lists**: Supported via parent chain tracking
7. **List continuation**: Subsequent paragraphs indented

### Known Limitations

From the crate documentation:

1. **No markdown flavors**: Does not support:
   - `-` or `+` for unordered lists
   - `##` or `==` for h3-h6 (only Setext for h1/h2)

2. **No code style detection**: Language must be specified via `class="language-xxx"`; no automatic detection

3. **Single table only**: Table handler skips descendants to avoid nested content issues

4. **No configuration**: Fixed opinionated output format

5. **No content extraction**: Unlike tools like readability, converts entire document including navigation, headers, footers

6. **No JavaScript handling**: Static HTML conversion only

---

## Performance Characteristics

- **Parsing**: Uses html5ever (Servo engine) - robust HTML5 parsing
- **Memory**: RcDom uses reference counting - moderate memory overhead
- **Regex**: Lazy-static compiled patterns - efficient reuse
- **No dependencies**: No heavy external dependencies (unlike some alternatives)

---

## Usage in Benchmark

From `src/scores.rs`:

```rust
#[cfg(feature = "html2md")]
"html2md" => {
    runner.run(output_name, |html| html2md::parse_html(html));
}
```

The extractor is:
- Enabled via `html2md` feature in Cargo.toml
- Uses default configuration (no custom handlers)
- Processes full HTML input
- Returns raw Markdown string

No configuration is passed - the crate has no configuration API.

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
    <h1>Main Heading</h1>
    <p>This is a <strong>bold</strong> and <em>italic</em> paragraph.</p>
    
    <h2>Section One</h2>
    <p>Here's a <a href="https://example.com">link</a> and an image:</p>
    <img src="photo.jpg" alt="A photo" title="My Photo">
    
    <h3>Code Example</h3>
    <pre><code class="language-rust">fn main() {
    println!("Hello!");
}</code></pre>
    
    <h3>List Section</h3>
    <ul>
        <li>First item</li>
        <li>Second item
            <ul>
                <li>Nested item</li>
            </ul>
        </li>
    </ul>
    
    <h3>Table Section</h3>
    <table>
        <tr><th>Name</th><th>Value</th></tr>
        <tr><td>Foo</td><td>100</td></tr>
    </table>
    
    <blockquote>
        <p>A blockquote example.</p>
    </blockquote>
    
    <hr>
    
    <p>End of <del>document</del>.</p>
</body>
</html>
```

### Expected Markdown Output

```markdown
Main Heading
==========

This is a **bold** and *italic* paragraph.

Section One
----------

Here's a [link](https://example.com) and an image:

![A photo](photo.jpg "My Photo")

### Code Example

```rust
fn main() {
    println!("Hello!");
}
```

### List Section

* First item
* Second item
  * Nested item

### Table Section

| Name | Value |
|:-----|------:|
| Foo  |   100 |

> A blockquote example.

---

End of ~~document~~.
```

---

## Comparison with Similar Extractors

| Feature | html2md | html2md-rs | fast_html2md | mdream |
|---------|---------|-------------|--------------|--------|
| **Heading style** | Setext/ATX mixed | ATX | ATX | Configurable |
| **List markers** | `*` only | `*` | `*`, `-` | Configurable |
| **Tables** | Full support | Full support | Full support | Full support |
| **Custom handlers** | Yes | No | No | Yes |
| **Configuration** | None | Minimal | None | Extensive |
| **License** | GPL-3.0 | MIT | Apache-2.0 | MIT |

---

## Conclusion

The `html2md` crate is a straightforward, dependency-light HTML-to-Markdown converter. It implements a sensible default set of conversions covering most common HTML elements. The architecture is clean and extensible via custom handlers, but lacks built-in configuration options.

The crate is well-suited for:
- Simple HTML-to-Markdown conversion
- Projects needing custom tag handling via `parse_html_custom()`
- Scenarios requiring the extended output mode (`parse_html_extended`)

It is less suited for:
- Content extraction use cases (no main content detection)
- Scenarios requiring configuration of output style
- Projects needing multiple Markdown flavors

In the benchmark context, it represents a "vanilla" conversion approach without preprocessing or content filtering.
