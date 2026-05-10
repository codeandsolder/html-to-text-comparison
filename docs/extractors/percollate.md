# Percollate Extractor Analysis

## Overview

**Percollate** is a Node.js command-line tool that converts web pages into formatted PDF, EPUB, HTML, or Markdown files. It was created by Dan Burzo and is available as an npm package. The tool uses a sophisticated pipeline that combines content extraction, DOM enhancement, and conversion utilities from the unified.js ecosystem.

This analysis focuses specifically on how percollate converts HTML to Markdown, which is the mode used in this benchmark.

---

## Repository and Package Information

- **GitHub Repository**: https://github.com/danburzo/percollate
- **npm Package**: https://www.npmjs.com/package/percollate
- **Current Version**: 4.3.0 (as of late 2023)
- **License**: MIT
- **Node.js Requirement**: 14.17.0 or later (supports Node 16+)

### Key Dependencies

```json
{
  "@mozilla/readability": "^0.6.0",
  "jsdom": "^21.1.0",
  "hast-util-from-dom": "^4.2.0",
  "hast-util-to-mdast": "^9.0.0",
  "mdast-util-gfm": "^2.0.2",
  "mdast-util-to-markdown": "^1.5.0",
  "dompurify": "^3.2.6",
  "hyphenopoly": "^5.3.0",
  "puppeteer": "^19.7.3"
}
```

---

## How Percollate Converts HTML to Markdown

### The Conversion Pipeline

Percollate uses a sophisticated multi-stage pipeline to convert HTML to Markdown:

```
HTML Input → JSDOM Parsing → DOM Enhancements → Readability Extraction 
→ HAST → MDAST → Markdown Output
```

#### Stage 1: HTML Parsing (JSDOM)

The incoming HTML is parsed using JSDOM, which creates a DOM representation of the page:

```javascript
const dom = new JSDOM(buffer, {
    contentType,
    url: final_url
});
const doc = dom.window.document;
```

This allows percollate to manipulate the DOM before extraction.

#### Stage 2: DOM Enhancements

Before passing the content to Readability, percollate runs a series of DOM enhancements (`src/enhancements.js`):

1. **ampToHtml**: Converts `<amp-img>` elements to regular `<img>` elements
2. **fixLazyLoadedImages**: Processes lazy-loaded images by moving `data-src`, `data-srcset` to `src` and `srcset`
3. **relativeToAbsoluteURIs**: Converts relative URLs to absolute URLs for both `<a>` and `<img>` elements
4. **imagesAtFullSize**: Extracts full-size images from wrapping `<a>` elements (e.g., `<a href="full.png"><img src="thumb.png"></a>` becomes `<img src="full.png">`)
5. **singleImgToFigure**: Wraps standalone images in `<figure>` elements with `<figcaption>` from alt text
6. **noUselessHref**: Marks links where the href equals the link text for later removal
7. **expandDetailsElements**: Forces `<details>` elements to be open
8. **wikipediaSpecific**: Removes edit links from Wikipedia pages
9. **githubSpecific**: Fixes heading anchor links in GitHub markdown
10. **wrapPreBlocks**: Wraps `<pre>` blocks in `<figure>` elements to ensure Readability preserves them

#### Stage 3: Content Extraction (@mozilla/readability)

The enhanced DOM is passed to Mozilla's Readability library, which is the same engine used by Firefox's Reader View:

```javascript
const R = new Readability(doc, {
    classesToPreserve: ['no-href', 'anchor'],
    serializer: el => el  // Return DOM element instead of HTML string
});
parsed = R.parse();
```

Readability performs the following:
- Identifies the main content container
- Removes navigation, sidebars, footers, comments, ads, and other non-essential elements
- Extracts metadata (title, byline, site name, publication date)
- Returns cleaned HTML content

#### Stage 4: Sanitization (DOMPurify)

The extracted content is sanitized using DOMPurify to prevent XSS and remove remaining dangerous content:

```javascript
const sanitizer = createDOMPurify(dom.window);
const textContent = sanitizer.sanitize(parsed.textContent || parsed.content.textContent);
```

#### Stage 5: HTML to HAST Conversion (hast-util-from-dom)

The sanitized HTML is converted to HAST (HTML Abstract Syntax Tree) using `hast-util-from-dom`:

```javascript
import { fromDom } from 'hast-util-from-dom';
const hast = fromDom(new JSDOM(html).window.document);
```

HAST is a virtual DOM representation that can be transformed by various utilities.

