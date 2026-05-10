# The llm_readability Extractor: An In-Depth Analysis

The `llm_readability` crate (version 0.0.17 used in this benchmark) is a Rust implementation of the classic Readability content extraction algorithm, maintained by Jeff Mendez as part of the spider-rs project. Despite its name, this extractor is NOT powered by an LLM—the "llm" prefix indicates that it is optimized for producing clean text suitable for LLM consumption, not that it uses LLM technology internally. This document provides a comprehensive technical analysis of this extractor's behavior, heritage, algorithm, and expected outputs.

## Overview and Critical Clarification

### The "LLM" Misconception

One of the most important aspects to understand about `llm_readability` is that despite its name suggesting LLM involvement, it is actually a traditional algorithmic content extractor that does not invoke any language model APIs or require LLM access. The crate was created and is used in production at Spider Cloud for data cleaning purposes, specifically to produce clean, well-formatted text that is ideal for feeding into Large Language Models for further processing.

The name can be interpreted in two ways: either as "readability for LLMs" (a tool that prepares content for LLM pipelines) or as a branding choice from the Spider project (which builds various LLM-related tools). Either interpretation confirms that the crate is designed to serve LLM data preparation workflows rather than being LLM-powered itself.

This is a significant distinction from other extractors in this benchmark that might genuinely use LLM APIs for extraction decisions. `llm_readability` runs entirely locally, requires no API keys, and produces deterministic output based purely on its algorithmic implementation.

## Heritage and Implementation

### Relationship to readability-rs

The `llm_readability` crate explicitly states in its documentation that it is "a rewrite of `readability-rs` for performance and bug fixes." This means it implements the same fundamental Arc90/Mozilla Readability algorithm that powers Firefox's Reader View, but with improvements targeted at performance optimization and known issue resolution.

The original `readability-rs` crate was a Rust port of the JavaScript Readability algorithm developed by Arc90 and refined by Mozilla. The `llm_readability` project took this implementation and refactored it to address performance concerns and fix various bugs that had accumulated in the original port.

### Core Dependencies

The crate relies on several key Rust libraries for its functionality:

- **html5ever** (^0.27 or ^0.39 in newer versions): A Rust implementation of the HTML5 parsing algorithm, providing safe and fast HTML parsing with excellent Rust memory safety guarantees.
- **markup5ever** (^0.13 or ^0.39): Provides the markup document handling capabilities, working in conjunction with html5ever.
- **markup5ever_rcdom**: The DOM tree construction implementation that builds the in-memory document structure.
- **regex** (^1): Used for pattern matching against class names, IDs, and other attributes to determine content relevance scores.
- **url** (^2): Provides URL parsing and resolution capabilities, essential for converting relative URLs to absolute URLs.
- **auto_encoder**: Used for encoding operations within the extraction pipeline.

This dependency set is notably focused on parsing and DOM manipulation rather than any AI or machine learning libraries, confirming the non-LLM nature of the implementation.

## API and Usage in the Benchmark

### Function Signature

In the benchmark, the extractor is invoked through the following code pattern visible in `src/scores.rs` around lines 132-141:

```rust
#[cfg(feature = "llm_readability")]
"llm_readability" => {
    let parsed_url = parsed_url.clone();
    runner.run(output_name, move |html| {
        let mut html = std::io::Cursor::new(html.as_bytes());
        llm_readability::extractor::extract(&mut html, &parsed_url)
            .unwrap()
            .text
    });
}
```

The extract function signature in version 0.0.17 is:

```rust
pub fn extract<R>(input: &mut R, url: &Url) -> Result<Product, Error>
where
    R: Read,
```

This takes a mutable reference to any type implementing the `Read` trait (in this case, a `Cursor` wrapping the HTML bytes) and a parsed `Url` object. It returns a `Result` containing a `Product` on success.

### The Product Struct

The extraction returns a `Product` struct containing two fields:

```rust
pub struct Product {
    pub content: String,  // The cleaned HTML content
    pub text: String,     // The plain text extraction
}
```

The benchmark specifically uses the `.text` field, which contains the plain text with all HTML tags removed, matching the behavior of the standard `readability` crate in this benchmark.

### The URL Parameter: Why It Is Required

