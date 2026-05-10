# Trafilatura Extractor Analysis

## Overview

Trafilatura is a Python package and command-line tool designed for web content extraction. It was developed as a PhD project at the Berlin-Brandenburg Academy of Sciences for creating text databases for linguistic research. The library is widely used by companies like HuggingFace, IBM, and Microsoft Research, as well as academic institutions.

**Key facts:**
- Current version: 2.0.0 (as of late 2024)
- License: Apache 2.0 (GPLv3+ for versions prior to 1.8.0)
- Python-only implementation
- Consistently outperforms other open-source libraries in benchmarks (F-score of ~0.91 in evaluations)
- Supports output formats: TXT, Markdown, CSV, JSON, HTML, XML, XML-TEI

---

## How Trafilatura Extracts Content

### Core Extraction Algorithm

Trafilatura uses a **multi-layered extraction approach** with intelligent fallbacks:

1. **Rule-based extraction** (primary): Custom heuristics based on HTML structure patterns, focusing on article-like content
2. **Readability fallback** (secondary): Uses Mozilla's Readability algorithm as a backup when rule-based extraction yields insufficient results
3. **jusText fallback** (tertiary): Another algorithm for handling difficult cases

The extraction process:
1. **HTML parsing**: Uses lxml for HTML parsing
2. **Tree sanitization**: Removes script, style, and other non-content elements early
3. **Content detection**: Identifies the main content container using heuristics
4. **Text extraction**: Extracts text while preserving structure
5. **Post-processing**: Applies cleaning, deduplication, and formatting

### Signals Used for Content Detection

Trafilatura relies on multiple signals to identify main content:

- **HTML semantic tags**: `<article>`, `<main>`, `<section>` elements
- **Class/id patterns**: Elements with content-related class names (article, post, content, entry, text)
- **Text density**: High ratio of text to markup
- **Element position**: Central placement in the document
- **Link density**: Low ratio of links to text (navigation has high link density)
- **DOM depth**: Preference for shallower tree depths
- **Language cues**: Sentence-like text patterns

The algorithm is specifically designed for article pages, blog posts, and main text portions. Results vary significantly for link lists, galleries, or catalog-style pages.

---

## Configuration Options

### TrafilaturaConfig Structure (from benchmark)

```rust
pub struct TrafilaturaConfig {
    pub favor_precision: bool,     // default: false
    pub favor_recall: bool,        // default: false
    pub include_comments: bool,    // default: true
    pub include_tables: bool,      // default: true
    pub include_images: bool,      // default: false
    pub include_formatting: bool,  // default: false
    pub include_links: bool,       // default: false
    pub deduplicate: bool,          // default: true
    pub with_metadata: bool,       // default: true
}
```

---

## favor_precision vs favor_recall

### favor_precision (default: false)

When enabled, the extractor:
- Focuses on the most central and relevant content
- Uses stricter heuristics to filter out noise
- Produces shorter output with higher accuracy
- Useful when results contain too much boilerplate or irrelevant content

**Effect on extraction:**
- More conservative content selection
- Lower recall but higher precision
- Less likely to include sidebars, footers, or peripheral content
- Can be combined with `prune_xpath` parameter to target specific HTML elements

### favor_recall (default: false)

When enabled, the extractor:
- Includes more elements when uncertain
- Prioritizes capturing all valid content over strict filtering
- Produces longer output with more comprehensive coverage

**Effect on extraction:**
- More permissive content selection
- Higher recall but potentially lower precision
- More likely to include content that might be peripheral
- Useful when parts of documents are being missed

### Default Behavior (balanced)

Without either flag set, trafilatura uses its standard balanced algorithm that strikes a balance between noise reduction and content comprehensiveness, achieving approximately:
- Precision: ~0.91
- Recall: ~0.90
- F-Score: ~0.91

**Benchmark results from 2022-05-18 (750 documents):**

