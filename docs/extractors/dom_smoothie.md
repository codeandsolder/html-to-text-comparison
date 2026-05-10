# dom_smoothie - Comprehensive Analysis

## Overview

**dom_smoothie** is a Rust crate (v0.17.0) for extracting readable content from web pages. It is a close port of Mozilla's [readability.js](https://github.com/mozilla/readability), bringing its algorithm to Rust with some enhancements and differences.

Repository: https://github.com/niklak/dom_smoothie
Crates.io: https://crates.io/crates/dom_smoothie
Documentation: https://docs.rs/dom_smoothie

The crate is used in this benchmark through the `Readability::new(html, url, config).parse()` entry point, returning an `Article` struct with `.text_content` (the output this benchmark measures).

---

## Integration in This Benchmark

**File**: `src/scores.rs` (lines 312-327)

```rust
#[cfg(feature = "dom_smoothie")]
"dom_smoothie" => {
    let cfg = states
        .states
        .get("dom_smoothie")
        .map(|s| s.config.dom_smoothie.clone())
        .unwrap_or_default();
    runner.run(output_name, move |html| {
        let dom_cfg = cfg.clone();
        dom_smoothie::Readability::new(html, None, dom_cfg.into_config())
            .unwrap()
            .parse()
            .unwrap()
            .text_content
            .to_string()
    });
}
```

The benchmark calls `.parse().unwrap()` — any `ReadabilityError` will cause a benchmark panic.

---

## Configuration (`DomSmoothieConfig`)

**File**: `src/extractor_config.rs` (lines 403-457)

```rust
pub struct DomSmoothieConfig {
    pub max_elements_to_parse: Option<usize>,
    pub text_mode: String,
    pub keep_classes: bool,
    pub classes_to_preserve: Vec<String>,
    pub disable_json_ld: bool,
    pub n_top_candidates: usize,
    pub char_threshold: usize,
    pub min_score_to_adjust: f32,
    pub candidate_select_mode: String,
}
```

Default values:
- `max_elements_to_parse`: `None` (0, meaning unlimited)
- `text_mode`: `"markdown"`
- `keep_classes`: `false`
- `classes_to_preserve`: `[]`
- `disable_json_ld`: `false`
- `n_top_candidates`: `5`
- `char_threshold`: `500`
- `min_score_to_adjust`: `5.0`
- `candidate_select_mode`: `"readability"`

### Config Conversion

The `into_config()` method (lines 418-440) maps string values to `dom_smoothie::Config`:

```rust
pub fn into_config(self) -> Option<dom_smoothie::Config> {
    let text_mode = match self.text_mode.as_str() {
        "raw" => dom_smoothie::TextMode::Raw,
        "formatted" => dom_smoothie::TextMode::Formatted,
        "markdown" | _ => dom_smoothie::TextMode::Markdown,
    };
    let candidate_select_mode = match self.candidate_select_mode.as_str() {
        "dom_smoothie" => dom_smoothie::CandidateSelectMode::DomSmoothie,
        "readability" | _ => dom_smoothie::CandidateSelectMode::Readability,
    };
    Some(dom_smoothie::Config {
        keep_classes: self.keep_classes,
        classes_to_preserve: self.classes_to_preserve,
        max_elements_to_parse: self.max_elements_to_parse.unwrap_or(0),
        disable_json_ld: self.disable_json_ld,
        n_top_candidates: self.n_top_candidates,
        char_threshold: self.char_threshold,
        min_score_to_adjust: self.min_score_to_adjust,
        candidate_select_mode,
        text_mode,
        ..Default::default()
    })
}
```

### max_elements_to_parse — What Happens When Exceeded

From `src/readability.rs` lines 924-940:

```rust
fn verify_doc(&self) -> Result<(), ReadabilityError> {
    if self.config.max_elements_to_parse > 0 {
        let total_elements = self
            .doc
            .root()
            .descendants_it()
            .filter(NodeRef::is_element)
            .count();
        if total_elements > self.config.max_elements_to_parse {
            return Err(ReadabilityError::TooManyElements(
                total_elements,
                self.config.max_elements_to_parse,
            ));
        }
    }
    Ok(())
}
```

If `max_elements_to_parse` is set and the document has more element nodes than the limit, `verify_doc()` returns `Err(ReadabilityError::TooManyElements(total, max))`. In the benchmark, this propagates through `.parse().unwrap()` and causes a panic. When the limit is 0 (the default, meaning unlimited), this check is skipped.