The URL parameter serves a critical function in the extraction process: resolving relative URLs to absolute URLs. When the HTML contains references to resources like images, links, or other assets using relative paths (such as `/images/logo.png` or `../styles.css`), the algorithm needs a base URL to convert these into proper absolute URLs.

This resolution is important for several reasons:

1. **Image Path Fixing**: The `fix_img_path` function in the scorer module uses the base URL to convert relative image sources to absolute URLs, ensuring that image references remain valid in the extracted content.

2. **Anchor Path Fixing**: Similarly, the `fix_anchor_path` function resolves relative link targets to absolute URLs, which is particularly important when the extracted content needs to maintain valid hyperlink references.

3. **Context Understanding**: The URL provides context about the page's origin, which can be used in some scoring decisions (though this varies by implementation).

The URL must be a properly parsed `url::Url` type, not a raw string. This is passed to the extract function and used internally for all URL resolution operations. Without a valid base URL, relative references would remain as-is, which could break when the content is used in isolation.

## Content Selection Algorithm

### Scoring Fundamentals

The `llm_readability` implementation uses the same fundamental scoring approach as the classic Readability algorithm. The algorithm works by analyzing the document structure and assigning scores to different elements based on multiple heuristics, then selecting the highest-scoring container as the main content area.

The scoring process begins by identifying candidate nodes that are likely to contain meaningful content. These primarily include paragraph elements (`p`), preformatted blocks (`pre`), table cells (`td`), heading elements (`h1` through `h6`), and certain div elements that survive initial filtering.

Each candidate node receives a base score determined by its tag type, with different tag types contributing different base values to the overall score. Text content within these elements is then analyzed for additional scoring signals.

### The Candidate System

The scorer module maintains several static lists that guide the scoring process:

- **POSITIVE_CANDIDATES**: Class/ID patterns that indicate likely content (like "content", "article", "entry", "post", "text", "blog", "story", "main")
- **NEGATIVE_CANDIDATES**: Class/ID patterns that indicate non-content (like "comment", "meta", "footer", "footnote", "sidebar", "aside", "advert", "social", "share", "nav", "menu", "header")
- **LIKELY_CANDIDATES**: Tags that commonly contain main content
- **UNLIKELY_CANDIDATES**: Tags that are rarely main content
- **PUNCTUATIONS_REGEX**: Used to count sentence-delimiting punctuation

The `is_candidate` function determines whether an element is a valid content candidate based on these patterns, while `is_useless` identifies elements that should be filtered out entirely.

### Score Propagation

After calculating individual node scores, the algorithm propagates scores upward through the DOM tree. Each parent element receives a percentage of its children's scores, with grandparents receiving a smaller percentage. This hierarchical scoring ensures that the container holding the most valuable content ultimately receives the highest score.

The `calc_content_score` function handles this propagation and also applies link density penalties, which are crucial for distinguishing between actual article content and navigation elements.

### Class Weight Calculation

The `get_class_weight` function examines an element's class and ID attributes against the positive and negative candidate patterns. Elements with matching positive patterns receive score boosts, while those matching negative patterns receive penalties. This heuristic is particularly effective for modern websites using semantic HTML and meaningful class names, though it can occasionally misfire on sites with unconventional naming schemes.

### Link Density Analysis

Link density is one of the most powerful signals in the Readability algorithm. It measures the proportion of text within an element that consists of hyperlink text. Navigation menus typically have very high link density (80-100%) because they consist almost entirely of links, while article content usually has low link density (under 20%) because most text is body content.

The `get_link_density` function calculates this by summing the length of all anchor text within an element and dividing by the total text length. Elements exceeding certain link density thresholds receive significant score penalties, effectively filtering out navigation blocks, related post links, and similar non-content elements.

### Preprocessing

The `preprocess` function handles initial DOM adjustments before scoring begins. This may include removing script tags, style tags, and other elements that should not influence the scoring process. The preprocessing step sets up a clean document structure for the scoring algorithm to work with.

## Element Handling

### Headings

Headings (h1 through h6) are preserved in the output as they are important structural elements that delineate sections of content. The heading text appears in the extracted plain text, maintaining the document's hierarchical structure through line breaks and positioning. The algorithm assigns positive scores to heading elements based on their tag type.

### Links

