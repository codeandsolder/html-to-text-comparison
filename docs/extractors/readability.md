# The Readability Extractor: An In-Depth Analysis

The `readability` crate used in this benchmark (version 0.3.0) is a Rust port of Arc90's Readability algorithm, which originally powered the famous Readability bookmarklet and later became the foundation for Firefox's Reader View. This document provides a comprehensive technical analysis of how this extractor works, what it preserves and discards, and what behaviors users can expect when using it in the html-to-text-comparison benchmark.

## Overview and Heritage

The `readability` crate is maintained by Hiroki Kumamoto and serves as a direct Rust port of the original JavaScript implementation from Mozilla. The algorithm was originally developed by Arc90, a consulting firm that released their work as open source in 2009. Mozilla later adopted and refined the algorithm for use in Firefox's Reader View, making it one of the most battle-tested content extraction algorithms in existence.

In the benchmark, this extractor is invoked through the following code pattern visible in `src/scores.rs` around lines 119-128:

```rust
"readability" => {
    let parsed_url = parsed_url.clone();
    runner.run(output_name, move |html| {
        let mut html = std::io::Cursor::new(html.as_bytes());
        readability::extractor::extract(&mut html, &parsed_url)
            .unwrap()
            .text
    });
}
```

The extractor takes a mutable reader containing the HTML bytes and a parsed URL object. It returns a `Product` struct containing three fields: `title`, `content` (the cleaned HTML), and `text` (plain text extraction). The benchmark specifically uses the `.text` field, which represents the plain text content with all HTML tags removed.

## Content Selection Algorithm

The Readability algorithm works by analyzing the document structure and assigning scores to different elements based on multiple heuristics. The fundamental approach is to score individual text-containing blocks and then propagate those scores upward through the DOM tree to identify the best container element.

### Scoring Fundamentals

The algorithm begins by identifying candidate nodes that are likely to contain meaningful content. These primarily include paragraph elements (`p`), preformatted blocks (`pre`), table cells (`td`), heading elements (`h1` through `h6`), and certain div elements that survive initial filtering. Each candidate node receives a base score determined by its tag type.

The scoring formula for text content considers several factors. Text shorter than 25 characters receives zero points because such short snippets rarely constitute meaningful content. The algorithm counts commas in the text, adding one point per comma found, since commas typically indicate complete sentences and therefore higher content density. A small bonus is awarded based on text length, capped at a maximum of three additional points for every 100 characters of text. This length-based scoring acknowledges that longer paragraphs are more likely to be substantive content rather than navigation or metadata.

After calculating individual node scores, the algorithm propagates these scores upward through the DOM tree. Each parent element receives a percentage of its children's scores, with grandparents receiving a smaller percentage. This approach ensures that the container holding the most valuable content ultimately receives the highest score, even if that content is distributed across multiple child elements.

### Class and ID Heuristics

The algorithm applies significant weight to CSS class and ID attributes when making scoring decisions. Elements with class or ID values containing certain keywords receive positive or negative score adjustments. Positive indicators include terms like "content", "article", "entry", "post", "text", "blog", "story", and "main". Negative indicators include "comment", "meta", "footer", "footnote", "sidebar", "aside", "advert", "social", "share", "nav", "menu", "header", and similar terms commonly used for non-content sections.

This heuristic proves particularly effective for modern websites that use semantic HTML and meaningful class names. However, it can occasionally misfire on sites that use unconventional naming schemes or that have content areas with neutral class names that get penalized.

### Link Density Scoring

One of the most powerful signals the algorithm uses is link density, which measures the proportion of text within an element that consists of hyperlink text. Navigation menus typically have very high link density because they consist primarily of links to other pages. Article content, by contrast, usually has low link density because most of the text is body content rather than links.

The algorithm calculates link density by summing the length of all anchor text within an element and dividing by the total text length of that element. Elements with link density exceeding certain thresholds receive significant score penalties. This mechanism effectively distinguishes between navigation blocks and actual content.

### Unlikely Candidates Removal