### n_top_candidates — How It Affects Scoring

`n_top_candidates` controls how many of the highest-scoring candidate nodes are kept after scoring (from `src/grab.rs` lines 299-305):

```rust
scored_candidates
    .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
scored_candidates
    .into_iter()
    .take(cfg.n_top_candidates)
    .map(move |c| c.0)
    .collect()
```

Only this many candidates are considered for top-candidate selection. A higher value (e.g., 10) increases the chance of finding the right content at the cost of more computation. Default is 5.

### char_threshold — When Parsing Succeeds

`char_threshold` (default: 500) is the minimum character count required for the extracted content to be considered successful. From `src/grab.rs` lines 36-48:

```rust
if let Some(ref article_node) = article_node {
    let text_length = article_node.normalized_char_count();
    if text_length >= self.config.char_threshold {
        return Some(doc);
    }
    // ...otherwise record as best_attempt and retry with fewer flags
}
```

If the first parsing attempt falls below `char_threshold`, the algorithm retries with progressively stripped heuristics (StripUnlikelys -> WeightClasses -> CleanConditionally). Only if all attempts fail does it return the longest result found.

### min_score_to_adjust — Performance Tuning for Link Density

```rust
let score = if prev_score > cfg.min_score_to_adjust {
    prev_score * (1.0 - link_density_fn(&candidate, None, |n| cc_cache.char_count(n)))
} else {
    prev_score
};
```

Link density adjustment (which reduces scores for content that is mostly links) is only performed on nodes with a score above `min_score_to_adjust`. Increasing this value (e.g., to 10 or 15) speeds up scoring at the cost of less accurate link density handling. Default is 5.0.

---

## The Readability Algorithm — Step by Step

### 1. Document Preparation (`prepare()`)

From `src/readability.rs` lines 203-222, the document is cleaned before scoring:

1. Remove empty images (`<img>` without src)
2. Unwrap `<noscript>` tags that contain only images
3. Remove `<script>` and `<style>` elements
4. Replace multiple consecutive `<br>` elements with a single `<br>`, then wrap following phrasing content in `<p>` tags
5. Convert `<font>` elements to `<span>` (removing all attributes)
6. Remove HTML comments

### 2. Element Collection (`collect_elements_to_score()`)

From `src/grab.rs` lines 527-612:

The function iterates through the DOM tree, collecting nodes to score and removing elements that won't contribute:

1. **Skip invisible elements**: Those with `hidden` attribute, `aria-hidden="true"`, `display:none` in style, or `visibility:hidden`
2. **Skip SVG elements** and **dialog elements** (with `role="dialog"` or `aria-modal`)
3. **Remove title-matching headings**: If metadata title is found and a heading has >75% text similarity to it, remove that heading
4. **Strip unlikely candidates** (if `StripUnlikelys` flag is set): Elements whose `id` or `class` match `UNLIKELY_CANDIDATES` patterns (e.g., "sidebar", "footer", "menu", "ad-", "breadcrumb") unless they also match `MAYBE_CANDIDATES` (e.g., "article", "body", "content", "main")
5. **Remove empty content-bearing elements**: `<div>` or `<section>` elements that contain no meaningful text
6. **Convert DIVs to Ps**: DIVs that contain only phrasing content get converted to `<p>` elements. DIVs with a single `<p>` child get replaced with that `<p>` directly. DIVs without child block elements get renamed to `<p>`
7. **Collect**: Elements matching `DEFAULT_TAGS_TO_SCORE` (`section`, `h2-h6`, `p`, `td`, `pre`) are collected for scoring

The `UNLIKELY_CANDIDATES` list:
```
"-ad-", "ai2html", "banner", "breadcrumbs", "combx", "comment", "community",
"cover-wrap", "disqus", "extra", "footer", "gdpr", "header", "legends", "menu",
"related", "remark", "replies", "rss", "shoutbox", "sidebar", "skyscraper",
"social", "sponsor", "supplemental", "ad-break", "agegate", "pagination",
"pager", "popup", "yom-remote"
```

The `MAYBE_CANDIDATES` list:
```
"and", "article", "body", "column", "content", "layout", "main", "mathjax", "shadow"
```

### 3. Scoring (`score_elements()`)

From `src/grab.rs` lines 229-306:

