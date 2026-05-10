# Turndown Extractor Analysis

## Overview

Turndown is a JavaScript library that converts HTML to Markdown. It is used in this benchmark via a Node.js subprocess that invokes the library with a configuration object passed from Rust.

### Basic Information

| Attribute | Value |
|-----------|-------|
| **Package Name** | turndown |
| **Version** | 7.2.4 |
| **Repository** | https://github.com/mixmark-io/turndown |
| **License** | MIT |
| **Author** | Dom Christie |
| ** npm Downloads** | ~11,100 stars on GitHub |
| **Node Version** | >= 18 |

### Dependencies

The library has a minimal dependency footprint:

- **@mixmark-io/domino** (^2.2.0): A minimal DOM parser for Node.js environments

---

## Architecture

### Design Philosophy

Turndown follows a **rule-based architecture** where each HTML element type is handled by a specific rule object. The library:

1. Parses HTML into a DOM tree (using domino)
2. Traverses the DOM tree recursively
3. For each element, finds the first matching rule
4. Calls the rule's `replacement` function to convert the element to Markdown

### Core Components

```
TurndownService
    |
    +-- Options (configuration)
    |
    +-- Rules (collection of conversion rules)
    |       |
    |       +-- CommonmarkRules (built-in)
    |       +-- Keep rules (optional)
    |       +-- Remove rules (optional)
    |       +-- Special rules (blank, default, keep, remove)
    |
    +-- Methods
            |
            +-- turndown(input) -> Markdown string
            +-- addRule(key, rule)
            +-- keep(filter)
            +-- remove(filter)
            +-- use(plugin)
            +-- escape(text) -> escaped text
```

### Conversion Pipeline

The conversion happens in these stages:

```
1. Input (HTML string or DOM node)
         |
         v
2. RootNode creation (wraps input, provides tree traversal)
         |
         v
3. Recursive processing (process function)
         |    For each node:
         |    - If text node: escape or pass through
         |    - If element: find matching rule, call replacement
         |
         v
4. Post-processing (postProcess function)
         |    - Append reference definitions
         |    - Trim whitespace
         |
         v
5. Output (Markdown string)
```

### Default Configuration

Defined in `src/extractor_config.rs` (lines 17-32):

| Option | Default Value | Description |
|--------|---------------|-------------|
| `heading_style` | `"setext"` | Heading format: `setext` or `atx` |
| `hr` | `"* * *"` | Horizontal rule text |
| `bullet_list_marker` | `"*"` | Unordered list bullet: `-`, `+`, or `*` |
| `code_block_style` | `"indented"` | Code block style: `indented` or `fenced` |
| `fence` | `"`"` | Fence character: `` ` `` or `~` |
| `em_delimiter` | `"_"` | Emphasis delimiter: `_` or `*` |
| `strong_delimiter` | `"**"` | Strong delimiter: `**` or `__` |
| `link_style` | `"inlined"` | Link style: `inlined` or `referenced` |
| `link_reference_style` | `"full"` | Reference style: `full`, `collapsed`, or `shortcut` |
| `preformatted_code` | `false` | Preserve code formatting |

---

## Rule System

### Built-in Rules (Commonmark Rules)

The library includes 17 built-in rules in `src/commonmark-rules.js`:

