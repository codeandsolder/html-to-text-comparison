# readable-readability Extractor Analysis

## Overview

`readable-readability` is a Rust crate (version 0.4.0) for extracting readable content from HTML pages. It is a fork of `loyd/readability.rs`, which itself is based on Mozilla's Readability.js and the original Arc90 readability experiment. It is used by the [Readable](https://github.com/readable-app/readable) application.

**Source:** https://github.com/readable-app/readability.rs  
** crates.io:** https://crates.io/crates/readable-readability  
**License:** MIT  
**Dependencies:** html5ever, kuchiki, lazy_static, log, regex, url

---

## Integration in This Benchmark

**Config struct:** `ReadableReadabilityConfig` (defined in `src/extractor_config.rs` lines 383-401)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReadableReadabilityConfig {
    pub strip_unlikelys: bool,
    pub weight_classes: bool,
    pub clean_conditionally: bool,
    pub clean_attributes: bool,
}
```

**Default values used in benchmark** (lines 585-593):
```rust
"readable-readability" => ExtractorConfig {
    readable_readability: ReadableReadabilityConfig {
        strip_unlikelys: true,
        weight_classes: true,
        clean_conditionally: false,  // NOTE: differs from crate default
        ..Default::default()
    },
    ..Default::default()
},
```

**Invocation** (lines 302-310 in `src/scores.rs`):
```rust
let mut parser = readable_readability::Readability::new();
parser.strip_unlikelys(cfg.strip_unlikelys);
parser.weight_classes(cfg.weight_classes);
parser.clean_conditionally(cfg.clean_conditionally);
parser.clean_attributes(cfg.clean_attributes);
let (node, _) = parser.parse(&html);
node.text_contents()
```

The crate default for `clean_conditionally` is `true`, but the benchmark sets it to `false`, which is a significant behavioral difference.

---

## Architecture: Two-Phase Tree Traversal

The algorithm uses a **single-pass depth-first traversal** with two conceptual phases per node, controlled by a `bubbling` boolean:

1. **Capturing phase** (`on_capturing`): Descends into the tree, removing unlikely candidates and transforming elements (e.g., converting `<div>` to `<p>`). This runs top-down.

2. **Bubbling phase** (`on_bubbling`): Accumulates statistics, scores nodes, and conditionally removes elements. This runs bottom-up via the same traversal when retreating to parent/sibling.

The traversal pattern (lines 474-505):
- Start at `top_level` (the `<body>` element)
- Descend recursively via `first_child()` until no more children
- Backtrack via `next_sibling()` or `parent()`, flipping `bubbling` to true when ascending
- Call both `on_capturing` and `on_bubbling` on each node during the appropriate phase

---

## Scoring Algorithm

### Phase 1: Node Scoring (`calculate_content_score`, line 731)

Only nodes matching `is_tag_to_score()` are scored:
```rust
fn is_tag_to_score(tag: &QualName) -> bool {
    matches!{
        *tag,
        tag!("section") | tag!("p") | tag!("td") | tag!("pre") |
        tag!("h2") | tag!("h3") | tag!("h4") | tag!("h5") | tag!("h6")
    }
}
```

Scoring for a qualifying node (line 745):
- Base: **+1 point**
- Per comma: **+1 point** (text containing commas correlates with well-written content)
- Per 100 characters of combined text+link length: **+1 point**, capped at **3 points**

So a paragraph's raw content score = `1 + commas + min((text_len + link_len) / 100, 3)`.

### Phase 2: Score Propagation (`propagate_score`, line 758)

The raw score propagates upward to ancestors (up to 3 levels):
- Self (level 0): `score / 1`
- Parent (level 1): `score / 2`
- Grandparent (level 2): `score / (3 * level)` = `score / 3`
- Great-grandparent (level 3+): `score / (3 * level)`

Each ancestor accumulates these partial scores. If an ancestor wasn't previously marked as a candidate, it gets added to the candidate list.

### Phase 3: Final Scoring (`score_candidates`, line 779)

For each collected candidate:
1. Start with `info.content_score` (accumulated propagated scores)
2. Add **tag-based bonus** via `tag_score()`:

| Tag | Score |
|-----|-------|
| section | +15 |
| div | +5 |
| pre, td, blockquote | +3 |
| address, form | -3 |
| dl, dt, dd | -3 |
| li, ol, ul | -3 |
| body | -5 |
| h1-h6 | -5 |
| th | -5 |

3. If `weight_classes` is enabled, add **class/id weight** via `class_score()` (see below)
4. Apply **link density penalty**: `score *= 1.0 - (link_len / text_len)`

The link density penalty is important: if more than ~20% of text is inside links, the score gets significantly reduced.

5. After all candidates are scored, sort by score descending and keep only those with score >= **75% of the top score**.

### Phase 4: Candidate Selection

- **find_common_candidate()**: Looks for a common ancestor of the top candidates. If at least 4 candidates share a common parent (excluding `<body>`), that parent becomes the selected content area. This handles cases where content is split across multiple sibling sections.

- **correct_candidate()**: Walks up the candidate's ancestors. If a parent's score is at least 1/3 of the current candidate's score AND is higher than the current candidate, the parent wins (content likely extends to a wrapper). If the top candidate is the only child of its parent and the parent isn't "shabby", the parent is used instead.

- If the final result is not a `div`, `article`, `section`, or `p`, it gets converted to a `<div>`.

---

## strip_unlikelys

When `strip_unlikelys` is `true` (default), the `is_unlikely_candidate()` check runs during the capturing phase (line 559). Elements matching the `UNLIKELY_CANDIDATE` regex are removed **unless** they also match `MAYBE_CANDIDATE`.

**UNLIKELY_CANDIDATE regex** (lines 87-91):
```
(?xi)
-ad-|ai2html|banner|breadcrumbs|combx|comment|community|cover-wrap|disqus|extra|footer|gdpr|header|legends|menu|
modal|related|remark|replies|rss|shoutbox|sidebar|skyscraper|social|sponsor|supplemental|
ad-break|agegate|pagination|pager|popup|yom-remote
```

**MAYBE_CANDIDATE regex** (lines 93-95):
```
(?xi)
and|article|body|column|main|shadow
```

So a `<div class="sidebar">` is removed, but a `<div class="sidebar main-content">` is kept because "main" and "body" are maybe-candidates. A `<div class="footer">` with class "article" would be kept because the "article" positive match overrides.

**`<a>` and `<body>` elements are exempt** from stripping (line 162).

**Benchmark default:** `strip_unlikelys: true`

---

## weight_classes

When `weight_classes` is `true`, every candidate's final score receives a bonus or penalty based on CSS class names and ID attributes.

**POSITIVE regex** (lines 97-99):
```
(?xi)
article|body|content|entry|hentry|h-entry|main|page|pagination|post|text|blog|story
```

**NEGATIVE regex** (lines 101-105):
```
(?xi)
-ad-|hidden|^hid$|\shid$|\shid\s|^hid\s|banner|combx|comment|com-|contact|foot|footer|footnote|
gdpr|masthead|media|meta|modal|outbrain|promo|related|scroll|share|shoutbox|sidebar|skyscraper|
sponsor|shopping|tags|tool|widget
```

Scoring (line 279-294):
- Class matches POSITIVE: **+25**
- Class matches NEGATIVE: **-25**
- ID matches POSITIVE: **+25**
- ID matches NEGATIVE: **-25**

So `<div class="article-content">` gets +25, `<div id="sidebar">` gets -25, and `<div class="sidebar article">` gets net 0 (negative and positive cancel).

This is applied in `score_candidates()` at line 807.

**Benchmark default:** `weight_classes: true`

---

## clean_conditionally

When `clean_conditionally` is `true`, additional filtering happens during the bubbling phase via `is_conditionally_acceptable()` (line 690). Elements failing this check are removed.

The function first checks element type:
- `form`, `fieldset`, `table`, `div` -> considered as lists/tables, subject to additional checks
- `ul`, `ol` -> considered acceptable lists
- Everything else -> auto-accepted

Then it applies multiple failing conditions (line 714-722). If **any** condition is true, the element is removed:

| Condition | Logic |
|-----------|-------|
| Negative class score | `class_score < 0` |
| Many images, few paragraphs | `img_count > 1 && p_count / img_count < 0.5` |
| Too many list items | `li_count > p_count + 100` (non-list elements only) |
| Too many inputs | `input_count * 3 > p_count` |
| Short text with wrong image count | `text_len < 25 && (img_count == 0 \|\| img_count > 2)` (non-list only) |
| Low class score + high link density | `class_score < 25 && link_density > 0.2` (non-list only) |
| High class score + very high link density | `class_score >= 25 && link_density > 0.5` |
| Single embed in short text, or multiple embeds | `embed_count == 1 && text_len < 75 \|\| embed_count > 1` |

The `is_list` flag changes behavior for `ul`/`ol` — some conditions are skipped for actual lists (the `!is_list &&` prefix skips certain checks).

Elements failing this check are marked `is_candidate = false` and `is_shabby = true` on their parent before removal. The `is_shabby` flag influences the single-child parent correction in `correct_candidate()`.

**Benchmark default in this project:** `clean_conditionally: false` (differs from crate default of `true`)

---

## clean_attributes

When `clean_attributes` is `true`, only the `style` attribute is removed from elements (line 328-331). No other attributes (including `href`, `src`, `alt`, `class`, `id`) are touched.

```rust
fn clean_attributes(attributes: &mut Attributes) {
    attributes.remove(attrib!("style"));
}
```

**Benchmark default:** `clean_attributes: true`

---

## Element-Specific Handling

### Headings (h1-h6)

- Headings are **scored negatively** (`tag_score` returns -5 for h1-h6) because they are considered noise rather than content in readability algorithms
- However, heading text **is** included in the output text contents
- The `calculate_content_score` function only processes `h2`-`h6` (not `h1`), so h1 elements do not receive content scores
- During the `on_capturing` phase, headings are NOT removed (they are exempt from `is_stuffed` checks unless they have no content)
- The `clean_conditionally` check does NOT apply to heading elements

### Links

- Link text **is preserved** in the output
- Link density affects scoring: high link density (>20% of text) reduces score via `1 - link_len/text_len` multiplier
- Link density > 0.5 with positive class score triggers removal under `clean_conditionally`
- Relative URLs are fixed to absolute URLs if `base_url` is set

### Images

- Images are **not included in text output** — the `text_contents()` method only returns text
- Image count affects cleaning decisions (too many images with too few paragraphs triggers removal)
- The metadata extractor pulls `og:image` and `twitter:image` for article images, but these are not part of the main text extraction

### Code Blocks

- `<pre>` tags receive a **+3 tag score bonus**
- `<pre>` content is preserved in text output
- Code block content is not specially formatted (no language detection, no fence generation)

### Tables

- `<table>`, `<td>`, `<th>` are not specially formatted
- Tables can trigger removal under `clean_conditionally` (table elements have `is_list = false` so all conditions apply)
- `tag_score` gives `td` a +3 bonus, `th` a -5 penalty

### Lists

- `<ul>` and `<ol>` have `is_list = true` in `is_conditionally_acceptable()`, skipping certain filtering conditions
- `<li>` items receive a -3 tag score penalty
- List content is preserved in output

### Emphasis

- `<strong>`, `<em>`, `<b>`, `<i>` etc. are not specially handled — their content is extracted but markup is lost
- No markdown formatting is generated

---

## What Gets Stripped

### During Capturing Phase

1. **Comments and DocumentFragments** (line 531): Always removed
2. **Empty text nodes** (line 532): Whitespace-only text nodes removed
3. **script, style, noscript** (line 534): Always removed
4. **Unlikely candidates** (line 559): If `strip_unlikelys` enabled, elements matching `UNLIKELY_CANDIDATE` but NOT `MAYBE_CANDIDATE`
5. **Byline containers** (line 550): Elements with `rel="author"` attribute

### During Bubbling Phase

6. **"Unstuffed" elements** (line 597): Elements that fail `is_stuffed()` check — this removes elements that have no meaningful content (no text, no images, no embeds/iframes except for table rows which need at least one of these)
7. **Conditionally unacceptable elements** (line 604): If `clean_conditionally` enabled, elements failing `is_conditionally_acceptable()`
8. **Trailing `<br>` before `<p>`** (line 620): `<br>` elements immediately preceding a `<p>` are removed

### Specifically NOT Stripped

- `nav`, `header`, `footer`, `aside` — these are NOT explicitly targeted for removal. They might be stripped if their class/id matches negative patterns, or if they fail the content checks, but there is no explicit tag-based removal for these elements (unlike `script`, `style`, `noscript`).
- Ads — ad-related class names are in negative patterns, so ads with appropriate class names (like `class="ad-banner"`) would be stripped, but there is no explicit ad detection.

---

## Title Extraction

Title extraction is handled in `metadata::extract()` (from the metadata module):

1. **First priority**: `og:title`, `twitter:title`, `dc:title`, `dcterm:title`, `weibo:article:title`, `weibo:webpage:title` meta tags
2. **Fallback**: If no meta title, look for a **single `<h1>`** — if there are multiple `<h1>` elements, give up
3. **Second fallback**: If no meta title and no valid `<h1>`, look for a **single `<h2>`**
4. `page_title` (from `<title>` tag) and `article_title` (from the above logic) are cross-filled: if one exists without the other, use the existing one for both fields

---

## Edge Cases

### 1. Empty Document / No Candidates

If after scoring there are no candidates (line 507-509 or 513-515), the original `<body>` element is returned unchanged.

### 2. Single Child Parent Correction

If the selected top candidate is the only child of its parent and that parent is not marked `is_shabby`, the parent is used instead. This helps when adjacent content is in a parent's sibling that should be joined.

### 3. Score Threshold for Candidates

Only candidates with score >= 75% of the top score are kept (line 828). This means if the top candidate scores 100, only candidates scoring 75+ are retained.

### 4. Minimum Text Length for Scoring

Nodes with fewer than 25 characters of text are not scored (line 741 in `calculate_content_score`).

### 5. Div Transformation

`<div>` elements are transformed during capturing (line 562-563, function `transform_div`):
- If `<div>` has only a single `<p>` child with no other text content, the `<div>` is replaced by that `<p>`
- If `<div>` has no block elements (no `<a>`, `<blockquote>`, `<dl>`, `<div>`, `<img>`, `<ol>`, `<p>`, `<pre>`, `<table>`, `<ul>`, `<select>`), the `<div>` is converted to `<p>`
- Otherwise, any bare text nodes inside the `<div>` are wrapped in `<p>` tags

### 6. Font Tag Conversion

`<font>` tags are converted to `<span>` during capturing (line 564-566).

### 7. Video Embed Detection

`<embed>` elements with `src` matching the VIDEO regex (dailymotion, youtube, vimeo) are **not counted** in the embed count used for cleaning decisions (line 669-671). This prevents removal of pages with legitimate video embeds.

### 8. Common Parent Detection

The algorithm looks for a common ancestor shared by at least 4 of the top candidates (MIN_CANDIDATES = 4, line 841). If found, that common ancestor becomes the content container. This handles fragmented content spread across sibling sections.

### 9. Link Density Multiplier

The final score multiplier `1 - link_len/text_len` can produce negative scores if `link_len > text_len`. The code doesn't clamp this, but the sort order handles it correctly (negative scores sort to the bottom).

### 10. Base URL for Relative URLs

If `base_url` is set via `parser.base_url(url)`, relative `href` and `src` attributes are resolved to absolute URLs using the `url` crate.

---

## Comparison to standard Readability

`readable-readability` is based on `loyd/readability.rs` which is itself based on Mozilla's Readability.js. Key differences:

1. **Clean conditionally default**: The crate defaults `clean_conditionally` to `true`, but this benchmark sets it to `false`. This significantly changes the aggressive cleaning behavior.

2. **No JSON-LD handling**: Unlike some readbility implementations, there is no JSON-LD metadata extraction in the metadata module.

3. **Simplified attribute cleaning**: Only `style` attribute is removed; no `class` or `id` stripping.

4. **No byline restoration**: The BYLINE regex exists in comments but is not used. Only `rel="author"` is checked.

5. **No image presence in output**: Unlike Arc90/Mozilla readability which can include image references, `text_contents()` only returns text, so images are completely absent from output.

---

## Sample Input/Output Demonstrating Config Effects

### Sample HTML

```html
<!DOCTYPE html>
<html>
<head>
    <title>Test Article - Example Site</title>
    <meta property="og:title" content="Test Article">
    <meta name="author" content="Jane Doe">
