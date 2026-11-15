# External extractor execution

This page documents the current tool boundary. Source of truth: `src/scores.rs` for command construction, `src/worker.rs` for isolation, `src/extractor_config.rs` for settings, and `src/runner.rs` for result semantics.

## Execution path

1. CLI (`--extractor NAME` or all) and web select extractors using the same `run_named_extractor` dispatch.
2. The root page was acquired once using HTTP or an explicitly selected Lightpanda or CloakBrowser navigation. The resulting HTML snapshot is identical for every converter. Jina Reader receives that HTML by POST, and Percollate is configured without AMP-alternate refetches or remote image inlining.
3. The parent serializes the exact HTML snapshot, URL, extractor identity, settings and destination directory to private temporary IPC files.
4. A separate instance of the application receives `--extract-worker` and runs exactly one extractor. That worker may spawn Node/Python/Go subprocesses. Browsers only run in the earlier acquisition stage.
5. The parent monitors a Unix process group or Windows Job Object. The per-extractor deadline applies to the entire child process group; expiration terminates descendants and reaps the leader.
6. The worker emits a typed result with `Success`, `NoContent`, `Failed`, `TimedOut` or `Panicked` and a human-readable diagnostic. The parent validates the output, records end-to-end time and deletes IPC/scratch files, including after timeouts.

The Rust converter contract is `Result<String, String>`. A text document beginning with `[ERROR]` is **not** treated as a failure; only an actual typed error is.

## Tool launch methods

| Extractor | Execution | Input and configuration |
|---|---|---|
| Turndown | `node -e` with pinned `turndown` module | HTML scratch file; `TurndownConfig` serialized to JS options |
| Percollate | Pinned `percollate/cli.js md -o -` | HTML scratch file; Markdown options from `PercollateConfig` |
| Resiliparse | uv with pinned requirements in tools/extractors/python/resiliparse/requirements.txt | Already-acquired HTML snapshot, Lexbor-based main-content plaintext extraction |
| Trafilatura | `uv run --no-project --with-requirements tools/extractors/python/trafilatura/requirements.txt` | HTML scratch file and JSON flags |
| html2text-py | `uv run --no-project --with-requirements tools/extractors/python/html2text/requirements.txt` | HTML scratch file and JSON flags |
| Markdownify | `uv run --no-project --with-requirements tools/extractors/python/markdownify/requirements.txt` | HTML scratch file and serialized options |
| Defuddle | `node tools/extractors/snapshot-parsers.mjs defuddle` | Snapshot file and original URL; `defuddle/node` with `useAsync:false`, never remote fallback |
| node-html-markdown | `node tools/extractors/snapshot-parsers.mjs node-html-markdown` | Local full-document conversion, no fetching |
| Mozilla Readability | `node -e` with `@mozilla/readability` + `linkedom` | Local HTML file; full upstream JS algorithm, no navigation |
| webforai | Node module webforai pinned in npm lockfile | Acquired HTML and source URL for local link resolution; no network fetch |
| Webclaw | `webclaw` or `HTML_TO_TEXT_WEBCLAW_BIN` | Temporary HTML file plus configured CLI arguments |
| E2M | `uv run --no-project --python 3.12 --with-requirements tools/extractors/python/e2m/requirements.txt` | HTML scratch file, language/engine/processing flags; release-tier integration test |
| html-to-markdown-go | `html2markdown` or `HTML_TO_MARKDOWN_GO_BIN` | Seekable file attached as stdin to avoid stdout/pipe deadlock; plugin flags |

The worker sets `TMPDIR`/`TMP`/`TEMP` to the parent-owned IPC directory, so child scratch files are removed even if the process group is forcibly terminated. Pinned npm packages live under `tools/extractors/node_modules` or the `HTML_TO_TEXT_NODE_MODULES` override.

## Timing and benchmark interpretation

`time_micros` is the conversion-call duration inside the worker, so native Rust library calls and subprocess-launching converters measure different work. `wall_time_micros` is end-to-end parent duration and includes process and IPC overhead. Neither field is a multi-repeat benchmark, memory measurement, or proof of extraction quality. Use fixed HTML snapshots and inspect output content before interpreting speed or reduction.

Web score data and artifacts are held together under the configured server data root. Deleting a score removes its whole artifact directory even if zero extractors ran; retained runs are count-bounded via `HTML_TO_TEXT_MAX_SAVED_RUNS`.


## Resource bounds

The parent worker enforces the configured per-extractor deadline and kills the process group on timeout. Individual external converter commands inherit that group; their stdout/stderr are written to bounded temporary files (not read into an unbounded pipe buffer). A configured `HTML_TO_TEXT_MAX_OUTPUT_BYTES` controls the maximum successful output size, default 64 MiB. If the cap is exceeded, the extractor yields a typed failed result and the output file contains a diagnostic instead of a truncated conversion. CLI file inputs and HTTP/browser snapshots separately obey `HTML_TO_TEXT_MAX_HTML_BYTES`.

The server applies the `HTML_TO_TEXT_MAX_CONCURRENT_RUNS` admission cap before acquiring a new page; it also covers previews, variant comparisons, and boilerplate analysis. CLI runs are single-request processes and do not share the server admission counter.
