# CLI-Based HTML-to-Text Extractors

This document details the 9 CLI-based HTML-to-text extractors implemented in `src/scores.rs`. Each section traces the exact command construction, argument handling, temp file usage, output parsing, and relationship to the config structs in `src/extractor_config.rs`.

---

## turndown

- **Tool location**: Node.js module at `/home/jan/git/turndown` (required via `require()` in inline code)
- **Invocation**: `node -e <inline_script> <temp_file>`
- **HTML input method**: Temp file (written to system temp directory)
- **Config bridge**: **NOT IMPLEMENTED** - `TurndownConfig` struct exists but is not passed to CLI
- **Pipeline**:
  1. Generate unique temp file path: `std::env::temp_dir().join(format!("turndown_{}.html", uuid::Uuid::new_v4()))`
  2. Write HTML content to temp file via `std::fs::write(&tmp, html)`
  3. Construct inline Node.js script that:
     - Requires turndown module from `/home/jan/git/turndown`
     - Creates new `td()` service instance
     - Reads HTML from file path argument (`process.argv[1]`)
     - Calls `svc.turndown(html)` to convert
     - Writes output to stdout via `process.stdout.write()`
  4. Execute: `std::process::Command::new("node").args(["-e", node_code, tmp.to_str().unwrap()]).output()`
  5. Delete temp file: `std::fs::remove_file(&tmp)`
  6. Parse output from `stdout`
- **Error handling**:
  - Exit code check: `if !o.status.success()` returns error with stderr
  - Empty output check: `if stdout.is_empty()` returns error with stderr
  - Process spawn failure: returns error message
- **Dependencies**: Node.js runtime, turndown npm package installed at `/home/jan/git/turndown`

**Exact command construction (lines 505-529)**:
```rust
fn run_turndown(html: &str) -> String {
    let tmp = std::env::temp_dir().join(format!("turndown_{}.html", uuid::Uuid::new_v4()));
    let _ = std::fs::write(&tmp, html);
    let node_code = r#"const fs = require('fs'); const td = require('/home/jan/git/turndown'); const svc = new td(); const html = fs.readFileSync(process.argv[1], 'utf8'); process.stdout.write(svc.turndown(html))"#;
    let out = std::process::Command::new("node")
        .args(["-e", node_code, tmp.to_str().unwrap()])
        .output();

    let _ = std::fs::remove_file(&tmp);
    // ... error handling
}
```

**Config struct (extractor_config.rs, lines 4-32)**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurndownConfig {
    pub heading_style: String,       // Not used in CLI
    pub hr: String,                  // Not used in CLI
    pub bullet_list_marker: String,  // Not used in CLI
    pub code_block_style: String,    // Not used in CLI
    pub fence: String,                // Not used in CLI
    pub em_delimiter: String,        // Not used in CLI
    pub strong_delimiter: String,    // Not used in CLI
    pub link_style: String,          // Not used in CLI
    pub link_reference_style: String,// Not used in CLI
    pub preformatted_code: bool,     // Not used in CLI
}
```

---

## percollate

- **Tool location**: `/home/jan/git/percollate/cli.js` (Node.js CLI script)
- **Invocation**: `node /home/jan/git/percollate/cli.js md -o - <temp_file>`
- **HTML input method**: Temp file (written to system temp directory)
- **Config bridge**: **NOT IMPLEMENTED** - `PercollateConfig` struct exists but is not passed to CLI
- **Pipeline**:
  1. Generate unique temp file path: `std::env::temp_dir().join(format!("percollate_in_{}.html", uuid::Uuid::new_v4()))`
  2. Write HTML content to temp file
  3. Execute percollate CLI with:
     - `md` - output format (Markdown)
     - `-o -` - output to stdout (dash means stdout)
     - `<temp_file>` - input HTML file
  4. Delete temp file
  5. Parse output from `stdout`
- **Error handling**:
  - Exit code check: non-zero exit returns error with stderr
  - Empty output check: empty stdout returns error with stderr
  - Process spawn failure: returns error message
- **Dependencies**: Node.js runtime, percollate npm package installed at `/home/jan/git/percollate`

**Exact command construction (lines 531-554)**:
```rust
fn run_percollate(html: &str) -> String {
    let tmp = std::env::temp_dir().join(format!("percollate_in_{}.html", uuid::Uuid::new_v4()));
    let _ = std::fs::write(&tmp, html);
    let out = std::process::Command::new("node")
        .args(["/home/jan/git/percollate/cli.js", "md", "-o", "-", tmp.to_str().unwrap()])
        .output();

    let _ = std::fs::remove_file(&tmp);
    // ... error handling
}
```

**Config struct (extractor_config.rs, lines 35-49)**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PercollateConfig {
    pub inline_images: bool,  // Not used in CLI
    pub hyphenate: bool,      // Not used in CLI
    pub fences: bool,         // Not used in CLI
}
```