Links are handled in a nuanced way depending on whether the output is HTML or plain text. In the plain text output (which the benchmark uses), links appear only as their anchor text—the URLs themselves are not visible. The presence of links contributes to link density calculations, so a paragraph with many hyperlinks would receive a lower score than one with mostly plain text.

The algorithm attempts to preserve the text content of links while stripping the anchor tags. If a link contains only text (no nested elements), that text is included in the output. Links that are nested within other content contribute to the overall text but don't appear as explicit hyperlinks in plain text output.

### Images

Images are handled through the image path fixing mechanism. The base URL is used to convert relative image `src` attributes to absolute URLs. In the plain text output, images are not directly represented (as they are not text), but the algorithm processes them as part of the content analysis.

The `fix_img_path` function specifically handles the conversion of relative image paths to absolute paths using the provided base URL. This is particularly important for content that will be used in isolation, where relative paths would become broken references.

### Code Blocks

Code blocks (pre and code elements) are preserved in the output. The text content within these elements maintains its whitespace formatting, which is critical for code readability. The plain text output includes the raw code content without any syntax highlighting or markup.

The algorithm assigns positive scores to pre elements because they typically contain substantive content (code snippets, terminal output, etc.) rather than navigation or metadata. The scoring algorithm recognizes that preformatted text blocks are generally meaningful content worth preserving.

### Tables

Table cells (td elements) are considered candidate content elements and can contribute to the overall content score. Table structure is preserved in the sense that the text content from cells is extracted, though in plain text output, the tabular formatting is lost and cells appear as sequential text paragraphs.

Tables can be important content carriers on many websites, particularly for data-heavy content, financial articles, or comparison pages. The Readability algorithm includes tables in its candidate analysis rather than filtering them out.

### Lists

List items (li elements) are preserved in the output. In plain text mode, the bullet markers or numbering are not explicitly preserved—list items appear as sequential text. The algorithm considers list items as content candidates and includes their text in the scoring process.

### Emphasis (Bold, Italic, Underline)

All HTML formatting, including bold (`strong`, `b`), italic (`em`, `i`), underline (`u`), and other text styling, is completely stripped in the plain text output. There is no way to distinguish bold or italic text in the extracted text—it all appears as plain characters. This is a fundamental characteristic of plain text extraction rather than a specific Readability limitation.

The plain text output contains only the textual content without any markup or styling information. If you need formatting preserved, you would use the `content` field (HTML output) or choose a different extractor that produces markdown or formatted text.

## What Gets Stripped

The extraction process systematically removes several categories of elements:

### Scripts and Styles

JavaScript tags (`<script>`) and their contents are completely removed. Similarly, style tags (`<style>`) and their CSS content are stripped. These elements are identified during preprocessing and do not contribute to the content scoring.

### Navigation Elements

Elements commonly used for navigation are typically filtered out based on their class/ID patterns or link density. Navigation menus (often in `nav` elements or divs with nav-related classes) have very high link density, causing them to fail the content density threshold and receive low scores.

The algorithm specifically penalizes elements with class/ID patterns like "nav", "menu", "navigation", "header", and "footer", making it effective at removing these non-content sections.

### Advertising and Sidebars

Advertisement elements and sidebar content typically have high link density or are identified through negative class patterns (like "ad", "advertisement", "sidebar", "aside", "social", "share"). These elements rarely survive the scoring process as main content candidates.

### Empty and Minimal Content

Elements with very little text content (typically under 25 characters) receive minimal or zero scores. This filters out metadata, short labels, and other minimal content that is unlikely to constitute main article content.

### High Link Density Containers

Any element where more than a certain percentage (typically around 50-60%) of the text consists of hyperlinks will receive significant score penalties, often eliminating them from consideration as the main content container.

## Edge Cases and Operational Characteristics

### No LLM Required

As established, `llm_readability` does not require any LLM access. It runs entirely locally using deterministic algorithms. This means:

- No API keys are needed
- No network calls to LLM services
- No latency from LLM processing
- No cost for LLM API usage
- Fully deterministic output (same input always produces same output)

### No External Dependencies Beyond Crates

The extractor has no runtime dependencies on external services. All processing happens locally within the Rust application. This makes it suitable for embedding in applications that need to work offline or in environments with restricted network access.

### Error Handling

The extract function returns a `Result<Product, Error>`, so callers must handle potential errors. Common error conditions might include:

