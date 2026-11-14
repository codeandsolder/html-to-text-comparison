# Mozilla Readability (upstream JavaScript)

The `mozilla-readability` converter executes `@mozilla/readability` **0.6.0** with a `linkedom` DOM parser (pinned in `tools/extractors/package.json` and `package-lock.json`). It is distinct from Rust `readability`, `llm_readability` and `readable-readability` ports.

Execution uses a local HTML snapshot file in the isolated Node worker, with no URL navigation or site request. The JS parser returns `textContent`. Its suitability and extraction fidelity depend on page structure; browser-rendered pages can be compared by selecting Lightpanda at acquisition time, so all extractors receive that exact rendered HTML.

Install the pinned Node packages with `PUPPETEER_SKIP_DOWNLOAD=1 npm ci --prefix tools/extractors --ignore-scripts --no-audit --no-fund`; then `--extractor mozilla-readability` runs it alone. A fixture-level regression test checks that a substantive article is extracted and navigation boilerplate is excluded. Renovate updates the npm packages.
