# E2m Extractor (wisup-e2m)

## Overview

**wisup-e2m** (version 0.1.61) is a multi-format parsing library produced by wisup-io that converts documents (HTML, PDF, DOCX, EPUB, PPT, URLs, etc.) into text and images. The `e2m` extractor in this benchmark uses its HTML parser via the `HtmlParser` class.

In the benchmark's Rust code (`src/scores.rs` line 986), the extraction is performed by invoking `uv run -- python3` with a one-liner that imports `wisup_e2m`, creates an `HtmlParser`, and calls `parse()` on the HTML content.

```python
from wisup_e2m.parsers.doc.html_parser import HtmlParser
p = HtmlParser(engine=sys.argv[2], langs=langs)
result = p.parse(text=open(sys.argv[1]).read(), skip_headers_and_footers=..., include_image_link_in_text=...)
print(result.text, end='')
```

## E2mConfig (src/extractor_config.rs line 215)

```rust
pub struct E2mConfig {
    pub engine: String,           // default: "unstructured"
    pub langs: Vec<String>,       // default: ["en", "zh"]
    pub skip_headers_and_footers: bool,  // default: true
    pub include_image_link_in_text: bool, // default: false
}
```

| Parameter | Default | Description |
|-----------|---------|-------------|
| `engine` | `"unstructured"` | The extraction engine (only `unstructured` is supported for HTML parsing) |
| `langs` | `["en", "zh"]` | Language codes for language detection in unstructured |
| `skip_headers_and_footers` | `true` | Passes through to unstructured; strips content inside `<header>` and `<footer>` tags |
| `include_image_link_in_text` | `false` | When true, images appear as `![](path)` Markdown references; when false, images are stripped entirely |

## Engine Options

The `engine` configuration option supports three values, but they map to **different parser classes** depending on whether you're parsing a URL or an HTML file:

| Engine | Parser Class | Use Case | How It Works |
|--------|-------------|----------|-------------|
| `unstructured` | `HtmlParser` | HTML file parsing | Uses `unstructured.partition.html.partition_html()` — local DOM parsing |
| `jina` | `UrlParser` | URL crawling | Calls `https://r.jina.ai/<url>` — remote API that returns Markdown |
| `firecrawl` | `UrlParser` | URL crawling | Uses the Firecrawl SDK (`firecrawl-py`) — remote API crawl service |

**Important**: For the `e2m` extractor in this benchmark (which receives raw HTML bytes), only the `unstructured` engine is used. The `jina` and `firecrawl` engines are only relevant for `UrlParser`, which fetches content over HTTP. This means the benchmark's e2m results reflect the behavior of **unstructured** only.

### unstructured engine

- Calls `unstructured.partition.html.partition_html()` with the raw HTML text
- Passes `languages` (for OCR/detection) and `skip_headers_and_footers` directly to unstructured
- The output from unstructured is a list of **element objects**, each with a `.category` and `.text`
- These elements are then passed through `_prepare_unstructured_data_to_e2m_parsed_data()` which applies title markers and image linking

### jina engine (URL parsing only)

- Prefixes the URL with `https://r.jina.ai/` and issues an HTTP GET
- The Jina Reader API returns Markdown-formatted content
- No local HTML parsing involved; entirely offloaded to Jina's service
- Supports image URL extraction and optional image downloading

### firecrawl engine (URL parsing only)

- Uses the `firecrawl-py` SDK to call the Firecrawl crawl API
- Returns Markdown for each crawled page via `parsed_text["markdown"]`
- Multiple pages are joined with double newlines

## Extraction Algorithm

The extraction pipeline for HTML (unstructured engine) is:

