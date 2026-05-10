# Boilerpipe Extractor Analysis

## Overview

**boilerpipe-rust** (crates.io: `boilerpipe` v0.6.0) is a Rust port of the famous [Java boilerpipe library](https://github.com/kohlschutter/boilerpipe) by Kohlschütter et al. It implements the **Article Extractor** — the standard/default extraction algorithm — and outputs **plain text only**, with no images, links, tables, or markup preserved. The library has no configuration options; it is a fixed-purpose black box.

The library is a direct port of a Go port of the original Java, tracing back to the OOPSLA 2009 paper by Kohlschütter, Fankhauser, and Nejdl: *"Boilerplate Removal from Web Pages — A Context-Based Approach"*.

---

## The Original Java Boilerpipe Algorithm (Kohlschütter et al.)

### Core Idea

Boilerpipe works by parsing HTML into a flat sequence of **text blocks**, then applying a cascade of deterministic label/processing rules that mark blocks as **content** or **boilerplate**. The key insight is that boilerplate removal is a classification problem at the **block level**, where blocks are text chunks delimited by block-level HTML tags.

The original paper describes three families of shallow features used for classification:

1. **Text density** — average word count per line (words / wrapped lines)
2. **Link density** — fraction of words that are inside `<a>` anchor tags
3. **Structure** — tag name, label markers (e.g., `Heading`, `List`), offset positions

The algorithm does NOT use machine learning, NLP, or DOM tree analysis beyond block boundary detection. It is a rule-based cascade.

### Text Block Model

A **TextBlock** is the fundamental unit. It is created whenever the parser encounters:
- Block-level HTML tags (`<p>`, `<div>`, `<tr>`, `<ul>`, `<h1>`–`<h3>`, `<blockquote>`, etc.)
- Forced flushes (certain tags trigger end-of-block)
- Text content bounded by these boundaries

Each TextBlock carries:
- `text` — raw text content (with anchor markers `$\u{e00a}<` and `>\u{e00a}$` injected)
- `num_words` — total word count
- `num_linked_words` — words inside anchor tags
- `num_wrapped_lines` — number of lines when wrapped at ~80 chars
- `num_words_in_wrapped_lines` — total words across all wrapped lines
- `tag_level` — nesting depth at the point of creation
- `label_map` — labels attached (e.g., `Heading`, `List`, `Title`, `EndOfText`)
- `is_content` — the boolean classification flag

Derived features:
```
link_density = num_linked_words / num_words
text_density = num_words_in_wrapped_lines / num_wrapped_lines
```

### Content vs Boilerplate Signals

| Signal | Content Signal | Boilerplate Signal |
|--------|---------------|-------------------|
| **Link density** | <= 0.333 (33%) | > 0.333 |
| **Text density** | >= 9 words/line | < 9 words/line |
| **Num words** | >= 25 in context | < 25 in context |
| **Label** | `Title`, `VeryLikelyContent`, `MightBeContent` | `EndOfText` |
| **Position** | Largest blocks at same tag level | Near navigation, comments sections |
| **Heading label** | Content-adjacent headings | Trailing headlines after content |
| **List label** | Lists at end of content blocks | |

---

## How the Rust Port Works

### Pipeline (in `Document::process()`)

The algorithm applies a sequence of transformations. Each returns `bool` (whether state changed), and all are run sequentially:

```rust
fn process(&mut self) -> bool {
    let mut has_changed = self.terminating_blocks();
    has_changed |= self.document_title_match();
    has_changed |= self.num_words_rules_classifier();
    has_changed |= self.ignore_block_after_content();
    has_changed |= self.trailing_headline_to_boilerplate();
    has_changed |= self.block_proximity_fusion(1, false, false);
    has_changed |= self.boilerplate_block();
    has_changed |= self.block_proximity_fusion(1, true, true);
    has_changed |= self.keep_largest_blocks();
    has_changed |= self.expand_title_to_content();
    has_changed |= self.large_block_same_tag_level_to_content();
    has_changed |= self.list_at_end();
    has_changed
}
```

Each step is described below.

---

### Step 1: `terminating_blocks()`

Finds text blocks that look like comment sections or footer boilerplate and labels them `EndOfText`.

Conditions (all must match):
- Block has fewer than 15 words
- Block text length >= 8 characters

Checklist:
- Text starts with (case-insensitive): "comments", "© reuters", "please rate this", "post a comment"
- Text contains: "what you think...", "add your comment", "add comment", "reader views", "have your say"
- Russian variants: "комментария", "комментариев", "оставьте комментарий", "расскажите нам, что вы думаете"
- Swedish: "rätta artikeln"
- **OR**: link density == 1.0 AND text is exactly "Comment" or "Комментарии"

This handles comment count widgets, "Reader Comments" sections at the bottom of articles.

---

### Step 2: `document_title_match()`

Matches blocks against the `<title>` tag content (extracted separately during parsing).

Title is normalized:
- Replace `\u{00a0}` (non-breaking space) with space
- Remove apostrophes
- Trim and lowercase

Generates "potential titles" via regex patterns:
- Split on `|»,|-` separators at increasing complexity
- Split on `|` and `-` with words >= 4 kept
- Strip trailing patterns like ` - [^-]+$`

Matches blocks against all potential title strings (with punctuation stripped). If a block matches, label it `Title`.

---

### Step 3: `num_words_rules_classifier()` — The Core Classifier

This is the heart of boilerpipe. For each block, it looks at a **sliding window of 3 blocks** (previous, current, next) and applies a decision tree.

```rust
fn classify_is_content(prev: &TextBlock, cur: &TextBlock, next: &TextBlock) -> bool {
    // Branch 1: cur.link_density <= 0.333
    if cur.link_density() <= 0.333 {
        // Branch 1a: prev.link_density <= 0.556
        if prev.link_density() <= 0.555556 {
            // Branch 1a-i: cur.num_words <= 16
            if cur.num_words <= 16 {
                // Branch 1a-i-alpha: next.num_words <= 15
                if next.num_words <= 15 {
                    // Branch: prev.num_words <= 4 -> NOT content
                    //          prev.num_words > 4 -> IS content
                    return prev.num_words <= 4;
                }
                // next.num_words > 15 -> IS content
                return true;
            }
            // cur.num_words > 16 -> IS content
            return true;
        }
        // Branch 1b: prev.link_density > 0.556
        else {
            // Both cur and next must be word-bounded to be content
            if cur.num_words <= 40 && next.num_words <= 17 {
                return false;
            }
            return true;
        }
    }
    // Branch 2: cur.link_density > 0.333 -> NOT content
    else {
        return false;
    }
}
```

**Thresholds:**
- `0.333` (1/3) — link density cap for content
- `0.556` (5/9) — previous block link density threshold
- `16` words — short content threshold
- `40` words — upper bound for short block consideration
- `15` / `17` — next block word count limits

This classifier effectively says:
- Low-link-density blocks with enough words are content
- Short blocks surrounded by low-link-density neighbors are content
- High-link-density blocks are boilerplate (navigation clusters)
- Blocks following highly-linked blocks need more words to be classified content

---

### Step 4: `ignore_block_after_content()`

Scans forward through blocks. Tracks cumulative `num_words` from content blocks (only counting blocks where `text_density >= 9.0`). Once it encounters a block labeled `EndOfText` after accumulating >= 60 such words, it marks **all subsequent blocks** as NOT content.

This removes "comments", "related articles", "more stories" sections that appear after the main article.

---

### Step 5: `trailing_headline_to_boilerplate()`

Iterates **in reverse** from the last block. Stops when it finds a non-content block. All content blocks with the `Heading` label encountered before that stopping point are marked as NOT content.

This removes section headers (H1/H2/H3) that trail after the main content (e.g., "Related Articles", "More from category").

---

### Step 6: `block_proximity_fusion(max_block_distance=1, content_only=false, same_tag_level_only=false)`

Merges consecutive blocks when:
- The gap (difference in `offset_block_end`) is <= `max_block_distance` (1)
- Both blocks have the same `is_content` status
- For `same_tag_level_only=true`: same `tag_level`

The merge combines:
- text (joined with `\n`)
- word counts, link counts, wrapped line stats
- `offset_block_start` = min, `offset_block_end` = max
- `tag_level` = min (keeps shallower depth)
- label maps combined

Called twice: first with `content_only=false` to merge general adjacent boilerplate, then again with `content_only=true, same_tag_level_only=true` to fuse content blocks at the same depth.

---

### Step 7: `boilerplate_block()`

Removes all blocks that are:
- NOT marked `is_content` AND
- Do NOT have the `Title` label

Effectively deletes all remaining boilerplate blocks.

---

### Step 8: `keep_largest_blocks()` — Largest Content Block Detection

Finds the content block with the most `num_words`. This is the "largest block".

**Label assignment:**
- Largest block gets `VeryLikelyContent`
- All other content blocks get `MightBeContent`

**Expand-to-same-level-text** logic:
- Determines `tag_level` of the largest block
- Iterates in **reverse**: any block with `tag_level < level` stops the scan
- At the same `tag_level`, any block with `num_words >= 150` becomes content
- Then iterates forward with the same rule

This means: if the main article is at depth 5, all other content-sized blocks at depth >= 5 with >= 150 words also become content.

**Threshold for being considered largest:**
```rust
fn is_largest_block(max_num_words: usize, tb: &TextBlock) -> bool {
    let min_word_percent = match max_num_words {
        n if n >= 1000 => 0.25,   // 25% of max for large articles
        n if n >= 500  => 0.60,   // 60% of max for medium articles
        _ => tb.is_content && tb.num_words == max_num_words
    };
    tb.is_content && tb.num_words >= (min_word_percent * max_num_words as f64).trunc() as usize
}
```

---

### Step 9: `expand_title_to_content()`

Finds the block labeled `Title` (matched against `<title>` tag) and the first content block. Any blocks between them labeled `MightBeContent` are promoted to content.

This ensures the title block and introductory content between title and first content block are included.

---

### Step 10: `large_block_same_tag_level_to_content()`

After `keep_largest_blocks`, any block (regardless of `is_content` flag) with:
- `num_words >= 100` AND
- `tag_level` equals the `tag_level` of any `VeryLikelyContent` block

is marked content.

This catches large sidebars or supplementary content at the same structural depth as the main article.

---

### Step 11: `list_at_end()`

Finds lists (`Label::List`) that are:
- At `tag_level >` the tag level of the largest `VeryLikelyContent` block
- Labeled `MightBeContent` AND `List`
- With `link_density == 0.0`

These are marked as content. This captures things like "Related Links" or "Further Reading" lists at the end of articles.

---

## Tag Actions (Parsing)

During HTML parsing, each tag triggers an **Action**:

| Tag(s) | Action | Effect |
|--------|--------|--------|
| `script`, `style`, `noscript`, `applet`, `object`, `option` | `Ignore` | Skip content, flush before/after |
| `title` | `Title` | Flush, text captured as document title |
| `time` (with `datetime` attr) | `Time` | Captures publication date |
| `body` | `Body` | Increment body_depth, flush on entry/exit |
| `a` | `Anchor` | Mark text as potentially linked, inject markers |
| `h1`, `h2`, `h3` | `BlockTagLabel([Heading, Heading1/2/3])` | Flush, add heading labels |
| `li` | `BlockTagLabel([List])` | Flush, add list label |
| `abbr`, `b`, `code`, `em`, `font`, `i`, `span`, `strike`, `strong`, `sub`, `sup`, `tt`, `u`, `var` | `Inline` | No block boundary |
| `br`, `hr`, `img`, `input`, `meta`, etc. | `IgnoreVoid` | No content, no depth change |
| Everything else | `Inline` (unknown tags) | No block boundary |

**Note**: The tag list is **limited** — there is no handling for `div`, `p`, `article`, `section`, `blockquote`, `td`, `th`, `form`, `table`, etc. These default to `Inline` behavior, meaning they do NOT cause block boundaries. This is a key limitation vs. the original Java library.

---

## Handling of Specific Content Types

### Headings (H1–H3)

- Tagged with `Heading` label and specific level (Heading1/2/3)
- Subject to `trailing_headline_to_boilerplate()` — headings at the END of content are removed
- `expand_title_to_content()` can promote heading blocks between title and content
- `large_block_same_tag_level_to_content()` can keep large heading blocks

### Links

- Link density is the PRIMARY signal for classification
- Anchor text is tracked separately (`num_linked_words`)
- Blocks with >33% linked words are almost always boilerplate
- The original anchor text markers (`$\u{e00a}<...>\u{e00a}$`) are stripped from output

### Images

- **NOT extracted**. The library outputs text-only. `<img>` tags trigger `IgnoreVoid` action and are completely dropped.
- No `alt` text is captured (the Rust port does not implement image extraction).

### Code Blocks

- No special handling. The Rust port does not implement any code block recognition.
- Text inside `<code>`, `<pre>` is treated as regular inline content (or block content if inside a block-level tag).
- The tokenization regex strips punctuation, which would corrupt code syntax.

### Tables

- `<table>`, `<tr>`, `<td>`, `<th>` are NOT in the tag list — they default to `Inline`
- Table cells parsed as inline content, interleaved with cell boundaries (no row/column structure preserved)
- No column or cell boundary markers in output

### Lists

- `<li>` triggers `BlockTagLabel([List])` and flush — lists get the `List` label
- `list_at_end()` can promote trailing lists to content if they're at deeper tag levels and have no links
- But there is NO markdown or structured list output — raw text only

### Emphasis (Bold, Italic, etc.)

- `<b>`, `<i>`, `<em>`, `<strong>`, `<u>`, `<strike>`, `<tt>`, `<sub>`, `<sup>`, `<abbr>`, `<var>`, `<font>` — all `Inline`
- No bold/italic markers in output; text is plain

---

## Output Format

**Plain text only.** No markdown, no HTML, no structure.

- Text from all `is_content=true` blocks, concatenated with `\n` between blocks
- No separators between merged blocks within a single text block
- Anchor markers stripped
- No保留任何 markup, markdown, or structured formatting
- Document title stored in `Document.title` but NOT included in `.content()` output
- Publication time stored in `Document.time` but NOT in output

```rust
// From scores.rs — the usage in this benchmark:
boilerpipe::parse_document(html).content().to_string()
```

---

## Edge Cases and Known Limitations

### 1. Limited Tag Set

The Rust port recognizes only a fixed set of ~40 tags. Unrecognized block-level tags like `div`, `p`, `article`, `section`, `blockquote`, `table`, `form` default to `Inline`, meaning they do NOT create block boundaries. This can cause content and boilerplate to be merged into the same block, degrading extraction quality.

The original Java library has a much more complete tag set.

### 2. No Configuration

The library exposes zero configuration — no selector options, no extractor choice, no parameters. It is a fixed algorithm. In the benchmark code, it is called as:

```rust
boilerpipe::parse_document(html).content().to_string()
```

### 3. Text-Only Output

There is no way to get links, images, tables, or structure back. Everything beyond plain text is discarded.

### 4. Programming Language Detection

There's a heuristic that filters out text chunks that look like code (≥10% of words are `string`, `array`, `bool`, `false`, `true`, `int`). This is overly broad and could filter legitimate content in technical articles.

### 5. JSON in HTML

A regex strips JSON-like fragments `[{...}]` from text before processing, which could remove valid content from pages embedding JSON data in the HTML body.

### 6. Language Sensitivity

The `terminating_blocks()` method hardcodes phrases in English, Russian, and Swedish. Pages in other languages with comment sections won't have them properly removed.

### 7. Fixed Line Length

Line wrapping is calculated at exactly 80 characters (`MAX_LINE_LENGTH: usize = 80`). This affects `text_density` calculation. No configuration is available.

### 8. No Image Alt Text

Images are ignored entirely; `alt` attributes are not extracted as fallback content.

### 9. No Table Structure

Table cells are treated as plain text sequences. There's no column or row boundary preservation.

### 10. Title Matching is Imperfect

The title matching normalizes and splits titles, but if the page title is very different from article headings (e.g., heavily truncated, or with site name prefix), the match may fail and the title block won't be labeled.

---

## Relationship to Original Java Boilerpipe

The Rust port (`0nkery/boilerpipe-rs`) is a port of `jlubawy/go-boilerpipe`, which is itself a Go port of the original Java. The Java library has:

- **Multiple extractors**: `ArticleExtractor`, `DefaultExtractor`, `LargestContentExtractor`, `NumWordsRulesExtractor`, `CanolaExtractor`
- **Full DepthContentObserver** that tracks all tag levels properly
- **Complete tag set** including `div`, `p`, `article`, `section`, `table`, etc.
- **Image extraction** via `ImageExtractor`
- **Fine-tuning via `BoilerpipeFilter` interfaces`
- **More sophisticated label propagation**

The Rust port implements **only** `ArticleExtractor` behavior (the default, best-performing extractor), and only the text content part. It is a faithful but simplified port.

---

## Example: Sample Input and Output

### Sample Input HTML

```html
<!DOCTYPE html>
<html>
<head>
  <title>Why Cats Purr — Pet Science Daily</title>
  <meta charset="utf-8">
</head>
<body>
  <nav>
    <a href="/">Home</a>
    <a href="/care">Care</a>
    <a href="/feeding">Feeding</a>
  </nav>

  <header>
    <h1>Pet Science Daily</h1>
  </header>

  <article>
    <h2>Why Cats Purr</h2>
    <p>Cats purr using their laryngeal muscles. The exact mechanism involves
    rapid oscillation of the vocal cords. Scientists believe purring evolved
    as a form of communication.</p>
    <p>Research shows that purring occurs at 25-150 Hz. These frequencies may
    promote bone density and healing. Not all cats can purr — big cats
    like lions cannot.</p>
    <img src="/cat.jpg" alt="A domestic cat">
    <p>Here are some related links:</p>
    <ul>
      <li><a href="/cats/effects">Health Benefits of Cat Ownership</a></li>
      <li><a href="/cats/anatomy">Feline Anatomy Overview</a></li>
    </ul>
  </article>

  <aside>
    <h3>Advertisement</h3>
    <p>Buy premium cat food today!</p>
  </aside>

  <footer>
    <a href="/about">About</a>
    <a href="/privacy">Privacy</a>
    <p>© 2025 Pet Science Daily. All rights reserved.</p>
  </footer>

  <script>
    // analytics tracking code
    var _gaq = [];
  </script>
</body>
</html>
```

### Expected Output (plain text)

```
Cats purr using their laryngeal muscles. The exact mechanism involves
rapid oscillation of the vocal cords. Scientists believe purring evolved
as a form of communication.

Research shows that purring occurs at 25-150 Hz. These frequencies may
promote bone density and healing. Not all cats can purr — big cats
like lions cannot.

Here are some related links:

Health Benefits of Cat Ownership
Feline Anatomy Overview
```

### What Gets Removed

- Navigation (`<nav>`, the `Home`, `Care`, `Feeding` links) — high link density
- Header (`<header><h1>`) — too few words in context
- The article heading `Why Cats Purr` — trailing headline after content
- Advertisement sidebar (`<aside>`) — low text density, high boilerplate signals
- Footer links and copyright — high link density, short text
- Script content (`<script>`) — stripped by ignore rules
- The image tag — not text, ignored
- The list items — `list_at_end()` would keep them if link_density=0 (these have links so they'd be stripped unless they qualify under list_at_end rules)

---

## Benchmark Integration Notes

From `scores.rs` lines 329–334:

```rust
#[cfg(feature = "boilerpipe")]
"boilerpipe" => {
    runner.run(output_name, |html| {
        boilerpipe::parse_document(html).content().to_string()
    });
}
```

The benchmark uses the default configuration with no parameters. The output is plain text written to `{output_name}.txt`.

**No extractor-specific configuration is defined** in `extractor_config.rs` for boilerpipe. The `ExtractorConfig` struct has no boilerpipe-specific fields — the library is used as-is.

---

## Key References

- Original Java: https://github.com/kohlschutter/boilerpipe
- OOPSLA 2009 Paper (Kohlschütter et al.): *"Boilerplate Removal from Web Pages — A Context-Based Approach"* — introduced the text block model, link/text density features, and cascading rule-based classification
- Go port: https://github.com/jlubawy/go-boilerpipe
- Rust port: https://github.com/0nkery/boilerpipe-rs (crates.io: `boilerpipe` v0.6.0)
- Related: *"Boilerplate detection using shallow text features"* (Kohlschütter et al., 2010) — extended feature set including POS tags and n-grams

---

## Summary

Boilerpipe is a **rule-based, block-level content extraction** algorithm. It does NOT use machine learning or DOM structure analysis beyond block boundary detection. The core insight is that boilerplate can be distinguished from content using just three signals: **link density**, **text density**, and **block size** in context. The algorithm applies a cascade of deterministic transformations that merge, label, and filter blocks until only content remains.

The Rust port faithfully implements the `ArticleExtractor` algorithm from the original Java, but with a reduced tag set and no configuration options. It is a pure text extractor — no links, images, tables, or markdown are preserved. The algorithm is deterministic and fast, but brittle to page structure variations outside its tag model.
