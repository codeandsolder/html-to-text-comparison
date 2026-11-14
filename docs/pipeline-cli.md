# External extractor execution

This page documents the current tool boundary. Source of truth: `src/scores.rs` for command construction, `src/worker.rs` for isolation, `src/extractor_config.rs` for settings, and `src/runner.rs` for result semantics.

## Execution path

1. CLI (`--extractor NAME` or all) and web select extractors using the same `run_named_extractor` dispatch.
2. The parent serializes the exact HTML snapshot, URL, extractor identity, settings and destination directory to private temporary IPC files.
3. A separate instance of the application receives `--extract-worker` and runs exactly one extractor. That worker may spawn Node/Python/Go/Lightpanda subprocesses.
4. The parent monitors a Unix process group or Windows Job Object. The per-extractor deadline applies to the entire child process group; expiration terminates descendants and reaps the leader.
5. The worker emits a typed result with `Success`, `NoContent`, `Failed`, `TimedOut` or `Panicked` and a human-readable diagnostic. The parent validates the output, records end-to-end time and deletes IPC/scratch files, including after timeouts.

The Rust converter contract is `Result<String, String>`. A text document beginning with `[ERROR]` is **not** treated as a failure; only an actual typed error is.

## Tool launch methods

| Extractor | Execution | Input and configuration |
|---|---|---|
| Turndown | `node -e` with pinned `turndown` module | HTML scratch file; `TurndownConfig` serialized to JS options |
| Percollate | Pinned `percollate/cli.js md -o -` | HTML scratch file; Markdown options from `PercollateConfig` |
| Trafilatura | `uv run --no-project --with trafilatura==2.3.1` | HTML scratch file and JSON flags |
| html2text-py | `uv run --no-project --with html2text==2025.4.15` | HTML scratch file and JSON flags |
| Markdownify | `uv run --no-project --with markdownify==1.2.3` | HTML scratch file and serialized options |
| Lightpanda | `docker exec lightpanda lightpanda fetch` | Page URL and configured wait options |
| Webclaw | `webclaw` or `HTML_TO_TEXT_WEBCLAW_BIN` | Temporary HTML file plus configured CLI arguments |
| E2M | `uv run --no-project --python 3.12 --with wisup-e2m==0.1.61` | HTML scratch file, language/engine/processing flags; release-tier integration test |
| html-to-markdown-go | `html2markdown` or `HTML_TO_MARKDOWN_GO_BIN` | Seekable file attached as stdin to avoid stdout/pipe deadlock; plugin flags |

The worker sets `TMPDIR`/`TMP`/`TEMP` to the parent-owned IPC directory, so child scratch files are removed even if the process group is forcibly terminated. Pinned npm packages live under `tools/extractors/node_modules` or the `HTML_TO_TEXT_NODE_MODULES` override.

## Timing and benchmark interpretation

`time_micros` is the conversion-call duration inside the worker, so native Rust library calls and subprocess-launching converters measure different work. `wall_time_micros` is end-to-end parent duration and includes process and IPC overhead. Neither field is a multi-repeat benchmark, memory measurement, or proof of extraction quality. Use fixed HTML snapshots and inspect output content before interpreting speed or reduction.

Web score data and artifacts are held together under the configured server data root. Deleting a score removes its whole artifact directory even if zero extractors ran; retained runs are count-bounded via `HTML_TO_TEXT_MAX_SAVED_RUNS`.