- Malformed HTML that cannot be parsed
- Invalid URL provided as the base
- Memory constraints on very large documents
- Empty or invalid input

The benchmark uses `.unwrap()` on the result, which would panic on error. In production use, proper error handling would be advisable.

### Performance Characteristics

The crate was explicitly rewritten for performance compared to the original readability-rs. The implementation uses efficient Rust data structures and avoids unnecessary allocations where possible. The html5ever parser provides fast HTML parsing with Rust's memory safety guarantees.

Performance characteristics would be favorable compared to any solution requiring external API calls, as there is no network latency involved.

### Version Considerations

The benchmark uses version 0.0.17 of llm_readability. Earlier versions (as seen in the crates.io documentation) had a different API signature that accepted a string URL and an optional third parameter. The current version uses the `url::Url` type directly, which is more type-safe but requires the caller to parse the URL first.

## Comparison with Standard Readability

The `llm_readability` crate and the standard `readability` crate (version 0.3.0 used in this benchmark) share the same fundamental algorithm—both implement the Arc90/Mozilla Readability approach. However, there are several distinctions:

### Performance Optimization

The `llm_readability` documentation explicitly states it is a rewrite "for performance and bug fixes." This suggests the implementation may have algorithmic improvements or implementation optimizations that improve runtime performance compared to the original readability-rs.

### Different Maintenance

The standard `readability` crate is maintained by Hiroki Kumamoto as a direct port of Mozilla's JavaScript Readability. The `llm_readability` crate is maintained by Jeff Mendez as part of the spider-rs project, which focuses on web scraping and data extraction tools.

### API Changes

The APIs are similar in concept (both accept HTML and URL, both return content and text), but the exact signatures differ. The benchmark code shows both being used in nearly identical ways for the purposes of this comparison.

### Output Similarity

For typical web pages, both extractors should produce very similar output since they implement the same algorithm. Differences might appear in edge cases where the implementations have diverged in their handling of specific DOM structures or in performance/robustness rather than output quality.

## Sample Input and Expected Output

Consider the following HTML input representing a typical blog article with navigation, sidebar, and main content:

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>Rust Programming Tutorial - DevBlog</title>
</head>
<body>
    <header class="site-header">
        <nav>
            <a href="/">Home</a>
            <a href="/tutorials">Tutorials</a>
            <a href="/about">About</a>
        </nav>
    </header>
    
    <aside class="sidebar">
        <h3>Popular Posts</h1>
        <ul>
            <li><a href="/post/javascript-basics">JavaScript Basics</a></li>
            <li><a href="/post/python-intro">Python Introduction</a></li>
            <li><a href="/post/go-web">Go Web Development</a></li>
        </ul>
        <div class="ad-container">
            <img src="/ads/banner.png" alt="Advertisement">
        </div>
    </aside>
    
    <main class="content">
        <article>
            <h1>Rust Programming: A Comprehensive Guide</h1>
            <p class="meta">Published by Jane Developer on January 15, 2024</p>
            
            <p>Rust is a systems programming language that prioritizes safety, concurrency, and performance. Unlike other systems languages, Rust guarantees memory safety without a garbage collector, making it ideal for embedded systems and performance-critical applications.</p>
            
            <h2>Why Choose Rust?</h2>
            <p>There are several compelling reasons to learn Rust:</p>
            
            <ul>
                <li><strong>Memory Safety:</strong> The ownership system prevents buffer overflows and null pointer dereferences at compile time.</li>
                <li><strong>Zero-Cost Abstractions:</strong> High-level features don't add runtime overhead.</li>
                <li><strong>Concurrent Safety:</strong> Data races are prevented at compile time.</li>
            </ul>
            
            <h2>Getting Started</h2>
            <p>To begin programming in Rust, you'll need to install the toolchain:</p>
            
            <pre><code>curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
rustc --version</code></pre>
            
            <p>Once installed, you can create your first program:</p>
            
            <pre><code>fn main() {
    println!("Hello, Rust!");
}</code></pre>
            
            <p>Run it with <code>rustc main.rs && ./main</code> to see your output.</p>
        </article>
    </main>
    
    <footer>
        <p>&copy; 2024 DevBlog. All rights reserved.</p>
    </footer>