#### Stage 6: HAST to MDAST Conversion (hast-util-to-mdast)

HAST is converted to MDAST (Markdown Abstract Syntax Tree) using `hast-util-to-mdast`:

```javascript
import { toMdast } from 'hast-util-to-mdast';
const mdast = toMdast(hast);
```

This transforms the HTML tree into a Markdown-focused tree structure.

#### Stage 7: GFM Extension (mdast-util-gfm)

GitHub Flavored Markdown features are added using `mdast-util-gfm`:

```javascript
import { gfmToMarkdown } from 'mdast-util-gfm';
```

This adds support for:
- Tables
- Strikethrough
- Task lists
- Autolinks

#### Stage 8: MDAST to Markdown (mdast-util-to-markdown)

Finally, the MDAST is converted to a markdown string:

```javascript
import { toMarkdown } from 'mdast-util-to-markdown';
const markdown = toMarkdown(mdast, {
    ...DEFAULT_MARKDOWN_OPTIONS,
    ...userMarkdownOptions,
    extensions: [gfmToMarkdown()]
});
```

---

## Configuration Options

### PercollateConfig Structure (from benchmark)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PercollateConfig {
    pub inline_images: bool,  // Embed images as base64 data URLs
    pub hyphenate: bool,      // Enable hyphenation
    pub fences: bool,          // Use fenced code blocks
}

impl Default for PercollateConfig {
    fn default() -> Self {
        Self {
            inline_images: false,
            hyphenate: true,
            fences: true,
        }
    }
}
```

### How the Benchmark Invokes Percollate

```rust
fn build_percollate_args(input_path: &std::path::Path, cfg: &PercollateConfig) -> Vec<String> {
    let mut args = vec![
        "/home/jan/git/percollate/cli.js".to_string(),
        "md".to_string(),
        "-o".to_string(),
        "-".to_string(),  // Output to stdout
    ];
    if cfg.inline_images {
        args.push("--inline".to_string());
    }
    args.push(if cfg.hyphenate {
        "--hyphenate".to_string()
    } else {
        "--no-hyphenate".to_string()
    });
    args.push(format!(
        "--md.fences={}",
        if cfg.fences { "true" } else { "false" }
    ));
    args.push(input_path.to_string_lossy().to_string());
    args
}
```

### Configuration Options Explained

#### `--inline` (inline_images)

**Purpose**: Embed images inline as base64-encoded data URLs instead of linking to remote URLs.

**How it works**: 
- When enabled, percollate fetches each image referenced in the content
- Converts images to base64 encoding
- Replaces `src` attributes with `data:image/...;base64,...` URLs

**Default**: `false` (disabled)

**Use case**: Produces self-contained markdown files that don't require external image fetching.

**Note**: This significantly increases output size and processing time.

#### `--hyphenate` / `--no-hyphenate` (hyphenate)

**Purpose**: Enable or disable hyphenation using Hyphenopoly.

**How it works**:
- When enabled, uses the `hyphenateDom()` function to add soft hyphens to words
- Language detection is performed using franc-all and iso-639-3-to-1
- The detected language is passed to Hyphenopoly for proper hyphenation rules

**Default for md command**: `false` (disabled) - unlike PDF which defaults to enabled

**Default in benchmark**: `true` (enabled)

**Note**: Hyphenation adds `<wbr>` elements and soft hyphen characters (`&shy;`) to the HTML before markdown conversion.

#### `--md.fences=true/false` (fences)

**Purpose**: Control whether code blocks use fenced syntax (triple backticks) or indented syntax.

**How it works**:
- `fences: true` → Code blocks output as ```` ``` ```` with optional language specifier
- `fences: false` → Code blocks output as indented blocks (4 spaces)

**Default**: `true` (enabled)

**Example output with fences=true**:
````markdown
```javascript
const hello = "world";
```
````

**Example output with fences=false**:
````markdown
    const hello = "world";
````

---

## Markdown Options (--md.*)

Percollate passes options to the underlying `mdast-util-to-markdown` library. The default options are:

```javascript
const DEFAULT_MARKDOWN_OPTIONS = {
    fences: true,
    emphasis: '_',
    strong: '_',
    resourceLink: true,
    rule: '-'
};
```

### Available Markdown Options

From `src/constants/markdown.js`:

| Option | Description | Default |
|--------|-------------|---------|
| `bullet` | Unordered list bullet character | `-` |
| `bulletOther` | Additional unordered bullet | `*` |
| `bulletOrdered` | Ordered list marker | `.` |
| `bulletOrderedOther` | Additional ordered marker | `)` |
| `closeAtx` | Close ATX headings with # | `true` |
| `emphasis` | Emphasis delimiter | `_` |
| `fence` | Single backtick for code fence | `` ` `` |
| `fences` | Use fenced code blocks | `true` |
| `incrementListMarker` | Increment list markers | `true` |
| `listItemIndent` | List item indentation | `tab` |
| `quote` | Blockquote marker | `>` |
| `resourceLink` | Enable resource links | `true` |
| `rule` | Horizontal rule character | `-` |
| `ruleRepetition` | HR repetition count | `3` |
| `ruleSpaces` | Add spaces in HR | `false` |
| `setext` | Use setext headings | `true` |
| `strong` | Strong emphasis delimiter | `_` |
| `tightDefinitions` | Tight definition lists | `false` |

### Example Usage

```bash
# Use * for emphasis, --- for horizontal rules
percollate md --md.emphasis='*' --md.rule='-' https://example.com

# Use fenced code blocks (default)
percollate md --md.fences=true https://example.com

# Use indented code blocks
percollate md --md.fences=false https://example.com
```

---

## How Percollate Handles Various HTML Elements

### Headings

**Processing**:
- Readability preserves heading elements in the content
- If `--toc-level` > 1 is specified, percollate generates a table of contents
- The `setIdsAndReturnHeadings` function assigns IDs to headings for linking

**Markdown Output**:
- Uses setext-style headings for h1 and h2 by default:
  ```markdown
  Heading
  =======
  
  Subheading
  ----------
  ```
- Can use ATX-style (hash) with `closeAtx: true`

### Links

**Processing**:
- Relative links are converted to absolute URLs by `relativeToAbsoluteURIs`
- Links where text equals href are marked with `no-href` class to avoid appending href
- In-page anchors (starting with `#`) are preserved

**Markdown Output**:
```markdown
[Link Text](https://example.com/page)
```

**Note**: Percollate does not append hrefs to link text like some other extractors.

### Images

**Processing**:
1. Lazy-loaded images are fixed by moving data attributes to src
2. Full-size images are extracted from wrapping anchors
3. Single images are wrapped in `<figure>` with `<figcaption>` from alt text
4. Relative src attributes are converted to absolute URLs

**With `--inline`**:
- Images are fetched and converted to base64 data URLs

**Markdown Output**:
```markdown
![Alt text](https://example.com/image.png)

<figure>
  ![Alt text](https://example.com/image.png)
  <figcaption>Caption from alt text</figcaption>
</figure>
```

### Code Blocks

**Processing**:
- `<pre>` blocks are wrapped in `<figure>` elements by `wrapPreBlocks` to ensure Readability preserves them
- The language class (e.g., `language-javascript`) is extracted if present

**Markdown Output** (with fences=true):
````markdown
```javascript
function hello() {
  console.log("Hello, world!");
}
```
````

### Tables

**Processing**:
- Tables are preserved through the Readability extraction
- GFM extension (`mdast-util-gfm`) provides table support

**Markdown Output**:
```markdown
| Column 1 | Column 2 |
|----------|----------|
| Cell 1   | Cell 2   |
| Cell 3   | Cell 4   |
```

### Lists

**Processing**:
- Ordered and unordered lists are preserved by Readability
- List processing is handled by mdast-util-to-markdown

**Markdown Output**:
```markdown
- Item 1
- Item 2
  - Nested item

1. First
2. Second
```

### Blockquotes

**Processing**:
- `<blockquote>` elements are preserved by Readability

**Markdown Output**:
```markdown
> This is a blockquote
> spanning multiple lines
```

### Emphasis

**Processing**:
- `<em>` and `<i>` are converted to emphasis
- `<strong>` and `<b>` are converted to strong emphasis

**Markdown Output** (default `_` delimiter):
```markdown
_emphasis_ and **strong** emphasis
```

### Horizontal Rules

**Processing**:
- `<hr>` elements are converted to markdown horizontal rules

**Markdown Output** (default `-`):
```markdown
---
```

---

## Content Removal and Stripping

### What Percollate Removes

Percollate uses multiple strategies to remove unwanted content:

#### 1. Readability Extraction

The `@mozilla/readability` library is the primary content extraction tool. It removes:
- Navigation menus
- Sidebars and aside elements
- Header and footer sections
- Comment sections
- Advertisement blocks
- Social media widgets
- Related articles sections
- Cookie consent banners
- Newsletter signup forms
- Many other non-essential page elements