| Rule | Filter | Description |
|------|--------|-------------|
| `paragraph` | `p` | Paragraphs as blank-line-separated text |
| `lineBreak` | `br` | Line breaks with configurable delimiter |
| `heading` | `h1-h6` | Headings in Setext or ATX style |
| `blockquote` | `blockquote` | Blockquotes with `>` prefix |
| `list` | `ul`, `ol` | Lists wrapped in blank lines |
| `listItem` | `li` | List items with configurable markers |
| `indentedCodeBlock` | function | Indented code blocks (4-space indent) |
| `fencedCodeBlock` | function | Fenced code blocks with ``` or ~~~ |
| `horizontalRule` | `hr` | Horizontal rules |
| `inlineLink` | function | Inlined Markdown links |
| `referenceLink` | function | Reference-style links |
| `emphasis` | `em`, `i` | Italic text |
| `strong` | `strong`, `b` | Bold text |
| `code` | `code` (inline) | Inline code with backticks |
| `image` | `img` | Images with `![]()` syntax |
| (implicit) | `a` | In referencestyle, anchor tags handled by link rules |
| (implicit) | text nodes | Text content with Markdown escaping |

### Rule Object Structure

Each rule is a plain JavaScript object with:

```javascript
{
  filter: 'tagname',          // String: single tag
  // OR
  filter: ['tag1', 'tag2'],  // Array: multiple tags
  // OR
  filter: function(node, options) { return boolean }, // Function: custom logic

  replacement: function(content, node, options) {
    return 'markdown string'  // Required: returns Markdown
  },

  // Optional:
  append: function(options) { return 'string' } // Called at end of conversion
}
```

### Rule Precedence

When Turndown processes an HTML element, it iterates through rules in this order (`src/rules.js`, lines 45-53):

1. **Blank rule** - Elements containing only whitespace
2. **Added rules** - Custom rules via `addRule()`
3. **Commonmark rules** - Built-in rules
4. **Keep rules** - Elements to preserve as HTML
5. **Remove rules** - Elements to strip entirely
6. **Default rule** - Fallback for unrecognized elements

The first rule whose `filter` matches is used.

### Filter Functions

Filters can be:

- **String**: Exact tag name match (case-insensitive)
  ```javascript
  filter: 'p'  // Matches <p> elements
  ```

- **Array**: Any of multiple tag names
  ```javascript
  filter: ['em', 'i']  // Matches <em> or <i>
  ```

- **Function**: Custom matching logic
  ```javascript
  filter: function(node, options) {
    return options.linkStyle === 'inlined' &&
           node.nodeName === 'A' &&
           node.getAttribute('href')
  }
  ```

### Special Rules

Three special rules handle edge cases:

| Rule | Purpose | Default Behavior |
|------|---------|------------------|
| `blankReplacement` | Elements with only whitespace | Returns `\n\n` for blocks, empty string for inline |
| `keepReplacement` | Elements to preserve as HTML | Returns outerHTML with blank line wrappers for blocks |
| `defaultReplacement` | Unrecognized elements | Returns content with blank line wrappers for blocks |

---

## Configuration Options In-Depth

### headingStyle

Controls how headings (`<h1>` through `<h6>`) are rendered.

| Value | h1-h2 | h3-h6 |
|-------|-------|-------|
| `setext` | Underline with `=` or `-` | Underline with `=` or `-` |
| `atx` | `#` to `######` | `#` to `######` |

**Setext style** (default):
```markdown
<h1>Title</h1>  ->  Title
                           ======

<h2>Subtitle</h2> ->  Subtitle
                           ---------
```

**ATX style**:
```markdown
<h1>Title</h1>  ->  # Title

<h3>Section</h3> ->  ### Section
```

The implementation (lines 21-36 in `commonmark-rules.js`):
- Setext only applies to `<h1>` and `<h2>` (hLevel < 3)
- ATX applies to all heading levels
- The underline length matches the content length exactly
- Two blank lines precede headings

### hr

The horizontal rule output (default: `"* * *"`).

Valid values: Any thematic break stringper CommonMark spec:
- `* * *` (default)
- `---`
- `___
- `* * * *`
- `- - -`
- etc.

The implementation uses the value exactly as provided:
```javascript
replacement: function(content, node, options) {
  return '\n\n' + options.hr + '\n\n'
}
```

### bulletListMarker

The marker for unordered list items.

| Value | Example Output |
|-------|---------------|
| `*` (default) | `* Item` |
| `-` | `- Item` |
| `+` | `+ Item` |

The implementation (lines 60-78 in `commonmark-rules.js`):
- Ordered lists (`<ol>`) always use `1.`, `2.`, etc. with two spaces after the number
- Unordered lists use the configured marker plus three spaces for indentation
- The `start` attribute on `<ol>` is respected for numbering

### codeBlockStyle

Controls how `<pre><code>` blocks are rendered.

| Value | Output Style |
|-------|-------------|
| `indented` (default) | 4-space indented code |
| `fenced` | Fenced code blocks with ``` or ~~~ |