Before the main scoring phase, the algorithm performs an initial pass to identify and remove "unlikely candidates" - elements that exhibit characteristics strongly associated with non-content areas. This filtering uses regex patterns to examine class names, IDs, and other attributes for patterns commonly found in navigation, advertising, and auxiliary page elements.

Elements matching these patterns are flagged early and typically excluded from scoring. However, the algorithm maintains a fallback mechanism: if the initial extraction produces insufficient content (less than the character threshold, which defaults to 500 characters), it retries with progressively relaxed filtering rules. This retry mechanism ensures that the algorithm can handle pages where aggressive filtering accidentally removes real content.

## Element-Type Handling

The Readability algorithm treats different HTML element types in specific ways, with each category receiving distinct processing logic.

### Headings

Heading elements (`h1` through `h6`) receive positive scoring weight because they typically denote section boundaries within articles. The algorithm preserves heading hierarchy in the output and includes their text content in the scoring calculation. In the plain text output, headings appear as their text content, typically with some form of visual separation from surrounding paragraphs, though the exact formatting depends on subsequent text processing.

### Links

Links are preserved in the output but their handling depends on whether the algorithm considers them part of the main content or auxiliary navigation. Content-relevant links (those within the selected article container) are retained along with their anchor text. Navigation links in removed sections are naturally eliminated during the content selection process.

The algorithm does not convert links to any particular format - they remain as they existed in the source HTML until the final text extraction step. In the plain text output, links appear as their anchor text, with the href information not included in the text version. This is a crucial distinction: the `.text` field contains no hyperlink destinations, only the visible link text.

### Images

Images within the selected content container are preserved in the HTML output (`content` field). The algorithm also attempts to fix image sources, converting relative URLs to absolute URLs when a base URL is provided. However, in the plain text output (`text` field), images are represented simply as their alt text if available, or omitted entirely if no alt text exists. The plain text version fundamentally cannot represent images as visual elements.

The algorithm detects lazy-loaded images and handles noscript fallbacks, attempting to extract the actual image source from within noscript elements that often contain fallback content for JavaScript-disabled browsers.

### Code Blocks

Preformatted code blocks (`pre` elements containing `code`) receive positive scoring weight because they typically represent substantive technical content. The algorithm preserves code block structure and content. In the plain text output, code appears with preserved whitespace and line breaks, though without any syntax highlighting since that information exists only in the HTML rendering layer.

### Tables

Table cells (`td` elements) are among the candidate node types that receive scoring weight. The algorithm can identify tabular data as content when it appears within the selected container. However, table structure (rows, columns, headers) is not explicitly preserved in the plain text output - tables are rendered as space-separated or newline-separated text content from the cells.

### Lists

List items (`li` elements) receive scoring consideration similar to paragraph elements. Ordered and unordered lists within the selected content area are preserved. In the plain text output, list items appear with their content, typically with some form of bullet or numbering prefix that the extraction process adds based on the list type.

### Emphasis

The algorithm preserves inline formatting elements like `strong`, `b`, `em`, and `i` within the HTML output. However, these formatting elements are stripped during the conversion to plain text. The `.text` field contains no bold, italic, or other typographic emphasis - only the raw text content without any markup indicates emphasis. This represents a significant limitation for users who need to preserve document structure and formatting.

## Content Stripping

The algorithm systematically removes several categories of content that it identifies as non-essential to the main article.

### Script and Style Elements

All JavaScript (`script` elements) are completely removed during preprocessing. This includes inline scripts, external script references, and any content within script tags. The algorithm recognizes that scripts are purely functional and never constitute article content.

CSS style elements and style attributes are similarly removed. The algorithm operates on the assumption that extracted content will be restyled by the consuming application, so preserving inline styles would create unnecessary noise.

### Navigation Elements

Navigation blocks are typically identified through a combination of methods. The unlikely candidates filtering catches navigation elements with obvious class or ID names containing "nav", "menu", "navigation", or similar terms. The high link density scoring penalizes navigation regions because they typically consist primarily of links. Semantic HTML5 nav elements are also recognized and usually excluded from content.

