# Rust HTML to Text Comparison

This compares various HTML-to-text and HTML-to-Markdown tools:

- [august](https://crates.io/crates/august) ![august](https://img.shields.io/crates/d/august)
- [boilerpipe](https://crates.io/crates/boilerpipe) ![boilerpipe](https://img.shields.io/crates/d/boilerpipe)
- [dom_smoothie](https://crates.io/crates/dom_smoothie) ![dom_smoothie](https://img.shields.io/crates/d/dom_smoothie)
- [fast_html2md](https://crates.io/crates/fast_html2md) ![fast_html2md](https://img.shields.io/crates/d/fast_html2md)
- [htmd](https://crates.io/crates/htmd) ![htmd](https://img.shields.io/crates/d/htmd)
- [html2md](https://crates.io/crates/html2md) ![html2md](https://img.shields.io/crates/d/html2md)
- [html2md-rs](https://crates.io/crates/html2md-rs) ![html2md-rs](https://img.shields.io/crates/d/html2md-rs)
- [html2text](https://crates.io/crates/html2text) ![html2text](https://img.shields.io/crates/d/html2text)
- [llm_readability](https://crates.io/crates/llm_readability) ![llm_readability](https://img.shields.io/crates/d/llm_readability)
- [markdownify](https://github.com/matthewwithanm/python-markdownify)
- [mdka](https://crates.io/crates/mdka) ![mdka](https://img.shields.io/crates/d/mdka)
- [nanohtml2text](https://crates.io/crates/nanohtml2text) ![nanohtml2text](https://img.shields.io/crates/d/nanohtml2text)
- [readability](https://crates.io/crates/readability) ![readability](https://img.shields.io/crates/d/readability)
- [readable-readability](https://crates.io/crates/readable-readability) ![readable-readability](https://img.shields.io/crates/d/readable-readability)

These crates are used for a variety of different purposes, ranging from displaying websites in a terminal to extracting text for use in LLMs.

I am particularly interested in picking one to use for [Scour](https://scour.ing), where I need to run websites through an LLM embedding model for semantic search.

## Usage

Install pinned Node extractor packages once (Python tools are managed through `uv`):

```sh
PUPPETEER_SKIP_DOWNLOAD=1 npm ci --prefix tools/extractors --ignore-scripts --no-audit --no-fund
cargo run --release -- --extractor nanohtml2text example.html
cargo run --release -- --acquire http https://example.com
cargo run --release -- --acquire lightpanda https://example.com
cargo run --release -- --acquire cloakbrowser https://example.com
```

CLI results are written to `out/`. Each invocation acquires **one immutable HTML snapshot** (HTTP or an explicitly selected Lightpanda/CloakBrowser navigation) and runs every enabled converter against those identical bytes. Lightpanda and CloakBrowser are **alternative acquisition backends, not additional extractors that revisit the site**. Optional Jina Reader receives the captured HTML in a POST instead of a URL-only refetch. Boilerplate analysis intentionally samples distinct linked pages, once each within that analysis. Every converter uses the same registry as the web UI and runs in its own killable process group. You can select a single extractor with `--extractor NAME`. Check the resulting text for missing, unwanted or misleading content; a short output is not necessarily a good output.

### Dependency maintenance and additional extractors

Renovate uses the shared [codeandsolder CI fleet policy](https://github.com/codeandsolder/ci-workflows/blob/main/renovate-fleet.json) to maintain Cargo, npm, pip-style requirement files, GitHub Actions SHA pins, the pinned CI `uv` bootstrap and the `diff` browser CDN version. Python extractors run via individual `tools/extractors/python/*/requirements.txt` files, so dependency upgrades stay independently reproducible. Major updates require review and tests; `wisup-e2m` requires the heavyweight release-tier integration test.

The comparison includes **Mozilla's upstream Readability JavaScript implementation** through `@mozilla/readability` + `linkedom`, **Defuddle** in strictly offline mode, **node-html-markdown** as a fast full-document baseline, **Resiliparse** as an independent Lexbor-based main-content plaintext extractor, and **webforai** as a locally run main-content Markdown engine with site adapters. All three use the same captured bytes as the Rust/Python converters.

CloakBrowser uses its Playwright-compatible Node wrapper and custom Chromium binary. The browser must be activated separately (e.g. `npx cloakbrowser login` or `CLOAKBROWSER_LICENSE_KEY`), and the first launch may download its signed browser build (~200 MB). This activation and download are prerequisites for a live CloakBrowser run, not steps performed by the benchmark. Both browser backends use a configurable process-group deadline, `HTML_TO_TEXT_ACQUIRE_TIMEOUT_MS` (default 120,000 ms). HTTP acquisition is always available without a browser.

### Web UI and storage

```sh
# Default: local-only, no cross-origin API access
HTML_TO_TEXT_MAX_SAVED_RUNS=100 cargo run --release -- --server 3000 /path/to/data
# For a trusted LAN or an authenticated reverse proxy:
HTML_TO_TEXT_BIND=0.0.0.0 cargo run --release -- --server 3000 /path/to/data
```

The web server listens on 127.0.0.1 by default. Use HTML_TO_TEXT_BIND to choose another IPv4/IPv6 interface. The API can acquire arbitrary URLs and run external tools, so do not expose it on an untrusted network without access controls; permissive cross-origin headers are deliberately disabled.

HTML acquisition has a 45-second total HTTP deadline and a default 64 MiB decoded HTML limit (HTML_TO_TEXT_MAX_HTML_BYTES, configurable to 1 GiB). Browser acquisition uses its own process deadline and the same output-size cap. The extractor settings file is saved atomically; saved-run metadata changes and pruning are serialized within the server process.

The server stores score metadata under `/path/to/data/scores`, with corresponding HTML snapshots and extractor outputs under `/path/to/data/scores/artifacts/<run-id>/`. The retention cap defaults to 100 saved runs, can be set through `HTML_TO_TEXT_MAX_SAVED_RUNS` (positive integer), and deletes old outputs together with their metadata. Failed/empty runs remain inspectable until deleted or expired. Live previews use ephemeral directories.

Every extractor has an independently configurable process deadline (`timeout_ms` in its state, editable in the web UI). Defaults are 10 seconds for native extractors, longer for external tools, and 120 seconds for E2M. A deadline kills the entire worker process group, including spawned tools.

### Interpreting measurements

- **Conversion time (`time_micros`)** measures the conversion call inside the isolated worker. For external Node/Python/Go tools this *includes* launching their child process and doing tool I/O.
- **End-to-end time (`wall_time_micros`)** measures the parent's full per-extractor operation, including preparing IPC data, starting and reaping the worker and reading its response. It is never a pure algorithm microbenchmark.
- Output-size reduction is reported only for successful conversions. Failed, timed-out and empty conversions have a `null` reduction value, not a misleading 100% reduction.
- The tool does **not** currently measure peak memory or statistical latency distributions. These require a separately designed repeatable benchmark mode.

Historical tables from the original prototype are deliberately not shown as current benchmark results: they predate process isolation, contain memory claims that the current harness does not measure, and used different error semantics. They remain accessible in the repository's earlier Git history.

## Optional: Jina `reader-lm`

You can also compare the results using the [jinaai/reader-lm](https://huggingface.co/jinaai/reader-lm-1.5b) model via their hosted API.

This isn't included by default because it isn't a totally fair comparison.
It's not running locally and requires an API key. The model has open weights so you could run it locally, but it is licensed under a non-commercial license.

To enable the comparison with the Jina Reader:

1. Get a free API key from https://jina.ai/reader
2. Build the crate with the `reader-lm-api` feature enabled
3. Set the `JINA_API_KEY` environment variable

```sh
JINA_API_KEY=... cargo run --release --features=reader-lm-api -- <url-to-scrape>
```
