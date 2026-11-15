# Defuddle — offline article extraction

Defuddle 0.19.6 is a main-content extractor from the Obsidian Web Clipper ecosystem. It can preserve details that Mozilla Readability drops, including structure for math, code, footnotes and metadata.

The benchmark uses `defuddle/node` + `linkedom` in `tools/extractors/snapshot-parsers.mjs`, feeding the *already acquired* HTML snapshot and original URL for link resolution. The call sets `{markdown:true,useAsync:false}`: this is important because Defuddle's normal async fallback can query third-party services on empty/special pages. We deliberately do **not** allow that, so the comparison runs offline. The result is Markdown or a typed error.

Install the pinned npm packages and run `--extractor defuddle` for a single test. The JS fixture test confirms meaningful article extraction and removal of nav text. Source: https://github.com/kepano/defuddle