### Header and Footer Content

Page headers and footers are typically stripped because they contain site-level metadata, branding, and auxiliary links rather than article content. These regions are identified through class/id heuristics ("header", "footer", "site-footer", "masthead", "branding") and through their position relative to the main content container.

### Advertising and Sidebar Content

Advertisements and sidebar content are removed through multiple mechanisms. The unlikely candidates filter catches elements with class/id patterns like "ad", "ads", "advertisement", "sidebar", "widget", "promo", "sponsor". High link density penalizes sidebar regions that often contain promotional links. The scoring algorithm ultimately selects against these regions because they lack the text density characteristics of main content.

### Form Elements

Form elements are generally stripped from the output. Input elements, buttons, select dropdowns, and text areas are identified as interactive elements rather than content and are removed during the cleanup phase.

### Iframes and Embedded Content

Iframe elements are removed unless they happen to contain content that the algorithm determines is the main article itself (which is rare). Embedded content from third-party sources is typically excluded because it represents external material rather than the article's core content.

### Comments

HTML comments are stripped from the output. The algorithm does not preserve developer comments, reader comments, or any other comment types since these are not part of the visible page content.

## Title Extraction

The title extraction process searches for the article title through multiple methods in priority order. The algorithm first checks for an OpenGraph title (`og:title` meta tag), which often contains a clean title without site branding. Next, it examines the HTML document title element. If multiple title elements exist (such as from template inheritance), it selects the most relevant one based on length and content analysis.

The algorithm also attempts to clean titles by removing site name suffixes. If the title contains a delimiter like a pipe, dash, or colon followed by the site name, these trailing portions are stripped to leave only the article title. This produces cleaner output for users who only need the article title without site attribution.

In cases where no clear title is found through these methods, the algorithm falls back to using the first heading element within the identified article container as the title.

## Edge Cases and Limitations

### Malformed HTML

The Readability algorithm uses html5ever for HTML parsing, which implements robust error handling for malformed HTML. The parser can handle unclosed tags, improperly nested elements, and invalid attribute values without completely failing. However, severely malformed HTML may produce unexpected results in the extraction, as the parser must make assumptions about the author's intent.

The algorithm attempts to normalize certain common HTML issues, such as unwrapping noscript elements and handling deprecated tags. However, extremely pathological cases may result in content being placed in unexpected locations or being incorrectly classified as non-content.

### Very Short Content

Pages with very little text content may fail extraction entirely. The default character threshold is 500 characters - if the selected content container contains fewer characters than this threshold, the algorithm treats the extraction as failed. As mentioned earlier, it then retries with relaxed filtering rules, but if the page genuinely lacks substantial content, no amount of relaxation can create it.

This threshold exists to prevent the algorithm from returning minimal snippets that don't represent actual articles. For users extracting from pages with naturally short content (like error pages, redirect pages, or very short announcements), the extractor may return an error or an empty result.

### Very Long Content

The algorithm handles long-form content well, as the scoring mechanism naturally gravitates toward containers that hold the majority of the page's text. Extremely long pages with multiple articles (like category pages or archive listings) may result in the algorithm selecting the wrong article, particularly if the first article is not the most prominent within the DOM structure.

There is no practical upper limit on content length, though performance will degrade proportionally with document size. The algorithm's complexity is roughly linear with the number of DOM elements, so a page with thousands of elements will take longer to process than a simple article page.

### Scripts and Styles

As mentioned, all JavaScript is removed. This has implications for pages that use JavaScript to render content - if the article content exists only in JavaScript-generated DOM elements that don't appear in the initial HTML, the Readability algorithm cannot extract it. This is a fundamental limitation of any HTML-only extractor and applies to Single Page Applications (SPAs) and pages with heavy JavaScript content loading.

The algorithm removes all CSS, including both style elements and inline style attributes. This means that any content that depends on CSS for visibility (such as content hidden by default or revealed through CSS transitions) may be incorrectly handled. If content is hidden via CSS display:none or visibility:hidden properties, the algorithm may still attempt to extract it, but the result will be a mix of visible and invisible content.