1. **HTML Parsing**: `unstructured.partition.html.partition_html()` parses the HTML string into a list of document elements. This uses `html.parser` (Python's built-in) to build a DOM-like structure, then applies heuristics to classify regions into semantic categories (Title, NarrativeText, ListItem, Table, CodeSnippet, Image, etc.).

2. **Header/Footer Stripping**: If `skip_headers_and_footers=true` (the default), unstructured removes any content that falls within `<header>` or `<footer>` tag regions before returning elements.

3. **Element Conversion**: Each unstructured element is mapped to a text chunk:
   - `Title` → prepend `# ` to text
   - `Header` (h2) → prepend `## ` to text
   - `Section-header` (h3+) → prepend `### ` to text
   - `Image` → output `![](image_name)` if `include_image_link_in_text=true`
   - `PageNumber` → skipped entirely when `ignore_page_number=true`
   - All other categories → output text as-is

4. **Image Path Resolution**: For image elements, the path from `element.metadata.image_path` is resolved relative to `work_dir` and moved into `image_dir`.

5. **Concatenation**: All text chunks are joined with `\n` newlines.

## What Gets Stripped vs Kept

### Stripped

- **`<script>` tags** — JavaScript is removed during parsing (unstructured does not execute JavaScript)
- **`<style>` tags** — CSS is stripped; styling information is not preserved
- **`<header>` / `<footer>` content** — when `skip_headers_and_footers=true` (default), these are excluded
- **Inline formatting** — `<strong>`, `<b>`, `<em>`, `<i>` etc. are rendered as plain text; no markdown emphasis markers are added
- **Link URLs** — anchor text is preserved but the `href` destination is discarded; links become plain text
- **Image pixels** — only the `alt` text survives; actual image data is not embedded; image paths are only included if `include_image_link_in_text=true`
- **Table structure** — table cells are concatenated into a single line of text (no Markdown table syntax)
- **Blockquote markers** — blockquote content is treated as plain `UncategorizedText`; no `>` prefix is added
- **Code block fences** — `CodeSnippet` elements output as plain text without triple-backtick fences
- **Horizontal rules** — `<hr>` elements produce no output at all

### Kept

- Text content of all elements
- Whitespace and newlines between elements
- Language detection metadata (via `languages` config)

## Element Handling Details

### Headings

Unstructured classifies heading elements by depth. The e2m post-processing adds Markdown heading markers:

| HTML Element | unstructured category | Output |
|---|---|---|
| `<h1>` | `Title` | `# Heading text` |
| `<h2>` | `Header` | `## Heading text` |
| `<h3>+` | `Section-header` | `### Heading text` |
| `<h4>`, `<h5>`, `<h6>` | `Section-header` | `### Heading text` (all grouped) |

### Links

Links (`<a>`) are classified as `UncategorizedText` by unstructured. Only the link text is kept; the `href` is completely lost. No Markdown link syntax (`[text](url)`) is produced.

**Example**: `<a href="https://example.com">Click here</a>` becomes just `Click here`.

### Images

Image elements (`<img>`) become `Image` category elements in unstructured with `element.text` set to the `alt` attribute value.

When `include_image_link_in_text=true`:
```
![](image_name)
```
Where `image_name` is the file name resolved relative to `work_dir`.

When `include_image_link_in_text=false` (default): Images produce no output text.

The actual image file is moved to `image_dir` by e2m but the benchmark does not verify image files.

### Code Blocks

`<pre><code>` or `<code>` blocks are classified as `CodeSnippet` by unstructured. The code text is preserved but:
- No triple-backtick fences are added
- No language specifier is included

**Example**: `<pre><code>def foo(): pass</code></pre>` becomes just `def foo(): pass`

### Tables

`<table>` elements are classified as `Table` by unstructured. The table content is rendered as a single text string with cells concatenated. No Markdown pipe-table syntax is produced.

**Example**:
```html
<table><tr><td>A</td><td>B</td></tr></table>
```
Output: `A B`

### Lists

`<ul>` and `<ol>` items are classified as `ListItem`. Each item appears on its own line, plain text. No Markdown list markers (`-`, `*`, `1.`) are added by default. Nested lists are flattened — items from nested sub-lists appear sequentially.

**Example**:
```html
<ul><li>Item 1<ul><li>Nested</li></ul></li><li>Item 2</li></ul>
```
Output:
```
Item 1
Nested
Item 2
```

### Blockquotes

`<blockquote>` elements are classified as `UncategorizedText`. The content is extracted as plain text with no `>` prefix added.

**Example**: `<blockquote>This is a quote.</blockquote>` becomes `This is a quote.`

### Emphasis (Bold/Italic)

**Both `<strong>`/`<b>` and `<em>`/`<i>` are completely stripped of formatting.** The text content is preserved but no Markdown `**`, `*`, `__`, or `_` delimiters are inserted.

**Example**: `<p>Hello <strong>bold</strong> and <em>italic</em></p>` becomes `Hello bold and italic`.

### Horizontal Rules

`<hr>` elements produce no output at all. The content before and after the `<hr>` is concatenated with a single newline.

### Malformed HTML

Unstructured is reasonably tolerant of malformed HTML (unclosed tags, nested divs, etc.). It uses Python's `html.parser` which performs best-effort parsing. Elements are extracted as text chunks even from deeply nested or incorrectly nested structures.

## Default Configuration in Benchmark

The benchmark default for `e2m` is:

```rust
pub struct E2mConfig {
    pub engine: "unstructured".to_string(),
    pub langs: vec!["en".to_string(), "zh".to_string()],
    pub skip_headers_and_footers: true,
    pub include_image_link_in_text: false,
}
```

So by default:
- `skip_headers_and_footers: true` — header/footer content is stripped
- `include_image_link_in_text: false` — images produce no text output
- `engine: "unstructured"` — HTML is processed locally by the unstructured library
- `langs: ["en", "zh"]` — language detection hint passed to unstructured

## Summary of Key Behaviors

| Feature | Behavior |
|---------|----------|
| Heading levels | Preserved with `#`, `##`, `###` prefixes |
| Links | Text only; URL lost; no `[text](url)` syntax |
| Images | Alt text kept only if `include_image_link_in_text=true` |
| Code blocks | Plain text; no fences or language tags |
| Tables | Plain text; cell concatenation; no pipe-table syntax |
| Lists | Plain text per item; no list markers; nested lists flattened |
| Blockquotes | Plain text; no `>` prefix |
| Bold/Italic | Stripped entirely; plain text only |
| `<hr>` | Silently dropped |
| `<script>`/`<style>` | Removed (not executed) |
| Header/footer | Stripped when `skip_headers_and_footers=true` |
| Malformed HTML | Best-effort parsing; text recovered |
| Languages | `["en", "zh"]` hint passed to unstructured |

## Sample HTML Input and Expected Markdown Output

### Input HTML

```html
<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><title>Sample Document</title></head>
<body>
<header><nav><a href="/">Home</a></nav></header>
<main>
<h1>Main Heading</h1>
<p>This is a paragraph with <strong>bold text</strong> and <em>italic text</em>.</p>
<h2>Section Heading</h2>
<p>An <a href="https://example.com">example link</a> and <img src="diagram.png" alt="Architecture diagram">.</p>
<pre><code>def hello():
    print("Hello, world!")
</code></pre>
<ul>
<li>First item</li>
<li>Second item
    <ul>
    <li>Nested item</li>
    </ul>
</li>
</ul>
<blockquote>A famous quote goes here.</blockquote>
<table>
<tr><td>Column A</td><td>Column B</td></tr>
<tr><td>Value 1</td><td>Value 2</td></tr>
</table>
<hr>
<p>After horizontal rule.</p>
</main>
<footer>&copy; 2026 Example</footer>
</body>
</html>
```

### Expected Markdown Output (with default config: engine=unstructured, skip_headers_and_footers=true, include_image_link_in_text=false)

```markdown
# Main Heading
This is a paragraph with bold text and italic text.
## Section Heading
An example link and .
def hello():
    print("Hello, world!")
First item
Second item
Nested item
A famous quote goes here.
Column A Column B
After horizontal rule.
```

### Notes on Expected Output

- `skip_headers_and_footers=true` strips the `<header>` (nav link "Home") and `<footer>` (copyright text)
- Heading markers `#` and `##` are added by e2m's `_prepare_unstructured_data_to_e2m_parsed_data()` with `add_title_marker=True`
- Inline formatting (`<strong>`, `<em>`) is completely stripped — no `**` or `*` appears
- Link URL is lost; link text "example link" appears as plain text
- Image alt text "Architecture diagram" is NOT included because `include_image_link_in_text=false` (the default); with `true` it would appear as `![](diagram.png)` before the code block
- Code block is plain text with no triple-backtick fence or language specifier
- List items are plain text with no `-` or `*` markers; nested items are flattened into sequential lines
- Blockquote is plain text with no `>` prefix
- Table cells are concatenated into a single line with spaces between them
- `<hr>` produces no output; text before and after is separated by a single newline
- JavaScript (none in this example) and CSS would be stripped if present