Each collected element gets a score based on:

**Base tag scoring** (from `src/score.rs` lines 27-41):
| Tag | Score |
|-----|-------|
| div | +5.0 |
| pre, td, blockquote | +3.0 |
| address, ol, ul, dl, dd, dt, li, form | -3.0 |
| h1-h6, th | -5.0 |

**Class/id weighting** (when `WeightClasses` flag is set): Classes and IDs are checked against positive/negative patterns. Positive patterns (e.g., "article", "content", "entry", "main", "post") add +25, negative patterns (e.g., "hidden", "banner", "sidebar", "ad-", "comment") subtract -25.

**Ancestor scoring**: The element's direct text content adds `2 + comma_count + min(content_len/100, 3)` to its score. This score is then divided and propagated up the ancestor chain (parent=1/1, grandparent=1/2, great-grandparent=1/3, etc.), accumulating in the `score_map`.

**Link density adjustment**: After the initial scoring, candidates with a score above `min_score_to_adjust` have their scores reduced proportionally to their link density: `score * (1 - link_density)`. Link density is the fraction of text characters inside `<a>` tags.

The top `n_top_candidates` candidates by score are kept.

### 4. Top Candidate Selection (`handle_candidates()`)

From `src/grab.rs` lines 79-155:

1. If no candidate is found or the top candidate is `<body>`, create a synthetic `<div>` containing all body children and use that as the candidate.

2. If `candidate_select_mode` is `DomSmoothie`: Find common ancestor with alternative candidates using intersection-based scoring.
   
   If `candidate_select_mode` is `Readability` (default): Use the Mozilla readability approach — look for a common ancestor shared by at least 3 top candidates. The original Mozilla approach requires at least 3 alternative candidates to have overlapping ancestors with the top candidate to find a shared parent. The dom_smoothie README notes this "magic number" doesn't always work well.

3. If the top candidate is the only child of its parent, consider using the parent instead.

4. Walk up the ancestor chain of the top candidate. If a parent's score exceeds `last_score * 3`, and is above the `score_threshold` (`top_candidate_score / 3`), make that parent the new top candidate. This "bonus system" helps capture content that has strong parent containers.

5. Sibling content is appended to the article: all siblings of the top candidate are evaluated. If a sibling has a score > `sibling_score_threshold` (`max(top_candidate_score * 0.2, 10.0)`), or has positive score but less than the threshold but contains a paragraph with >80 chars and link density <0.25, it is included. Non-standard block elements get converted to `<div>`.

6. The article content node is marked with `id="readability-page-1"` and `class="page"`.

### 5. Article Preparation (`prep_article()`)

From `src/prep_article.rs` lines 353-406:

The order of cleaning operations matters:

1. **Remove share elements**: Elements whose id/class contains "share" or "sharedaddy" and have fewer than `char_threshold` characters are removed
2. **Mark data tables**: Tables are analyzed for row/column counts. Tables are marked as "data tables" if they have `summary` attribute, contain `<caption>`, `<col>`, `<colgroup>`, `<tfoot>`, `<thead>`, or `<th>`, have rows>=10 or cols>4, or rows*cols>10. Nested tables are marked as layout tables.
3. **Fix lazy images**: Images with `loading="lazy"` or classes containing "lazy" have their `data-src` or similar attributes moved to `src`. Short base64 data URIs are removed if other attributes reference real images.
4. **Clean conditionally — forms/fieldsets**: Forms and fieldsets with low class weights, high link density, or suspicious patterns are removed
5. **Clean — object/embed/footer/link/aside/iframe/input/textarea/select/button**: Embedded video from allowed domains (youtube, vimeo, dailymotion, etc.) is preserved; all others are removed
6. **Clean headers**: Headings with negative class weights are removed (e.g., if class contains "hidden", "sidebar")
7. **Clean conditionally — tables/lists/divs**: Nodes are evaluated for removal based on: few commas + many embeds + ad/loading words + high list density + high li-to-p ratio + many inputs + high link density + missing text density. Single-cell tables get special handling.
8. **Replace H1 with H2**: H1s are renamed to H2 (since H1 should be the page title displayed separately)
9. **Clean styles**: Presentational attributes (align, background, bgcolor, border, etc.) are removed; width/height removed from table/th/td/hr/pre
10. **Remove empty paragraphs**: Paragraphs with no content and no embedded media are removed
11. **Fix BR sequences**: Consecutive `<br>` elements followed by a `<p>` get the `<br>` removed
12. **Fix single-cell tables**: Tables with only one cell get converted to `<p>` (if the cell contains phrasing content) or `<div>`

