# htmd (HTML to Markdown) Extractor

## Overview

| Property | Value |
|----------|-------|
| **Crate** | [`htmd`](https://crates.io/crates/htmd) v0.5.4 |
| **Repository** | https://github.com/letmutex/htmd |
| **License** | Apache-2.0 |
| **Language** | Rust |
| **Primary Dependency** | [html5ever](https://github.com/servo/html5ever) |
| **Inspiration** | [turndown.js](https://github.com/mixmark-io/turndown) |
| **Performance** | ~16ms for 1.37MB Wikipedia page (Apple M4) |
| **Python Binding** | [htmd](https://github.com/lmmx/htmd) by @lmmx |
| **Elixir Binding** | [htmd](https://github.com/kasvith/htmd) by @kasvith |

htmd is a Rust-native HTML-to-Markdown converter directly inspired by turndown.js. It aims for feature parity with turndown.js and passes all of turndown.js's test cases. The design philosophy centers on a **rule-based handler system** where each HTML tag (or group of tags) is processed by a dedicated handler. It offers two translation modes: **Pure** (always convert to Markdown, accepting information loss) and **Faithful** (preserve HTML when Markdown cannot represent the structure accurately).

---

## Core Architecture

### DOM Walking

htmd uses html5ever to parse HTML into an RcDom tree (reference-counted DOM). The DOM walker (`dom_walker.rs`) traverses the tree and dispatches each node to the appropriate element handler. Key behaviors:

1. **Text nodes**: Text is escaped and compressed (multiple whitespace -> single space) unless inside `<pre>` or `<code>`.
2. **Element nodes**: Dispatched to `handlers.handle()` which looks up the tag name in `tag_to_handler_indices` and calls the matching handler.
3. **Adjacent text combining**: The walker combines adjacent inline text nodes with identical formatting tags to prevent unwanted whitespace breaks.
4. **Markdown flag**: Each handler returns a `HandlerResult { content, markdown_translated }`. The `markdown_translated` flag tracks whether this node was fully converted to Markdown or fell back to HTML serialization (in Faithful mode).

### Handler System

The `ElementHandlers` struct maintains:
- A `Vec<Box<dyn ElementHandler>>` of all registered handlers
- A `HashMap<String, Vec<usize>>` mapping tag names to handler indices
- A shared `Options` struct

Handlers are registered in `ElementHandlers::new()`:

```
img           -> img_handler
a             -> AnchorElementHandler
ol, ul        -> list_handler
li            -> list_item_handler
blockquote    -> blockquote_handler
code          -> code_handler
strong, b     -> bold_handler
i, em         -> italic_handler
h1-h6         -> headings_handler
br            -> br_handler
hr            -> hr_handler
table         -> table_handler
td, th        -> td_th_handler
tr            -> tr_handler
tbody         -> tbody_handler
thead         -> thead_handler
caption       -> caption_handler
p             -> p_handler
pre           -> pre_handler
head, body    -> head_body_handler
html          -> html_handler
span          -> span_handler
[30+ block tags] -> block_handler
```

The `Handlers` trait provides handlers access to:
- `fallback(element)` - skip current handler and try previous one
- `handle(node)` - process a node through all handlers
- `walk_children(node)` - process children of a node
- `options()` - access conversion options

### Options

All options are in `htmd::options::Options`:

```rust
pub struct Options {
    pub heading_style: HeadingStyle,           // default: Atx
    pub hr_style: HrStyle,                   // default: Asterisks
    pub br_style: BrStyle,                   // default: TwoSpaces
    pub link_style: LinkStyle,               // default: Inlined
    pub link_reference_style: LinkReferenceStyle, // default: Full
    pub code_block_style: CodeBlockStyle,   // default: Fenced
    pub code_block_fence: CodeBlockFence,   // default: Backticks
    pub bullet_list_marker: BulletListMarker, // default: Asterisk
    pub ul_bullet_spacing: u8,               // default: 3
    pub ol_number_spacing: u8,               // default: 2
    pub preformatted_code: bool,             // default: false
    pub translation_mode: TranslationMode,   // default: Pure
}
```

---

## HTML-to-Markdown Tag Mapping

### Headings (h1-h6)

**ATX style** (default): Hash prefixes with space:
```markdown
# H1
## H2
### H3
#### H4
##### H5
###### H6
```

**Setex style** (for h1/h2 only): Underline with `=` or `-`:
```markdown
H1
======

H2
------
```

Setex is only applied to h1 and h2. h3-h6 always use ATX regardless of the `heading_style` setting. The heading handler extracts the level from the tag name (`h1` -> level 1 via `tag.chars().nth(1)`), processes children, and wraps with the appropriate markers.

### Paragraphs (p)

```html
<p>This is a paragraph.</p>
```
Output:
```markdown
This is a paragraph.
```

Paragraphs get `\n\n` padding. Child text is walked, trimmed of surrounding newlines, and wrapped with double newlines. Empty paragraphs produce no output.

### Links (a)

Three link styles:

**Inlined** (`link_style: Inlined`):
```markdown
[link text](https://example.com "optional title")
```

**Inlined Prefer Autolinks** (`link_style: InlinedPreferAutolinks`): If link text exactly equals the URL, produces an autolink `<https://example.com>` instead.

**Referenced** (`link_style: Referenced`): Link definitions are collected and output at the end. Three reference styles:
- **Full**: `[text][1]` ... `[1]: url "title"`
- **Collapsed**: `[text][]` ... `[text]: url "title"`
- **Shortcut**: `[text]` ... `[text]: url "title"`

The `AnchorElementHandler` stores link definitions in a thread-local `Vec<String>` and appends them via the `append()` method after all content is processed.

**Title processing**: Newlines in titles are sanitized and `"` is escaped as `\"`.

**URL escaping**: Parentheses in URLs are escaped as `\(`, `\)` to prevent Markdown link syntax breakage. If the URL contains spaces, it's wrapped in `<>` angle brackets.

### Images (img)

```html
<img src="https://example.com/image.png" alt="Alt text" title="Image title">
```
Output:
```markdown
![Alt text](<https://example.com/image.png> "Image title")
```

If the URL contains spaces, it's wrapped in angle brackets. Alt text has newlines replaced with spaces, and `"` is escaped as `\"` within alt and title.

### Emphasis (strong/b, i/em, span)

- `<strong>` and `<b>` -> `**bold**`
- `<i>` and `<em>` -> `*italic*`

The `emphasis_handler` extracts leading/trailing whitespace separately from the content so that `**text**` doesn't break across line breaks improperly. It uses `strip_leading_whitespace` and `strip_trailing_whitespace` which operate on inline whitespace only (not document whitespace like newlines).

### Inline Code (code)

Smart backtick selection:
- `<code>text</code>` -> `` `text` ``
- `<code>`starting with backtick`` -> ``` `` `starting with backtick `` ``` (double backticks with spaces)
- `<code>literal backtick (`) here</code>` -> ``` `` literal backtick (`) here `` ``` (double backticks required)

The handler scans for single backticks in the content and switches to double-backtick delimiters if needed. Leading/trailing backticks trigger space padding.

### Code Blocks (pre/code)

**Fenced** (default, `code_block_style: Fenced`):
```markdown
```rust
fn main() {
    println!("hello");
}
```
```

**Indented** (`code_block_style: Indented`):
```markdown
    fn main() {
        println!("hello");
    }
```

For fenced blocks, the fence marker length is dynamically determined: if content contains triple backticks ```` ``` ````, it upgrades to 4 backticks, and if that also conflicts, to 5. The fence uses backticks by default or tildes if `code_block_fence: Tildes`.

**Language detection**: The handler looks for `class="language-rust"` or `class="lang-rust"` on either the `<code>` or `<pre>` element. The class value after `language-` or `lang-` is extracted as the language identifier.

**`<pre><code>` handling**: In Faithful mode, a simple `<pre><code>code</code></pre>` sequence is recognized and converted to a Markdown fenced code block. If the structure is more complex, it falls back to HTML serialization.

### Tables

HTML tables are converted to Markdown pipe tables:

```html
<table>
  <thead>
    <tr><th>Language</th><th>Type</th><th>Year</th></tr>
  </thead>
  <tbody>
    <tr><td>Rust</td><td>Systems</td><td>2010</td></tr>
    <tr><td>Python</td><td>Interpreted</td><td>1991</td></tr>
  </tbody>
</table>
```
Output:
```markdown
| Language | Type        | Year |
| -------- | ----------- | ---- |
| Rust     | Systems     | 2010 |
| Python   | Interpreted | 1991 |
```

**Table structure requirements**:
- If `translation_mode == Faithful`, all children must be Markdown-translatable or the whole table falls back to HTML
- If no `<thead>` with `<th>` cells is found, the first `<tr>` with `<th>` cells becomes the header
- If no headers are found at all, the table falls back to walking children as block content
- Column widths are computed from content (max character count per column)
- The separator row uses `---` padded to match column widths
- Captions (from `<caption>`) are prepended as plain text lines above the table

### Lists (ul, ol)

**Unordered lists** (`bullet_list_marker: Asterisk` or `Dash`):
```markdown
* Item 1
* Item 2
```
or with Dash:
```markdown
- Item 1
- Item 2
```

**Ordered lists** (`ol_number_spacing: 2`):
```markdown
1.  First item
2.  Second item
3.  Third item
```

The number spacing is computed as `" ".repeat(ol_number_spacing + digits(highest_index) - index_str.len())`. For `ol_number_spacing: 2` with 3 items: `2 + 1 - 1 = 2` spaces for item 1, `2 + 1 - 1 = 2` for item 2, `2 + 1 - 1 = 2` for item 3.

The `start` attribute on `<ol>` is respected: `<ol start="5">` produces items numbered 5, 6, 7, etc.

**Nested lists**: If a list is inside an `<li>`, the outer handler joins the nested content without extra `\n\n` padding, instead using single `\n`.

**Whitespace handling**: `ul_bullet_spacing: 3` means 3 spaces between the bullet and content. Content is indented on all lines except the first via `indent_text_except_first_line()`.

### Blockquotes (blockquote)

```html
<blockquote>Some quoted text</blockquote>
```
Output:
```markdown
> Some quoted text
```

Each line gets `> ` prefix. Content is trimmed of leading/trailing document whitespace but internal lines are preserved. Multi-line quotes:
```markdown
> This is a multi-line
> quote that spans two lines
```

### Horizontal Rules (hr)

Three styles:
- `HrStyle::Asterisks`: `* * *`
- `HrStyle::Dashes`: `- - -`
- `HrStyle::Underscores`: `_ _ _`

All surrounded by `\n\n` padding.

### Line Breaks (br)

- `BrStyle::TwoSpaces` (default): `  \n` (two trailing spaces + newline)
- `BrStyle::Backslash`: `\\\n`

### Block Handler (fallback for unhandled tags)

For tags without a dedicated handler, `block_handler` wraps child content with `\n\n` padding. In Faithful mode, these fall back to HTML serialization.

---

## Skip Tags Mechanism

htmd's `skip_tags` works by **adding handlers that return `None`**, which means the element and ALL its descendants are completely removed from the output. This differs from `ignore_tags` in html2md-rs.

```rust
// In builder:
builder = builder.skip_tags(skip_tags.iter().map(|s| s.as_str()).collect());

// Internally adds a handler:
// pub fn skip_tags(self, tags: Vec<&str>) -> Self {
//     self.add_handler(tags, |_: &dyn Handlers, _: Element| None)
// }
```

Since the handler returns `None` for both the skipped tag AND its children are never walked, the entire subtree is pruned. Default skip tags in this project:

```rust
const DEFAULT_SKIP_TAGS: &[&str] = &[
    "nav", "script", "style", "header", "footer", "img", "svg", "iframe",
];
```

These are registered both as global `skip_tags` AND as `htmd.skip_tags`. Note that `svg` and `iframe` are skipped via skip_tags but `img` requires special consideration since it has a dedicated handler - the skip_tags mechanism overrides the img_handler by registering a `None` handler that runs first.

---

## Translation Modes: Pure vs Faithful

### Pure Mode (default)

Always convert HTML to Markdown, even when that means losing information (attributes that can't be represented in Markdown, etc.).

### Faithful Mode

Preserve the original HTML by embedding HTML tags when Markdown cannot faithfully represent the structure. This involves:

1. The `serialize_if_faithful!` macro checks if an element has more than the allowed number of attributes. If so, it serializes the element back to HTML rather than converting to Markdown.

2. Tags like `table`, `ol`, `ul` check if their children can all be Markdown-translated. If not, the whole element falls back to HTML.

3. Comments are preserved as HTML comments `<!-- comment -->`.

4. The `serialize_element()` function converts a DOM node back to its HTML string representation, with special handling for block vs inline elements per the CommonMark spec.

Example of the difference:

**Pure mode** for a table with a `bgcolor` attribute:
```markdown
| Header |
| ------ |
| Cell   |
```
(The `bgcolor` attribute is silently dropped)

**Faithful mode** for the same:
```html
<table bgcolor="red">
  <thead><tr><th>Header</th></tr></thead>
  <tbody><tr><td>Cell</td></tr></tbody>
</table>
```

---

## Heading Style: ATX vs Setex

### ATX (AsciiDoc-style)

```markdown
## Section Title
```

- Prefixed with 1-6 hash characters matching the heading level
- A space MUST follow the hashes (e.g., `## Title`, not `##Title`)
- Can be closed with trailing hashes matching the prefix count (optional in most parsers)
-Blank line required after

### Setex ( underlined)

```markdown
Section Title
=============
```

- Only valid for h1 (`=====`) and h2 (`-----`)
- Underline characters must be at least as long as the heading text
- No space between text and underline
- h3-h6 do NOT use setex; they always fall back to ATX even if `heading_style: Setex` is configured

**htmd implementation** (from `headings.rs`):
```rust
if (level == 1 || level == 2) && handlers.options().heading_style == HeadingStyle::Setex {
    // Setext style
    result.push_str(content);
    result.push('\n');
    let ch = if level == 1 { "=" } else { "-" };
    result.push_str(&ch.repeat(content.chars().count()));
} else {
    // ATX style
    result.push_str(&"#".repeat(level as usize));
    result.push(' ');
    result.push_str(content);
}
```

---

## Custom Tag Handlers

htmd allows adding custom handlers via the builder:

```rust
use htmd::{Element, HtmlToMarkdown, element_handler::Handlers};

let converter = HtmlToMarkdown::builder()
    .add_handler(vec!["svg"], |_handlers: &dyn Handlers, _: Element| {
        Some("[Svg Image]".into())
    })
    .build();
assert_eq!("[Svg Image]", converter.convert("<svg></svg>").unwrap());
```

The handler function receives the `Handlers` trait object (for delegating to child processing) and the `Element` struct with:
- `node: &'a Rc<Node>` - the html5ever node
- `tag: &'a str` - tag name
- `attrs: &'a [Attribute]` - attributes
- `markdown_translated: bool` - whether children were Markdown-translated
- `skipped_handlers: usize` - number of handlers skipped (for fallback)

Handlers can return `None` to skip processing (and defer to fallback if available), or `Some(HandlerResult)`.

---

## Edge Cases

### Malformed HTML

html5ever is a standards-compliant HTML5 parser. Malformed HTML is parsed per the HTML5 spec's error-correction rules. Unclosed tags are handled, nested tags may be corrected, etc.

### Scripts and Styles

Both `<script>` and `<style>` are in the default skip_tags list. Their content is never processed - the tags and all descendants are removed entirely. If these tags are NOT in skip_tags, their content is still NOT rendered as Markdown (since they are in the block_handler list which wraps content with `\n\n` padding but the text content would be treated as plain text).

### Deeply Nested Structures

The DOM walker handles arbitrary nesting depth. The `walk_node` and `walk_children` functions are recursive but use reference counting (Rc<Node>), so memory is managed automatically. Block elements at any nesting level trigger appropriate trimming behavior.

### Whitespace Handling

Three whitespace concepts in htmd:

1. **Inline whitespace** (per CommonMark spec): space, tab, newline/carriage return - used for emphasis markers
2. **Document whitespace**: tab, newline, carriage return, space - used for trimming document boundaries
3. **Pre-formatted** (`<pre>`, `<code>` inside `<pre>`): whitespace is preserved as-is except for escape sequences

The `text_util.rs` provides:
- `trim_document_whitespace()` - trims document whitespace from ends
- `compress_whitespace()` - collapses runs of whitespace to single spaces (but preserves newlines as spaces)
- `indent_text_except_first_line()` - indents all lines except the first

### HTML Entities

HTML entities in text (e.g., `&lt;`, `&gt;`, `&amp;`) are decoded by html5ever during parsing, so they appear as their Unicode characters in the Markdown output. The `html_escape.rs` module handles escaping of Markdown special characters.

### Plain Text Detection

`is_plain_text()` in `dom_walker.rs` returns `false` for text containing:
- Markdown special characters: `\`, `*`, `_`, `` ` ``, `[`, `]`
- Whitespace sequences (multiple spaces)
- Markdown heading-like patterns (`=`、`~`、`>`、`-` followed by more text on the same line)

This is used to prevent double-escaping of text that should be treated as raw.

---

## Performance Characteristics

| Metric | Value |
|--------|-------|
| Parse + Convert | ~16ms for 1.37MB Wikipedia page (Apple M4) |
| Memory | Single-threaded per conversion; shared converter is thread-safe |
| Dependencies | Minimal: only html5ever + markup5ever_rcdom |
| Thread Safety | `HtmlToMarkdown` is Send + Sync when only using built-in handlers |

**Performance design choices**:
- PHF (perfect hash function) set for block element lookup - O(1), no hash collision
- Thread-local storage for link references (avounces a mutex)
- `concat_strings!` macro pre-allocates String capacity
- Adjacent text node combining reduces string allocations during tree walking
- Reference-counted DOM avoids memory allocation overhead

---

## Comparison with html2md-rs

| Aspect | htmd | html2md-rs |
|--------|------|------------|
| **Inspiration** | turndown.js | ruby-undead/html2md |
| **Parser** | html5ever | lol_html (C crate via bindings) |
| **Skip Tags** | Handler returns `None`; removes tag AND children | `ignore_tags`: removes element but processes children |
| **Tables** | Proper pipe table with column width alignment | Simple pipe table |
| **Custom Handlers** | Yes, via builder | No |
| **Link Reference** | Yes (Full, Collapsed, Shortcut) | No |
| **Translation Modes** | Pure + Faithful | No (always Pure) |
| **Code Fences** | Backticks or tildes, dynamic length | Backticks only, 3-char minimum |
| **Performance** | ~16ms/1.37MB | Unknown |
| **Heading Styles** | ATX + Setex | ATX only |
| **Code Block Style** | Fenced + Indented | Fenced only |
| **Faithful Mode** | Yes (preserves HTML for unsupported constructs) | No |

**Key philosophical difference**: html2md-rs uses the lol_html rewriter API which processes HTML as a byte stream with handler callbacks. htmd builds a full DOM tree and uses a handler-per-tag approach. This makes htmd more flexible (custom handlers, Faithful mode) but potentially more memory-intensive for very large documents.

---

## Default Configuration (in this project)

From `extractor_config.rs`:

```rust
"htmd" => ExtractorConfig {
    skip_tags: ["nav", "script", "style", "header", "footer", "img", "svg", "iframe"],
    htmd: HtmdConfig {
        skip_tags: ["nav", "script", "style", "header", "footer", "img", "svg", "iframe"],
        heading_style: "atx",
        hr_style: "asterisks",
        br_style: "two_spaces",
        link_style: "inlined",
        link_reference_style: "full",
        code_block_style: "fenced",
        code_block_fence: "backticks",
        bullet_list_marker: "*",
        ul_bullet_spacing: 3,
        ol_number_spacing: 2,
        preformatted_code: false,
        translation_mode: "pure",
    },
    ..
}
```

---

## Sample Input/Output

### Input HTML

```html
<!DOCTYPE html>
<html>
<head>
    <title>Sample Document</title>
    <style>body { font: sans-serif; }</style>
</head>
<body>
    <nav>This is navigation, should be skipped.</nav>

    <h1>Main Heading (ATX)</h1>

    <p>This is a <strong>paragraph</strong> with <em>emphasis</em> and
    inline <code>code</code> samples.</p>

    <h2>Section with Setex Heading</h2>

    <blockquote>
        <p>A wise quote about programming.</p>
        <a href="https://example.com" title="Click for wisdom">Learn more</a>
    </blockquote>

    <h3>Code Example</h3>

    <pre><code class="language-rust">fn main() {
    println!("Hello, world!");
}</code></pre>

    <h4>Unordered List</h4>
    <ul>
        <li>First item</li>
        <li>Second item
            <ol>
                <li>Nested ordered item</li>
                <li>Another nested item</li>
            </ol>
        </li>
        <li>Third item</li>
    </ul>

    <h5>Table Example</h5>
    <table>
        <thead>
            <tr><th>Name</th><th>Role</th></tr>
        </thead>
        <tbody>
            <tr><td>Alice</td><td>Engineer</td></tr>
            <tr><td>Bob</td><td>Designer</td></tr>
        </tbody>
    </table>

    <hr>

    <p>Image example:</p>
    <img src="https://example.com/logo.png" alt="Company Logo" title="Our Logo">

    <p>Autolink example: <a href="https://www.rust-lang.org">https://www.rust-lang.org</a></p>

    <script>console.log("This should be skipped");</script>
</body>
</html>
```

### Expected Output (default options: atx, fenced code, inlined links, pure mode)

```markdown
# Main Heading (ATX)

This is a **paragraph** with *emphasis* and inline `code` samples.

## Section with Setex Heading

> A wise quote about programming.
> [Learn more](https://example.com "Click for wisdom")

### Code Example

```rust
fn main() {
    println!("Hello, world!");
}
```

#### Unordered List

* First item
* Second item
    1.  Nested ordered item
    2.  Another nested item
* Third item

##### Table Example

| Name | Role    |
| ---- | ------- |
| Alice | Engineer |
| Bob   | Designer |

* * *

Image example:

![Company Logo](<https://example.com/logo.png> "Our Logo")

Autolink example: <https://www.rust-lang.org>
```

### Expected Output (setex headings, indented code, referenced links)

```markdown
Main Heading (ATX)
======

This is a **paragraph** with *emphasis* and inline `code` samples.

Section with Setex Heading
------

> A wise quote about programming.
> [Learn more][1]

    fn main() {
        println!("Hello, world!");
    }

#### Unordered List

- First item
- Second item
    1.  Nested ordered item
    2.  Another nested item
- Third item

##### Table Example

| Name | Role    |
| ---- | ------- |
| Alice | Engineer |
| Bob   | Designer |

_ _ _

Image example:

![Company Logo](<https://example.com/logo.png> "Our Logo")

Autolink example: <https://www.rust-lang.org>

[1]: https://example.com "Click for wisdom"
```

---

## Key Source Files

| File | Purpose |
|------|---------|
| `src/lib.rs` | Public API: `convert()`, `HtmlToMarkdown`, builder |
| `src/options.rs` | All configuration enums and the `Options` struct |
| `src/dom_walker.rs` | Tree walker, block element detection, text escaping |
| `src/element_handler/mod.rs` | `ElementHandler` trait, `ElementHandlers`, `Handlers` trait |
| `src/element_handler/handlers.rs` | Builtin handler registration |
| `src/element_handler/headings.rs` | ATX/Setex heading conversion |
| `src/element_handler/anchor.rs` | Link handling with reference collection |
| `src/element_handler/img.rs` | Image to Markdown |
| `src/element_handler/code.rs` | Inline and fenced code blocks |
| `src/element_handler/table.rs` | HTML table to Markdown table |
| `src/element_handler/list.rs` | OL/UL container handling |
| `src/element_handler/li.rs` | List item handling |
| `src/element_handler/blockquote.rs` | Blockquote with `> ` prefix |
| `src/element_handler/pre.rs` | Preformatted code blocks |
| `src/element_handler/emphasis.rs` | Bold/italic with whitespace handling |
| `src/element_handler/hr.rs` | Horizontal rule |
| `src/element_handler/br.rs` | Line breaks |
| `src/element_handler/p.rs` | Paragraph wrapping |
| `src/element_handler/element_util.rs` | `serialize_element()` for Faithful mode |
| `src/text_util.rs` | Text processing utilities |
| `src/html_escape.rs` | HTML entity escaping |
