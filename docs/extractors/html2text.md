# html2text Rust Crate: Deep-Dive Analysis

## Overview

The `html2text` crate is a mature Rust library (version 0.17.1) that converts HTML to plain text. It uses the Servo project's HTML parser (`html5ever`) to build a DOM and renders it into either plain text or text with rich annotations (for terminal color support).

**Repository**: https://github.com/jugglerchris/rust-html2text  
**Crate**: https://crates.io/crates/html2text  
**Documentation**: https://docs.rs/html2text/0.17.1

### Benchmark Integration

In this benchmark, `html2text` is invoked with the following configuration (from `src/scores.rs` around line 139):

```rust
let mut render = html2text::config::plain()
    .max_wrap_width(width)
    .raw_mode(cfg.raw_mode);
if cfg.no_link_wrapping {
    render = render.no_link_wrapping();
}
render
    .string_from_read(&mut html, width)
    .unwrap_or_default()
```

**Default benchmark configuration** (from `src/extractor_config.rs`):

```rust
Html2TextConfig {
    max_wrap_width: 1000,
    raw_mode: false,
    no_link_wrapping: false,
}
```

---

## Configuration Options

### max_wrap_width

**Type**: `usize`  
**Default**: 1000 (in benchmark), 80 (crate default for `from_read`)

The maximum line length for text wrapping. When set, paragraphs will be wrapped to that width even if there is more total width available. The benchmark defaults to 1000, which effectively disables wrapping in most practical scenarios.

Key behaviors:

- The `width` parameter passed to `string_from_read()` is used for the actual wrapping
- `max_wrap_width` sets a ceiling: paragraphs won't exceed this width even if there's more available space
- Minimum wrap width defaults to 3 characters (configurable via `min_wrap_width()`)

### raw_mode

**Type**: `bool`  
**Default**: `false`

Raw extraction mode. When enabled:

- Text in table cells is rendered together as if the table had a single column
- Every cell is treated as its own row
- Implies `no_table_borders()` - table borders are suppressed
- Useful for extracting tabular data as continuous text

### no_link_wrapping

**Type**: `bool`  
**Default**: `false`

When enabled, URLs are not wrapped at line breaks. Some terminals handle long URLs better when not pre-wrapped, as wrapping can interfere with URL parsing.

---

## Text Wrapping Behavior

### How Wrapping Works

The html2text crate implements intelligent text wrapping using the `unicode-width` crate to correctly handle multi-byte Unicode characters (CJK characters count as 2 columns, emoji as 2, etc.).

**Core wrapping behavior:**

1. **Paragraph-level wrapping**: Text within block-level elements (p, div, td, etc.) is wrapped to the specified width
2. **Preserves line breaks**: Explicit `<br>` tags and block-level elements create new lines in output
3. **Collapses whitespace**: Multiple consecutive whitespace characters are normalized to single spaces
4. **Unicode-aware**: Uses `unicode-width` to count display columns, not byte count

### Whitespace Handling

- **Normalizes whitespace**: Sequences of spaces, tabs, and newlines are collapsed to single spaces
- **Leading/trailing whitespace**: Trimmed from the beginning and end of blocks
- **Preserves intentional line breaks**: Block elements (`div`, `p`, `tr`, etc.) and `<br>` tags create explicit line breaks

### Width Behavior Examples

| Width Setting | Behavior |
|--------------|----------|
| 80 (default) | Standard terminal width wrapping |
| 1000 (benchmark) | Effectively no wrapping for typical content |
| 0 | Falls back to minimum width (3) |

The benchmark uses `max_wrap_width: 1000` which is extremely wide, so wrapping only triggers for extremely long paragraphs or URLs that exceed this width.

---

## Heading Handling

**Key insight**: By default, html2text produces **plain text only** - no markdown formatting.

### Default Behavior (PlainDecorator)

Headings are rendered as **plain text without any prefix characters**:

```html
<h1>Main Title</h1>
<h2>Subtitle</h2>
<h3>Section</h3>
```

Output:
```
Main Title
Subtitle
Section
```

### With Markdown-like Decoration (do_decorate())

When `do_decorate()` is enabled, headings get prefix characters similar to markdown:

