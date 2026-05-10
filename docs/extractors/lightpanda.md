# Lightpanda Extractor Analysis

## Overview

The `lightpanda` extractor is a headless browser-based HTML-to-Markdown converter. Unlike static HTML converters that parse raw HTML, Lightpanda is a complete browser engine that executes JavaScript before extracting content. This allows it to handle modern web applications built with React, Vue, Angular, and other JavaScript frameworks.

### Basic Information

| Attribute | Value |
|-----------|-------|
| **Name** | Lightpanda Browser |
| **Type** | Headless browser with native Markdown output |
| **Language** | Written in Zig (with V8 JavaScript engine) |
| **License** | AGPL-3.0 |
| **Repository** | https://github.com/lightpanda-io/browser |
| **Website** | https://lightpanda.io |
| **Stars** | 30.2k (as of 2026) |
| **Running Method** | Docker container via `docker exec lightpanda lightpanda fetch` |

### Benchmark Integration

In this benchmark, Lightpanda is invoked via Docker:

```rust
// From src/scores.rs:728-747
fn build_lightpanda_args(parsed_url: &url::Url, cfg: &LightpandaConfig) -> Vec<String> {
    let mut args = vec![
        "exec".to_string(),
        "lightpanda".to_string(),
        "lightpanda".to_string(),
        "fetch".to_string(),
        "--dump".to_string(),
        "markdown".to_string(),
    ];
    if !cfg.wait_until.is_empty() {
        args.push("--wait-until".to_string());
        args.push(cfg.wait_until.clone());
    }
    if cfg.wait_ms > 0 {
        args.push("--wait-ms".to_string());
        args.push(cfg.wait_ms.to_string());
    }
    args.push(parsed_url.to_string());
    args
}
```

Default configuration from `src/extractor_config.rs:130-149`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightpandaConfig {
    pub strip_js: bool,      // Default: true
    pub strip_css: bool,     // Default: true
    pub strip_ui: bool,      // Default: false
    pub wait_until: String,  // Default: "done"
    pub wait_ms: u64,        // Default: 5000
}
```

---

## Architecture

### How Lightpanda Works

Lightpanda is not a Chromium fork or WebKit derivative. It is a purpose-built headless browser written from scratch in Zig, a low-level systems programming language. This is a fundamentally different approach from tools like Puppeteer or Playwright that wrap Chrome/Chromium.

#### Core Components

1. **HTML Parser**: Uses `html5ever` (from the Servo project) to parse HTML into a DOM tree
2. **JavaScript Engine**: Embeds V8 (Chrome's JavaScript engine) for JS execution
3. **DOM Implementation**: Full DOM APIs (document, elements, events, etc.)
4. **Network Layer**: Uses libcurl for HTTP requests
5. **Markdown Converter**: Built-in conversion from the accessibility tree to Markdown

#### Execution Flow

```
1. Fetch URL → HTTP request (via libcurl)
2. Parse HTML → DOM tree (html5ever)
3. Execute JavaScript → DOM mutations, XHR/fetch, client-side rendering
4. Build Accessibility Tree → Semantic structure from DOM
5. Convert to Markdown → CommonMark output
6. Dump to stdout
```

### Key Architectural Decisions

1. **No Graphical Rendering**: Lightpanda has no GPU or display pipeline, reducing memory footprint by 16x compared to Chrome
2. **Zig Implementation**: Explicit memory management, no garbage collection, minimal runtime
3. **V8 Integration**: Uses Google's V8 engine for JavaScript (not a custom JS implementation)
4. **Accessibility Tree First**: The Markdown output is generated from the accessibility tree, not directly from the DOM

---

## Configuration Options

### strip_js, strip_css, strip_ui

These options control what content gets stripped from the final Markdown output.

**Note**: In the benchmark, these options are defined in `LightpandaConfig` but are **not actually passed** to the Docker command. The current implementation only passes `--wait-until` and `--wait-ms`. This means the `--strip-mode` flag is not being used.

#### Available Strip Modes (from CLI docs)

| Mode | Tags Removed |
|------|--------------|
| `js` | `<script>`, `<link as="script">`, `<link rel="preload">` |
| `css` | `<style>`, `<link rel="stylesheet">` |
| `ui` | `<img>`, `<picture>`, `<video>`, `<svg>`, `<style>`, `<link rel="stylesheet">` |
| `full` | Combines js + ui + css |

#### Behavior in Benchmark

- **strip_js** (default: true): Currently not applied
- **strip_css** (default: true): Currently not applied  
- **strip_ui** (default: false): Currently not applied

The `--strip-mode` flag would need to be added to `build_lightpanda_args()` to actually use these options.

### wait_until

The `--wait-until` option controls what event triggers the extraction after page load. This is critical for JavaScript-heavy pages.

| Value | Description |
|-------|-------------|
| `load` | Wait for the `load` event - all resources (images, stylesheets, scripts) are fully loaded |
| `domcontentloaded` | Wait for DOMContentLoaded - DOM is parsed but subresources may still be loading |
| `networkidle` | Wait for no network requests for a short period |
| `done` | Wait for both network idle AND JavaScript execution to complete (default) |

#### Detailed Behavior

- **load**: Fires when the page and all dependent resources (CSS, images, iframes) are completely loaded. May miss dynamically loaded content.

- **domcontentloaded**: Fires when the initial HTML document has been completely loaded and parsed, without waiting for stylesheets, images, and subframes to finish loading. Faster but may miss initial JS-rendered content.

- **networkidle**: Waits until there are no more than 0 network connections for a specified duration. Good for pages that stop making requests after initial load. Risky for pages with periodic updates (chat, notifications, live data).

- **done**: Combines networkidle with waiting for JavaScript to settle. This is the default and most reliable for SPAs (Single Page Applications).

#### Default in Benchmark

```rust
wait_until: "done".to_string()
```

This is the most conservative option, ensuring both network requests have settled and JavaScript has finished executing.

### wait_ms

The `--wait-ms` option adds an additional time-based wait **after** the `--wait-until` condition is met. This is a safety net for pages that:

1. Use timers (setTimeout, setInterval) to render content
2. Have delayed initializations
3. Load content progressively after the main event fires

#### Default in Benchmark

```rust
wait_ms: 5000  // 5 seconds
```

#### Interaction with wait_until

The wait_ms timer starts **after** the wait_until condition is satisfied, not from the beginning of page load. This means:

1. Page loads → wait_until condition met (e.g., "done")
2. Additional wait_ms milliseconds pass
3. Markdown extraction occurs

This two-stage waiting (event + time) provides robustness for edge cases while the default 5 seconds should handle most timer-based content loading.

---

## Content Element Handling

### Headings

Lightpanda converts HTML headings to Markdown headings using the `#` syntax:

| HTML | Markdown |
|------|----------|
| `<h1>` | `# Heading 1` |
| `<h2>` | `## Heading 2` |
| `<h3>` | `### Heading 3` |
| `<h4>` | `#### Heading 4` |
| `<h5>` | `##### Heading 5` |
| `<h6>` | `###### Heading 6` |

Heading levels are preserved based on the accessibility tree's heading role.

### Links

Links are converted with the standard Markdown syntax:

```markdown
[Link Text](https://example.com)
```

The accessibility tree provides the link text and href, enabling accurate conversion. Relative URLs are preserved as-is.

### Images

Images are NOT included in Markdown output by default. The `--strip-mode ui` flag would need to be used to include them.

When included (with strip_ui disabled), images appear as:

```markdown
![Alt Text](image-url.jpg)
```

### Code Blocks

Code blocks are converted based on the accessibility tree's code role:

| HTML | Markdown |
|------|----------|
| `<pre><code>` | Fenced code block with ``` |
| `<code>` (inline) | `inline code` |

The language is not automatically detected - it would need to be specified or extracted from class names.

### Tables

Tables are converted to Markdown table syntax:

```markdown
| Header 1 | Header 2 |
|----------|----------|
| Cell 1   | Cell 2   |
```

This is based on the accessibility tree's table structure.

### Lists

Both ordered and unordered lists are supported:

**Unordered**:
```markdown
- Item 1
- Item 2
  - Nested item
