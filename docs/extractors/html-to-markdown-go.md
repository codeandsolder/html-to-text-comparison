# html-to-markdown-go (html2markdown) Extractor Analysis

## Overview

**html-to-markdown-go** is a Go-based CLI tool (`html2markdown`) that converts HTML to Markdown. It wraps the `github.com/JohannesKaufmann/html-to-markdown/v2` Go library. The binary at `/tmp/html2markdown` is version v2.5.1.

This extractor is notably different from most others in this benchmark: it is a **generic HTML-to-Markdown converter** without built-in content extraction (like Readability). It processes raw HTML as-is, converting everything, so its effectiveness depends heavily on whether the input HTML is clean content or includes navigation/clutter.

---

## Repository and Package Information

- **GitHub Repository**: https://github.com/JohannesKaufmann/html-to-markdown
- **CLI Binary Name**: `html2markdown` (compiled from `cli/html2markdown`)
- **Current Version**: v2.5.1 (as of May 2026)
- **License**: MIT
- **Language**: Go
- **Library Import**: `github.com/JohannesKaufmann/html-to-markdown/v2`

### Architecture

The converter is built on a plugin-based architecture:

```
HTML Input → Parser → Converter (with plugins) → Markdown Output
```

The default configuration loads:
1. **base plugin** - Handles removing unwanted elements, whitespace collapsing, text transformation
2. **commonmark plugin** - Implements standard CommonMark markdown conversion

---

## How the Benchmark Invokes This Extractor

### HtmlToMarkdownGoConfig Structure

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HtmlToMarkdownGoConfig {
    pub domain: String,           // Base URL for resolving relative links
    pub plugins: Vec<String>,    // Additional plugins to enable
    pub include_selector: String, // CSS selector for content to include
    pub exclude_selector: String, // CSS selector for content to exclude
}

