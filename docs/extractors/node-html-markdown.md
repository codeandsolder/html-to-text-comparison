# node-html-markdown — fast full-document baseline

`node-html-markdown` 2.0.0 converts a supplied HTML string into Markdown without fetching or identifying a special primary article. This contrasts with Defuddle, Mozilla Readability and Trafilatura, which try to remove boilerplate.

The tool runs in the same isolated worker as all other converters and receives the exact snapshot produced by the selected HTTP/Lightpanda/CloakBrowser acquisition. It uses `NodeHtmlMarkdown.translate(html)`, as exposed through `tools/extractors/snapshot-parsers.mjs`. Unit tests cover headings and strong text. No browser, network or API key is involved.

Source: https://github.com/crosstype/node-html-markdown