- `<h1>`: No prefix (or underline with `===` depending on configuration)
- `<h2>`: No prefix (or underline with `---`)
- `<h3>`-`<h6>`: Prefixed with `#` characters

However, **the benchmark does NOT enable this option** - it uses the default `plain()` decorator which produces unadorned text.

### Heading Levels

The `header_prefix()` method on TextDecorator returns an empty string for all heading levels in PlainDecorator. There is no visual distinction between h1 through h6 in plain mode.

---

## Link Handling

### Default Behavior

Links are rendered with the URL appended in parentheses:

```html
<a href="https://example.com">Click here</a>
```

Output:
```
Click here (https://example.com)
```

### no_link_wrapping()

When this option is enabled, URLs are not wrapped at line breaks - they remain on a single line even if they exceed the wrap width.

### link_footnotes()

When enabled via `link_footnotes(true)`, links are instead rendered as numbered footnotes at the end of the document:

```html
<a href="https://example.com">Click here</a>
```

Output:
```
Click here [1]

[1] https://example.com
```

### Link Wrapping Behavior

- Links are wrapped as part of the paragraph text
- With `no_link_wrapping: true`, URLs remain intact and may extend beyond wrap width
- The URL is always included (no option to hide URLs in plain mode)

---

## Image Handling

### Default Behavior

Images are rendered with alt text if available, otherwise a placeholder:

```html
<img src="photo.jpg" alt="A beautiful sunset">
<img src="icon.png">
```

Output:
```
[A beautiful sunset]
[Image: icon.png]
```

### empty_img_mode

Configurable via `empty_img_mode(ImageRenderMode)`:

- `IgnoreEmpty`: Don't render images without alt text
- `ShowAlways`: Always show image placeholders
- `Replace(...)`: Replace with custom text

### Image Source

The image source (src attribute) is not included in the default output - only alt text or a placeholder is shown.

---

## Table Handling

### Default Behavior

Tables are rendered with ASCII borders:

```html
<table>
  <tr><th>Header 1</th><th>Header 2</th></tr>
  <tr><td>Cell 1</td><td>Cell 2</td></tr>
</table>
```

Output:
```
+----------+----------+
| Header 1 | Header 2 |
+----------+----------+
| Cell 1   | Cell 2   |
+----------+----------+
```

### no_table_borders()

When enabled, tables are rendered without borders - just space-separated columns:

```
Header 1 Header 2
Cell 1   Cell 2
```

### raw_mode

When `raw_mode(true)` is enabled, tables are traversed as if they had a single column - every cell's content is rendered sequentially:

```
Header 1 Header 2 Cell 1 Cell 2
```

This is useful for extracting tabular data as continuous plain text.

### Table Cell Wrapping

- Cell content is wrapped to the specified width
- Each cell maintains its own wrapping context
- Column widths are determined by the widest content in each column

---

## Code Block Handling

### Inline Code

```html
Use the <code>printf()</code> function
```

Output:
```
Use the `printf()` function
```

Code is wrapped in backticks.

### Code Blocks

```html
<pre>
function hello() {
  console.log("Hello!");
}
</pre>
```

Output:
```
function hello() {
  console.log("Hello!");
}
```

Preformatted text is rendered with whitespace preserved - no wrapping occurs within `<pre>` blocks.

---

## Emphasis Handling

### Default Behavior (PlainDecorator)

**No visual markers** are added for emphasis:

```html
<em>emphasized</em> and <strong>strong</strong>
```

Output:
```
emphasized and strong
```

The text is rendered identically - no asterisks, underscores, or other markers are added.

### With do_decorate() Enabled

When markdown-like decoration is enabled:

- `<em>`: Wrapped in underscores (`_emphasized_`)
- `<strong>`: Wrapped in double asterisks (`**strong**`)
- `<del>` (strikeout): Uses strikethrough characters or unicode combining characters

The benchmark does NOT use `do_decorate()`, so emphasis is lost in the default configuration.

### RichDecorator (Terminal Color Mode)

When using `html2text::config::rich()` instead of `plain()`:

- Emphasis is annotated with color information
- Terminal can render colors based on annotations
- No character-based markers added

---

## Skip Tags Behavior