---

## trafilatura

- **Tool location**: Python package managed via `uv` (trafilatura pip package)
- **Invocation**: `uv run -- python3 -c <inline_python_script> <temp_file>`
- **HTML input method**: Temp file (written to system temp directory)
- **Config bridge**: **NOT IMPLEMENTED** - `TrafilaturaConfig` struct exists but is not passed to CLI
- **Pipeline**:
  1. Generate unique temp file path: `std::env::temp_dir().join(format!("trafilatura_{}.html", uuid::Uuid::new_v4()))`
  2. Write HTML content to temp file
  3. Execute Python via uv with inline script that:
     - Imports `trafilatura` module
     - Reads HTML from file path (`sys.argv[1]`)
     - Calls `trafilatura.extract(html, output_format='markdown', include_links=True)`
     - Prints result (empty string if None)
  4. Delete temp file
  5. Parse output from `stdout`
- **Error handling**:
  - Exit code check: non-zero exit returns error with stderr
  - Empty output check: empty stdout returns error with stderr
  - Process spawn failure (uv not found): returns error message
- **Dependencies**: Python 3, uv package manager, trafilatura pip package

**Exact command construction (lines 556-580)**:
```rust
fn run_trafilatura(html: &str) -> String {
    let tmp = std::env::temp_dir().join(format!("trafilatura_{}.html", uuid::Uuid::new_v4()));
    let _ = std::fs::write(&tmp, html);
    let out = std::process::Command::new("uv")
        .args(["run", "--", "python3", "-c", "import trafilatura; import sys; html=open(sys.argv[1]).read(); result=trafilatura.extract(html, output_format='markdown', include_links=True); print(result if result else '', end='')"])
        .arg(tmp.to_str().unwrap())
        .output();

    let _ = std::fs::remove_file(&tmp);
    // ... error handling
}
```

**Config struct (extractor_config.rs, lines 73-99)**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafilaturaConfig {
    pub favor_precision: bool,     // Not used in CLI
    pub favor_recall: bool,        // Not used in CLI
    pub include_comments: bool,    // Not used in CLI (hardcoded to True in CLI)
    pub include_tables: bool,      // Not used in CLI (hardcoded to True in CLI)
    pub include_images: bool,      // Not used in CLI
    pub include_formatting: bool,  // Not used in CLI
    pub include_links: bool,       // Not used in CLI (hardcoded to True in CLI)
    pub deduplicate: bool,         // Not used in CLI (hardcoded to True in CLI)
    pub with_metadata: bool,       // Not used in CLI
}
```

---

## html2text-py

- **Tool location**: Python package managed via `uv` (html2text pip package)
- **Invocation**: `uv run -- python3 -c <inline_python_script> <temp_file>`
- **HTML input method**: Temp file (written to system temp directory)
- **Config bridge**: **NOT IMPLEMENTED** - `Html2TextPythonConfig` struct exists but is not passed to CLI
- **Pipeline**:
  1. Generate unique temp file path: `std::env::temp_dir().join(format!("h2t_py_{}.html", uuid::Uuid::new_v4()))`
  2. Write HTML content to temp file
  3. Execute Python via uv with inline script that:
     - Imports `HTML2Text` class from html2text module
     - Creates handler with hardcoded settings:
       - `ignore_links=False`
       - `ignore_images=False`
       - `body_width=78`
     - Reads HTML from file path
     - Calls `h.handle(html)` and prints result
  4. Delete temp file
  5. Parse output from `stdout`
- **Error handling**:
  - Exit code check: non-zero exit returns error with stderr
  - Empty output check: empty stdout returns error with stderr
  - Process spawn failure: returns error message
- **Dependencies**: Python 3, uv package manager, html2text pip package

**Exact command construction (lines 582-606)**:
```rust
fn run_html2text_py(html: &str) -> String {
    let tmp = std::env::temp_dir().join(format!("h2t_py_{}.html", uuid::Uuid::new_v4()));
    let _ = std::fs::write(&tmp, html);
    let out = std::process::Command::new("uv")
        .args(["run", "--", "python3", "-c", "from html2text import HTML2Text; import sys; h=HTML2Text(); h.ignore_links=False; h.ignore_images=False; h.body_width=78; html=open(sys.argv[1]).read(); print(h.handle(html), end='')"])
        .arg(tmp.to_str().unwrap())
        .output();

    let _ = std::fs::remove_file(&tmp);
    // ... error handling
}
```

**Config struct (extractor_config.rs, lines 102-128)**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Html2TextPythonConfig {
    pub ignore_links: bool,     // Not used in CLI (hardcoded to False)
    pub ignore_images: bool,    // Not used in CLI (hardcoded to False)
    pub ignore_emphasis: bool,  // Not used in CLI
    pub body_width: usize,      // Not used in CLI (hardcoded to 78)
    pub unicode_snob: bool,     // Not used in CLI
    pub escape_snob: bool,      // Not used in CLI
    pub inline_links: bool,     // Not used in CLI
    pub google_doc: bool,       // Not used in CLI
    pub dash_unordered_list: bool, // Not used in CLI
}
```

