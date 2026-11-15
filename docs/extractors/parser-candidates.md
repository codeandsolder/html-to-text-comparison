# Parser candidate evaluation — 2026-11-15

Criteria: distinct HTML-to-text/Markdown behavior, active maintenance, can accept *an acquired HTML string or DOM* without independent URL fetching, straightforward comparison, limited heavyweight dependencies.

| Parser | Status | Why |
|---|---|---|
| **Defuddle** 0.19.6 | **Included** | Different article/boilerplate heuristic and good structural preservation; `useAsync:false` avoids third-party fallback acquisition |
| **node-html-markdown** 2.0.0 | **Included** | Fast full-page converter complementing Turndown, nanohtml2text and html2text Rust |
| **@extractus/article-extractor** | Defer (overlaps Mozilla Readability) | Has explicit `extractFromHtml(html,url)` that accepts immutable HTML; returns *clean HTML*, so markdown comparison needs an additional converter or separate HTML-quality scorer |
| **Resiliparse** 1.0.9 | **Included** | Independent Lexbor main-content selection from the acquired HTML, emitting plaintext; technical tables can lose cell boundaries, which our fixtures capture |
| **Goose3** 3.1.22 | Evaluate on corpus | Can consume raw HTML offline; short pilot omitted a relevant paragraph, so measure recall before default inclusion |
| **webforai** 5.1.0 | Highest-priority next Markdown baseline | Local htmlToMarkdown with an HTML string and base URL; includes site adapters and an offline block classifier. Its own 60-page development corpus is acknowledged as biased; validate on independent fixtures |
| **Xberg / Kreuzberg HTML-to-Markdown** | Evaluate standalone engine first | The Rust HTML converter may add a useful independent baseline; importing a full 97-format document framework for one HTML string would add unnecessary dependencies |
| **jusText** | Lower priority | Boilerplate extractor with a different text-density model, but Trafilatura already uses jusText as a fallback; standalone comparison could isolate the algorithm |
| **MarkItDown / Docling / MinerU-HTML / ReaderLM v2** | Separate heavyweight/ML tier | Primarily document/multimodal or model-based conversion; different runtime and cost profile; not fair as default per-HTML-snapshot parser |

All new parsers must consume the captured HTML without using their URL-fetch CLI. Keep next candidate implementations separate from already validated results, and measure actual quality on an immutable test corpus before presenting rank claims.

Research: https://defuddle.md/docs, https://jsr.io/@extractus/article-extractor, https://github.com/xberg-io/html-to-markdown, https://webforai.dev/benchmarks/