</body>
</html>
```

When processed by the `llm_readability` extractor with the URL parameter set to "https://devblog.example.com/tutorials/rust-guide", the expected plain text output would be:

```
Rust Programming: A Comprehensive Guide

Published by Jane Developer on January 15, 2024

Rust is a systems programming language that prioritizes safety, concurrency, and performance. Unlike other systems languages, Rust guarantees memory safety without a garbage collector, making it ideal for embedded systems and performance-critical applications.

Why Choose Rust?

There are several compelling reasons to learn Rust:

Memory Safety: The ownership system prevents buffer overflows and null pointer dereferences at compile time.

Zero-Cost Abstractions: High-level features don't add runtime overhead.

Concurrent Safety: Data races are prevented at compile time.

Getting Started

To begin programming in Rust, you'll need to install the toolchain:

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
rustc --version

Once installed, you can create your first program:

fn main() {
    println!("Hello, Rust!");
}

Run it with rustc main.rs && ./main to see your output.
```

Several important observations about this output:

1. **Navigation and sidebar removed**: The header with navigation links, the sidebar with popular posts and advertisement, and the footer are all completely eliminated from the output.

2. **Main content preserved**: The article content within the main element is preserved, including headings, paragraphs, lists, and code blocks.

3. **Title included**: The main heading "Rust Programming: A Comprehensive Guide" appears at the top of the output, extracted from the h1 element within the article.

4. **Metadata preserved**: The byline "Published by Jane Developer on January 15, 2024" is included because it appeared within the article content area.

5. **Code blocks maintain formatting**: The preformatted code blocks preserve their whitespace and line breaks, which is essential for code readability.

6. **List structure flattened**: The unordered list items appear as separate paragraphs with the bullet points implicit rather than explicit. The leading "li" markers are not included.

7. **All formatting stripped**: There is no way to distinguish bold text (the strong elements in the list items) in the output. The emphasis is lost in plain text mode.

8. **Links appear only as text**: The anchor text is preserved but the URLs are not visible. The navigation links, sidebar links, and inline links all appear only as their display text.

9. **Images not represented**: The advertisement image is completely removed, and in the text output, there is no representation of image content (images aren't text).

10. **Whitespace preserved in code**: The code blocks preserve the exact whitespace from the original, including indentation and line breaks.

## Configuration Options

Unlike some extractors in this benchmark that expose numerous configuration options through the `ExtractorConfig` system, `llm_readability` does not have any exposed configuration settings in the benchmark. The crate's design philosophy follows the same approach as the original Readability—optimize for simplicity and robustness with sensible defaults.

The `extractor_config.rs` file in this benchmark does not define any specific configuration options for `llm_readability`. It is simply enabled or disabled via the feature flag in Cargo.toml:

```toml
llm_readability = { version = "0.0.17", optional = true }
```

The extraction behavior is controlled entirely by the algorithm's internal heuristics and the URL parameter provided at runtime. There is no way to customize:

- Scoring weights
- Positive/negative pattern lists
- Link density thresholds
- Content length thresholds
- Output format preferences

This is consistent with the Readability philosophy of providing a "just works" extraction experience without requiring users to understand or tune the underlying algorithm.

## Summary

The `llm_readability` crate is a Rust implementation of the classic Arc90/Mozilla Readability algorithm, rewritten for performance and bug fixes from the original readability-rs. Despite its name suggesting LLM involvement, it is entirely algorithmic and requires no LLM access—the "llm" prefix indicates it is designed for LLM data preparation workflows.

Key characteristics:

- **Algorithm**: Classic Readability scoring based on content density, link density, class/id heuristics, and tag type analysis
- **URL requirement**: Essential for resolving relative URLs to absolute URLs for images and links
- **Output**: Plain text with all HTML formatting stripped; also provides HTML output via the `content` field
- **Elements preserved**: Headings, paragraphs, lists, code blocks (with whitespace), table text content
- **Elements stripped**: Scripts, styles, navigation, sidebars, ads, high link-density containers
- **No external dependencies**: Runs entirely locally, no API keys required, no network calls needed
- **Performance**: Optimized Rust implementation designed for efficiency

This extractor is well-suited for scenarios where the goal is to extract clean, plain text article content from web pages. It is particularly appropriate for preparing content for LLM processing pipelines, as indicated by its name, where deterministic output and no external dependencies are valuable characteristics.
