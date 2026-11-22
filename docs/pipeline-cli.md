# External extractor execution

This page documents the current tool boundary. Source of truth: `src/scores.rs` for command construction, `src/worker.rs` for isolation, `src/extractor_config.rs` for settings, and `src/runner.rs` for result semantics.

## Execution path

1. CLI (`--extractor NAME` or all) and web select extractors using the same `run_named_extractor` dispatch.
2. The root page was acquired once using HTTP or an explicitly selected Lightpanda or CloakBrowser navigation. The resulting HTML snapshot is identical for every converter. Jina Reader receives that HTML by POST, and Percollate is configured without AMP-alternate refetches or remote image inlining.
3. The parent writes the HTML snapshot exactly once as `source.html` in the run-owned artifacts directory. Each worker's small IPC JSON contains only the source path, URL, extractor identity, settings and destination directory; it **never serializes the HTML bytes again**. The worker validates that the source is its run's `source.html` and loads it read-only. CLI, saved-score comparisons and preview all use this same contract.
4. A separate instance of the application receives `--extract-worker` and runs exactly one extractor. That worker may spawn Node/Python/Go subprocesses. Browsers only run in the earlier acquisition stage.
5. The parent monitors a Unix process group or Windows Job Object. The per-extractor deadline applies to the entire child process group; expiration terminates descendants and reaps the leader.
6. The worker emits a typed result with `Success`, `NoContent`, `Failed`, `TimedOut` or `Panicked` and a human-readable diagnostic. The parent validates the output, records end-to-end time and deletes IPC/scratch files, including after timeouts.

The Rust converter contract is `Result<String, String>`. A text document beginning with `[ERROR]` is **not** treated as a failure; only an actual typed error is.

## Tool launch methods

| Extractor | Execution | Input and configuration |
|---|---|---|
| Turndown | `snapshot-parsers.mjs turndown` with pinned `turndown` module | HTML scratch file; `TurndownConfig` serialized to JS options |
| Percollate | Pinned `percollate/cli.js md -o -` | HTML scratch file; Markdown options from `PercollateConfig` |
| Resiliparse | Pre-provisioned CPython + bubblewrap | Already-acquired HTML snapshot, Lexbor-based main-content plaintext extraction |
| Trafilatura | Pre-provisioned CPython + bubblewrap | HTML scratch file and JSON flags |
| html2text-py | Pre-provisioned PyPy + bubblewrap | HTML scratch file and JSON flags |
| Markdownify | Pre-provisioned PyPy + bubblewrap | HTML scratch file and serialized options |
| Defuddle | `node tools/extractors/snapshot-parsers.mjs defuddle` | Snapshot file and original URL; `defuddle/node` with `useAsync:false`, never remote fallback |
| node-html-markdown | `node tools/extractors/snapshot-parsers.mjs node-html-markdown` | Local full-document conversion, no fetching |
| Mozilla Readability | `snapshot-parsers.mjs mozilla-readability` with `@mozilla/readability` + `linkedom` | Local HTML file; full upstream JS algorithm, no navigation |
| webforai | Node module webforai pinned in npm lockfile | Acquired HTML and source URL for local link resolution; no network fetch |
| Webclaw | `webclaw` or `execution.webclaw_bin` | Temporary HTML file plus configured CLI arguments |
| E2M | Pre-provisioned CPython 3.12 + bubblewrap | HTML scratch file, language/engine/processing flags; release-tier integration test |
| html-to-markdown-go | `html2markdown` or `execution.html_to_markdown_go_bin` | Seekable file attached as stdin to avoid stdout/pipe deadlock; plugin flags |

The worker sets `TMPDIR`/`TMP`/`TEMP` to the parent-owned IPC directory, so child scratch files are removed even if the process group is forcibly terminated. All Node scripts and their adjacent `node_modules` installation come from the one explicit `execution.extractor_assets` root (default `tools/extractors`); subprocesses do not inherit `NODE_PATH`, and direct ESM package imports must resolve inside the selected installation.

## Timing and benchmark interpretation

`time_micros` is the conversion-call duration inside the worker, so native Rust library calls and subprocess-launching converters measure different work. `wall_time_micros` is end-to-end parent duration and includes process and IPC overhead. Neither field is a multi-repeat benchmark, memory measurement, or proof of extraction quality. Use fixed HTML snapshots and inspect output content before interpreting speed or reduction.

Web score data and artifacts are held together under the configured server data root. Deleting a score removes its whole artifact directory even if zero extractors ran; retained runs are count-bounded via `server.max_saved_runs`.


## Resource bounds

The parent worker enforces the configured per-extractor deadline and kills the process group on timeout. Individual external converter commands inherit that group; their stdout/stderr are written to bounded temporary files (not read into an unbounded pipe buffer). A configured `execution.max_output_bytes` controls the maximum successful output size, default 64 MiB. If the cap is exceeded, the extractor yields a typed failed result and the output file contains a diagnostic instead of a truncated conversion. CLI file inputs and HTTP/browser snapshots separately obey `execution.max_html_bytes`.

The server applies the `server.max_concurrent_runs` admission cap before acquiring a new page; it also covers previews, variant comparisons, and boilerplate analysis. CLI runs are single-request processes and do not share the server admission counter.


## Python reference extractors

Provision the runtime and package installation before running any Python extractor.
For html2text-py and Markdownify use PyPy 3.12 (tested version 8.0.1).
Resiliparse, Trafilatura and E2M currently require separate native-extension
environments; their pinned dependencies do not have a complete PyPy wheel
set. Use a uv-managed CPython 3.12 for those reference adapters.

```sh
tools/extractors/provision-python.sh tools/extractors /path/to/pypy/bin/pypy3 html2text markdownify
uv python install 3.12.15 --no-bin
tools/extractors/provision-python.sh tools/extractors "$(uv python find 3.12.15 --managed-python)" trafilatura resiliparse
# Optional heavyweight reference:
tools/extractors/provision-python.sh tools/extractors "$(uv python find 3.12.15 --managed-python)" e2m
```

Use the selected `execution.extractor_assets` root instead of the checkout
path for a deployment. Each adapter has its own `python/<name>/runtime`
symlink to a relocatable interpreter distribution and a `packages/` tree.
The provisioner refuses to overwrite an existing installation; create
a fresh asset tree for an upgrade. Runtime directories must be complete
relocatable distributions, not virtualenvs or the host /usr prefix.
E2M downloads its NLTK data during provisioning, not conversion.

Conversion requires bubblewrap and user namespaces. It fails if provisioning
or isolation is unavailable. Each conversion receives only the read-only
runtime, packages, HTML input, host system libraries, isolated /dev and /proc,
and a fresh /tmp, with a cleared environment and no network. No uv, pip,
dependency downloads, host home directory or unconfined fallback at runtime.
The outer worker retains its existing output and deadline limits.