**Important**: The html2text crate does **not** have a built-in "skip_tags" configuration like some other extractors.

The benchmark handles skip tags differently:

- `DEFAULT_SKIP_TAGS` is defined in `extractor_config.rs`: `["nav", "script", "style", "header", "footer", "img", "svg", "iframe"]`
- This constant is used for **other extractors** (htmd, html2md-rs)
- The html2text extractor does NOT use this - it processes all HTML elements

To skip tags with html2text, you would need to:

1. Pre-process the HTML to remove unwanted elements, OR
2. Use the CSS feature with `display: none` rules

### CSS-based Removal

When the `css` feature is enabled:

- Elements with `display: none` in CSS are removed
- Elements with `overflow: hidden` and zero height are removed

This can be used to filter content, but requires CSS processing to be enabled.

---

## Raw Mode vs Normal Mode

### Normal Mode (raw_mode: false)

- Tables are rendered with proper grid formatting
- Cell content is wrapped within column constraints
- Borders are shown (unless `no_table_borders()` is set)

### Raw Mode (raw_mode: true)

- Tables treated as linear content
- Cells rendered sequentially without grid structure
- Implies `no_table_borders()` automatically
- Useful for extracting data from tables as readable text

### When to Use Each

| Mode | Use Case |
|------|----------|
| Normal | Document rendering, readable output |
| Raw | Data extraction, scraping tabular data |

---

## no_link_wrapping Option

### Disabled (default)

URLs are wrapped as part of normal text flow:
```
Check out this https://very-long-url.example.com/some/path
/and/more/path?query=parameter for more info
```

### Enabled

URLs remain on single lines, potentially exceeding wrap width:
```
Check out this https://very-long-url.example.com/some/path/and/more/path?query=parameter
for more info
```

This is useful for terminals that handle URLs better when not wrapped, as wrapping can break URL parsing in some contexts.

---

## Edge Cases

### Deeply Nested HTML

The html2text crate uses html5ever to build a full DOM tree. Deep nesting is handled correctly - the renderer flattens the tree structure into text output regardless of nesting depth.

```html
<div><div><div><div><p>Deep content</p></div></div></div></div>
```

Output:
```
Deep content
```

### Malformed Tags

The html5ever parser is lenient with malformed HTML:

- Unclosed tags are handled gracefully
- Unknown tags are treated as neutral elements
- Malformed attributes don't cause parsing failures

### Very Long Documents

Performance characteristics:

- Memory usage scales with document size (DOM tree)
- Wrapping is performed efficiently
- Large documents (MBs of HTML) can be processed without issues

### Empty Elements

- Empty paragraphs: No output
- Empty table cells: Rendered as empty (with border in table mode)
- Elements with only whitespace: Typically suppressed

### Unicode Content

- Full Unicode support
- CJK characters display correctly (counted as 2 columns per character)
- Emoji handled correctly
- Right-to-left text (RTL) may not display correctly (limited support)

---

## Configuration Options Summary

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `max_wrap_width` | usize | 80/1000 | Maximum wrap width |
| `raw_mode` | bool | false | Linear table extraction |
| `no_link_wrapping` | bool | false | Don't wrap URLs |
| `no_table_borders` | bool | false | Remove table borders |
| `do_decorate` | bool | false | Add markdown-like marks |
| `link_footnotes` | bool | false | Use footnotes for URLs |
| `pad_block_width` | bool | false | Pad to full width |
| `min_wrap_width` | usize | 3 | Minimum wrap width |
| `allow_width_overflow` | bool | false | Allow overflow vs error |

---

## Sample Input/Output Demonstrating Wrapping

### Example 1: Simple Paragraph at Different Widths

**Input HTML:**
```html
<p>This is a long paragraph that contains enough text to demonstrate how the html2text wrapper handles line breaks at different width settings. The text will be wrapped at the specified column width.</p>
```

**Output at width=40:**
```
This is a long paragraph that contains
enough text to demonstrate how the
html2text wrapper handles line breaks
at different width settings. The text
will be wrapped at the specified
column width.
```

**Output at width=80:**
```
This is a long paragraph that contains enough text to demonstrate how the
html2text wrapper handles line breaks at different width settings. The text
will be wrapped at the specified column width.
```