```

**Ordered**:
```markdown
1. First item
2. Second item
```

List structure is derived from the accessibility tree's list roles.

### Blockquotes

Blockquotes use the `>` syntax:

```markdown
> This is a blockquote
> spanning multiple lines
```

### Emphasis

- **Strong** (`<strong>`, `<b>`): `**bold text**`
- **Emphasis** (`<em>`, `<i>`): `_italic text_`

### Horizontal Rules

Horizontal rules (`<hr>`) are converted to:

```markdown
---
```

---

## What Gets Stripped

### By Default (No Strip Mode)

Without any `--strip-mode` flag:
- JavaScript is **executed** but not included in Markdown
- CSS is **applied** but not included in Markdown
- UI elements (images, videos, SVGs) **are included** in Markdown output

### With strip_js (js mode)

```bash
--strip-mode js
```
Removes:
- `<script>` elements
- `<link as="script">` (preload scripts)
- `<link rel="preload">` for scripts

### With strip_css (css mode)

```bash
--strip-mode css
```
Removes:
- `<style>` elements
- `<link rel="stylesheet">` elements

### With strip_ui (ui mode)

```bash
--strip-mode ui
```
Removes:
- `<img>` elements
- `<picture>` elements
- `<video>` elements
- `<svg>` elements
- `<style>` elements
- `<link rel="stylesheet">` elements

### With strip_ui=false (default in benchmark)

Images, videos, and SVGs are included in Markdown output.

---

## Edge Cases

### Slow Pages

For pages that take a long time to render:
- **wait_until="done"** ensures network and JS settle
- **wait_ms=5000** adds 5 seconds buffer
- For extremely slow pages, increase wait_ms or use a selector-based wait

### JavaScript-Heavy Content

Lightpanda excels at JS-heavy pages because:
1. It executes JavaScript using V8
2. It waits for the "done" state by default
3. It builds the accessibility tree from the fully-rendered DOM

Examples where Lightpanda outperforms static converters:
- React/Vue/Angular SPAs
- Pages with infinite scroll
- Client-side rendered content
- Dynamic form loading

### Single Page Applications (SPAs)

SPAs are Lightpanda's strength:
- Initial HTML may be nearly empty
- Content is rendered by JavaScript after page load
- Static converters see only the initial HTML
- Lightpanda sees the final rendered DOM

### Dynamic Content (Infinite Scroll, Lazy Loading)

For pages that load more content on scroll:
- Default 5 second wait may not be enough
- Consider using `--wait-selector` to wait for specific content
- Or increase `--wait-ms` significantly

### Pages with Periodic Network Activity

Pages with live updates (chat, stock tickers, notifications):
- **networkidle** may never trigger
- **done** is safer as it includes JS settling
- Consider `--wait-selector` instead of time-based waits

### Form-Heavy Pages

Lightpanda handles:
- Input fields (text, checkbox, radio, select)
- Form submission detection
- Button detection

### Iframe Content

By default, iframe content is NOT included. Use `--with-frames` to include it.

### Error Handling

Lightpanda is in beta. From the documentation:
> "Lightpanda is in Beta and currently a work in progress. Stability and coverage are improving and many websites now work. You may still encounter errors or crashes."

---

## How It Differs from Static HTML Converters

### Fundamental Difference

| Aspect | Static Converters | Lightpanda |
|--------|-------------------|------------|
| **Input** | Raw HTML string | URL (fetches over HTTP) |
| **JavaScript** | Not executed | Fully executed via V8 |
| **DOM** | Parsed from HTML | Built from executed page |
| **Rendering** | None | Full browser engine |
| **Output Timing** | Immediate | After wait conditions |

### Specific Differences

1. **JavaScript Rendering**
   - Static: Only sees what's in the initial HTML
   - Lightpanda: Sees the final DOM after JS execution

2. **Dynamic Content**
   - Static: Misses content loaded via XHR/fetch
   - Lightpanda: Captures XHR-loaded content

3. **SPA Support**
   - Static: Often fails completely on React/Vue apps
   - Lightpanda: Fully supports SPAs

4. **Resource Usage**
   - Static: Minimal (just HTML parsing)
   - Lightpanda: Higher but still 16x less than Chrome

5. **Network Dependency**
   - Static: Works offline with HTML input
   - Lightpanda: Must fetch URL over network

6. **Configuration**
   - Static: HTML parsing options
   - Lightpanda: Browser navigation options (wait conditions, timeouts)

### Performance Comparison

From Lightpanda benchmarks (933 real web pages on AWS m5.large):

| Metric | Lightpanda | Headless Chrome |
|--------|------------|-----------------|
| Memory (100 pages) | 123 MB | 2 GB |
| Execution time | 5s | 46s |
| Difference | 16x less memory | 9x slower |

---

## Sample Input and Expected Output

### Sample HTML Input

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>Example Article</title>
</head>
<body>
    <article>
        <h1>The Future of AI in Web Browsers</h1>
        
        <p>Artificial intelligence is <strong>revolutionizing</strong> how we interact with web browsers. 
        From predictive loading to <em>intelligent content extraction</em>, the possibilities are endless.</p>
        
        <h2>Why Traditional Browsers Struggle</h2>
        
        <p>Traditional browsers like Chrome were designed for human users, not AI agents. They consume 
        <strong>massive amounts of memory</strong> and are <em>slow to start</em>.</p>
        
        <blockquote>
            <p>"The future of browser automation lies in purpose-built engines."</p>
        </blockquote>
        
        <h2>Key Advantages</h2>
        
        <ul>
            <li>16x less memory usage</li>
            <li>9x faster execution</li>
            <li>Native JavaScript support</li>
        </ul>
        
        <h3>Performance Metrics</h3>
        
        <table>
            <tr><th>Metric</th><th>Traditional</th><th>Modern</th></tr>
            <tr><td>Memory</td><td>2GB</td><td>123MB</td></tr>
            <tr><td>Startup</td><td>46s</td><td>5s</td></tr>
        </table>
        
        <p>For more information, visit <a href="https://lightpanda.io">Lightpanda</a>.</p>
        
        <pre><code>// Example JavaScript
const browser = new Browser();
await browser.navigate('https://example.com');
const content = await browser.getMarkdown();</code></pre>
        
        <hr>
        
        <p><small>Published in 2026</small></p>
    </article>
</body>
</html>
```

