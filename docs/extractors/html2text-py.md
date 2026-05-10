# html2text-py: In-Depth Analysis

## Overview

**html2text-py** is a Python library that converts HTML into clean, readable plain text formatted as Markdown. Originally written by Aaron Swartz and now maintained by Alir3z4, it parses HTML using Python's built-in `html.parser.HTMLParser` and produces Markdown-compatible output.

**Repository**: https://github.com/Alir3z4/html2text  
**PyPI Package**: https://pypi.org/project/html2text/  
**License**: GPLv3  
**Stars**: ~2.1k

---

## Integration in this Benchmark

The html2text-py extractor is invoked from `/home/jan/git/html-to-text-comparison/src/scores.rs` via the function `run_html2text_py` (line 692):

```rust
fn run_html2text_py(html: &str, cfg: &Html2TextPythonConfig) -> String {
    let tmp = std::env::temp_dir().join(format!("h2t_py_{}.html", uuid::Uuid::new_v4()));
    let _ = std::fs::write(&tmp, html);
    let cfg_json = serde_json::to_string(cfg).unwrap();
    let script = r#"from html2text import HTML2Text; import json, sys; cfg = json.loads(sys.argv[2]); h = HTML2Text(); h.ignore_links = cfg['ignore_links']; h.ignore_images = cfg['ignore_images']; h.ignore_emphasis = cfg['ignore_emphasis']; h.body_width = cfg['body_width']; h.unicode_snob = cfg['unicode_snob']; h.escape_snob = cfg['escape_snob']; h.inline_links = cfg['inline_links']; h.google_doc = cfg['google_doc']; h.dash_unordered_list = cfg['dash_unordered_list']; html = open(sys.argv[1]).read(); print(h.handle(html), end='')"#;
    let out = std::process::Command::new("uv")
        .args(["run", "--", "python3", "-c", script])
        .arg(tmp.to_str().unwrap())
        .arg(cfg_json)
        .output();
    // ... error handling
}
```

The configuration struct from `/home/jan/git/html-to-text-comparison/src/extractor_config.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
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
```

---

## Core Conversion Mechanism

### Parsing Approach

html2text-py uses Python's `html.parser.HTMLParser` (a SAX-style parser) as its base class. The `HTML2Text` class extends this parser and implements:

1. **Feed phase**: HTML is fed to the parser
2. **Handle phase**: The `handle()` method processes the parsed content
3. **Wrap phase**: The `optwrap()` method applies text wrapping based on `body_width`

The conversion works by:
- Overriding `handle_starttag()` and `handle_endtag()` to capture HTML structure
- Overriding `handle_data()`, `handle_charref()`, and `handle_entityref()` to capture text content
- Using an internal `outtextlist` to accumulate output characters
- The `o()` method handles whitespace, indentation, and output formatting

### Output Generation

The final output is generated via:
```python
def finish(self) -> str:
    self.close()
    self.pbr()
    self.o("", force="end")
    outtext = "".join(self.outtextlist)
    # Handle nbsp replacement
    if self.unicode_snob:
        nbsp = html.entities.html5["nbsp;"]
    else:
        nbsp = " "
    outtext = outtext.replace("&nbsp_place_holder;", nbsp)
    return outtext
```

---

## Configuration Options

### ignore_links

**Type**: `bool`  
**Default**: `false`

When `True`, all `<a>` tags are completely ignored during conversion. The link text is preserved but the hyperlink is removed.

```python
h = HTML2Text()
h.ignore_links = True
print(h.handle('<p>Visit <a href="https://example.com">Example</a></p>'))
# Output: Visit Example
```

When `False` (default), links are rendered as Markdown: `[text](url)` or as reference links.

---

### ignore_images

**Type**: `bool`  
**Default**: `false`

When `True`, all `<img>` tags are completely ignored. No alt text or image references appear in output.

```python
h = HTML2Text()
h.ignore_images = True
print(h.handle('<p>Photo: <img src="cat.jpg" alt="A cat"></p>'))
# Output: Photo:
```

