# August — HTML to Plain Text Converter

**Crate:** [august](https://crates.io/crates/august) (v2.4.0)  
**Repository:** https://gitlab.com/alantrick/august/  
**License:** LGPL-3.0-or-later  
**Maintainer:** alantrick  

---

## Overview

August is a Rust crate for converting HTML to **plain text**, with a specific focus on email rendering. The project goals explicitly state:

- No round-trip capability (HTML cannot be reconstructed from output) — this is intentional, as extra markup would impede readability in email contexts.
- Tables are rendered nicely because emails commonly use `<table>` for layout due to patchy CSS support.
- Strict whitespace handling to prevent merged words like `textlikethis` or awkward spacing around element boundaries.

August is **plain text only** — it does **not** output Markdown, HTML, or any structured markup. Everything is converted to human-readable Unicode text.

---

## API

```rust
pub fn convert(input: &str, width: Width) -> String
pub fn convert_unstyled(input: &str, width: Width) -> String
pub fn convert_io(input: impl std::io::Read, output: impl std::io::Write, width: Width) -> io::Result<()>
pub fn convert_dom(dom: &RcDom, width: Width) -> String
pub fn convert_dom_unstyled(dom: &RcDom, width: Width) -> String
pub fn convert_dom_io(dom: &RcDom, width: Width, output: impl std::io::Write) -> io::Result<()>
pub fn convert_dom_io_unstyled(dom: &RcDom, width: Width, output: impl std::io::Write) -> io::Result<()>
```

The `convert` function is the primary entry point. The `unstyled` variants skip all text styling (no bold, italic, underline, strike, uppercase transformations).

---

## `max_width` Parameter

**Type:** `Width = usize` (grapheme count)

The `width` parameter controls **character wrapping** via the `textwrap` crate.

### How it works

When rendering block-level content, `width` is passed to `textwrap::Wrapper::new(width)` which wraps long lines at the specified column boundary. The wrapper uses **grapheme clusters** (via `unicode-segmentation`) to count width, so it handles Unicode correctly (e.g., CJK characters, emojis count as a single display width).

### Where width is used

1. **`inline_block_write`** (line 847-858): Wraps inline text within a block element using `textwrap::Wrapper::new(data.width)`.
2. **`generic_block_write`** (line 864-945): When `block_data` is present (i.e., rendering in block context with width known), wraps the final accumulated inline text.
3. **`Block::block_write`** (line 523-533): Passes width down to child blocks. For `<hr>` (Rule), it repeats `-` to fill the full width.
4. **`tr_text`** (line 1035-1107): When rendering table cells with width constraints, each cell's text is wrapped/block-written to fit its column width.

### How width affects rendering

- **Default (usize::MAX ≈ 1.8e19):** Effectively no wrapping. Lines are as long as they are.
- **Small values (e.g., 40, 80):** Lines wrap aggressively. Wrapping respects word boundaries.
- **`BlockType::Rule` special case:** When width is provided and block type is `Rule`, outputs `"-".repeat(width)`. When width is `None`, outputs `"---"` (3 dashes).

### Width and nested blocks

When a block has a prefix (e.g., list item numbers, `> ` for blockquotes), the **effective width** for content is reduced:

```rust
// line 895
let width = Some(data.width - data.first_line_prefix.len());
```

So a list item with prefix `* ` (2 chars) in an 80-column terminal has 78 characters available for content.

---

## Element Handling

### Headings (`<h1>`–`<h6>`)

Headings are rendered as **block elements** (own paragraph), with `StyleData::set_uppercase` applied. This means heading text is **UPPERCASED** in the output. No `#` prefix or any other Markdown-style marker is added. They are just plain text paragraphs with uppercase styling.

### Links (`<a>`)

Links use a custom `ReplaceFn` (`a_element`, line 222-238):

| Link type | Output format |
|-----------|----------------|
| `mailto:` | `"Display Name <email@example.com>"` |
| `http://` / `https://` | `"Display Name (https://...)"` |
| Other | Just the link text (no URL appended) |

The URL is only appended for web links. No bracket-style or reference-style link notation is used. The output is pure plain text.

### Images (`<img>`)

Images use `img_element` (line 259-268): Output is **only** the `alt` attribute text. If no `alt`, output is empty string. The image URL is discarded.

### Code (`<code>`, `<pre>`, `<kbd>`)

| Element | Transformation |
|---------|---------------|
| `<code>` | Wrapped in backticks: `` `code` `` |
| `<kbd>` | Style only (uppercase), no delimiters |
| `<pre>` | Block type + `preserve_whitespace` style. Whitespace is preserved verbatim, no backtick fencing. |
| `<var>` | Wrapped in backticks: `` `variable` `` |

The `<pre>` element does **not** use fenced code block syntax (``` ``` ```). It simply preserves whitespace and renders the content as a block.

### Tables (`<table>`, `<thead>`, `<tbody>`, `<tfoot>`, `<tr>`, `<th>`, `<td>`)

Tables receive significant attention in August, reflecting its email-use focus. Column widths are calculated proportionally based on content width hints. When width constraints exist, columns are balanced to fit within the available space using a custom proportional redistribution algorithm.

- **Columns** are separated by `"  "` (two spaces, `COLUMN_SEP`).
- **Cells** are padded with spaces to fill their column width.
- **Multiline cells** are handled by treating each line as a row in the final output, with all columns kept in sync.
- **`colspan`** is supported — a cell spanning N columns gets allocated the sum of those column widths.
- **Table header cells (`<th>`)** are rendered with uppercase transformation (`StyleData::set_uppercase`).
- **Table structure (`<thead>`, `<tbody>`, `<tfoot>`)** is flattened; rows are just rendered in document order.

The table rendering algorithm (lines 948-1167):
1. `table_column_widths` computes width hints per column per row using `Ratio<Width>` for fractional sizing.
2. `recalculate_column_widths` distributes available width proportionally across columns using a balanced floor/ceil assignment.
3. `tr_text` renders each row into fixed-width columns, wrapping cells and padding with spaces.

### Lists (`<ul>`, `<ol>`, `<li>`)

| List type | Output |
|-----------|--------|
| Unordered (`<ul>`) | `BlockType::UList` — prefixed with `* ` per item |
| Ordered (`<ol>`) | `BlockType::OList` — prefixed with `N. ` (right-aligned, zero-padded to max item count width) |
| List item (`<li>`) | Inherits prefix from parent list |

The item number width is calculated based on the total item count in the list. For example, in a 12-item ordered list, item numbers are ` 1.`, ` 2.`, ..., `12.` (3 characters wide, right-aligned).

List continuation lines (when content wraps) use a `next_line_prefix` of spaces equal to the item number width + 2. For unordered lists, continuation uses two spaces.

### Blockquotes (`<blockquote>`)

`BlockType::Quote` uses `"> "` as both `first_line_prefix` and `next_line_prefix`. Nested blockquotes within blockquotes would each add their own `"> "` prefix via the recursive block structure. No `| ` or other markers are used.

### Emphasis (`<b>`, `<strong>`, `<em>`, `<i>`, `<s>`, `<u>`, `<mark>`, `<del>`, `<ins>`)

| Element | Transformation |
|---------|---------------|
| `<b>`, `<strong>` | Wrapped in `*`: `*bold*` |
| `<em>`, `<i>` | Wrapped in `/`: `/italic/` (via `formatted_element!("/{}/")` macro) |
| `<s>`, `<del>` | Unicode strikethrough: each grapheme followed by U+0336 |
| `<u>` | Unicode underline: each grapheme followed by U+0332 |
| `<mark>` | Wrapped in `>` and `<`: `>text<` |
| `<ins>` | No transformation (plain) |
| `<cite>`, `<dfn>` | Wrapped in `/`: `/text/` |
| `<q>` | Wrapped in curly quotes: `"text"` |
| `<abbr>` | Expanded on first use: `"Text (full form)"` |

All emphasis uses **ASCII punctuation** for delimiters (asterisks, slashes, angle brackets), not Unicode box-drawing or other special characters. The output is pure plain text.

### Whitespace Preservation

- **`preserve_whitespace: true`** is set for `<pre>` elements and when explicitly configured.
- In `VNodeType::from_text` (line 661-682): if `preserve_whitespace` is false, **all internal whitespace sequences are collapsed to a single space** via `WHITESPACE_AFFIX` regex (`\s+` → ` `).
- Newlines at the very start/end of text are stripped via `NEWLINE_EDGES` regex.

### Ignored / Blank Elements

These elements produce **no output**:
- Metadata: `<base>`, `<head>`, `<link>`, `<meta>`, `<title>`, `<style>`
- Media/Embedded: `<area>`, `<col>`, `<colgroup>`, `<map>`, `<source>`, `<track>`, `<iframe>`
- Scripting: `<canvas>`, `<noscript>`, `<script>`
- Forms (partial): `<button>`, `<datalist>`, `<fieldset>`, `<form>`, `<legend>`, `<optgroup>`, `<option>`, `<output>`, `<progress>`, `<textarea>` (some are block, some blank)
- Interactive: `<menu>`, `<menuitem>`, `<slot>`, `<template>`
- Replaced inputs: `<input>` (rendered as `[alt text]` or `[]`), `<select>` (same)

### Unsupported / Partially Supported

- `<bdo>`, `<sup>`, `<sub>`: Declared as TODO, currently treated as plain inline.
- `<ruby>` and related: Not supported; falls back to content-only.
- CSS: Not supported at all.

---

## Output Format Details

### Plain Text Only

August outputs **pure plain text** with no Markdown, no HTML, no structured markup. The examples in the source show emphasis using `*bold*` or `/italic/` but these are just punctuation-based conventions from the styled text — not a Markdown output mode.

### Styling in Output

Text styling is applied inline using Unicode combining characters:
- **Strikethrough:** U+0336 (COMBINING LONG STRIKEOVER) after each grapheme
- **Underline:** U+0332 (COMBINING LOW LINE) after each grapheme
- **Uppercase:** Applied via Rust's `to_uppercase()` on the text content

No ANSI color codes, HTML tags, or other markup is present.

### Links

Web links are appended in parentheses: `Click here (https://example.com)`. Email links show angle brackets: `John Doe <john@example.com>`. No markdown-style `[text][ref]` or `[text](url)` patterns are used.

### Horizontal Rules

`<hr>` renders as a line of `-` characters. With width constraint, it fills the full available width. Without width, it defaults to `"---"`.

### Tables

Rendered with two-space column separators. No box-drawing characters or border lines. Example:

```
Header 1  Header 2
Cell 1    Cell 2
Long cell content here  Next column
```

Cells are space-padded to align columns. If a cell wraps, subsequent lines of that row are space-padded to maintain alignment.

---

## Email-Specific Handling

August is explicitly designed for email HTML conversion:

1. **Table-heavy layout support:** Email clients notoriously use tables for layout. August has sophisticated table rendering with column width balancing and cell wrapping.
2. **No Markdown output:** Email clients generally display plain text, and August respects this by outputting plain text directly.
3. **mailto: link support:** Email addresses in links are handled specially with the `<Display Name> <email>` convention.
4. **Whitespace normalization:** Prevents the "textlikethis" problem that occurs when inline elements have no space between them at element boundaries.
5. **Block-level separation:** Paragraphs and block elements are clearly separated with blank lines, giving email text a readable structure.

---

## Edge Cases

### Empty content
Empty text nodes produce no output. Empty block elements (e.g., empty `<p>`) produce no blank lines unless text content triggers one.

### Whitespace-only text
Whitespace is collapsed to a single space in normal text. `EdgeState::Blank` vs `EdgeState::White` tracking determines whether a space is inserted between adjacent elements.

### Nested blocks
Nested block elements render recursively. For example, a `<li>` containing a `<blockquote>` produces the list prefix, then the blockquote prefix on each line.

### Zero width
Not tested — passing `0` as width would likely cause `textwrap::Wrapper::new(0)` to create a wrapper that wraps at width 0, producing every character on its own line.

### Very long lines
Without width constraint (`usize::MAX`), lines are unlimited. The `textwrap` library handles wrapping when width is set.

### Table colspan
When a cell has `colspan="N"`, it receives the combined width of N columns. The content is then wrapped within that combined space.

### Table rowspan
**Not supported.** August does not implement rowspan. Cells with rowspan will be treated as regular cells. In multi-row tables, this can cause misalignment.

### Links with no text
If an `<a>` tag has an `href` but no visible text, the output will be the URL (for http/https links) or empty (for other links).

### Abbreviation deduplication
The `abbr_element` function maintains a `doc_state` `HashSet<String>` to track which abbreviations have been expanded. On first occurrence of an abbreviation, it outputs `Text (full form)`. On subsequent occurrences, it outputs only the short form. This is per-document.

---

## Benchmark Configuration

In this benchmark, `august` is configured via:

```rust
// extractor_config.rs line 616-619
"august" => ExtractorConfig {
    augus_max_width: usize::MAX,  // effectively disabled
    ..Default::default()
},
```

The default in `ExtractorStates::default()` sets `augus_max_width: usize::MAX`. The config key in the web interface accepts either `max_width` or `augus_max_width` (line 389-394 of `web.rs`).

The call site in `scores.rs` (line 342):
```rust
august::convert(html, cfg)  // cfg = states.states.get("august").map(|s| s.config.augus_max_width).unwrap_or(usize::MAX)
```

So by default, **no wrapping occurs** — lines are as long as the content requires.

---

## Sample Input/Output

### Input HTML

```html
<!DOCTYPE html>
<html>
<body>
<h1>Welcome to the Newsletter</h1>

<p>This is a <strong>very important</strong> message about <em>weekly updates</em>.</p>

<p>Check out our <a href="https://example.com/blog/post">latest blog post</a> or contact us at <a href="mailto:newsletter@example.com">newsletter@example.com</a>.</p>

<h2>Topics This Week</h2>

<ul>
<li>First topic with a long description that might need to wrap at some point</li>
<li>Second <code>code sample</code> item</li>
<li>Third item</li>
</ul>

<blockquote>
<p>This is a blockquote that explains something important.</p>
</blockquote>

<hr>

<table border="1">
<tr><th>Product</th><th>Price</th></tr>
<tr><td>Widget A</td><td>$19.99</td></tr>
<tr><td>Widget B with a longer name</td><td>$29.99</td></tr>
</table>

<p><img src="photo.jpg" alt="Team photo"></p>

<p>Press <kbd>Ctrl</kbd> + <kbd>C</kbd> to copy.</p>
</body>
</html>
```

### Output with max_width = 80

```
WELCOME TO THE NEWSLETTER

This is a *very important* message about /weekly updates/.

Check out our latest blog post (https://example.com/blog/post) or contact us at
newsletter@example.com.

TOPICS THIS WEEK

* First topic with a long description that might need to wrap at some
  point
* Second `code sample` item
* Third item

>
> This is a blockquote that explains something important.
>

--------------------------------------------------------------------------------

Product    Price
Widget A   $19.99
Widget B   $29.99
with a
longer
name

Team photo

Press CTRL + C to copy.
```

### Effect of max_width

| max_width | Behavior |
|-----------|----------|
| `usize::MAX` | No wrapping. Lines are as long as content. |
| `80` | Lines wrap at 80 characters, preserving word boundaries. List continuation indented by 2 spaces. Blockquote lines prefixed with `> `. |
| `40` | Aggressive wrapping at 40 columns. Table columns compressed. |
| `0` | Would wrap every character on its own line (textwrap behavior with width=0). |

The `width` parameter primarily affects:
1. **Text block wrapping** via `textwrap::Wrapper`
2. **HR element width** (`-`.repeat(width))
3. **Table column layout** (columns balanced to fit within width)
4. **List/blockquote continuation indentation** (prefix length subtracted from effective content width)

---

## Dependencies

From `Cargo.lock` (august v2.4.0):
- `argparse ^0.2.2` — command-line argument parsing
- `html5ever ^0.24.1` — HTML parsing
- `itertools ^0.8.1` — iterator utilities
- `lazy_static ^1.4.0` — static regex compilation
- `num-rational ^0.2.2` — fractional width calculations for tables
- `num-traits ^0.2.8` — numeric trait definitions
- `regex ^1.3.1` — whitespace/edge regexes
- `term_size ^0.3.1` *(optional)* — terminal size detection
- `textwrap ^0.11.0` — line wrapping
- `unicode-segmentation ^1.3.0` — grapheme-aware width counting

---

## Summary

| Aspect | August behavior |
|--------|----------------|
| **Output format** | Plain text only, no Markdown |
| **max_width controls** | Line wrapping, HR width, table column balancing |
| **Headings** | UPPERCASED, no prefix markers |
| **Links** | `"Text (url)"` for web, `"Name <email>"` for mailto |
| **Images** | Alt text only |
| **Code** | Backtick-wrapped: `` `code` `` |
| **Preformatted** | Whitespace preserved, no fences |
| **Tables** | Two-space column separators, space-padded cells, proportional column widths |
| **Lists** | `* ` and `N. ` with indented continuations |
| **Blockquotes** | `> ` prefixed lines |
| **Emphasis** | `*bold*`, `/italic/`, strikethrough/underline via Unicode combining chars |
| **Email focus** | Table layout support, mailto handling, whitespace normalization |
| **CSS support** | None |
| **Round-trip** | Not possible, by design |