---

## markdownify

- **Tool location**: Python package resolved by `uv --with markdownify`
- **Invocation**: `uv run --with markdownify -- python3 -c <inline_python_script> <temp_file> <config_json>`
- **HTML input method**: Temp file (written to system temp directory)
- **Config bridge**: `MarkdownifyConfig` is serialized to JSON and passed through to `markdownify.markdownify(...)`
- **Special validation**: `strip` and `convert` are treated as mutually exclusive and return an explicit error if both are set
- **Dependencies**: Python 3, uv package manager, markdownify pip package

---

## lightpanda

- **Tool location**: Docker container named `lightpanda` with `lightpanda` binary in PATH
- **Invocation**: `docker exec lightpanda lightpanda fetch --dump markdown <url>`
- **HTML input method**: **URL-based (not HTML content)** - fetches live from URL
- **Config bridge**: **NOT IMPLEMENTED** - `LightpandaConfig` struct exists but is not passed to CLI
- **Pipeline**:
  1. Receive `parsed_url: &url::Url` as input (not HTML string)
  2. Execute docker exec command:
     - `docker exec lightpanda lightpanda fetch` - run inside container
     - `--dump markdown` - output format
     - `<parsed_url.to_string()>` - URL to fetch
  3. No temp file cleanup needed (no temp file created)
  4. Parse output from `stdout`
- **Error handling**:
  - Exit code check: non-zero exit returns error with stderr
  - Empty output check: empty stdout returns error with stderr
  - Docker exec failure: returns error message
- **Dependencies**: Docker daemon running, container named `lightpanda` with lightpanda binary installed

**Exact command construction (lines 608-632)**:
```rust
fn run_lightpanda(parsed_url: &url::Url) -> String {
    let out = std::process::Command::new("docker")
        .args([
            "exec", "lightpanda", "lightpanda", "fetch",
            "--dump", "markdown",
            parsed_url.to_string().as_str(),
        ])
        .output();

    // ... error handling
}
```