When `False`, images are rendered as `![alt](src)` (with optional link in parentheses).

---

### ignore_emphasis

**Type**: `bool`  
**Default**: `false`

When `True`, all emphasis (bold, italic, strikethrough) is stripped from the output. The text content is preserved but without any formatting.

```python
h = HTML2Text()
h.ignore_emphasis = True
print(h.handle('<p>This is <strong>bold</strong> and <em>italic</em></p>'))
# Output: This is bold and italic
```

---

### body_width

**Type**: `int`  
**Default**: `78`

Controls the maximum character width for line wrapping. When set to `0`, no wrapping occurs.

**How wrapping works**:

1. The `optwrap()` method processes the output text line by line
2. Each paragraph (separated by newlines) is wrapped individually
3. Uses Python's `textwrap.wrap()` function with:
   - `width`: body_width value
   - `break_long_words=False` (prevents breaking long words/URLs)
   - `subsequent_indent`: varies by context

**Special handling**:
- Code blocks (triple backticks) are never wrapped
- List item continuations get `"    "` indent (double indent)
- Blockquote continuations get `"> "` prefix
- Links in reference style are skipped when `wrap_links=False`

```python
# Example: body_width=40 vs body_width=80 vs body_width=0
h = HTML2Text()
html = '<p>This is a long paragraph with quite a lot of text that needs to be wrapped at different widths.</p>'

h.body_width = 40
print(h.handle(html))
# Output:
# This is a long paragraph with quite a
# lot of text that needs to be wrapped at
# different widths.

h.body_width = 80
print(h.handle(html))
# Output:
# This is a long paragraph with quite a lot of text that needs to be wrapped at
# different widths.

h.body_width = 0
print(h.handle(html))
# Output:
# This is a long paragraph with quite a lot of text that needs to be wrapped at different widths.
```

---

### unicode_snob

**Type**: `bool`  
**Default**: `false`

When `True`, html2text uses Unicode characters directly instead of ASCII pseudo-replacements.

**What's affected**:
| ASCII Replacement | Unicode Character |
|------------------|-------------------|
| `&nbsp;` → ` ` | `&nbsp;` → actual non-breaking space (U+00A0) |
| `&copy;` → `(C)` | `&copy;` → `©` |
| `&mdash;` → `--` | `&mdash;` → `—` |
| `&ndash;` → `-` | `&ndash;` → `–` |
| `&rsquo;` → `'` | `&rsquo;` → `'` |
| `&rdquo;` → `"` | `&rdquo;` → `"` |
| `&rarr;` → `->` | `&rarr;` → `→` |
| `&larr;` → `<-` | `&larr;` → `←` |
| `&middot;` → `*` | `&middot;` → `·` |

```python
h = HTML2Text()
html = '<p>Copyright &copy; 2024 &mdash; em dash &ndash; en dash</p>'

h.unicode_snob = False
print(h.handle(html))
# Output: Copyright (C) 2024 -- em dash - en dash

h.unicode_snob = True
print(h.handle(html))
# Output: Copyright © 2024 — em dash – en dash
```

---

### escape_snob

**Type**: `bool`  
**Default**: `false`

When `True`, ALL special Markdown characters are escaped with backslashes:
- `` ` `` `` ` `` `` ` `` `` ` `` `_` `*` `#` `+` `-` `.` `!` `[` `]` `(` `)` `{` `}` `` \ ``

This produces less readable output but avoids corner-case formatting issues where Markdown might interpret content incorrectly.

```python
h = HTML2Text()
html = '<p>Code: `var = 1` and **bold** and *italic*</p>'

h.escape_snob = False
print(h.handle(html))
# Output: Code: `var = 1` and **bold** and *italic*

h.escape_snob = True
print(h.handle(html))
# Output: Code: \`var = 1\` and \*\*bold\*\* and \*italic\*
```

---

### inline_links

**Type**: `bool`  
**Default**: `true`

Controls how links and images are rendered in the output:

**When `True` (default)**:
- Links: `[text](url)`
- Images: `![alt](url)`