#### 2. DOM Enhancements (Pre-processing)

Some enhancements actually remove elements:
- **wikipediaSpecific**: Removes `.mw-editsection` (edit links)
- **expandDetailsElements**: Expands `<details>` but doesn't remove content

#### 3. Sanitization (DOMPurify)

DOMPurify further cleans the content by:
- Removing script elements and event handlers
- Removing inline event attributes (`onclick`, `onerror`, etc.)
- Removing `javascript:` URLs
- Removing potentially dangerous HTML elements

### What Percollate Preserves

- Main article content
- Headings (h1-h6)
- Paragraphs
- Links (with proper href resolution)
- Images (with src resolution)
- Code blocks (with language hints)
- Tables
- Lists (ordered and unordered)
- Blockquotes
- Emphasis (strong and em)
- Horizontal rules
- Definition lists

---

## CLI Interface

### Basic Usage

```bash
percollate <command> [options] url [url]...
```

### Available Commands

| Command | Description |
|---------|-------------|
| `percollate pdf` | Generate PDF file |
| `percollate epub` | Generate EPUB file |
| `percollate html` | Generate HTML file |
| `percollate md` | Generate Markdown file |

### Common Options

| Option | Description |
|--------|-------------|
| `-o, --output=<path>` | Output file path (use `-` for stdout) |
| `-u, --url=<url>` | Set base URL for stdin input |
| `-w, --wait=<sec>` | Wait seconds between processing URLs |
| `-t, --title=<title>` | Set bundle title |
| `-a, --author=<author>` | Set bundle author |
| `--individual` | Export each URL as separate file |
| `--toc` | Generate table of contents |
| `--toc-level=<level>` | Heading depth for ToC (1-6) |
| `--cover` | Generate cover page |
| `--inline` | Embed images inline |
| `--hyphenate` | Enable hyphenation |
| `--no-hyphenate` | Disable hyphenation |
| `--md.<option>=<value>` | Pass options to markdown stringifier |
| `--unsafe` | Disable JSDOM validation |
| `--debug` | Print debug information |

### Markdown-Specific Options

```bash
# Output markdown to stdout
percollate md -o - https://example.com

# With inline images
percollate md --inline -o - https://example.com

# With hyphenation
percollate md --hyphenate -o - https://example.com

# Without fenced code blocks
percollate md --md.fences=false -o - https://example.com

# Read from stdin
curl https://example.com | percollate md -o - -u https://example.com -
```

---

## Edge Cases and Limitations

### Malformed HTML

**Handling**: 
- Percollate uses JSDOM to parse HTML, which is generally forgiving
- The `--unsafe` flag disables some JSDOM validations that may throw errors on invalid HTML
- For severely malformed HTML, parsing may fail or produce unexpected results

**Example**:
```bash
# Handle some malformed HTML
percollate md --unsafe -o - https://example.com/bad-html
```

### AMP Pages

**Behavior**:
- By default, percollate checks for `<link rel="amphtml">` and prefers the AMP version
- This can be disabled with `--no-amp`

**Rationale**: AMP pages are typically simpler and better-suited for extraction.

### Web Feeds

**Handling**:
- Percollate has built-in support for RSS and Atom feeds
- When a feed URL is detected, each entry becomes an article
- Content is extracted from the entry (not re-fetched from the original URL)

**Example**:
```bash
percollate md --individual https://example.com/feed.xml
```

### Offline Usage

**Considerations**:
- Percollate requires network access to fetch web pages
- For offline use, you can pipe pre-fetched HTML via stdin
- The `--url` option is required when using stdin to resolve relative URLs

**Example**:
```bash
# Fetch and convert in one pipeline
curl -s https://example.com | percollate md -o - -u https://example.com -
```

### JavaScript-Rendered Content

**Limitation**: 
- Percollate does not execute JavaScript
- Content rendered client-side only may not be captured
- For such content, consider using puppeteer-based extraction or the `webclaw` extractor in this benchmark

### Performance Considerations

- **Parallel processing**: URLs are processed in parallel by default
- **Sequential processing**: Use `--wait` to process sequentially
- **Image inlining**: Significantly increases processing time and memory usage

---

## Sample Input and Expected Output