**Output at width=1000 (benchmark default):**
```
This is a long paragraph that contains enough text to demonstrate how the html2text wrapper handles line breaks at different width settings. The text will be wrapped at the specified column width.
```

### Example 2: Mixed Content

**Input HTML:**
```html
<h1>Document Title</h1>
<p>This is the introduction paragraph that explains what this document is about.</p>
<h2>Section One</h2>
<p>Here is some content in section one. It contains <em>emphasized text</em> and <strong>bold text</strong> and some <code>inline code</code>.</p>
<ul>
<li>First list item</li>
<li>Second list item with more text to show wrapping</li>
<li>Third list item</li>
</ul>
<h2>Section Two</h2>
<p>Another paragraph with <a href="https://example.com">a link to somewhere</a> and an image <img src="test.png" alt="test image">.</p>
```

**Output at width=60 (normal mode):**
```
Document Title
==============

This is the introduction paragraph that explains what this
document is about.

Section One
-----------

Here is some content in section one. It contains emphasized text
and bold text and some `inline code`.

* First list item
* Second list item with more text to show wrapping
* Third list item

Section Two
-----------

Another paragraph with a link to somewhere
(https://example.com) and an image [test image].
```

### Example 3: Table at Different Widths

**Input HTML:**
```html
<table>
  <tr><th>Name</th><th>Description</th></tr>
  <tr><td>Alice</td><td>A very long description that demonstrates how table cells are wrapped at narrow widths</td></tr>
  <tr><td>Bob</td><td>Short</td></tr>
</table>
```

**Output at width=40 (normal mode):**
```
+------+----------------------------------------+
| Name | Description                          |
+------+----------------------------------------+
| Alice | A very long description that       |
|       | demonstrates how table cells are   |
|       | wrapped at narrow widths           |
+------+----------------------------------------+
| Bob  | Short                                |
+------+----------------------------------------+
```

**Output at width=40 (raw_mode: true):**
```
Name Description
Alice A very long description that demonstrates how table cells are wrapped at narrow widths
Bob Short
```

**Output at width=40 (no_table_borders: true):**
```
Name Description
Alice A very long description that demonstrates how table cells are wrapped at narrow widths
Bob Short
```

### Example 4: Links at Different Settings

**Input HTML:**
```html
<p>Visit <a href="https://very-long-url-example.com/some/path/that/is/really/long">this website</a> for more information.</p>
```

**Output at width=40 (default):**
```
Visit this website
(https://very-long-url-example.com/some/path/that/is/really/long)
for more information.
```

**Output at width=40 (no_link_wrapping: true):**
```
Visit this website
(https://very-long-url-example.com/some/path/that/is/really/long)
for more information.
```

Note: In this case, the URL is already on its own line due to length, so the setting has minimal effect.

### Example 5: Code Blocks

**Input HTML:**
```html
<p>Here is some code:</p>
<pre>
function calculate(a, b) {
    return a + b;
}
</pre>
<p>And inline: <code>x = y + 1</code></p>
```

**Output (wrapping disabled - width=1000):**
```
Here is some code:
function calculate(a, b) {
    return a + b;
}
And inline: `x = y + 1`
```

---

## Benchmark-Specific Behavior

In this benchmark's default configuration:

```rust
Html2TextConfig {
    max_wrap_width: 1000,
    raw_mode: false,
    no_link_wrapping: false,
}
```

The output characteristics are:

1. **No wrapping** for typical content (width 1000 is very wide)
2. **No markdown formatting** (plain decorator)
3. **No emphasis markers** (em/strong are rendered as plain text)
4. **Tables have borders** (normal mode)
5. **Links inline** with parentheses
6. **Images show alt text or placeholder**

This produces plain, readable text without any markup styling - suitable for scenarios where pure text extraction is needed without any markdown or formatting conventions.

---

## Comparison with Python html2text

It's worth noting that there is also a **Python** `html2text` library (used in this benchmark as `html2text-py`). The Rust and Python libraries are **separate projects** with different:

- HTML parsing implementations
- Configuration APIs
- Default behaviors
- Output formatting

The Rust version (`html2text` crate) tends to be faster due to Rust's performance characteristics, but the Python version may have different output formatting by default.