**When `False`**:
- Links use reference-style: `[text][1]` with definitions at bottom
- Images use reference-style: `![alt][1]`

```python
h = HTML2Text()
html = '<p>Visit <a href="https://example.com">Example</a></p>'

h.inline_links = True
print(h.handle(html))
# Output: Visit [Example](https://example.com)

h.inline_links = False
print(h.handle(html))
# Output: Visit [Example][1]
#
# [1]: https://example.com
```

---

### google_doc

**Type**: `bool`  
**Default**: `false`

When `True`, enables special handling for HTML exported from Google Docs. This addresses specific quirks in Google Docs HTML output:

**What it does**:

1. **CSS-based emphasis handling**: Parses inline styles to detect bold (`font-weight: bold/700/800/900`), italic (`font-style: italic`), and strikethrough (`text-decoration: line-through`) - even when no `<strong>`, `<em>`, or `<del>` tags are present

2. **List nesting detection**: Uses `margin-left` CSS property to determine nesting depth:
   ```python
   nest_count = int(style["margin-left"][:-2]) // self.google_list_indent
   ```
   - Default `google_list_indent = 36` pixels per level
   - Indents nested lists accordingly (two spaces per level)

3. **Height-based paragraph breaks**: Google Docs sometimes uses CSS `height` attributes on `<p>` and `<div>` tags - these are interpreted as paragraph breaks

4. **Strikethrough handling**: When combined with `--hide-strikethrough` option (not exposed in this benchmark), strikethrough text is hidden

```python
h = HTML2Text()
# Google Docs HTML often has inline styles like:
# <span style="font-weight: bold;">Bold text</span>
# <span style="font-style: italic;">Italic text</span>

h.google_doc = True
# Now inline styles are parsed for emphasis
```

---

### dash_unordered_list

**Type**: `bool`  
**Default**: `false`

When `True`, unordered list items use `-` (dash) instead of `*` (asterisk) as the bullet marker.

```python
h = HTML2Text()
html = '<ul><li>Item 1</li><li>Item 2</li></ul>'

h.dash_unordered_list = False
print(h.handle(html))
# Output:
# * Item 1
# * Item 2

h.dash_unordered_list = True
print(h.handle(html))
# Output:
# - Item 1
# - Item 2
```

---

## HTML Element Handling

### Headings (`<h1>` through `<h6>`)

Converted to ATX-style Markdown with `#` through `######`:

```python
h = HTML2Text()
html = '<h1>Title</h1><h2>Subtitle</h2><h3>Section</h3>'
print(h.handle(html))
# Output:
# Title
# ======
#
# Subtitle
# --------
#
# ### Section
```

**Special cases**:
- Inside links, headings become Setext-style underlines instead of ATX:
  ```html
  <a href="#"><h1>Linked Heading</h1></a>
  ```
  Becomes:
  ```markdown
  [Linked Heading
  ========
  ](url)
  ```

- When heading is inside an anchor tag, `#` is used within the link text

### Links (`<a>`)

**Default behavior** (inline_links=True):
```python
print(h.handle('<a href="https://example.com">Example</a>'))
# Output: [Example](https://example.com)
```

**Reference-style** (inline_links=False):
```python
print(h.handle('<a href="https://example.com">Example</a>'))
# Output: [Example][1]
#
# [1]: https://example.com
```

**Special behaviors**:
- Internal links (href="#anchor") are skipped by default (controlled by `skip_internal_links`)
- Links with same href and text become automatic links:
  ```python
  print(h.handle('<a href="https://example.com">https://example.com</a>'))
  # Output: <https://example.com>
  ```
- Title attributes become `"title"` after URL:
  ```python
  print(h.handle('<a href="url" title="Optional Title">Link</a>'))
  # Output: [Link](url "Optional Title")
  ```
- Links can be protected from line breaks with `protect_links` (adds `<` and `>`)

### Images (`<img>`)

| Option | Output Format |
|--------|---------------|
| Default | `![alt](src)` |
| `images_as_html=True` | `<img src='...' width='...' height='...' alt='...'/>` |
| `images_to_alt=True` | Just the alt text (no image markup) |
| `images_with_size=True` | Same as images_as_html when width/height present |

