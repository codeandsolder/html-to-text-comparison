# Experimental Zipp snapshot runtime

The five JavaScript snapshot parsers are ordinary ESM scripts that can use
the **official Zipp v0.0.38** native CLI selected through
`execution.snapshot_js_bin`. The default is Node. Selection is explicit and
there is no fallback if the selected executable fails. Both runtimes use the
same adjacent package tree and the pinned npm lockfile.

## Pinned native release

Zipp [v0.0.38](https://github.com/f2i-com/zipp.org/releases/tag/v0.0.38)
(source `8cb8399f4f63fa3bebb3298da6f25cda0ecf5395`) implements
`import.meta.resolve` and `process.exitCode` correctly for our ESM
adapters. The former temporary patch for upstream [#99](https://github.com/f2i-com/zipp.org/issues/99)
has been removed; **do not build or ship the v0.0.37 patch**.

Download the upstream native release to an explicitly chosen application
tool directory; verify the official executable digest before use:

```sh
gh release download v0.0.38 -R f2i-com/zipp.org \
  -p 'zipp-0.0.38-linux-x86_64' -D /path/to/runtime
printf '%s  %s\n' \
  f65ec4e76ffb5f43fab703a6b44decbe7301a040b5b8e22f98acfc2f7626c03a \
  /path/to/runtime/zipp-0.0.38-linux-x86_64 | sha256sum --check
chmod +x /path/to/runtime/zipp-0.0.38-linux-x86_64
node tools/zipp/verify-runtime.mjs /path/to/runtime/zipp-0.0.38-linux-x86_64
```

Specify the exact executable in your explicit runtime JSON:

```json
{"execution":{"snapshot_js_bin":"/path/to/runtime/zipp-0.0.38-linux-x86_64"}}
```

The verification script compares Node ESM package exports/subpaths,
detached nested resolution, missing dependencies and exit-code behavior.
All five unmodified production adapters produced Node-identical output on
the pinned regression page with the official v0.0.38 executable.

## Performance and scope

Interleaved, 5-trial, same-process-source fresh-CLI comparisons on EPYC 9655
and Celeron N4100 confirmed real improvements over v0.0.37: Defuddle
1.17–1.21× faster; WebforAI 1.21–1.24× faster. They still lag Node for
this DOM workload, while Readability and NodeHtmlMarkdown can beat Node.
The upstream follow-up is [#101](https://github.com/f2i-com/zipp.org/issues/101).

Zipp v0.0.38 can import the unmodified pure-Python html2text and
Markdownify packages ([#100](https://github.com/f2i-com/zipp.org/issues/100)),
but **Python is not a deployment target**: its native CLI still adds an
unrequested final LF and silently truncates output at about 64 KiB,
[#108](https://github.com/f2i-com/zipp.org/issues/108), and our actual
extractors are 5–12× slower than CPython in fresh-process tests.
Python reference extractors remain explicitly provisioned inside
bubblewrap, with PyPy for the two pure-Python adapters and CPython for
native-extension-dependent ones.

Browser acquisition and Percollate remain on Node; the snapshot selector
intentionally does not affect their host-runtime interfaces. This
experiment stays a draft until its runtime-performance and operational
tradeoffs warrant adoption.
