# Experimental Zipp snapshot runtime

The five snapshot parsers are ordinary ESM scripts and can be launched with
the patched native Zipp CLI through `execution.snapshot_js_bin` in the
explicit runtime configuration. The default remains Node. There is no
fallback from a selected runtime. The asset tree and npm lockfile are
identical for both engines.

Upstream [issue #99](https://github.com/f2i-com/zipp.org/issues/99) concerns
missing `import.meta.resolve` and ignored `process.exitCode` in file modules.
The temporary source patch targets Zipp v0.0.37, commit
`a5c9ef93a71cbb3f3a060ed448bdc39870de4b3e`. The Zipp CLI uses workspace path
dependencies, so apply the patch to the pinned checkout before building it
with Cargo; no extra linked interpreter or runtime shim is needed.

```sh
git clone --branch v0.0.37 https://github.com/f2i-com/zipp.org.git /path/to/zipp
git -C /path/to/zipp apply /path/to/html-to-text-comparison/tools/zipp/patches/0001-node-module-resolver-and-exit.patch
cd /path/to/zipp
cargo build --locked --release -p zipp-cli --no-default-features --features python,wasm-jit --target-dir /path/to/build/zipp
```

Build through the existing managed compiler cache. Select the actual
Cargo-reported executable with e.g. `"snapshot_js_bin": "/path/to/zipp"`
under `"execution"` in config JSON.

Pure-Python html2text and Markdownify are not currently compatible with
Zipp; [issue #100](https://github.com/f2i-com/zipp.org/issues/100) records
the missing `html`, `urllib`, `warnings` and `sys.warnoptions` support.
Other Python adapters depend on native CPython extensions. The current
pinned uv/CPython runtime remains necessary. Browser acquisition and
Percollate require host capabilities outside the pure snapshot experiment.

The patch is provisional pending upstream resolution and validation.
