# markdownify

`markdownify` is the Python HTML-to-Markdown library from [matthewwithanm/python-markdownify](https://github.com/matthewwithanm/python-markdownify).

In this benchmark it is executed through `uv` and the Python API, not the standalone CLI:

```text
uv run --with markdownify -- python3 -c "<inline script>" <temp_html_file> <config_json>
```

The benchmark exposes the serializable upstream options that map cleanly onto the web UI and saved extractor state:

- `strip` and `convert`
- `autolinks`, `default_title`
- `heading_style`, `bullets`, `strong_em_symbol`
- `sub_symbol`, `sup_symbol`, `newline_style`
- `code_language`
- `escape_asterisks`, `escape_underscores`, `escape_misc`
- `keep_inline_images_in`
- `table_infer_header`
- `wrap`, `wrap_width`
- `strip_document`, `strip_pre`
- `bs4_parser`

`strip` and `convert` are validated as mutually exclusive before the Python process is started, so invalid settings fail obviously instead of being silently ignored.

Two upstream hooks are intentionally not exposed here:

- `code_language_callback`, because it requires passing executable Python callbacks through saved JSON config
- arbitrary `bs4_options` kwargs, because the benchmark UI stores plain serializable fields rather than free-form Python objects