### Sample HTML Input

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>Sample Article</title>
</head>
<body>
    <article>
        <h1>Understanding Markdown Conversion</h1>
        
        <p>Markdown is a lightweight markup language that you can use to add formatting elements to plaintext text documents.</p>
        
        <h2>Why Use Markdown?</h2>
        
        <p>Markdown is popular because:</p>
        
        <ul>
            <li>It's easy to read and write</li>
            <li>It converts easily to HTML</li>
            <li>It's widely supported</li>
        </ul>
        
        <h2>Code Examples</h2>
        
        <pre><code class="language-javascript">function greet(name) {
    return `Hello, ${name}!`;
}</code></pre>
        
        <h2>Links and Images</h2>
        
        <p>Check out the <a href="https://example.com">example website</a>.</p>
        
        <img src="diagram.png" alt="A sample diagram">
        
        <blockquote>
            <p>Creativity is intelligence having fun.</p>
        </blockquote>
        
        <hr>
        
        <p>Thanks for reading!</p>
    </article>
</body>
</html>
```

### Expected Markdown Output

```markdown
# Understanding Markdown Conversion

Markdown is a lightweight markup language that you can use to add formatting elements to plaintext text documents.

## Why Use Markdown?

Markdown is popular because:

- It's easy to read and write
- It converts easily to HTML
- It's widely supported

## Code Examples

```javascript
function greet(name) {
    return `Hello, ${name}!`;
}
```

## Links and Images

Check out the [example website](https://example.com).

![A sample diagram](diagram.png)

> Creativity is intelligence having fun.

---

Thanks for reading!
```

### With Fenced Code Blocks Disabled

If `--md.fences=false` is used:

```markdown
# Understanding Markdown Conversion

Markdown is a lightweight markup language that you can use to add formatting elements to plaintext text documents.

## Why Use Markdown?

Markdown is popular because:

- It's easy to read and write
- It converts easily to HTML
- It's widely supported

## Code Examples

    function greet(name) {
        return `Hello, ${name}!`;
    }

## Links and Images

Check out the [example website](https://example.com).

![A sample diagram](diagram.png)

> Creativity is intelligence having fun.

---

Thanks for reading!
```

---

## Comparison with Other Extractors

| Feature | Percollate | Turndown | Trafilatura |
|---------|------------|----------|-------------|
| **Core Library** | hast-util-to-mdast | turndown-service | trafilatura (Python) |
| **Content Extraction** | @mozilla/readability | None (raw HTML) | trafilatura's built-in |
| **GFM Support** | Yes (mdast-util-gfm) | Yes (turndown-plugin-gfm) | Limited |
| **Hyphenation** | Yes (Hyphenopoly) | No | Yes |
| **Image Handling** | Inline or remote | Remote only | Remote only |
| **Sanitization** | DOMPurify | DOMPurify | lxml cleaning |
| **Table Support** | Yes | Yes | Yes |
| **Configuration** | Limited (md.* options) | Extensive | Extensive |

---

## Key Takeaways

1. **Percollate is a full pipeline solution**: It handles fetching, extraction, and conversion in one package.

2. **Readability is the key differentiator**: Unlike pure HTML-to-markdown converters, percollate uses Mozilla's Readability to extract the main content, filtering out navigation, ads, and other non-essential elements.

3. **Unified.js ecosystem**: The conversion uses the well-maintained unified.js ecosystem (hast, mdast, remark), ensuring reliable and standards-compliant output.

4. **GitHub Flavored Markdown**: Full GFM support is built-in, including tables, strikethrough, and task lists.

5. **Image handling flexibility**: The `--inline` option allows producing self-contained markdown files, though at the cost of larger file sizes.

6. **Hyphenation support**: Unique among Node.js-based extractors in this benchmark, percollate supports hyphenation via Hyphenopoly.

7. **CLI-first design**: While usable as a library, percollate is designed primarily as a CLI tool, making it well-suited for batch processing and pipelines.

---

## References

- Percollate GitHub: https://github.com/danburzo/percollate
- Percollate npm: https://www.npmjs.com/package/percollate
- @mozilla/readability: https://github.com/mozilla/readability
- unified.js: https://unifiedjs.com/
- hast-util-to-mdast: https://github.com/syntax-tree/hast-util-to-mdast
- mdast-util-to-markdown: https://github.com/syntax-tree/mdast-util-to-markdown
- mdast-util-gfm: https://github.com/syntax-tree/mdast-util-gfm
- DOMPurify: https://github.com/cure53/DOMPurify
- Hyphenopoly: https://github.com/ffd8/hyphenopoly
