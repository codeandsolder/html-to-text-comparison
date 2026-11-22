# Live extraction-settings tuner (HN top-20)

Run the standalone tuning mode from the existing Rust main executable:

    html-to-text-comparison --config /home/jan/services/html-extraction-tuner/config.json --tune-server

The service listens on loopback TCP only. The Certifex ingress service
extract-tune = 18580 publishes https://extract-tune.onhir.eu to the Tailscale
network. Deployment unit: deploy/html-extraction-tuner.service.

It validates all 20 original HTML sources by SHA-256, exposes exactly the 20
qualified extractor backends from bench/shortlist.json, loads previously saved
default extracted outputs from the initial and supplemental benchmark results,
and never fetches a page. The old voting app and SQLite votes are retained.

## Interaction

- Select exactly one extractor and two distinct original HN HTML snapshots.
- The two page selectors, previous/next buttons and A/D/J/L shortcuts switch
  pages independently. An original URL and the exact archived HTML source
  (served as plain text) are linked above each extraction.
- Every setting that the Rust backend actually deserializes is shown, with
  type-correct checkboxes, number inputs, enums, tri-state controls and
  comma-separated list editors. Unsupported backend tuning controls are
  not invented. Editing live settings debounces the two independent extraction
  workers, and obsolete client requests are ignored.
- Each pane can switch between tuned and default outputs. Original defaults
  use the preserved baseline, with no unnecessary extraction rerun.
- Hide link targets is display-only; archived outputs and actual extracted
  content are not modified. Unchecking displays original Markdown syntax.
- Reset defaults, restore saved and save profile are explicit. Profiles are
  stored atomically in state/profiles.json; the server does not overwrite
  prior benchmark results. A bounded memory LRU caches changed settings
  per extractor and page; force refresh bypasses that cache.

## Exposed settings

These entries are sourced from the actual Rust serde config groups, so
new fields added to them appear automatically:

- HTML2Text: width, raw/decoration, link footnotes/wrapping, table borders.
- HTMD: excluded tags, heading, rules, breaks, code block, link references,
  list markers, spacing and translation mode.
- Mdka: balanced/minimal and tri-state presentation refinements.
- Readable Readability: unlikely-content, class weight/conditional/attribute cleanup.
- DOM Smoothie: candidate scoring and thresholds, raw/Markdown formatting,
  class/JSON-LD handling, and parse limits.
- Turndown: heading, list, link reference, code fence, and emphasis format.
- Mdream: main-content isolation, link cleanup, frontmatter, Tailwind cleanup,
  CSS filtering and extraction selectors.
- Python html2text: links, images, wrapping and escaping.
- Markdownify: output delimiters, headings, links, tables, wrapping and parsing.
- Webclaw: main-content, CSS inclusion/exclusion and output format.
- Go html2markdown: plugins, domain override, include/exclude CSS selectors.
- **Newly wired upstream options**: Resiliparse article/metadata/links/format
  flags; Mozilla Readability candidate scoring, class retention and link
  density; node-html-markdown code, list, emphasis and link reference styles;
  fast_html2md CommonMark and preserved spans; html2md preserved spans.
- Fixed-wrapper engines (Boilerpipe, nanohtml2text, readability,
  llm_readability) honestly display no editor controls.

The core runtime code routes all of these to actual extractor APIs, and the
same core preview path is used by the older comparison interface. Missing
tool binaries are not mistaken for performance characteristics.

## Runtime assets and safety

Execution policy and tuner settings are explicitly loaded from JSON. Example
production configuration (create config.json alongside the installed binary):

```json
{
  "execution": {
    "extractor_assets": "/home/jan/services/html-extraction-tuner/extractors",
    "node_modules": "/home/jan/services/html-extraction-tuner/extractors/node_modules"
  },
  "tuner": {
    "listen": "127.0.0.1:18580",
    "corpus_dir": "/home/jan/datasets/html-to-text-comparison/hn-top20-20261115",
    "state_dir": "/home/jan/services/html-extraction-tuner/state",
    "max_concurrent_previews": 3
  }
}
```

The asset directory contains the Node adapter script and uv requirements.
Webclaw and the Go binary are installed via stable user PATH links.

The tuner uses the existing isolated worker process group, caps output,
enforces backend-specific timeouts, and owns all temporary files under the
systemd service TMPDIR. At most three worker previews can run concurrently;
abandoned browser requests do not release a permit prematurely. The app
only binds to 127.0.0.1, refuses unknown page/engine/config values, rejects
cross-origin browser writes, and renders output with textContent and CSP
rather than trusting the source HTML as executable markup. A bounded cache
limits retained preview text to 48 MiB/160 entries.

The dataset and outputs remain private to Acer. Source and tuning UI code
are in GitHub; profiles and browser snapshot data are **not**.