| Mode | Precision | Recall | Accuracy | F-Score | Speed |
|------|-----------|--------|----------|---------|-------|
| fast | 0.914 | 0.886 | 0.902 | 0.900 | 4.8x |
| precision | 0.932 | 0.874 | 0.905 | 0.902 | 9.4x |
| standard | 0.914 | 0.904 | 0.910 | 0.909 | 7.1x |

---

## Element Preservation Options

### include_comments (default: true)

When enabled, trafilatura attempts to extract comment sections from the HTML. Comments are typically found:
- In dedicated comment sections at the bottom of articles
- Within `<section>` or `<div>` elements with comment-related class names
- Inside elements with "comment", "response", or "replies" in their identifiers

**What happens:**
- If found, comments are extracted and included in the output
- Comments are placed after the main text content
- Requires the main content extraction to succeed first
- Minimum comment length thresholds apply (configurable via settings)

### include_tables (default: true)

When enabled, text content from HTML `<table>` elements is extracted and preserved.

**What gets preserved:**
- Table text content (cell values)
- Table structure is converted to a text representation

**What does NOT get preserved:**
- Cell borders and styling
- Column/row spanning information (collapsed to text)
- Table headers may or may not be specially marked depending on output format

### include_images (default: false)

When enabled, image information is tracked and included in the output.

**What gets preserved (when enabled):**
- Image `src` attribute (URL or file path)
- Image `alt` text
- Image `title` attribute
- Image position in content (when relevant)

**Output formats:**
- In XML/TEI: Properly tagged as image elements
- In Markdown: May be converted to reference format or included as links
- In plain text: Limited representation

**Note:** This feature is marked as experimental in the documentation.

### include_formatting (default: false)

When enabled, structural elements related to text formatting are preserved.

**What gets preserved:**
- `<strong>` and `<b>` tags - converted to bold markup
- `<em>` and `<i>` tags - converted to italic markup
- Other inline formatting elements
- Basic text structure

**Important:** This option is most valuable with XML output formats. For plain text or Markdown, the benefit is limited because these formats have limited formatting support.

### include_links (default: false)

When enabled, hyperlinks are preserved in the output.

**What gets preserved:**
- Link text (anchor text)
- Link target URL (`href` attribute)

**Formats:**
- In Markdown: May appear as `[link text](url)` or in reference-style format
- In XML: Properly tagged as link elements
- In plain text: Often stripped or converted to parenthetical references

**Note:** This feature is marked as experimental.

---

## deduplicate Option

### How Deduplication Works

Trafilatura implements two types of duplicate detection:

#### 1. Element-Level (Paragraph) Deduplication

Uses a **Least Recently Used (LRU) cache** mechanism:
- Tracks text content of extracted elements
- When an element's text has been seen before, it's flagged as a duplicate
- Configurable parameters:
  - `min_duplcheck_size`: Minimum text length to consider for deduplication (default: 100 characters)
  - `max_repetitions`: Maximum allowed repetitions of a segment (default: 2)

#### 2. Document-Level (Near-Duplicate) Detection