```python
h = HTML2Text()
print(h.handle('<img src="photo.jpg" alt="A sunset">'))
# Output: ![A sunset](photo.jpg)

h.images_to_alt = True
print(h.handle('<img src="photo.jpg" alt="A sunset">'))
# Output: A sunset
```

### Code Blocks and Inline Code

**`<pre>` blocks**:
- Indented with 4 spaces (standard Markdown)
- Optionally wrapped with `[code]...[/code]` when `mark_code=True`
- When `backquote_code_style=True`, uses triple backticks: ```code```

```python
h = HTML2Text()
html = '<pre>def hello():\n    print("world")</pre>'
print(h.handle(html))
# Output:
#
#     def hello():
#         print("world")
```

**`<code>`, `<kbd>`, `<tt>` inline**:
- Wrapped with backticks: `` `code` ``

```python
print(h.handle('<p>Use <code>ls -la</code> to list files</p>'))
# Output: Use `ls -la` to list files
```

**Edge case**: Empty emphasis inside code gets dropped to avoid broken Markdown.

### Tables

Tables can be rendered in multiple ways:

1. **Markdown format** (default):
   ```python
   h = HTML2Text()
   html = '<table><tr><th>Name</th><th>Age</th></tr><tr><td>Alice</td><td>30</td></tr></table>'
   print(h.handle(html))
   # Output:
   # | Name | Age |
   # | --- | --- |
   # | Alice | 30 |
   ```

2. **Bypass tables** (bypass_tables=True): Output raw HTML
   ```python
   h = HTML2Text()
   h.bypass_tables = True
   print(h.handle('<table><tr><td>Cell</td></tr></table>'))
   # Output: <table><tr><td>Cell</td></tr></table>
   ```

3. **Ignore tables** (ignore_tables=True): Text content only, no structure
   ```python
   h = HTML2Text()
   h.ignore_tables = True
   print(h.handle('<table><tr><td>Text</td></tr></table>'))
   # Output: Text
   ```

### Unordered Lists (`<ul>`)

**Standard behavior**:
```python
html = '<ul><li>Item 1</li><li>Item 2</li></ul>'
print(h.handle(html))
# Output:
# * Item 1
# * Item 2
```

**Nested lists** (two spaces per level):
```python
html = '<ul><li>Top<ul><li>Nested</li></ul></li></ul>'
print(h.handle(html))
# Output:
# * Top
#   * Nested
```

**dash_unordered_list=True**:
```python
h.dash_unordered_list = True
print(h.handle('<ul><li>Item</li></ul>'))
# Output:
# - Item
```

### Ordered Lists (`<ol>`)

**Standard behavior**:
```python
html = '<ol><li>First</li><li>Second</li></ol>'
print(h.handle(html))
# Output:
# 1. First
# 2. Second
```

**start attribute**:
```python
html = '<ol start="5"><li>Fifth</li><li>Sixth</li></ol>'
print(h.handle(html))
# Output:
# 5. Fifth
# 6. Sixth
```

### Blockquotes (`<blockquote>`)

```python
html = '<blockquote><p>Quoted text</p></blockquote>'
print(h.handle(html))
# Output:
# > Quoted text
```

Nested content inside blockquotes maintains the `>` prefix.

### Emphasis (`<strong>`, `<b>`, `<em>`, `<i>`, `<u>`, `<del>`)

| Tag | Default Output | With asterisk_emphasis |
|-----|---------------|------------------------|
| `<strong>`, `<b>` | `**bold**` | `**bold**` |
| `<em>`, `<i>` | `_italic_` | `*italic*` |
| `<del>`, `<s>`, `<strike>` | `~~strikethrough~~` | `~~strikethrough~~` |

```python
print(h.handle('<p><strong>Bold</strong> and <em>italic</em> and <del>strike</del></p>'))
# Output: **Bold** and _italic_ and ~~strike~~
```

