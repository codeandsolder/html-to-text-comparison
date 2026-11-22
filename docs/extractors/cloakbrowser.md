# CloakBrowser — third acquisition backend

CloakBrowser **acquires HTML** using a single navigation through custom Chromium. It is not counted as an HTML-to-Markdown extractor because doing so would make an extra HTTP/browser request compared with the other extractors.

```sh
# Install pinned JS package and Playwright core using npm ci
PUPPETEER_SKIP_DOWNLOAD=1 npm ci --prefix tools/extractors --ignore-scripts --no-audit --no-fund

# First activate CloakBrowser with the vendor's free license (one-time)
npm exec --prefix tools/extractors -- cloakbrowser login
# Alternatively set CLOAKBROWSER_LICENSE_KEY externally

cargo run --release -- --acquire cloakbrowser --extractor defuddle https://example.com
```

CloakBrowser 0.7.2 is pinned alongside a compatible `playwright-core` version in `tools/extractors/package.json`. The file `tools/extractors/cloakbrowser-acquire.mjs` calls `launch({headless:true})`, `browser.newPage()`, `page.goto(url, {waitUntil:'domcontentloaded'})`, waits 1 second for basic hydration, obtains `page.content()` and always closes the browser. Every extractor receives that single resulting snapshot, exactly as with HTTP and Lightpanda acquisition.

CloakBrowser requires a license key and can auto-download a ~200 MB custom Chromium binary on first launch. In unattended/server mode, arrange activation beforehand; the Rust program does not try to register for a license. `execution.browser_timeout_ms` sets the hard deadline for the entire browser process group (default 120,000, allowed 1,000–900,000). A timeout kills and reaps the browser tree. Node-side deterministic tests verify one navigation and cleanup without downloading Chromium; a live proprietary-browser smoke test requires a configured license.

The browser may make subresource requests for scripts, styles and images **within the one navigation**; those do not constitute additional top-level acquisitions. The captured DOM snapshot may differ from the raw HTTP response because JavaScript has run, so compare acquisition backends separately. There is no automatic retry or fallback from CloakBrowser to HTTP/Lightpanda, avoiding silent mixed-source results.

Source: https://github.com/CloakHQ/CloakBrowser