### 6. Post-Processing (`post_process_content()`)

From `src/readability.rs` lines 872-886:

1. **Fix links**: `javascript:` links get their href removed or become `<span>`; links without href get removed if they have no children
2. **Simplify nested elements**: `<td>` children of non-`<tr>` elements get flattened; nested divs with single child div/section get merged (attributes propagate to child)
3. **Remove score attributes**: All `data-readability-score` and `data-readability-table` attributes are removed
4. **Clean classes**: Unless `keep_classes` is true, all class attributes are removed. If `classes_to_preserve` is set, only those classes are kept on `.page` elements

### 7. Relative URL Resolution (`fix_relative_uris()`)

From `src/readability.rs` lines 974-1020:

- Relative `<a href>` URLs are made absolute using the document URL
- `src`, `poster`, and `srcset` on media elements are also made absolute

### 8. Text Content Extraction — The Three TextModes

From `src/readability.rs` lines 488-492:

```rust
let text_content = match self.config.text_mode {
    TextMode::Raw => root_node.text(),
    TextMode::Formatted => root_node.formatted_text(),
    TextMode::Markdown => root_node.md(None),
};
```

**Raw**: Returns the raw DOM text content of the article node. No formatting is applied. Whitespace is collapsed but the structure is flat.

**Formatted**: Returns `formatted_text()` which applies some layout-like formatting — paragraph breaks are preserved, block structure is somewhat maintained, but tables are not aligned. The README notes this "does not preserve table structures, meaning table data may be output as plain text without column alignment."

**Markdown**: Returns `md(None)` which converts the HTML to Markdown. This produces structured output with proper headings, lists, code blocks, links, etc.

The Article struct also has `.content` (HTML) which is separate from `.text_content`.

---

## What Gets Stripped

**Removed during preparation**:
- `<script>`, `<style>`, `<noscript>` (content removed)
- HTML comments
- `<font>` tags (converted to `<span>`, attributes removed)
- Consecutive `<br>` sequences replaced with paragraph-wrapped content

**Removed during element collection**:
- Invisible elements (`hidden`, `aria-hidden`, `display:none`, `visibility:hidden`)
- Dialog elements (`<dialog>`, `role="dialog"`, `aria-modal`)
- Headings with >75% text similarity to the article title
- Unlikely candidates (matched by id/class against `UNLIKELY_CANDIDATES` list, unless also in `MAYBE_CANDIDATES`)
- Elements with `role` attribute matching `UNLIKELY_ROLES` (`menu`, `menubar`, `complementary`, `navigation`, `alert`, `alertdialog`, `dialog`)
- Empty content-bearing elements (no text, only `<br>`/`<hr>`)

**Removed during scoring**:
- Elements with fewer than 25 characters of content
- Candidates with non-positive scores

**Removed during article preparation**:
- Share-related elements (id/class containing "share", "sharedaddy") below char_threshold
- Forms and fieldsets (conditionally)
- Embeds/objects iframes (except YouTube, Vimeo, Dailymotion, etc.)
- Headings with negative class weight
- Conditionally: divs/uls/tables with high link density, many inputs, ad-related content, list-to-paragraph ratio issues, low comma count + suspicious embedding
- Single-cell tables (converted to div/p)
- Presentational attributes everywhere
- Empty paragraphs, stray `<br>` sequences

**Removed during post-processing**:
- `javascript:` links (content unwrapped or converted to span)
- Links without href and no children
- Nested div/section flattening
- Classes (unless `keep_classes` or in `classes_to_preserve`)
- Score metadata attributes

---

## Positive and Negative Scoring Patterns

**Positive class/id patterns** (from `glob.rs`):
```
article, body, content, entry, hentry, h-entry, main, page, post, text, blog, story
```
These add +25 to the element's score when `WeightClasses` is enabled.

**Negative class/id patterns**:
```
"-ad-", hidden, banner, combx, comment, com-, contact, footer, gdpr, masthead, 
media, meta, outbrain, promo, related, scroll, share, shoutbox, sidebar, 
skyscraper, sponsor, shopping, tags, widget
```
These subtract -25 from the element's score.

