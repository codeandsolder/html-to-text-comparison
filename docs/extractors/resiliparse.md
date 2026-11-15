# Resiliparse — snapshot-only main-content plaintext

Resiliparse 1.0.9 uses the Lexbor DOM implementation and independent main-content extraction heuristics. Its pinned Python binding is run through uv and the existing isolated worker.

This benchmark passes the acquired HTML string to resiliparse.extract.html2text.extract_plain_text with main_content=True and preserve_formatting=True. The converter is never passed a URL and does not perform another acquisition. Its result is formatted plaintext, not Markdown, which is important when comparing code blocks, tables, and link fidelity.

Two immutable synthetic fixtures exercise this path:
- fixtures/article-with-navigation.html: three article markers must survive, while navigation, sidebar, and footer sentinels must disappear.
- fixtures/technical-reference.html: register names, numeric offsets and code constants must survive; irrelevant documentation navigation must not.

One observed limitation: Resiliparse may concatenate adjacent HTML table cell values in plaintext. Do not interpret this extractor's compact output as lossless Markdown and do not rank it solely on output size. Tests check content retention but do not claim general quality or rank.

See https://resiliparse.chatnoir.eu/en/stable/api/extract/html2text.html for upstream semantics.
