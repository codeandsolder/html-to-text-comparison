# webforai — snapshot-only Markdown and main-content extraction

webforai 5.1.0 is a TypeScript library with local HTML-to-Markdown conversion, site-aware adapters, and a local block classifier for extracting main content. The benchmark uses the synchronous library function htmlToMarkdown(html, {baseUrl:url}) on the *already acquired* HTML string. It does not use the URL loader, CLI crawler or hosted API and therefore does not acquire a second copy of the site.

The converter runs in the existing isolated Node worker via tools/extractors/snapshot-parsers.mjs, with the npm-pinned dependency tracked by Renovate. It emits Markdown, unlike the plaintext-only Resiliparse baseline.

On immutable synthetic fixture inputs:
- The article sample retained all three content markers and removed unrelated navigation/sidebar.
- The technical documentation sample retained a proper Markdown table with separate register, offset, and bit columns, plus a fenced code block. Resiliparse preserved the values but concatenated adjacent cells.

The upstream webforai benchmark uses a corpus curated during development, which its maintainers acknowledge can favor their engine. This project does not adopt those rankings; independently collected immutable fixtures and content-quality metrics are required.

Reference: https://webforai.dev/ and https://webforai.dev/benchmarks/.