</head>
<body>
    <nav class="sidebar navigation">
        <a href="/">Home</a>
        <a href="/about">About</a>
    </nav>
    
    <header class="header">
        <h1>Site Title</h1>
    </header>
    
    <div class="article-content">
        <h2>Main Heading</h2>
        <p>This is the first paragraph of the article. It contains some meaningful content
           with multiple sentences, and even a <a href="/link">link to somewhere</a>.</p>
        
        <p>Second paragraph here, with a <strong>bold word</strong> and 
           an <em>italic word</em> for emphasis.</p>
        
        <div class="sidebar-ad">
            <img src="/ads/banner.jpg" alt="Advertisement">
            <p>Buy stuff now!</p>
        </div>
        
        <blockquote>
            <p>A notable quote from someone important.</p>
        </blockquote>
        
        <pre><code>
function hello() {
    console.log("Hello, world!");
}
        </code></pre>
        
        <table class="data-table">
            <tr><th>Name</th><th>Value</th></tr>
            <tr><td>Foo</td><td>42</td></tr>
        </table>
        
        <ul>
            <li>First list item</li>
            <li>Second list item</li>
            <li>Third list item</li>
        </ul>
    </div>
    
    <footer class="footer">
        <p>&copy; 2024 Example Site</p>
    </footer>
