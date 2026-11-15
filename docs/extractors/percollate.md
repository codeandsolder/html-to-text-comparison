# Percollate (snapshot-based Markdown extractor)

The `percollate` command-line extractor uses the pinned npm `percollate` package (currently 4.3.0) with its local HTML snapshot passed as a `file://` document. The code in `src/scores.rs` builds the complete CLI argument vector from `PercollateConfig`, including hyphenation and Markdown formatting settings.

**Acquisition policy:** `--no-amp` disables the package's usual remote AMP-alternate lookup. Image `--inline` is deliberately never enabled because it would fetch image resources from the site. There is no independent top-level URL acquisition by this converter.

The extractor runs as a child of the isolated worker's process group, with a backend-specific deadline and parent-owned temporary input. Its output is typed success/error and stored alongside the original captured HTML.

Unit tests verify CLI argument construction and Markdown settings; more representative quality fixtures belong in the corpus phase.
