# Parser candidate evaluation — 2026-11-15

Criteria: distinct HTML-to-text/Markdown behavior, active maintenance, can accept *an acquired HTML string or DOM* without independent URL fetching, straightforward comparison, limited heavyweight dependencies.

| Parser | Status | Why |
|---|---|---|
| **Defuddle** 0.19.6 | **Included** | Different article/boilerplate heuristic and good structural preservation; `useAsync:false` avoids third-party fallback acquisition |
| **node-html-markdown** 2.0.0 | **Included** | Fast full-page converter complementing Turndown, nanohtml2text and html2text Rust |
| **@extractus/article-extractor** | Next candidate | Has explicit `extractFromHtml(html,url)` that accepts immutable HTML; returns *clean HTML*, so markdown comparison needs an additional converter or separate HTML-quality scorer |
| **webforai** | Evaluate against fixed corpus | Recent main-content/Markdown pipeline with cached-page benchmarks; claimed performance is author-reported, requires verifying local API and dependencies |
| **Kreuzberg html-to-markdown** | Potential Rust baseline | Fast parser for malformed HTML; test whether distinct from existing `html-to-markdown-go` and whether API/license/memory justify dependency |
| **jusText** | Lower priority | Boilerplate extractor with a different text-density model, but Trafilatura already uses jusText as a fallback; standalone comparison could isolate the algorithm |
| **MarkItDown / Docling / MinerU-HTML / ReaderLM v2** | Separate heavyweight/ML tier | Primarily document/multimodal or model-based conversion; different runtime and cost profile; not fair as default per-HTML-snapshot parser |

All new parsers must consume the captured HTML without using their URL-fetch CLI. Keep next candidate implementations separate from already validated results, and measure actual quality on an immutable test corpus before presenting rank claims.

Research: https://defuddle.md/docs, https://jsr.io/@extractus/article-extractor, https://github.com/xberg-io/html-to-markdown, https://webforai.dev/benchmarks/