**Config struct (extractor_config.rs, lines 131-149)**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightpandaConfig {
    pub strip_js: bool,      // Not used in CLI
    pub strip_css: bool,     // Not used in CLI
    pub strip_ui: bool,      // Not used in CLI
    pub wait_until: String,  // Not used in CLI
    pub wait_ms: u64,        // Not used in CLI
}
```

**Important distinction**: Unlike other extractors, `lightpanda` does NOT receive HTML content. It receives a URL and fetches the page itself. This is the only URL-based extractor in this group.

---

## webclaw

- **Tool location**: `/home/jan/git/webclaw/webclaw_bin` (custom binary) OR `webclaw` (system PATH)
- **Invocation**: `webclaw_bin <args>` or `webclaw <args>`
- **HTML input method**: Temp file (written to system temp directory)
- **Config bridge**: **IMPLEMENTED** - `WebclawConfig` fields are passed as CLI arguments
- **Pipeline**:
  1. Retrieve config from `ExtractorStates`: `states.states.get("webclaw").map(|s| s.config.webclaw.clone())`
  2. Generate unique temp file path: `std::env::temp_dir().join(format!("webclaw_{}.html", uuid::Uuid::new_v4()))`
  3. Write HTML content to temp file
  4. Build argument list dynamically:
     - Always: `--file <temp_file>`
     - Conditional: `--only-main-content` if `cfg.only_main_content`
     - Conditional: `--include <cfg.include_css>` if not empty
     - Conditional: `--exclude <cfg.exclude_css>` if not empty
     - Always: `-f <format>` (defaults to "markdown" if empty)
  5. Check if binary exists at `/home/jan/git/webclaw/webclaw_bin`
  6. Execute the binary with built args
  7. Delete temp file
  8. Parse output from `stdout`
- **Error handling**:
  - Exit code check: non-zero exit returns error with stderr
  - Empty output check: empty stdout returns error with stderr
  - Binary not found: returns specific error message
- **Dependencies**: webclaw binary (compiled Rust binary)

**Exact command construction (lines 634-679)**:
```rust
fn run_webclaw(html: &str, states: &ExtractorStates) -> String {
    let cfg = states
        .states
        .get("webclaw")
        .map(|s| s.config.webclaw.clone())
        .unwrap_or_default();
    let tmp = std::env::temp_dir().join(format!("webclaw_{}.html", uuid::Uuid::new_v4()));
    let _ = std::fs::write(&tmp, html);
    let mut args = vec!["--file".to_string(), tmp.to_string_lossy().to_string()];
    if cfg.only_main_content {
        args.push("--only-main-content".to_string());
    }
    if !cfg.include_css.is_empty() {
        args.push("--include".to_string());
        args.push(cfg.include_css.clone());
    }
    if !cfg.exclude_css.is_empty() {
        args.push("--exclude".to_string());
        args.push(cfg.exclude_css.clone());
    }
    args.push("-f".to_string());
    args.push(if cfg.format.is_empty() { "markdown".to_string() } else { cfg.format.clone() });
    let bin = std::path::Path::new("/home/jan/git/webclaw/webclaw_bin");
    let out = if bin.exists() {
        std::process::Command::new(bin).args(&args).output()
    } else {
        std::process::Command::new("webclaw").args(&args).output()
    };

    let _ = std::fs::remove_file(&tmp);
    // ... error handling
}
```

**Config struct (extractor_config.rs, lines 152-168)**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebclawConfig {
    pub only_main_content: bool,  // Passed as --only-main-content flag
    pub include_css: String,       // Passed as --include <value>
    pub exclude_css: String,       // Passed as --exclude <value>
    pub format: String,            // Passed as -f <value> (default: markdown)
}
```

---

## e2m

- **Tool location**: Python package managed via `uv` (wisup_e2m pip package)
- **Invocation**: `uv run -- python3 -c <inline_python_script> <temp_file>`
- **HTML input method**: Temp file (written to system temp directory)
- **Config bridge**: **PARTIALLY IMPLEMENTED** - `E2mConfig.engine` is passed to CLI
- **Pipeline**:
  1. Retrieve config from `ExtractorStates`: `states.states.get("e2m").map(|s| s.config.e2m.clone())`
  2. Determine engine: if `cfg.engine` is empty, default to "unstructured"
  3. Generate unique temp file path: `std::env::temp_dir().join(format!("e2m_{}.html", uuid::Uuid::new_v4()))`
  4. Write HTML content to temp file
  5. Execute Python via uv with inline script that:
     - Imports `HtmlParser` from wisup_e2m
     - Creates parser with engine parameter from config: `HtmlParser(engine='{engine}')`
     - Reads HTML from file path
     - Calls `p.parse(text=..., include_image_link_in_text=False)`
     - Accesses `.text` property and prints result
  6. Delete temp file
  7. Parse output from `stdout`
- **Error handling**:
  - Exit code check: non-zero exit returns error with stderr
  - Empty output check: empty stdout returns error with stderr
  - Process spawn failure: returns error message
- **Dependencies**: Python 3, uv package manager, wisup_e2m pip package

**Exact command construction (lines 681-713)**:
```rust
fn run_e2m(html: &str, states: &ExtractorStates) -> String {
    let cfg = states
        .states
        .get("e2m")
        .map(|s| s.config.e2m.clone())
        .unwrap_or_default();
    let engine = if cfg.engine.is_empty() { "unstructured" } else { &cfg.engine };
    let tmp = std::env::temp_dir().join(format!("e2m_{}.html", uuid::Uuid::new_v4()));
    let _ = std::fs::write(&tmp, html);
    let out = std::process::Command::new("uv")
        .args(["run", "--", "python3", "-c", &format!(
            "import sys; from wisup_e2m import HtmlParser; p=HtmlParser(engine='{}'); result=p.parse(text=open(sys.argv[1]).read(), include_image_link_in_text=False); print(result.text, end='')",
            engine
        ), tmp.to_str().unwrap()])
        .output();

    let _ = std::fs::remove_file(&tmp);
    // ... error handling
}
```