**Edge case**: When emphasis follows certain characters, a space is added to ensure Markdown renders correctly:
```python
# Without space: foo_bar_ would render as one word in Markdown
# With space: foo_bar_ renders as "foo" + emphasis + "bar" + emphasis
```

### Horizontal Rules (`<hr>`)

```python
html = '<hr>'
print(h.handle(html))
# Output:
# * * *
```

### Other Elements

**`<q>` (quotations)**: Renders with configurable quotes (default `""`):
```python
h = HTML2Text()
print(h.handle('<q>Quoted text</q>'))
# Output: "Quoted text"

h.open_quote = "'"
h.close_quote = "'"
print(h.handle('<q>Quoted text</q>'))
# Output: 'Quoted text'
```

**`<abbr>`**: Creates abbreviation definitions at end:
```python
html = '<p>The <abbr title="HyperText Markup Language">HTML</abbr> is great.</p>'
print(h.handle(html))
# Output:
# The HTML is great.
#
# *[HTML]: HyperText Markup Language
```

**`<sup>` and `<sub>`**: Only when `include_sup_sub=True`:
```python
h.include_sup_sub = True
print(h.handle('<p>x<sup>2</sup> and H<sub>2</sub>O</p>'))
# Output: x<sup>2</sup> and H<sub>2</sub>O
```

---

## What Gets Stripped

The following are completely removed from output (not rendered):

| Tag/Element | Reason |
|-------------|--------|
| `<head>` | Metadata container |
| `<script>` | JavaScript code |
| `<style>` | CSS styles |
| `<nav>` | Navigation (not explicitly, but often empty in practice) |
| `<iframe>` | Embedded content |
| `<svg>` | Vector graphics |
| `<form>` | Form elements (not rendered as text) |
| `<input>` | Form inputs |
| `<button>` | Interactive elements |
| `<meta>` | Metadata |
| `<link>` | Resource links |

Note: `<nav>`, `<header>`, `<footer>` are NOT explicitly ignored - they depend on content inside them.

---

## Edge Cases and Known Behaviors

### Empty Links

```python
# Empty link text with href
html = '<a href="https://example.com"></a>'
print(h.handle(html))
# Output: (nothing visible, but link recorded for reference style)
```

### Nested Emphasis

```python
html = '<p><strong><em>nested</em></strong></p>'
print(h.handle(html))
# Output: **_nested_**
```

### Consecutive Line Breaks

```python
html = '<p>Line 1</p><p>Line 2</p><p>Line 3</p>'
print(h.handle(html))
# Output:
# Line 1
#
# Line 2
#
# Line 3
```

### Tables Without Headers

```python
html = '<table><tr><td>Cell1</td></tr><tr><td>Cell2</td></tr></table>'
print(h.handle(html))
# Output:
# | Cell1 |
# | --- |
# | Cell2 |
```

### Unclosed Tags

The parser is relatively forgiving and handles unclosed tags.

### Unicode Entities

All named HTML entities are converted to their character equivalents:
```python
html = '&amp; &lt; &gt; &quot;'
print(h.handle(html))
# Output: & < > "
```

---

## Sample HTML Input and Expected Output

### Example 1: Basic HTML with Various Elements

**Input**:
```html
<!DOCTYPE html>
<html>
<head><title>Sample</title></head>
<body>
<h1>Main Heading</h1>
<p>This is a paragraph with <strong>bold</strong> and <em>italic</em> text.</p>
<h2>Section One</h2>
<p>Visit <a href="https://www.example.com">Example Site</a> for more info.</p>
<img src="photo.jpg" alt="A beautiful sunset">
<blockquote>
    <p>This is a quoted passage.</p>
</blockquote>
<pre>def hello():
    print("Hello, World!")</pre>
</body>
</html>
```

**Output (default settings: body_width=78, inline_links=True, ignore_images=False)**:
```
Main Heading
============

This is a paragraph with **bold** and _italic_ text.

Section One
-----------

Visit [Example Site](https://www.example.com) for more info.

![A beautiful sunset](photo.jpg)

> This is a quoted passage.

    def hello():
        print("Hello, World!")
```