impl Default for HtmlToMarkdownGoConfig {
    fn default() -> Self {
        Self {
            domain: String::new(),
            plugins: vec!["commonmark".to_string()],  // Note: only commonmark, not base
            include_selector: String::new(),
            exclude_selector: String::new(),
        }
    }
}
```

### Argument Building

```rust
fn build_html_to_markdown_go_args(
    parsed_url: &url::Url,
    cfg: &HtmlToMarkdownGoConfig,
) -> Vec<String> {
    // Domain: use config value, or fall back to URL origin
    let domain = if cfg.domain.trim().is_empty() {
        parsed_url.origin().ascii_serialization()
    } else {
        cfg.domain.trim().to_string()
    };
    
    let mut args = vec![format!("--domain={}", domain)];
    
    // Include selector (only convert matching elements)
    if !cfg.include_selector.trim().is_empty() {
        args.push(format!("--include-selector={}", cfg.include_selector.trim()));
    }
    
    // Exclude selector (remove matching elements before conversion)
    if !cfg.exclude_selector.trim().is_empty() {
        args.push(format!("--exclude-selector={}", cfg.exclude_selector.trim()));
    }
    
    // Additional plugins (skip "commonmark" and "base" as they're always enabled)
    for plugin in cfg.plugins.iter()
        .map(|plugin| plugin.trim())
        .filter(|plugin| !plugin.is_empty() && *plugin != "commonmark" && *plugin != "base")
    {
        args.push(format!("--plugin-{}", plugin));
    }
    
    args
}
```

### Execution

```rust
fn run_html_to_markdown_go(
    html: &str,
    parsed_url: &url::Url,
    cfg: &HtmlToMarkdownGoConfig,
) -> String {
    let args = build_html_to_markdown_go_args(parsed_url, cfg);
    let out = std::process::Command::new("/tmp/html2markdown")
        .args(&args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();
    // ... handle input/output
}
```

HTML is passed via stdin, markdown is read from stdout.

---

## CLI Interface

### `--help` Output

```
# html2markdown - convert html to markdown [version v2.5.1]

Convert HTML to Markdown. Even works with entire websites!

## Basics

By default the "Commonmark" Plugin will be enabled. You can customize the options,
for example changing the appearance of bold with --opt-strong-delimiter="__"

Other Plugins can also be enabled. For example "GitHub Flavored Markdown" (GFM)
extends Commonmark with more features.

## Relative / Absolute Links

Use --domain="https://example.com" to convert *relative* links to *absolute* links.
The same also works for images.

## Escaping

Some characters have a special meaning in markdown. The library escapes these — if necessary.

## Security

Once you convert this markdown *back* to HTML you need to be careful of malicious content.
Use a HTML sanitizer before displaying the HTML in the browser!

## Flags

    -v, --version      show the version
    --help             show help

    --input PATH       Input file, directory, or glob pattern (instead of stdin)
    --output PATH      Output file or directory (instead of stdout)
    --output-overwrite Replace existing files

    --domain           The url of the web page, used to convert relative links to absolute links.
    --exclude-selector CSS query selector to exclude parts of the input
    --include-selector CSS query selector to only include parts of the input

    --opt-strong-delimiter  Make bold text: "**" or "__" (default: "**")
    --opt-table-cell-padding-behavior [for --plugin-table] "aligned", "minimal", or "none"
    --opt-table-header-promotion     [for --plugin-table] first row as header
    --opt-table-newline-behavior    [for --plugin-table] "skip" or "preserve"
    --opt-table-presentation-tables  [for --plugin-table] convert role="presentation" tables
    --opt-table-skip-empty-rows     [for --plugin-table] omit empty rows
    --opt-table-span-cell-behavior  [for --plugin-table] "empty" or "mirror"

    --plugin-strikethrough enable the plugin ~~strikethrough~~
    --plugin-table      enable the plugin table
```

---

## The `domain` Parameter

### Purpose

The `--domain` parameter specifies the base URL used to convert **relative URLs** to **absolute URLs** in links and images.

### What It Affects

1. **Links**: `<a href="/about">` becomes `[about](https://example.com/about)`
2. **Images**: `<img src="/images/logo.png">` becomes `![](/images/logo.png)` → `![](/images/logo.png)` with domain applied
3. **Any element with src/href attributes**: base, link, etc.

### How It Works

The domain is passed to the Go converter using `converter.WithDomain()`. When the converter encounters a relative URL (starting with `/` or without a scheme), it joins it with the provided domain.

### Benchmark Behavior

In the benchmark:
- If `cfg.domain` is empty, it uses `parsed_url.origin()` from the page URL
- This means each page's own domain is used as the base for resolving relative links
- Example: processing `https://docs.example.com/page` with no domain config → domain becomes `https://docs.example.com`

### Important Notes

- The domain must include the scheme (`https://`)
- Relative links within the same domain remain relative unless domain is specified
- This is critical when processing locally-saved HTML files that originally had relative paths

---

## The `plugins` Parameter

### Available Plugins

| Plugin | Description | CLI Flag |
|--------|-------------|----------|
| **commonmark** | Base CommonMark support (enabled by default) | Built-in |
| **base** | Base functionality (removing tags, collapsing whitespace) | Built-in |
| **strikethrough** | Converts `<strike>`, `<s>`, `<del>` to `~~text~~` | `--plugin-strikethrough` |
| **table** | Converts `<table>` to markdown tables | `--plugin-table` |

### How Plugins Are Specified

In the benchmark, plugins are passed as a vector:
```rust
plugins: vec!["commonmark".to_string(), "table".to_string(), "strikethrough".to_string()]
```

The CLI translates each plugin to `--plugin-{name}`:
- `"table"` → `--plugin-table`
- `"strikethrough"` → `--plugin-strikethrough`

### Plugin Filtering in Benchmark

The `build_html_to_markdown_go_args` function **filters out** "commonmark" and "base" when building CLI arguments because:
1. "base" and "commonmark" are always loaded (built-in)
2. They don't need explicit CLI flags
3. Only additional plugins need `--plugin-` prefixes

### How to Enable Additional Plugins

```bash
# Enable strikethrough and table plugins
html2markdown --plugin-strikethrough --plugin-table --input input.html
```

---

## How It Handles Various HTML Elements

### Headings

**Supported tags**: `<h1>` through `<h6>`

**Output styles**:
- **ATX style** (default): `# Heading` with optional closing `#`
- **Setext style**: Available but not default

**Configuration**: `--opt-heading-style` (not exposed in benchmark)

**Example**:
```html
<h1>Main Title</h1>
<h2>Subtitle</h2>
```
```markdown
# Main Title

## Subtitle
```

### Links

**Supported tags**: `<a>` with `href` attribute

**Features**:
- Converts to inline markdown: `[text](url)`
- Respects `--domain` for relative URLs
- Handles empty hrefs (configurable behavior)
- Handles links without content (configurable behavior)

**Configuration options** (via Go API, not CLI):
- `WithLinkEmptyHrefBehavior` - how to render `<a href="">`
- `WithLinkEmptyContentBehavior` - how to render `<a href="/page"></a>`

**Example**:
```html
<a href="https://example.com">Example Site</a>
<a href="/about">About Us</a>
```
```markdown
[Example Site](https://example.com)
[About Us](https://example.com/about)
```

### Images

**Supported tags**: `<img>` with `src` attribute

**Output**: `![alt text](url)` or `![](url)` if no alt

**Features**:
- Alt text becomes the markdown alt text
- Resolves relative URLs with `--domain`
- Handles srcset (uses first src)

**Example**:
```html
<img src="/images/logo.png" alt="Company Logo">
```
```markdown
![Company Logo](https://example.com/images/logo.png)
```

### Code Blocks

**Supported tags**: `<pre><code>`, `<code>` (block), `<pre>`

**Output formats**:
- **Fenced code blocks** (default): ``` language\ncode\n```
- **Indented**: 4-space indentation

**Configuration**: `--opt-code-block-fence` (e.g., `~~~` or ```)

**Features**:
- Language detection from class (e.g., `language-javascript` → ```javascript)
- Escapes characters that would interfere with markdown
- Preserves newlines within code blocks

**Example**:
```html
<pre><code class="language-javascript">
function hello() {
  console.log("Hello!");
}
</code></pre>
```
```markdown
```javascript
function hello() {
  console.log("Hello!");
}
```
```

### Tables (with `--plugin-table`)

**Supported tags**: `<table>`, `<thead>`, `<tbody>`, `<tr>`, `<th>`, `<td>`

**Features**:
- Column alignment detection (left, right, center via CSS)
- Rowspan and colspan support
- Header row promotion
- Empty row skipping

**Configuration options**:
| Option | Values | Default | Description |
|--------|--------|---------|-------------|
| `--opt-table-cell-padding-behavior` | `aligned`, `minimal`, `none` | `aligned` | Cell padding in output |
| `--opt-table-header-promotion` | `true`, `false` | `false` | Treat first row as header |
| `--opt-table-newline-behavior` | `skip`, `preserve` | `skip` | Handle newlines in cells |
| `--opt-table-presentation-tables` | `true`, `false` | `false` | Convert role="presentation" tables |
| `--opt-table-skip-empty-rows` | `true`, `false` | `false` | Omit empty rows |
| `--opt-table-span-cell-behavior` | `empty`, `mirror` | `empty` | How to render spanned cells |

**Example**:
```html
<table>
  <tr><th>Name</th><th>Age</th></tr>
  <tr><td>Alice</td><td>30</td></tr>
  <tr><td>Bob</td><td>25</td></tr>
</table>
```
```markdown
| Name | Age |
|------|-----|
| Alice | 30 |
| Bob | 25 |
```

### Lists

**Supported tags**: `<ul>`, `<ol>`, `<li>`

**Features**:
- Ordered and unordered list support
- Nested lists (unlimited depth)
- Mixed nesting
- List item content wrapping

**Configuration**: `--opt-bullet-list-marker` (default: `-`)

**Example**:
```html
<ul>
  <li>Item 1</li>
  <li>Item 2
    <ul>
      <li>Nested item</li>
    </ul>
  </li>
</ul>
```
```markdown
- Item 1
- Item 2
  - Nested item
```

### Blockquotes

**Supported tags**: `<blockquote>`

**Features**:
- Nested blockquotes supported
- Multi-line content
- Other elements within blockquotes

**Example**:
```html
<blockquote>
  <p>This is a quote.</p>
  <p>It spans multiple paragraphs.</p>
</blockquote>
```
```markdown
> This is a quote.
> 
> It spans multiple paragraphs.
```

### Emphasis

**Supported tags**: `<em>`, `<i>`, `<strong>`, `<b>`

**Delimiters** (configurable):
- Emphasis: `*` (default) or `_`
- Strong: `**` (default) or `__`

**Configuration**: `--opt-strong-delimiter`

**Example**:
```html
<em>emphasized</em>
<strong>strong emphasis</strong>
<strong><em>nested</em></strong>
```
```markdown
*emphasized*
**strong emphasis**
***nested***
```

### Horizontal Rules

**Supported tags**: `<hr>`

**Output**: `* * *` (default) — three asterisks with spaces

**Configuration**: `--opt-horizontal-rule` (default: `* * *`)

**Example**:
```html
<hr>
```
```markdown
* * *
```

---

## What Gets Stripped

### Automatic Removal (via base plugin)

The base plugin removes these elements entirely:
- `<head>` - Metadata section
- `<script>` - JavaScript code
- `<style>` - CSS styles
- `<link>` - Link elements (not rendered)
- `<meta>` - Meta tags
- `<iframe>` - Inline frames
- `<noscript>` - No-script content
- `<input>` - Form inputs
- `<textarea>` - Text areas
- HTML comments (`<!-- -->`)

### Explicit Removal (via TagType)

These are registered with `TagTypeRemove`:
- Comments (via `#comment`)
- All the above elements

### Whitespace Handling

The base plugin also:
- Collapses consecutive whitespace
- Trims leading/trailing whitespace from output
- Removes unnecessary newlines
- Handles block vs inline node distinction for proper spacing

---

## Escaping Behavior

### Smart Escaping (Default)

The library uses "smart" escaping mode by default, which means:

Characters that get escaped with backslash when necessary:
- `\` (backslash itself)
- `*` (emphasis, lists, HR)
- `_` (emphasis in some contexts)
- `-` (HR, list items)
- `+` (list items)
- `.` (setext headers, links)
- `>` (blockquotes)
- `|` (tables)
- `$` (math)
- `#` (headings)
- `=` (setext headers)
- `[` `]` (links)
- `(` `)` (links)
- `!` (images)
- `~` (strikethrough)
- `` ` `` (code)
- `"` `'` (quotes)

### Why Escaping Matters

Example from the documentation:
```html
<p>fake **bold** and real <strong>bold</strong></p>
```

With smart escaping (default):
```markdown
fake \*\*bold\*\* and real **bold**
```

Without escaping:
```markdown
fake **bold** and real **bold**
```

Both render identically, but the escaped version preserves the information that one was "fake" bold from HTML.

### Configuration

In Go API (not exposed via CLI):
```go
converter.WithEscapeMode("smart")  // or "disabled"
```

---

## Edge Cases

### Malformed HTML

The library uses Go's `html.Parse()` from `golang.org/x/net/html` which is generally forgiving. However:
- Unclosed tags may produce unexpected output
- Nested block/inline elements may affect whitespace
- Malformed tables may not convert cleanly

### Nested Formatting

Nested emphasis works correctly:
```html
<strong><em>nested</em></strong> → ***nested***
```

### Links Inside Code

Links within code blocks are not processed as links — code fences take precedence.

### Empty Elements

| Element | Behavior |
|---------|----------|
| `<a href="">text</a>` | Renders as link with empty href |
| `<a>text</a>` (no href) | Renders as plain text |
| `<img src="">` | Renders as `![]()` |
| `<img>` (no src) | Removed |

### Whitespace-Only Content

Text nodes with only whitespace are collapsed or removed during the collapse phase.

### Mixed Block/Inline

The library properly handles the distinction between block-level and inline elements, ensuring proper spacing in output.

---

## Sample HTML Input and Expected Markdown Output

### Example 1: Basic Formatting

**Input HTML:**
```html
<!DOCTYPE html>
<html>
<head><title>Sample</title></head>
<body>
    <h1>Welcome to the Document</h1>
    
    <p>This is a <strong>paragraph</strong> with <em>multiple</em> formatting.</p>
    
    <h2>Sections</h2>
    
    <p>Here is a <a href="https://example.com">link</a> and an image:</p>
    
    <img src="/images/hero.png" alt="Hero Image">
    
    <blockquote>
        <p>This is a blockquote with multiple lines.</p>
    </blockquote>
    
    <pre><code class="language-go">
func main() {
    fmt.Println("Hello")
}
    </code></pre>
    
    <hr>
    
    <ul>
        <li>Item one</li>
        <li>Item two</li>
        <li>Item three</li>
    </ul>
</body>
</html>
```

**Expected Markdown Output** (with domain=https://example.com):
```markdown
# Welcome to the Document

This is a **paragraph** with *multiple* formatting.

## Sections

Here is a [link](https://example.com) and an image:

![Hero Image](https://example.com/images/hero.png)

> This is a blockquote with multiple lines.

```go
func main() {
    fmt.Println("Hello")
}
```

* * *

- Item one
- Item two
- Item three
```

### Example 2: With Table Plugin

**Input HTML:**
```html
<table>
    <tr><th>Name</th><th>Role</th><th>Location</th></tr>
    <tr><td>Alice</td><td>Engineer</td><td>NYC</td></tr>
    <tr><td>Bob</td><td>Designer</td><td>LA</td></tr>
</table>
```

**Expected Markdown Output** (with --plugin-table):
```markdown
| Name | Role | Location |
|------|------|----------|
| Alice | Engineer | NYC |
| Bob | Designer | LA |
```

### Example 3: With Strikethrough Plugin

**Input HTML:**
```html
<p>This is <strike>old</strike> <del>deleted</del> and <s>outdated</s> text.</p>
```

**Expected Markdown Output** (with --plugin-strikethrough):
```markdown
This is ~~old~~ ~~deleted~~ and ~~outdated~~ text.
```

### Example 4: Complex Nesting

**Input HTML:**
```html
<article>
    <h1>Main Heading</h1>
    <p>Paragraph with <strong>bold</strong> and <em>italic</em> and <code>inline code</code>.</p>
    <ol>
        <li>First <a href="/page1">link</a></li>
        <li>Second with <strong>nested <em>emphasis</em></strong></li>
    </ol>
    <h2>Code Block Section</h2>
    <pre><code class="language-javascript">const x = 42; // comment</code></pre>
</article>
```

**Expected Markdown Output**:
```markdown
# Main Heading

Paragraph with **bold** and *italic* and `inline code`.

1. First [link](/page1)
2. Second with **nested *emphasis***

## Code Block Section

```javascript
const x = 42; // comment
```
```

### Example 5: Include/Exclude Selectors

**Input HTML:**
```html
<body>
    <nav><a href="/">Home</a></nav>
    <article>
        <h1>Main Content</h1>
        <p>Article text here.</p>
    </article>
    <aside><p>Sidebar content</p></aside>
    <footer>Footer</footer>
</body>
```

**With --include-selector=article**:
```markdown
# Main Content

Article text here.
```

**With --exclude-selector=nav,footer,aside**:
```markdown
[Home](/)

# Main Content

Article text here.

Sidebar content

Footer
```

Note: Exclude removes matching elements before conversion. Include limits conversion to only matching elements.

---

## Comparison with Other Extractors

| Feature | html-to-markdown-go | Percollate | Turndown | Trafilatura |
|---------|---------------------|------------|----------|-------------|
| **Content Extraction** | None (raw HTML) | @mozilla/readability | None | Built-in |
| **CommonMark Support** | Yes | Yes (via mdast) | Yes | Limited |
| **Table Plugin** | Yes (optional) | Yes (GFM) | Yes (plugin) | Yes |
| **Strikethrough** | Yes (optional) | Yes (GFM) | Yes (plugin) | No |
| **Link Resolution** | Via --domain | Built-in | Manual | Built-in |
| **Code Blocks** | Fenced or indented | Fenced | Fenced | Indented |
| **Selective Parsing** | Via CSS selectors | Limited | No | Via extraction |
| **Language** | Go | Node.js | JavaScript | Python |
| **Configuration** | Limited CLI | Extensive | Extensive | Extensive |

### Key Differences

1. **No content extraction**: Unlike Percollate/Readability-based extractors, this tool converts all HTML to markdown. It does not identify "main content" vs navigation/sidebars.

2. **Plugin-based architecture**: The functionality can be extended through plugins (currently table and strikethrough).

3. **CSS selector filtering**: Can use `--include-selector` and `--exclude-selector` to filter content before conversion.

4. **Relative URL resolution**: The `--domain` parameter enables proper handling of relative URLs in locally-saved HTML.

---

## Configuration in Default Benchmark State

The default `HtmlToMarkdownGoConfig` used in this benchmark:

```rust
HtmlToMarkdownGoConfig {
    domain: String::new(),        // Will use URL origin
    plugins: vec!["commonmark"],  // Only commonmark (base always loaded)
    include_selector: String::new(),
    exclude_selector: String::new(),
}
```

This means:
- Domain defaults to the page being processed
- Only CommonMark support (no tables, no strikethrough)
- No content filtering (all HTML is converted)

---

## Limitations

1. **No content extraction**: Raw HTML is converted as-is. If input contains full page HTML with navigation, sidebars, etc., those will appear in output.

2. **Limited CLI options**: Many library options (heading style, emphasis delimiters, etc.) are not exposed via CLI flags.

3. **No JavaScript execution**: Like most converters, JavaScript-rendered content is not captured.

4. **Single-page conversion**: Not designed for multi-page website conversion (unlike the library's `ConvertWebsite` functionality which the CLI doesn't expose).

5. **No automatic sanitization**: Output markdown may contain potentially dangerous content if input HTML contains scripts. The documentation explicitly warns about this and recommends using an HTML sanitizer if converting back to HTML.

---

## Security Considerations

As noted in the tool's documentation:

> **Security**: Once you convert this markdown *back* to HTML you need to be careful of malicious content. Use a HTML sanitizer before displaying the HTML in the browser!

The converter does not sanitize content — it only transforms HTML to markdown. If the resulting markdown is later converted back to HTML (e.g., for display), care should be taken to sanitize first.

---

## References

- Main Repository: https://github.com/JohannesKaufmann/html-to-markdown
- Library Documentation: https://pkg.go.dev/github.com/JohannesKaufmann/html-to-markdown/v2
- CLI Documentation: Part of the main repo
- Escaping Documentation: https://github.com/JohannesKaufmann/html-to-markdown/blob/main/ESCAPING.md