### Expected Markdown Output

```markdown
# The Future of AI in Web Browsers

Artificial intelligence is **revolutionizing** how we interact with web browsers. From predictive loading to _intelligent content extraction_, the possibilities are endless.

## Why Traditional Browsers Struggle

Traditional browsers like Chrome were designed for human users, not AI agents. They consume **massive amounts of memory** and are _slow to start_.

> "The future of browser automation lies in purpose-built engines."

## Key Advantages

- 16x less memory usage
- 9x faster execution
- Native JavaScript support

### Performance Metrics

| Metric | Traditional | Modern |
|--------|-------------|--------|
| Memory | 2GB | 123MB |
| Startup | 46s | 5s |

For more information, visit [Lightpanda](https://lightpanda.io).

```
// Example JavaScript
const browser = new Browser();
await browser.navigate('https://example.com');
const content = await browser.getMarkdown();
```

---

Published in 2026
```

### Dynamic JavaScript Example

For a React-based page that loads content via JavaScript:

**Initial HTML (what static converters see):**
```html
<div id="root"></div>
<script src="app.js"></script>
```

**Final DOM (what Lightpanda sees after JS execution):**
```html
<div id="root">
  <h1>React Application</h1>
  <p>Content loaded via JavaScript</p>
  <button>Click me</button>
</div>
```

Lightpanda converts the final DOM to:
```markdown
# React Application

Content loaded via JavaScript

[Click me]()  (button becomes link in accessibility tree)
```

---

## Current Benchmark Implementation Limitations

### Missing Features

1. **No strip_mode passed**: The `strip_js`, `strip_css`, `strip_ui` config options are defined but not used in the Docker command. Adding strip mode would require modifying `build_lightpanda_args()`.

2. **No --obey-robots**: Robots.txt compliance is not enabled by default.

3. **No --with-frames**: Iframe content is not included.

4. **No --wait-selector**: Selector-based waiting is not available.

5. **No proxy configuration**: HTTP proxy options are not exposed.

### Potential Improvements

To make full use of the LightpandaConfig options, the `build_lightpanda_args()` function could be extended:

```rust
fn build_lightpanda_args(parsed_url: &url::Url, cfg: &LightpandaConfig) -> Vec<String> {
    let mut args = vec![
        "exec".to_string(),
        "lightpanda".to_string(),
        "lightpanda".to_string(),
        "fetch".to_string(),
        "--dump".to_string(),
        "markdown".to_string(),
    ];
    
    // Build strip mode from config options
    let mut strip_modes = Vec::new();
    if cfg.strip_js { strip_modes.push("js"); }
    if cfg.strip_css { strip_modes.push("css"); }
    if cfg.strip_ui { strip_modes.push("ui"); }
    if !strip_modes.is_empty() {
        args.push("--strip-mode".to_string());
        args.push(strip_modes.join(","));
    }
    
    if !cfg.wait_until.is_empty() {
        args.push("--wait-until".to_string());
        args.push(cfg.wait_until.clone());
    }
    if cfg.wait_ms > 0 {
        args.push("--wait-ms".to_string());
        args.push(cfg.wait_ms.to_string());
    }
    args.push(parsed_url.to_string());
    args
}
```

---

## Summary

Lightpanda represents a fundamentally different approach to HTML-to-Markdown conversion:

1. **Headless Browser**: Full browser engine with JavaScript execution
2. **Zig-based**: Purpose-built for automation, not derived from existing browsers
3. **Accessibility Tree**: Markdown generated from semantic accessibility layer
4. **Wait Configuration**: Configurable wait conditions for dynamic content
5. **Performance**: 16x less memory, 9x faster than Chrome-based alternatives

For the benchmark, it provides a unique capability: converting JavaScript-rendered web pages to Markdown, which no static HTML converter can match. The trade-off is network dependency (must fetch URL) and higher resource usage compared to pure parsers.