Additionally, the word "hid" in a class/id (as a standalone word, not part of "hidden") subtracts -25 via `CLASSES_NEGATIVE_WORDS`.

**Tag-based scores**:
- `div`: +5
- `pre`, `td`, `blockquote`: +3
- `address`, `ol`, `ul`, `dl`, `dd`, `dt`, `li`, `form`: -3
- `h1-h6`, `th`: -5

---

## Handling of Specific Element Types

### Headings (h1-h6)
- H1 is renamed to H2 during article preparation (H1 reserved for page title)
- Headings with negative class weight are removed during `clean_headers()`
- Heading ancestors contribute to scoring (divided by depth level)
- Headings are included in the formatted/markdown output

### Links
- Link text is included in output (with formatting in markdown mode)
- `javascript:` links are stripped (content unwrapped or converted to span)
- Links without href and no children are removed
- Relative URLs are made absolute in post-processing

### Images
- Empty images (no src) are removed during preparation
- Lazy-loaded images have `data-src` attributes moved to `src`
- Short base64 placeholders are removed
- Image URLs are made absolute
- Images are included in markdown output (as `![](url)` if markdown mode)

### Code Blocks (`<pre>`, `<code>`)
- `<pre>` gets +3 base score
- Presentational attributes (width/height) are removed from `<pre>`
- Code blocks are preserved in all text modes (with varying formatting)

### Tables
- Tables are scored via `<td>` (+3 base)
- Data tables are marked to prevent conditional removal
- Single-cell tables are converted to `<p>` or `<div>` based on content type
- Tables with layout characteristics (nested tables, single row/col, high rows*cols) are marked as non-data
- Table structure is preserved in HTML content, but in Formatted/Text modes may become plain text

### Lists (`<ul>`, `<ol>`, `<li>`)
- Lists contribute -3 to parent scoring (form, ol, ul, etc.)
- List density is checked in conditional cleaning (high list density = likely unwanted)
- Lists are preserved with their structure in markdown output

### Blockquotes
- `<blockquote>` gets +3 base score
- Blockquotes are preserved in all text modes

### Emphasis (`<em>`, `<strong>`, `<i>`, `<b>`)
- These are phrasing content and don't affect scoring significantly
- In Markdown mode, emphasis is preserved as `*` or `_` markers
- In Raw/Formatted modes, the text content is preserved without markup

---

## Element Handling Summary

| Element | Scoring Effect | Preserved? | Notes |
|---------|---------------|------------|-------|
| div | +5 | Yes | Converted to p if no block children |
| pre, td, blockquote | +3 | Yes | |
| address, ol, ul, dl, form | -3 | Yes | |
| h1-h6, th | -5 | Yes (h1 renamed h2) | Removed if negative class weight |
| p | 0 | Yes | |
| a | 0 | Yes | javascript: links stripped |
| img | 0 | Yes (if non-empty) | Empty src removed, lazy fixed |
| script, style | N/A | No | Removed in prepare() |
| noscript | N/A | Partially | Images unwrapped |
| br | 0 | Yes | Consecutive brs collapsed |
| font | N/A | No | Converted to span, attrs stripped |

---

## Edge Cases

### Empty Output
If no candidate is found and the body has no content, the algorithm creates a synthetic div from body children (see line 93-100 of grab.rs). If even this fails, `parse()` returns `Err(ReadabilityError::GrabFailed)` which causes `.unwrap()` panic in the benchmark.

### Panics
The benchmark calls `.parse().unwrap()` which panics on:
- `ReadabilityError::TooManyElements(total, max)` — when `max_elements_to_parse` is exceeded
- `ReadabilityError::GrabFailed` — when no content could be extracted
- `ReadabilityError::BadDocumentURL` — if document URL is provided but not absolute (not triggered in benchmark since URL is `None`)

### Whitespace Handling
The `normalize_spaces()` function collapses multiple spaces/newlines into single spaces. `normalized_char_count()` counts Unicode grapheme clusters.

### TextContent Squashing
The README acknowledges that `text_content` "may squash words together if element nodes don't have a whitespace before closing, and currently, I have no definitive opinion on this matter." This is inherited from readability.js behavior.

---

## ParsePolicy — parse_with_policy

While `parse()` tries all four policies sequentially (Strict → Moderate → Clean → Raw) and keeps the best result, `parse_with_policy()` uses only one:

| Policy | Flags | Behavior |
|--------|-------|----------|
| Strict (default) | All flags | Removes unlikelys, weights classes, cleans conditionally |
| Moderate | WeightClasses, CleanConditionally | Skips unlikely removal |
| Clean | CleanConditionally only | Minimal filtering |
| Raw | None | No cleaning heuristics |

The loop in `grab_article()` progressively strips flags until content above `char_threshold` is found or all flags are exhausted.

---

## Candidate Selection Modes

**Readability** (default): Uses Mozilla's original algorithm — looks for a common ancestor shared by at least 3 top candidates. The README notes this requires "at least three other candidates" to trigger adjustment, which can fail with only 2 significant candidates.

**DomSmoothie**: Uses an intersection-based approach — counts how many top candidates share each ancestor of the top candidate, then picks the ancestor that appears across the most candidates (tiebreak by proximity to top candidate). This "may produce a less 'clean' result but can capture more meaningful content."

---

## Differences from Mozilla Readability.js

From the README:

1. **URL normalization**: dom_smoothie does not modify absolute URLs; readability.js may add trailing slashes and normalize case
2. **DOM simplification**: dom_smoothie more aggressively removes parent `<div>` elements with only single `<div>` or `<p>` children
3. **Attribute cleanup**: dom_smoothie removes all `<font>` attributes; readability.js preserves them
4. **Empty links**: dom_smoothie removes `<a>` without href or children; readability.js keeps them
5. **Class preservation**: dom_smoothie preserves `class="page"` only for the article node; readability.js preserves it everywhere
6. **Filtering order**: In versions ≤0.16.0, dom_smoothie filters globally then per-attempt vs readability's combined filter+score approach. This can cause duplicate headings and bylines.

---

## Sample Input/Output

### Sample Input HTML

```html
<!DOCTYPE html>
<html>
<head>
    <title>Understanding Rust Ownership - Tech Blog</title>
    <meta property="og:title" content="Understanding Rust Ownership">
    <meta name="author" content="Jane Developer">
    <meta name="description" content="A deep dive into Rust's ownership system">
</head>
<body>
    <header class="site-header">
        <nav>
            <a href="/">Home</a>
            <a href="/about">About</a>
        </nav>
    </header>

    <main class="content">
        <article>
            <h1>Understanding Rust Ownership</h1>
            <p class="byline">By Jane Developer</p>
            
            <p>Rust's ownership system is one of its most distinctive features. 
            It allows memory safety without garbage collection.</p>
            
            <h2>What is Ownership?</h2>
            <p>Every value in Rust has a single owner. When the owner goes out of scope, 
            the value is dropped.</p>
            
            <pre><code>fn main() {
    let s = String::from("hello");
    println!("{}", s);
}</code></pre>
            
            <blockquote>
                <p>Rust combines the efficiency of low-level languages with 
                the safety of high-level ones.</p>
            </blockquote>
            
            <h2>Borrowing</h2>
            <p>You can reference a value without taking ownership via references:</p>
            
            <ul>
                <li>Immutable references: <code>&amp;T</code></li>
                <li>Mutable references: <code>&amp;mut T</code></li>
            </ul>
            
            <p><img src="/example.png" alt="Example diagram" /></p>
            
            <table>
                <tr><th>Concept</th><th>Description</th></tr>
                <tr><td>Own</td><td>Complete control</td></tr>
                <tr><td>Borrow</td><td>Temporary access</td></tr>
            </table>
        </article>
    </main>

    <aside class="sidebar">
        <h3>Related Posts</h3>
        <ul>
            <li><a href="/rust-lifetimes">Understanding Lifetimes</a></li>
            <li><a href="/rust-macros">Rust Macros Explained</a></li>
        </ul>
    </aside>

    <footer class="footer">
        <p>&copy; 2026 Tech Blog</p>
    </footer>
</body>
</html>
```

### Expected Output — text_mode: "raw"

```
Understanding Rust Ownership
By Jane Developer

Rust's ownership system is one of its most distinctive features. It allows memory safety without garbage collection.

What is Ownership?
Every value in Rust has a single owner. When the owner goes out of scope, the value is dropped.

fn main() {
    let s = String::from("hello");
    println!("{}", s);
}

Rust combines the efficiency of low-level languages with the safety of high-level ones.

Borrowing
You can reference a value without taking ownership via references:

Immutable references: &T
Mutable references: &mut T


ConceptDescription
OwnComplete control
BorrowTemporary access
```