**Indented style** (default):
```markdown
<pre><code>code here</code></pre>  ->      code here
                                         (4 spaces + newlines)
```

**Fenced style**:
```markdown
<pre><code>code here</code></pre>  ->  ``` 
                                         code here
                                         ```
```

When `fenced` is selected, the `fence` option determines the fence character.

### fence

The fence character for fenced code blocks (default: `` ` ``).

| Value | Example |
|-------|---------|
| `` ` `` (default) | ```code``` |
| `~` | ~~~code~~~ |

The implementation (lines 109-132 in `commonmark-rules.js`):
- Extracts language from `<code class="language-javascript">`
- Dynamically increases fence length if the code contains fence characters
- Trims trailing newline from code content

### emDelimiter

Delimiter for italic text (`<em>` and `<i>`).

| Value | Example Output |
|-------|---------------|
| `_` (default) | `_italic_` |
| `*` | `*italic*` |

The implementation adds the delimiter around content:
```javascript
replacement: function(content, node, options) {
  if (!content.trim()) return ''
  return options.emDelimiter + content + options.emDelimiter
}
```

### strongDelimiter

Delimiter for bold text (`<strong>` and `<b>`).

| Value | Example Output |
|-------|---------------|
| `**` (default) | `**bold**` |
| `__` | `__bold__` |

Same pattern as emphasis:
```javascript
replacement: function(content, node, options) {
  if (!content.trim()) return ''
  return options.strongDelimiter + content + options.strongDelimiter
}
```

### linkStyle

Controls how anchor links (`<a>`) are rendered.

| Value | Description |
|-------|-------------|
| `inlined` (default) | `[text](url "title")` inline |
| `referenced` | `[text][id]` with definitions |

**Inlined style** (default):
```markdown
<a href="url" title="title">text</a>  ->  [text](url "title")
```

**Referenced style**:
```markdown
<a href="url" title="title">text</a>  ->  [text][1]

[1]: url "title"
```

The referenced style requires `linkReferenceStyle` to define the reference format.

### linkReferenceStyle

When `linkStyle: "referenced"` is set, this controls the reference format.

| Value | Link Syntax | Definition Syntax |
|------|-------------|-------------------|
| `full` (default) | `[text][1]` | `[1]: url "title"` |
| `collapsed` | `[text][]` | `[text]: url "title"` |
| `shortcut` | `[text]` | `[text]: url "title"` |

**Full references** (default):
```markdown
Link text [1]
[1]: https://example.com "Example"
```

**Collapsed references**:
```markdown
Link text []
[Link text]: https://example.com "Example"
```

**Shortcut references**:
```markdown
Link text
[Link text]: https://example.com "Example"
```

The implementation (lines 160-205 in `commonmark-rules.js`):
- Maintains an internal `references` array during conversion
- The `append` function is called at the end to output all definitions
- References are separated by newlines and wrapped in blank lines

### preformattedCode

Controls whether code blocks preserve whitespace formatting.

| Value | Behavior |
|-------|----------|
| `false` (default) | Convert code to indented or fenced blocks |
| `true` | Preserve original whitespace (experimental) |

When `true`, code elements:
- Keep their original text formatting
- Are not escaped
- May render incorrectly in some Markdown processors

> Note: This is experimental. See https://github.com/lucthev/collapse-whitespace/issues/16

---

## HTML Element Handling

### Headings

`<h1>` through `<h6>` elements:

- **Setext style** (default): Underline with `=` (h1) or `-` (h2)
- **ATX style**: `#` through `######` with optional closing `#`

| Element | Setext Output | ATX Output |
|--------|--------------|-----------|
| `<h1>` | `Title\n======` | `# Title` |
| `<h2>` | `Title\n------` | `## Title` |
| `<h3>` | `Title\n------` | `### Title` |
| `<h4>` | `Title\n------` | `#### Title` |
| `<h5>` | `Title\n------` | `##### Title` |
| `<h6>` | `Title\n------` | `###### Title` |

### Links

`<a>` elements with `href`:

**Inlined** (default with `linkStyle: "inlined"`):
```markdown
<a href="https://example.com" title="Example">Click here</a>
```
Output:
```markdown
[Click here](https://example.com "Example")
```

**Referenced** (with `linkStyle: "referenced"`):
```markdown
[Click here][1]

[1]: https://example.com "Example"
```

Link destinations are escaped:
- `<` and `>` are wrapped in angle brackets if they contain spaces
- Other special characters (`<()`, `)>`) are backslash-escaped

### Images

`<img>` elements:

```html
<img src="image.png" alt="Alt text" title="Image title">
```
Output:
```markdown
![Alt text](image.png "Image title")
```

If no `src` attribute, outputs empty string.

### Code Blocks

`<pre><code>` blocks:

**Indented style** (`codeBlockStyle: "indented"`):
```markdown
<pre><code>def hello():
    print("world")</code></pre>
```
Output:
```markdown
    def hello():
        print("world")
```

**Fenced style** (`codeBlockStyle: "fenced"`):
```markdown
<pre><code class="language-python">def hello():
    print("world")</code></pre>
```
Output:
```markdown
​```python
def hello():
    print("world")
​```
```

Language is extracted from the `class` attribute using `language-(\S+)`.

### Inline Code

`<code>` elements (not inside `<pre>`):

```html
<code>print("hello")</code>
```
Output:
```markdown
`print("hello")`
```

The implementation handles backticks in the content by using longer delimiters.

### Lists

**Unordered** (`<ul>`):
```html
<ul>
  <li>Item 1</li>
  <li>Item 2</li>
</ul>
```
Output (with default `bulletListMarker: "*"`):
```markdown
*   Item 1

*   Item 2
```

**Ordered** (`<ol>`):
```html
<ol start="5">
  <li>Item 5</li>
  <li>Item 6</li>
</ol>
```
Output:
```markdown
5.  Item 5

6.  Item 6
```

Nested lists are handled based on parent element context.

### Blockquotes

`<blockquote>` elements:

```html
<blockquote>
  <p>Quoted text</p>
</blockquote>
```
Output:
```markdown
> Quoted text
```

Each line gets a `>` prefix.

### Emphasis

`<em>` and `<i>` elements:

```html
<em>italic text</em>
```
Output (default):
```markdown
_italic text_
```

### Strong

`<strong>` and `<b>` elements:

```html
<strong>bold text</strong>
```
Output (default):
```markdown
**bold text**
```

### Horizontal Rules

`<hr>` elements:

```html
<hr>
```
Output (default):
```markdown

* * *

```

### Line Breaks

`<br>` elements:

Output: Two spaces + newline (`"  \n"`) by default (configurable via `br` option).

### Paragraphs

`<p>` elements:

```html
<p>Paragraph text.</p>
```
Output:
```markdown

Paragraph text.

```

Paragraphs are wrapped in blank lines.

### Other Elements

Elements without specific rules fall through to the **default rule**, which:
- Outputs text content
- Wraps block elements in blank lines
- Outputs inline content as-is

---

## What Gets Stripped

Turndown does not inherently strip any elements. However, the benchmark configuration uses a **skip tags** approach in other extractors, but Turndown has its own mechanism:

### Elements Not Explicitly Stripped

By default, Turndown attempts to convert ALL elements, including:

- `<script>` - Converts to text content (dangerous in output!)
- `<style>` - Converts to text content
- `<nav>` - Converts according to contained elements
- `<header>` - Converts according to contained elements
- `<footer>` - Converts according to contained elements

### Default Behavior for Unknown Elements

The default rule (lines 30-32 in `turndown.js`):

```javascript
defaultReplacement: function(content, node) {
  return node.isBlock ? '\n\n' + content + '\n\n' : content
}
```

Unknown elements output their text content (not outerHTML), which is usually the desired behavior.

### Using `remove()` to Strip Elements

The library provides a `remove()` method to strip specific elements:

```javascript
const turndownService = new TurndownService()
turndownService.remove(['script', 'style', 'nav', 'header', 'footer'])
```

The benchmark does NOT currently use this mechanism for Turndown.

---

## Markdown Escaping

Turndown escapes Markdown special characters in text content (not in code elements).

### Escaping Rules

Defined in `src/utilities.js` (lines 82-96):

| Pattern | Escaped |
|--------|---------|
| `\` | `\\` |
| `*` | `\*` |
| `-` | `\-` (at line start) |
| `+ ` | `\+ ` (at line start) |
| `=` | `\=` (at line start) |
| `# ` | `\# ` (at line start) |
| `` ` `` | `` \` `` |
| `~~~` | `\~~~` |
| `[` | `\[` |
| `]` | `\]` |
| `>` | `\>` (at line start) |
| `_` | `\_` |
| `\d+. ` | `\1. ` (at line start, prevents list parsing) |

### Escaping Scope

- Text inside `<code>` elements is NOT escaped
- The full content of every element is processed through the escape regexes
- This can be aggressive but prevents Markdown interpretation errors

### Customizing Escape Behavior

Override `TurndownService.prototype.escape`:

```javascript
const turndownService = new TurndownService()
turndownService.escape = function(text) {
  // Custom escaping logic
  return text.replace(/[_*]/g, '\\$&')
}
```

---

## Extending Turndown

### Adding Custom Rules

```javascript
const turndownService = new TurndownService()

turndownService.addRule('strikethrough', {
  filter: ['del', 's', 'strike'],
  replacement: function(content) {
    return '~' + content + '~'
  }
})
```

### Keeping Elements as HTML

```javascript
// Keep <del> and <ins> as HTML in output
turndownService.keep(['del', 'ins'])
// Output: Hello <del>world</del><ins>World</ins>
```

### Removing Elements

```javascript
// Remove specified elements entirely
turndownService.remove(['script', 'style'])
// Output: (elements removed, only text content from other elements)
```

### Using Plugins

Turndown has a plugin system for common extensions:

```javascript
// Example: turndown-plugin-gfm
const turndownPluginGfm = require('turndown-plugin-gfm')
turndownService.use(turndownPluginGfm.gfm)
// Enables: tables, strikethrough, task lists
```

---

## Sample Input/Output

### Sample 1: Basic Elements

**Input HTML:**
```html
<h1>Document Title</h1>
<p>This is a <strong>bold</strong> statement with <em>emphasis</em>.</p>
<h2>Subsection</h2>
<p>A link <a href="https://example.com" title="Example">here</a>.</p>
<ul>
  <li>First item</li>
  <li>Second item</li>
</ul>
<blockquote>
  <p>A quoted paragraph.</p>
</blockquote>
<hr>
<p>End of document.</p>
```

**Output (defaults):**
```markdown
Document Title
=============

This is a **bold** statement with _emphasis_.

Subsection
----------

A link [here](https://example.com "Example").

*   First item

*   Second item

> A quoted paragraph.

* * *

End of document.
```

### Sample 2: ATX Headings, Referenced Links

**Configuration:**
```javascript
{
  headingStyle: 'atx',
  linkStyle: 'referenced',
  linkReferenceStyle: 'full'
}
```

**Input:**
```html
<h1>ATX Heading</h1>
<p>See <a href="https://example.com">the link</a>.</p>
```

**Output:**
```markdown
# ATX Heading

See [the link][1].

[1]: https://example.com
```

### Sample 3: Fenced Code Blocks

**Configuration:**
```javascript
{
  codeBlockStyle: 'fenced',
  fence: '~~~'
}
```

**Input:**
```html
<pre><code class="language-javascript">function hello() {
  console.log("world");
}</code></pre>
```

**Output:**
```markdown
​```javascript
function hello() {
  console.log("world");
}
​```
```

### Sample 4: All Configurations Combined

**Configuration:**
```javascript
{
  headingStyle: 'atx',
  hr: '---',
  bulletListMarker: '-',
  codeBlockStyle: 'fenced',
  fence: '```',
  emDelimiter: '*',
  strongDelimiter: '__',
  linkStyle: 'referenced',
  linkReferenceStyle: 'collapsed',
  preformattedCode: false
}
```

**Input:**
```html
<h1>Full Config Demo</h1>
<p>All the <em>options</em> combined with <strong>everything</strong>.</p>
<ul>
  <li>Option 1</li>
  <li>Option 2</li>
</ul>
<ol>
  <li>Numbered 1</li>
  <li>Numbered 2</li>
</ol>
<pre><code class="language-rust">fn main() {
    println!("Hello!");
}</code></pre>
<hr>
<p>End <a href="https://example.com">link</a>.</p>
```

**Output:**
```markdown
# Full Config Demo

All the *options* combined with __everything__.

- Option 1

- Option 2

1.  Numbered 1

2.  Numbered 2

​```rust
fn main() {
    println!("Hello!");
}
​```

---

[Example link]: https://example.com

End [link][].
```

### Sample 5: Tables (with turndown-plugin-gfm)

**Input:**
```html
<table>
  <thead>
    <tr><th>Header 1</th><th>Header 2</th></tr>
  </thead>
  <tbody>
    <tr><td>Cell 1</td><td>Cell 2</td></tr>
    <tr><td>Cell 3</td><td>Cell 4</td></tr>
  </tbody>
</table>
```

**Output (with GFM plugin):**
```markdown
| Header 1 | Header 2 |
|----------|----------|
| Cell 1   | Cell 2   |
| Cell 3   | Cell 4   |
```

Note: Tables require the `turndown-plugin-gfm` plugin.

---

## Benchmark Integration

### How Turndown is Called

In `src/scores.rs` (lines 557-591):

```rust
fn run_turndown(html: &str, cfg: &TurndownConfig) -> String {
    // Create temporary HTML file
    let tmp = std::env::temp_dir().join(format!("turndown_{}.html", uuid::Uuid::new_v4()));
    let _ = std::fs::write(&tmp, html);

    // Build options JSON
    let options = serde_json::json!({
        "headingStyle": cfg.heading_style,
        "hr": cfg.hr,
        "bulletListMarker": cfg.bullet_list_marker,
        "codeBlockStyle": cfg.code_block_style,
        "fence": cfg.fence,
        "emDelimiter": cfg.em_delimiter,
        "strongDelimiter": cfg.strong_delimiter,
        "linkStyle": cfg.link_style,
        "linkReferenceStyle": cfg.link_reference_style,
        "preformattedCode": cfg.preformatted_code,
    });

    // Execute via Node.js
    let node_code = r#"const fs = require('fs'); ... "#;
    let out = std::process::Command::new("node")
        .args(["-e", node_code, tmp.to_str().unwrap(), &options.to_string()])
        .output();
    // ...
}
```

### Key Insight

Turndown is invoked via Node.js subprocess with:
1. HTML written to a temporary file
2. Configuration passed as JSON string in `process.argv`
3. Output written to stdout

---

## Limitations and Notes

### Not Stripping Elements by Default

Unlike other extractors in this benchmark, Turndown does NOT strip navigation, scripts, or styles by default. It converts them all to text. For content extraction, you would need to use the `remove()` method or pre-process the HTML.

### No Table Support in Core

Tables require the `turndown-plugin-gfm` plugin and are not enabled by default in the benchmark.

### Reference Collisions

With `linkReferenceStyle: "full"`, referenced links use numeric IDs which could collide if the same page is processed multiple times. However, per-conversion isolation prevents this.

### Block vs Inline Detection

Turndown uses heuristics to determine if an element is block-level based on tag name (defined in `utilities.js` `blockElements` array). This affects blank line insertion.

### Code Blocks Detection

Indentation-based detection checks if `<pre><code>` contains no siblings, which could miss some code blocks in malformed HTML.

---

## Related Files

- **Source**: `/home/jan/git/turndown/src/turndown.js` - Main Turndown service
- **Rules**: `/home/jan/git/turndown/src/rules.js` - Rule management
- **CommonMark Rules**: `/home/jan/git/turndown/src/commonmark-rules.js` - Built-in rules
- **Utilities**: `/home/jan/git/turndown/src/utilities.js` - Helper functions
- **Rust Integration**: `/home/jan/git/html-to-text-comparison/src/scores.rs` - `run_turndown` function
- **Config**: `/home/jan/git/html-to-text-comparison/src/extractor_config.rs` - `TurndownConfig` struct