</body>
</html>
```

### Expected Outputs

#### With default benchmark config (`strip_unlikelys=true, weight_classes=true, clean_conditionally=false`):

```
Main Heading

This is the first paragraph of the article. It contains some meaningful content
           with multiple sentences, and even a link to somewhere.

Second paragraph here, with a bold word and 
           an italic word for emphasis.


A notable quote from someone important.


function hello() {
    console.log("Hello, world!");
}


Name
Value
Foo
42

First list item
Second list item
Third list item
```

**Observations:**
- Nav and header elements are stripped because they match `UNLIKELY_CANDIDATE` patterns (nav has "sidebar", header is a likely candidate)
- The "sidebar-ad" div is stripped because class matches negative "-ad-" pattern
- The footer is stripped
- Article content is preserved
- Blockquote, pre, table, and list content all preserved
- Text formatting (bold, italic) is lost, only text content remains
- Images are not shown (text_contents() doesn't include them)

#### If `strip_unlikelys=false`:

Nav, header, and footer would be **included** in the output, because the strip_unlikelys check is skipped. The algorithm would need to rely on other heuristics (link density, class weights) to deprioritize them.

#### If `clean_conditionally=true` and `weight_classes=false`:

The conditional cleaning would aggressively remove:
- The sidebar-ad div (negative class score)
- Elements with high link density
- Short text with wrong image ratios

Without class weighting, positive indicators like "article-content" wouldn't provide +25 bonus, making it easier for content to be incorrectly removed.

#### If `clean_conditionally=true` and `strip_unlikelys=false`:

Both filters active. Even if nav/header/footer aren't stripped by the unlikelys filter, they would likely be removed by conditional cleaning (high link density in nav, no meaningful content in footer, etc.).

---

## Config Option Summary

| Option | Default (Benchmark) | Crate Default | Effect |
|--------|-------------------|---------------|--------|
| `strip_unlikelys` | `true` | `true` | Removes elements with class/id matching ad/footer/sidebar patterns |
| `weight_classes` | `true` | `true` | Adds +/-25 based on positive/negative class/id patterns |
| `clean_conditionally` | `false` | `true` | Removes elements with high link density, too many images, etc. |
| `clean_attributes` | `true` | `true` | Removes `style` attribute from elements |