Raw mode produces flat text with whitespace normalized but minimal structure preservation. Table data is concatenated without alignment. Code blocks are preserved with their formatting but without syntax highlighting.

### Expected Output — text_mode: "formatted"

```
Understanding Rust Ownership
By Jane Developer

Rust's ownership system is one of its most distinctive features. It allows memory safety without garbage collection.

What is Ownership?
Every value in Rust has a single owner. When the owner goes out of scope, the value is dropped.

fn main() {
    let s = String::from("hello");
    println!("{}", s);
}

Rust combines the efficiency of low-level languages with the safety of high-level ones.

Borrowing
You can reference a value without taking ownership via references:

Immutable references: &T
Mutable references: &mut T

ConceptDescription
OwnComplete control
BorrowTemporary access
```

Formatted mode is similar to raw but may preserve some paragraph breaks and block structure. The exact output depends on how `formatted_text()` handles block elements in the dom_query library.

### Expected Output — text_mode: "markdown"

```markdown
## Understanding Rust Ownership

*By Jane Developer*

Rust's ownership system is one of its most distinctive features. It allows memory safety without garbage collection.

## What is Ownership?

Every value in Rust has a single owner. When the owner goes out of scope, the value is dropped.

```
fn main() {
    let s = String::from("hello");
    println!("{}", s);
}
```

> Rust combines the efficiency of low-level languages with the safety of high-level ones.

## Borrowing

You can reference a value without taking ownership via references:

- Immutable references: `&T`
- Mutable references: `&mut T`

![Example diagram](/example.png)

| Concept | Description |
| --- | --- |
| Own | Complete control |
| Borrow | Temporary access |
```

Markdown mode produces structured output with:
- ATX-style headings (##)
- Emphasis preserved (*for byline*)
- Code blocks with backticks
- Blockquotes with >
- Unordered lists with -
- Images as ![alt](url)
- Tables with pipes

Note: H1 was converted to H2 (page title is separate).

---

## Metadata Extraction

dom_smoothie extracts extensive metadata from:

1. **JSON-LD** (`<script type="application/ld+json">`): title, author, description, publisher site name, published/modified times, image, URL. Only article types are considered (Article, NewsArticle, BlogPosting, etc.)

2. **OpenGraph meta tags**: og:title, og:description, og:image, og:site_name

3. **Dublin Core**: dc:title, dc:creator, dc:description

4. **Twitter Card**: twitter:title, twitter:description, twitter:image

5. **Standard meta tags**: author, description, etc.

6. **HTML**: lang attribute, favicon link tags

The `Article` struct contains: title, byline, content (HTML), text_content, length, excerpt, site_name, dir, lang, published_time, modified_time, image, favicon, url.

---

## Defaults vs Benchmark Configuration

| Parameter | Default | Benchmark Default |
|-----------|---------|-------------------|
| max_elements_to_parse | 0 (unlimited) | None |
| text_mode | "raw" | "markdown" |
| keep_classes | false | false |
| disable_json_ld | false | false |
| n_top_candidates | 5 | 5 |
| char_threshold | 500 | 500 |
| min_score_to_adjust | 5.0 | 5.0 |
| candidate_select_mode | "readability" | "readability" |

Note: The crate's own `Config::default()` has `text_mode: TextMode::Raw`, but `DomSmoothieConfig::default()` has `text_mode: "markdown"`. The benchmark uses `DomSmoothieConfig::default()` which maps to Markdown mode, so the benchmark output is Markdown-formatted text.

---

## Key Files Reference

| File | Purpose |
|------|---------|
| `src/readability.rs` | Main Readability struct, parse logic, metadata extraction |
| `src/grab.rs` | Element collection, scoring, candidate selection |
| `src/score.rs` | Node scoring, class weight calculation |
| `src/prep_article.rs` | Post-extraction cleaning and preparation |
| `src/config.rs` | Config struct, ParsePolicy, TextMode, CandidateSelectMode enums |
| `src/glob.rs` | All static patterns, matchers, word lists |
| `src/helpers.rs` | Utility functions: link_density, text_density, visibility |
| `src/matching.rs` | Pattern matching helpers, schema.org URL validation |
| `src/grab_flags.rs` | FlagSet for StripUnlikelys, WeightClasses, CleanConditionally |