### Example 2: Lists at Different body_width Values

**Input**:
```html
<ul>
<li>First item with some additional text that might need wrapping</li>
<li>Second item</li>
<li>Third item with a <a href="http://example.com">link</a> inside</li>
</ul>
<ol>
<li>Ordered first</li>
<li>Ordered second</li>
</ol>
```

**body_width=30**:
```
* First item with some
  additional text that
  might need wrapping
* Second item
* Third item with a
  [link](http://example.com)
  inside

1. Ordered first
2. Ordered second
```

**body_width=0** (no wrapping):
```
* First item with some additional text that might need wrapping
* Second item
* Third item with a [link](http://example.com) inside

1. Ordered first
2. Ordered second
```

### Example 3: Tables at Different body_width Values

**Input**:
```html
<table>
<tr><th>Name</th><th>Description</th></tr>
<tr><td>Alice</td><td>A person who loves programming and long walks on the beach</td></tr>
<tr><td>Bob</td><td>Another developer</td></tr>
</table>
```

**body_width=60**:
```
| Name | Description |
| --- | --- |
| Alice | A person who
| loves programming
| and long walks on
| the beach |
| Bob | Another
| developer |
```

**body_width=0**:
```
| Name | Description |
| --- | --- |
| Alice | A person who loves programming and long walks on the beach |
| Bob | Another developer |
```

### Example 4: Google Docs Style HTML

**Input** (simplified Google Docs export):
```html
<div>
<span style="font-weight: bold;">Bold text without tag</span><br>
<span style="font-style: italic;">Italic text without tag</span><br>
<span style="text-decoration: line-through;">Strikethrough</span>
</div>
<ul>
<li style="margin-left: 36px;">Nested item 1</li>
<li style="margin-left: 72px;">Nested item 2</li>
</ul>
```

**google_doc=False (default)**:
```
Bold text without tag
Italic text without tag
Strikethrough

* Nested item 1
* Nested item 2
```

**google_doc=True**:
```
**Bold text without tag**
_Italic text without tag_
~~Strikethrough~~

* Nested item 1
  * Nested item 2
```

### Example 5: Reference Links vs Inline Links

**Input**:
```html
<p>Check <a href="https://example.com">Example</a>, 
<a href="https://test.com">Test</a>, and 
<a href="https://demo.com">Demo</a> for details.</p>
```

**inline_links=True**:
```
Check [Example](https://example.com), [Test](https://test.com), and [Demo](https://demo.com) for details.
```

**inline_links=False**:
```
Check [Example][1], [Test][2], and [Demo][3] for details.

[1]: https://example.com
[2]: https://test.com
[3]: https://demo.com
```

### Example 6: Unicode vs ASCII

**Input**:
```html
<p>Copyright &copy; 2024 &mdash; em dash &ndash; en dash &nbsp; non-breaking space</p>
<p>Arrows: &larr; and &rarr;</p>
<p>Quotes: &ldquo;smart&rdquo; and &lsquo;single&rsquo;</p>
```

**unicode_snob=False**:
```
Copyright (C) 2024 -- em dash - en dash   non-breaking space
Arrows: <- and ->
Quotes: "smart" and 'single'
```

**unicode_snob=True**:
```
Copyright © 2024 — em dash – en dash   non-breaking space
Arrows: ← and →
Quotes: "smart" and 'single'
```

---

## Summary

html2text-py is a mature, well-maintained HTML-to-Markdown converter that:

- Uses Python's built-in HTMLParser for parsing
- Produces clean Markdown-compatible output
- Offers extensive configuration options for customization
- Handles most common HTML elements with sensible defaults
- Provides special Google Docs HTML handling
- Supports flexible text wrapping with body_width
- Can use Unicode characters or ASCII replacements based on preference

The benchmark uses the default configuration (body_width=78, inline_links=True, unicode_snob=False, escape_snob=False, ignore_links=False, ignore_images=False, ignore_emphasis=False, google_doc=False, dash_unordered_list=False) for the html2text-py extractor, which represents the standard use case for this library.