## The URL Parameter

The URL parameter serves a critical function in the extraction process: it provides the base for converting relative URLs to absolute URLs. When the algorithm encounters relative references in attributes like `src`, `href`, or `srcset`, it uses the provided URL's origin, scheme, and path components to construct complete absolute URLs.

This URL conversion is essential for several reasons. First, extracted HTML content is often saved and rendered in contexts different from the original page, where relative links would be broken. Second, the URL provides the algorithm with context about the page's origin, which can inform certain extraction decisions. Third, having the absolute URL allows the algorithm to correctly resolve relative paths for images, scripts, stylesheets, and other linked resources.

In the benchmark code, the URL is parsed using the `url` crate before being passed to the extractor:

```rust
let parsed_url = url::Url::parse(&url).expect("run_extraction requires a valid URL");
// ...
readability::extractor::extract(&mut html, &parsed_url)
```

The URL must be a valid, parseable URL. The benchmark expects a valid URL and will panic if an invalid one is provided. This is appropriate for the benchmark's use case but represents a difference from some other extractors that may be more lenient with URL input.

Without a URL, the algorithm would produce output with relative paths that would break when the content is saved and viewed in isolation. The URL parameter is therefore not optional for proper operation, though some alternative implementations may default to using an empty or placeholder URL when none is provided.

## Output Format

The Readability crate produces two distinct output formats in the `Product` struct.

### HTML Output (Content Field)

The `content` field contains cleaned HTML representing the extracted article. This HTML has undergone several transformations: scripts and styles have been removed, relative URLs have been converted to absolute URLs, unnecessary wrapper elements have been stripped, and class attributes have been removed (unless explicitly configured to be preserved). The HTML is semantic and suitable for display in reader interfaces.

### Plain Text Output (Text Field)

The `text` field contains the plain text content extracted from the HTML. This is what the benchmark uses. The conversion process strips all HTML tags, meaning that structural information is lost. Bold text becomes indistinguishable from regular text, links lose their href information and appear only as their anchor text, and images disappear entirely (or appear as their alt text if present).

The plain text output has no markdown-like formatting - it is literally plain text without any markup or styling indicators. This represents both a limitation and a feature, depending on the use case. For applications that need only the raw text content, this format is ideal. For applications that need to preserve formatting or structure, the HTML output would be more appropriate.

The text extraction uses an inner text algorithm that extracts the text content of elements while preserving the rough structure through whitespace and newlines. Paragraphs and headings are separated by newlines, list items are separated appropriately, and preformatted blocks maintain their whitespace.

## Sample Input and Expected Output

To illustrate the behavior of the Readability extractor, consider the following sample HTML input:

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <title>How to Build a Rust Application - TechBlog</title>
    <meta name="description" content="A tutorial on building applications with Rust">
</head>
<body>
    <header>
        <nav>
            <a href="/">Home</a>
            <a href="/about">About</a>
            <a href="/contact">Contact</a>
        </nav>
    </header>
    
    <aside class="sidebar">
        <h3>Recent Posts</h3>
        <ul>
            <li><a href="/post1">Post One</a></li>
            <li><a href="/post2">Post Two</a></li>
        </ul>
        <div class="ad-banner">Advertisement</div>
    </aside>
    
    <main>
        <article>
            <h1>How to Build a Rust Application</h1>
            <p class="byline">By Jane Developer</p>
            <p>Rust is a systems programming language that runs blazingly fast, prevents segfaults, and guarantees thread safety. In this tutorial, we'll explore how to create a basic Rust application from scratch.</p>
            
            <h2>Getting Started</h2>
            <p>First, you'll need to install Rust using rustup. The installation process is straightforward and works on Linux, macOS, and Windows.</p>
            
            <pre><code>fn main() {
    println!("Hello, world!");
}</code></pre>
            
            <p>Once installed, you can verify your installation by running:</p>
            
            <pre><code>cargo --version</code></pre>
            
            <h2>Building Your First Project</h2>
            <p>Use cargo to create a new project:</p>
            
            <ul>
                <li>cargo new my_project</li>
                <li>cd my_project</li>
                <li>cargo build</li>
            </ul>
            
            <p>Congratulations! You've built your first Rust application.</p>
        </article>
    </main>
    
    <footer>
        <p>&copy; 2024 TechBlog. All rights reserved.</p>
    </footer>
    
    <script>console.log("analytics");</script>