Uses **SimHash** (Charikar's hash) with **Locality-Sensitive Hashing (LSH)**:
- Generates a digital fingerprint for each document
- Compares using Hamming distance
- Returns similarity score between 0 and 1
- Threshold-based filtering for near-duplicate detection

**Key points:**
- Deduplication is NOT enabled by default in the extract() function
- Must be explicitly enabled with `deduplicate=True`
- Tracking is per-thread/process (each process maintains its own duplicate list)
- Performance impact: Minimal overhead for paragraph deduplication, more significant for SimHash on large document sets

---

## with_metadata Option

### What Gets Extracted

When enabled (`with_metadata=True`, which is the default), trafilatura extracts:

#### Essential Metadata:
- **Title**: From `<title>` tag, `<h1>`, or Open Graph / Twitter Card meta tags
- **Author**: From author meta tags, bylines, or Dublin Core metadata
- **Date**: Publication date using the separate `htmldate` library
- **URL**: The source URL of the document

#### Additional Metadata (when available):
- **Site name**: From meta tags
- **Description**: From meta description or Open Graph tags
- **categories**: From meta tags
- **tags**: From meta tags
- **language**: Detected language of the content
- **license**: From link tags
- **image**: Featured image from Open Graph or Twitter Card

### Metadata Output Formats

The format depends on the selected `output_format`:

**JSON:**
```json
{
  "title": "Article Title",
  "author": "Author Name",
  "date": "2024-01-15",
  "url": "https://example.com/article",
  "excerpt": "Article description...",
  "categories": ["category1", "category2"],
  "tags": ["tag1", "tag2"],
  "sitename": "Site Name"
}
```

**CSV:**
- Metadata as header columns, content in rows

**XML/TEI:**
- Structured metadata elements within the document

**Markdown:**
- Metadata is added as YAML front matter at the top of the document

**Plain text:**
- Limited or no metadata representation

---

## What Gets Stripped

### Completely Removed Elements

The following HTML elements are removed during extraction:
- `<script>` - JavaScript code
- `<style>` - CSS stylesheets
- `<noscript>` - Noscript fallback content
- `<iframe>` - Embedded content (often for ads or tracking)
- `<embed>` - Embedded objects
- `<object>` - Embedded objects
- `<form>` - Form elements
- `<input>` - Form inputs
- `<button>` - Form buttons

### Boilerplate Elements (heuristically removed)

Trafilatura attempts to filter out:
- Navigation menus (header, nav elements with high link density)
- Sidebars (left/right columns with peripheral content)
- Footers (footer elements, copyright notices)
- Advertisement placeholders
- Social sharing widgets
- Related articles/links sections
- Comment counts and engagement widgets
- Cookie notices and banners

### Attributes Stripped

Most HTML attributes are removed during extraction:
- All inline styles (`style` attribute)
- Event handlers (`onclick`, `onload`, etc.)
- Class names (unless relevant for extraction)
- IDs (unless relevant for extraction)
- Data attributes

### Whitespace Handling

Trafilatura sanitizes text:
- Removes excessive whitespace
- Normalizes line breaks
- Trims leading/trailing whitespace from elements

---

## How Trafilatura Handles Specific Elements

### Headings

**Behavior:**
- `<h1>` through `<h6>` are recognized and preserved in the output
- Converted to appropriate Markdown heading syntax (`#` through `######`)
- Heading text content is extracted
- Heading hierarchy is maintained

**Example:**
```html
<h1>Main Title</h1>
<h2>Subsection</h2>
```

**Output:**
```markdown
# Main Title

## Subsection
```

**Notes:**
- In plain text output, headings may be rendered in uppercase or with underlines
- Heading level detection depends on proper HTML nesting

### Links

**Behavior (when `include_links=True`):**
- Anchor text is preserved
- Link target (href) is preserved
- Converted to Markdown link format: `[anchor text](url)`
- Or reference-style links depending on output format

**Behavior (when `include_links=False`):**
- Anchor text is preserved
- Link targets are stripped

**Example:**
```html
<a href="https://example.com">Click here</a>
```

**Output with links enabled:**
```markdown
[Click here](https://example.com)
```

**Output without links:**
```markdown
Click here
```

### Images

**Behavior (when `include_images=True`):**
- Image source (src) is preserved
- Alt text is preserved
- Title attribute is preserved
- Position in content is maintained

**Example:**
```html
<img src="photo.jpg" alt="A beautiful sunset" title="Sunset view">
```

**Output (Markdown):**
```markdown
![A beautiful sunset](photo.jpg)
```

**Behavior (when `include_images=False`, which is default):**
- Images are completely stripped from output

### Code Blocks

**Behavior:**
- `<pre>` and `<code>` elements are recognized
- Preserved in the output
- Converted to Markdown code block syntax (fenced with triple backticks)

**Example:**
```html
<pre><code>def hello():
    print("Hello, world!")</code></pre>
```

**Output:**
```markdown
def hello():
    print("Hello, world!")
```

**Notes:**
- Language hints (class names like `language-python`) may be preserved as info string
- Inline code (`<code>` without `<pre>`) uses backtick formatting

### Tables

**Behavior (when `include_tables=True`):**
- Table content is extracted
- Structure is converted to text representation

**Example:**
```html
<table>
  <tr><th>Name</th><th>Age</th></tr>
  <tr><td>Alice</td><td>30</td></tr>
  <tr><td>Bob</td><td>25</td></tr>
</table>
```

**Output (Markdown):**
```markdown
Name | Age
----|---
Alice | 30
Bob | 25
```

**Behavior (when `include_tables=False`):**
- Table content may be lost entirely or minimally represented

### Lists

**Behavior:**
- Ordered (`<ol>`) and unordered (`<ul>`) lists are recognized
- Converted to Markdown list syntax
- Indentation is preserved for nested lists

**Example:**
```html
<ul>
  <li>First item</li>
  <li>Second item
    <ul>
      <li>Nested item</li>
    </ul>
  </li>
</ul>
```

**Output:**
```markdown
- First item
- Second item
  - Nested item
```

### Blockquotes

**Behavior:**
- `<blockquote>` elements are recognized
- Converted to Markdown blockquote syntax (`>`)

**Example:**
```html
<blockquote>
  <p>This is a quote.</p>
</blockquote>
```

**Output:**
```markdown
> This is a quote.
```

### Emphasis

**Behavior (when `include_formatting=True`):**
- `<strong>` and `<b>` - bold markup
- `<em>` and `<i>` - italic markup

**Example:**
```html
<p>This is <strong>bold</strong> and <em>italic</em> text.</p>
```

**Output:**
```markdown
This is **bold** and *italic* text.
```

**Behavior (when `include_formatting=False`):**
- Emphasis markers are stripped
- Plain text only

---

## extract() vs bare_extract()

### extract()

The main wrapper function that provides the easiest interface for text extraction:

```python
import trafilatura
result = trafilatura.extract(html, output_format='markdown')
```

**Characteristics:**
- Returns a string in the chosen output format
- Default output format: plain text ('txt')
- Handles output conversion internally
- Includes metadata handling when enabled
- Handles all formatting and post-processing

**Parameters:**
- `output_format`: 'txt', 'markdown', 'json', 'csv', 'html', 'xml', 'xmltei'
- `with_metadata`: Include metadata in output
- All extraction options (precision, recall, include_*, etc.)

**Returns:** String (formatted output) or None if extraction fails

### bare_extraction()

Internal function returning raw Python variables:

```python
from trafilatura import bare_extraction
result = bare_extraction(html)
```

**Characteristics:**
- Returns Python objects directly
- Default output format: 'python' (returns Document object or dict)
- Provides access to raw extracted data before formatting
- More efficient for programmatic processing

**Returns:**
- Document object with `.as_dict()` method
- Or dictionary with keys: 'text', 'comments', 'tables', 'images', 'links', 'metadata', etc.
- Or None if extraction fails

**When to use bare_extraction():**
- When you need programmatic access to extracted elements
- When you want to process extracted content in Python before output
- When you need to inspect intermediate results

---

## Edge Cases

### JavaScript-Rendered Pages

**Behavior:**
- Trafilatura processes **raw HTML only**
- Does NOT execute JavaScript
- Content loaded dynamically via JavaScript will NOT be extracted

**Recommendation:**
- For JavaScript-heavy pages, use a browser automation library before extraction:
  - Playwright
  - Selenium
  - Puppeteer (via nodriver in Python)
  - browserforge

**Alternative:**
- Use the `--archived` option on CLI to fetch from Internet Archive
- Or fetch rendered HTML from Common Crawl dumps

### Very Short Content

**Behavior:**
- Minimum output length thresholds apply (configurable)
- Default minimum extracted size: 250 characters
- Default minimum output size: 1 character

**What happens:**
- If extracted content is below minimum threshold, extraction may fail
- Fallback algorithms are triggered
- If all fail, returns empty result

**Handling:**
- Adjust `MIN_EXTRACTED_SIZE` and `MIN_OUTPUT_SIZE` in settings
- Use `baseline()` or `html2txt()` functions as more permissive fallbacks

### Malformed HTML

**Behavior:**
- Trafilatura uses lxml for parsing, which handles malformed HTML gracefully
- Tries to repair broken HTML structure
- Most parsing errors are handled internally

**Edge cases that may cause issues:**
- Extremely malformed HTML
- Encoding issues
- Very large documents (timeout protection exists)

### Empty or Non-Content Pages

**Behavior:**
- Pages with no meaningful content (link lists, galleries, etc.) yield poor results
- Trafilatura is specifically designed for article pages and main text content
- Returns empty or minimal output for non-article pages

**Recommendation:**
- Check `is_probably_readerable()` before full extraction:
  ```python
  from trafilatura.readability_lxml import is_probably_readerable
  if is_probably_readerable(html):
      result = trafilatura.extract(html)
  ```

### Language Detection

**Behavior:**
- Target language can be specified using ISO 639-1 codes
- If detected language doesn't match target, content may be discarded

**Example:**
```python
result = trafilatura.extract(html, target_language="de")
```

**Requirements:**
- Requires `py3langid` package (`pip install trafilatura[all]`)
- Depends on model availability and performance

---

## Performance Considerations

### Speed Optimization

Trafilatura offers speed optimization options:

1. **fast mode** (`fast=True`):
   - Skips fallback algorithms
   - About 2x faster than standard mode

2. **Minimal extraction**:
   ```python
   result = trafilatura.extract(
       html,
       include_comments=False,
       include_tables=False,
       no_fallback=True  # or fast=True in newer versions
   )
   ```

3. **Baseline function**:
   - Faster alternative with simpler heuristics
   - Good balance of speed and accuracy

### Memory Management

- Uses caches for extraction and cleaning processes
- In large-scale applications, memory leaks may occur
- Can reset caches:
  ```python
  from trafilatura.meta import reset_caches
  reset_caches()
  ```

---

## Sample HTML Input and Expected Markdown Output

### Sample 1: Simple Article

**Input HTML:**
```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>Understanding Python Decorators</title>
    <meta name="author" content="Jane Smith">
    <meta name="description" content="A guide to Python decorators">
</head>
<body>
    <header>
        <nav>
            <a href="/">Home</a>
            <a href="/about">About</a>
        </nav>
    </header>
    <main>
        <article>
            <h1>Understanding Python Decorators</h1>
            <p class="meta">By Jane Smith | Published January 15, 2024</p>
            <p>Python decorators are a powerful feature that allow you to modify the behavior of functions or methods. They provide a clean way to add functionality without modifying the original code.</p>
            
            <h2>Basic Syntax</h2>
            <p>Here's a simple example of a decorator:</p>
            <pre><code>def my_decorator(func):
    def wrapper():
        print("Before function call")
        func()
        print("After function call")
    return wrapper

@my_decorator
def say_hello():
    print("Hello!")</code></pre>
            
            <h2>Use Cases</h2>
            <ul>
                <li>Logging</li>
                <li>Timing functions</li>
                <li>Authentication</li>
                <li>Caching</li>
            </ul>
            
            <blockquote>
                <p>"Decorators are one of Python's most powerful features."</p>
            </blockquote>
            
            <p>For more information, visit the <a href="https://docs.python.org">official Python documentation</a>.</p>
        </article>
    </main>
    <footer>
        <p>&copy; 2024 Python Tips Blog</p>
    </footer>
</body>
</html>
```

**Expected Markdown Output (default settings):**
```markdown
# Understanding Python Decorators

By Jane Smith | Published January 15, 2024

Python decorators are a powerful feature that allow you to modify the behavior of functions or methods. They provide a clean way to add functionality without modifying the original code.

## Basic Syntax

Here's a simple example of a decorator:

```
def my_decorator(func):
    def wrapper():
        print("Before function call")
        func()
        print("After function call")
    return wrapper

@my_decorator
def say_hello():
    print("Hello!")
```

## Use Cases

- Logging
- Timing functions
- Authentication
- Caching

> "Decorators are one of Python's most powerful features."

For more information, visit the official Python documentation.
```

**With metadata enabled and Markdown output:**
```markdown
---
title: Understanding Python Decorators
author: Jane Smith
date: '2024-01-15'
excerpt: A guide to Python decorators
---

# Understanding Python Decorators

By Jane Smith | Published January 15, 2024

Python decorators are a powerful feature that allow you to modify the behavior of functions or methods. They provide a clean way to add functionality without modifying the original code.

## Basic Syntax

Here's a simple example of a decorator:

```
def my_decorator(func):
    def wrapper():
        print("Before function call")
        func()
        print("After function call")
    return wrapper

@my_decorator
def say_hello():
    print("Hello!")
```

## Use Cases

- Logging
- Timing functions
- Authentication
- Caching

> "Decorators are one of Python's most powerful features."

For more information, visit the official Python documentation.
```

---

### Sample 2: Article with Comments and Tables

**Input HTML:**
```html
<!DOCTYPE html>
<html>
<head>
    <title>Web Development Trends 2024</title>
</head>
<body>
    <nav>Navigation content here</nav>
    
    <article>
        <h1>Web Development Trends in 2024</h1>
        
        <p>The web development landscape continues to evolve rapidly. Here are the key trends to watch this year.</p>
        
        <h2>Popular Frameworks</h2>
        <table>
            <tr><th>Framework</th><th>Market Share</th><th>Growth</th></tr>
            <tr><td>React</td><td>45%</td><td>+5%</td></tr>
            <tr><td>Vue</td><td>25%</td><td>+3%</td></tr>
            <tr><td>Angular</td><td>20%</td><td>-2%</td></tr>
            <tr><td>Svelte</td><td>10%</td><td>+8%</td></tr>
        </table>
        
        <h2>Key Technologies</h2>
        <ol>
            <li><strong>Server Components</strong> - New architecture paradigm</li>
            <li><em>Edge Computing</em> - Faster content delivery</li>
            <li>AI Integration - Smart applications</li>
        </ol>
    </article>
    
    <section class="comments">
        <h3>Comments</h3>
        <p><strong>Alice:</strong> Great article! I especially agree with the framework predictions.</p>
        <p><strong>Bob:</strong> Would love to see more about AI integration.</p>
    </section>
</body>
</html>
```

**Expected Output (with default settings, comments enabled):**
```markdown
# Web Development Trends in 2024

The web development landscape continues to evolve rapidly. Here are the key trends to watch this year.

## Popular Frameworks

Framework | Market Share | Growth
---- | ---- | ----
React | 45% | +5%
Vue | 25% | +3%
Angular | 20% | -2%
Svelte | 10% | +8%

## Key Technologies

1. **Server Components** - New architecture paradigm
2. *Edge Computing* - Faster content delivery
3. AI Integration - Smart applications

---

## Comments

Alice: Great article! I especially agree with the framework predictions.

Bob: Would love to see more about AI integration.
```

---

## Benchmark Usage

In this HTML-to-text comparison benchmark, trafilatura is invoked via:

```python
result = trafilatura.extract(
    html,
    output_format='markdown',
    favor_precision=cfg['favor_precision'],
    favor_recall=cfg['favor_recall'],
    include_comments=cfg['include_comments'],
    include_tables=cfg['include_tables'],
    include_images=cfg['include_images'],
    include_formatting=cfg['include_formatting'],
    include_links=cfg['include_links'],
    deduplicate=cfg['deduplicate'],
    with_metadata=cfg['with_metadata']
)
```

The function is called through a Rust wrapper that:
1. Writes HTML to a temporary file
2. Calls Python via `uv run -- python3 -c "..."`
3. Reads and returns the result

---

## References

- Official documentation: https://trafilatura.readthedocs.io/
- GitHub repository: https://github.com/adbar/trafilatura
- Research paper: Barbaresi, A. (2021). "Trafilatura: A Web Scraping Library and Command-Line Tool for Text Discovery and Extraction." Proceedings of ACL/IJCNLP 2021.
- DOI: https://doi.org/10.18653/v1/2021.acl-demo.15