**Config struct (extractor_config.rs, lines 171-181)**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E2mConfig {
    pub engine: String,  // Passed to HtmlParser(engine='{}')
}
```

**Note**: The `engine` field is the only config field used. Other potential engine options are not exposed through this CLI wrapper.

---

## html-to-markdown-go

- **Tool location**: `/tmp/html2markdown` (compiled Go binary)
- **Invocation**: `/tmp/html2markdown --domain=<domain>` with stdin for HTML input
- **HTML input method**: **stdin** (piped, not temp file)
- **Config bridge**: **PARTIALLY IMPLEMENTED** - `HtmlToMarkdownGoConfig.domain` passed, but plugins NOT used
- **Pipeline**:
  1. Extract domain from parsed_url: `parsed_url.origin().ascii_serialization()`
  2. Spawn process with piped stdin/stdout/stderr:
     ```rust
     std::process::Command::new("/tmp/html2markdown")
         .arg(format!("--domain={}", domain))
         .stdin(std::process::Stdio::piped())
         .stdout(std::process::Stdio::piped())
         .stderr(std::process::Stdio::piped())
         .spawn()
     ```
  3. Write HTML to stdin: `stdin.write_all(html.as_bytes())`
  4. Wait for completion and capture output: `child.wait_with_output()`
  5. No temp file created or cleaned up
  6. Parse output from `stdout`
- **Error handling**:
  - Exit code check: non-zero exit returns error with stderr
  - Empty output check: empty stdout returns error with stderr
  - Process spawn failure: returns error message
  - Wait failure: returns error message
- **Dependencies**: Compiled Go binary at `/tmp/html2markdown`

**Exact command construction (lines 715-750)**:
```rust
fn run_html_to_markdown_go(html: &str, parsed_url: &url::Url) -> String {
    let domain = parsed_url.origin().ascii_serialization();
    let out = std::process::Command::new("/tmp/html2markdown")
        .arg(format!("--domain={}", domain))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();

    match out {
        Ok(mut child) => {
            use std::io::Write;
            if let Some(ref mut stdin) = child.stdin {
                stdin.write_all(html.as_bytes()).ok();
            }
            let result = child.wait_with_output();

            match result {
                Ok(o) => {
                    // ... error handling
                }
                Err(e) => format!("[ERROR] html-to-markdown-go wait failed: {}\n", e),
            }
        }
        Err(e) => format!("[ERROR] html2markdown spawn failed: {}\n", e),
    }
}
```

**Config struct (extractor_config.rs, lines 184-196)**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HtmlToMarkdownGoConfig {
    pub domain: String,      // Passed as --domain=<value>
    pub plugins: Vec<String>, // NOT USED in CLI (hardcoded to commonmark)
}
```

**Note**: Unlike other extractors, this is the only one using stdin for input. Also, the `plugins` config field is not passed to the CLI - the CLI hardcodes "commonmark" as the plugin.

---

## Summary of Config Bridge Usage

| Extractor | Config Used in CLI? | Fields Passed |
|-----------|---------------------|---------------|
| turndown | NO | None - all config fields ignored |
| percollate | NO | None - all config fields ignored |
| trafilatura | NO | None - all config fields hardcoded |
| html2text-py | NO | None - all config fields hardcoded |
| markdownify | YES | strip, convert, autolinks, default_title, heading_style, bullets, strong_em_symbol, sub_symbol, sup_symbol, newline_style, code_language, escape_asterisks, escape_underscores, escape_misc, keep_inline_images_in, table_infer_header, wrap, wrap_width, strip_document, strip_pre, bs4_parser |
| lightpanda | NO | None - URL-based, no HTML input |
| webclaw | YES | only_main_content, include_css, exclude_css, format |
| e2m | PARTIAL | engine (only field available) |
| html-to-markdown-go | PARTIAL | domain (plugins not used) |

## Common Patterns

### Temp File Usage (8 extractors)
All extractors except `lightpanda` and `html-to-markdown-go` use temp files:
- Location: `std::env::temp_dir()` (system temp directory)
- Naming: `{extractor_name}_{uuid}.html`
- Cleanup: Always attempted via `std::fs::remove_file(&tmp)` after execution

### Error Handling Pattern
All CLI extractors follow identical error handling:
1. Check `o.status.success()` - non-zero exit = error
2. Check stdout is not empty - empty = error
3. Include stderr in error messages for debugging
4. Return formatted error string prefixed with `[ERROR]`

### uv Wrapper (Python extractors)
Four extractors use `uv run --` to execute Python:
- trafilatura
- html2text-py
- markdownify
- e2m

This ensures consistent Python environment management.