</body>
</html>
```

When processed by the Readability extractor with the URL parameter set to "https://techblog.example.com/article/rust-tutorial", the expected plain text output would be:

```
How to Build a Rust Application

By Jane Developer

Rust is a systems programming language that runs blazingly fast, prevents segfaults, and guarantees thread safety. In this tutorial, we'll explore how to create a basic Rust application from scratch.

Getting Started

First, you'll need to install Rust using rustup. The installation process is straightforward and works on Linux, macOS, and Windows.

fn main() {
    println!("Hello, world!");
}

Once installed, you can verify your installation by running:

cargo --version

Building Your First Project

Use cargo to create a new project:

- cargo new my_project
- cd my_project
- cargo build

Congratulations! You've built your first Rust application.
```

Several observations about this output are worth noting. First, the navigation, sidebar, advertisement, header, footer, and script elements have been completely removed. Second, the title "How to Build a Rust Application" appears as the first line, extracted from the article's h1 element. Third, the byline "By Jane Developer" is preserved because it appeared within the article content. Fourth, the code blocks maintain their formatting with preserved whitespace, though without syntax highlighting. Fifth, the list items have been converted to simple bullet-prefixed lines. Sixth, all HTML tags have been stripped, including emphasis - there is no way to distinguish bold or italic text in the output. Seventh, the links appear only as their anchor text - the URLs themselves are not visible in the plain text output.

The extracted text contains approximately 520 characters, which exceeds the default threshold of 500 characters, so this would be considered a successful extraction. Had the content been shorter, the algorithm might have retried with relaxed filtering rules.

## Configuration Options

The `readability` crate in version 0.3.0 provides a relatively simple API without extensive configuration options. Unlike some other extractors in the benchmark that expose numerous settings, this crate is designed to work well with default settings. The primary configuration is the URL parameter, which as discussed is essential for proper URL resolution.

The crate depends on several key libraries that enable its functionality: html5ever for HTML parsing, markup5ever_rcdom for DOM construction, regex for pattern matching, and the url crate for URL handling. The design prioritizes simplicity and robustness over configurability, reflecting the philosophy that the Readability algorithm works best when its defaults are used.

## Comparison with Other Extractors

Compared to other extractors in this benchmark, the Readability algorithm occupies a specific niche. Unlike converters that produce markdown (like htmd, html2md-rs, or mdream), Readability produces plain text by default. Unlike full-formatting converters, Readability strips all formatting emphasis. The algorithm's strength lies in its aggressive removal of non-content elements and its sophisticated scoring mechanism that identifies the main article content even on complex pages.

The Readability algorithm is particularly well-suited for scenarios where the goal is to extract the raw article text without any formatting or markup. It is less suitable for scenarios where the user needs to preserve the original HTML structure, maintain formatting like bold and italic, or produce markdown output.

## Summary

The `readability` crate implements the classic Arc90/Mozilla Readability algorithm for content extraction. It uses a scoring mechanism based on text density, link density, class/id heuristics, and tag type analysis to identify the main article content within a web page. The algorithm systematically removes navigation, advertising, sidebars, scripts, and styles while preserving headings, paragraphs, lists, code blocks, and other substantive content elements.

The URL parameter is essential for converting relative URLs to absolute URLs in the output. The extractor produces both HTML and plain text output, with the benchmark using the plain text variant. All HTML formatting is stripped in the plain text output, including emphasis, links (as hyperlinks), and images (except alt text).

This extractor represents a battle-tested approach to content extraction that has powered Firefox Reader View for years. Its design prioritizes simplicity and robustness over configurability, making it a reliable choice for extracting plain text article content from web pages.
