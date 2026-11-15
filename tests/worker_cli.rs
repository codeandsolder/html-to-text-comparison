use std::{fs, process::Command};

#[test]
fn cli_executes_native_extractor_in_isolated_worker() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let input = root.path().join("example.html");
    fs::write(&input, "<html><body><h1>Hello worker</h1></body></html>")?;

    let output = Command::new(env!("CARGO_BIN_EXE_html-to-text-comparison"))
        .arg("--extractor")
        .arg("nanohtml2text")
        .arg(&input)
        .current_dir(root.path())
        .output()?;

    assert!(
        output.status.success(),
        "cli stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let content = fs::read_to_string(root.path().join("out/nanohtml2text.txt"))?;
    assert!(content.contains("Hello worker"), "{content}");
    let source = fs::read_to_string(root.path().join("out/source.html"))?;
    assert_eq!(source, "<html><body><h1>Hello worker</h1></body></html>");
    assert!(!root.path().join("out/html.html").exists());
    let leftovers = fs::read_dir(root.path().join("out"))?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|ty| ty.is_dir()))
        .collect::<Vec<_>>();
    assert!(leftovers.is_empty(), "worker IPC directory was leaked");
    Ok(())
}

#[test]
fn http_capture_is_acquired_once_for_extraction() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    };
    use std::thread;
    use std::time::Duration;

    let root = tempfile::tempdir()?;
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    listener.set_nonblocking(true)?;
    let url = format!("http://{}/snapshot", listener.local_addr()?);
    let requests = Arc::new(AtomicUsize::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let count = Arc::clone(&requests);
    let stop_child = Arc::clone(&stop);
    let server = thread::spawn(move || -> std::io::Result<()> {
        while !stop_child.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
                    let mut request = [0_u8; 2048];
                    let _ = stream.read(&mut request)?;
                    count.fetch_add(1, Ordering::SeqCst);
                    let body = "<html><body><h1>Acquired once</h1></body></html>";
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )?;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    });

    let run = Command::new(env!("CARGO_BIN_EXE_html-to-text-comparison"))
        .args(["--acquire", "http", "--extractor", "nanohtml2text", &url])
        .current_dir(root.path())
        .output();
    stop.store(true, Ordering::Relaxed);
    server.join().map_err(|_| "local HTTP server panicked")??;
    let output = run?;
    assert!(
        output.status.success(),
        "cli stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(requests.load(Ordering::SeqCst), 1);
    let saved = fs::read_to_string(root.path().join("out/nanohtml2text.txt"))?;
    assert!(saved.contains("Acquired once"));
    Ok(())
}

#[test]
fn node_snapshot_parsers_run_through_real_worker_without_fetching(
) -> Result<(), Box<dyn std::error::Error>> {
    let workspace = tempfile::tempdir()?;
    let source = workspace.path().join("fixture.html");
    let article = "An immutable HTML snapshot is shared across all converters. ".repeat(24);
    fs::write(
        &source,
        format!(
            "<!doctype html><html><head><title>Fixture</title></head><body><nav>Unrelated navigation</nav><article><h1>Benchmark</h1><p>{article}</p></article></body></html>"
        ),
    )?;
    for name in ["defuddle", "node-html-markdown"] {
        let result = Command::new(env!("CARGO_BIN_EXE_html-to-text-comparison"))
            .args(["--extractor", name])
            .arg(&source)
            .current_dir(workspace.path())
            .output()?;
        assert!(
            result.status.success(),
            "{name} failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let output = fs::read_to_string(workspace.path().join("out").join(format!("{name}.txt")))?;
        assert!(
            output.contains("An immutable HTML snapshot"),
            "{name} lost the source article"
        );
        if name == "defuddle" {
            assert!(!output.contains("Unrelated navigation"));
        }
    }
    Ok(())
}

#[test]
fn http_acquisition_rejects_oversized_response_before_extraction(
) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    let workspace = tempfile::tempdir()?;
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let url = format!("http://{}/too-big", listener.local_addr()?);
    let serve = thread::spawn(move || -> std::io::Result<()> {
        let (mut socket, _) = listener.accept()?;
        let mut request = [0_u8; 1024];
        let _ = socket.read(&mut request)?;
        let body = "x".repeat(100);
        write!(
            socket,
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )?;
        Ok(())
    });
    let output = Command::new(env!("CARGO_BIN_EXE_html-to-text-comparison"))
        .env("HTML_TO_TEXT_MAX_HTML_BYTES", "64")
        .args(["--acquire", "http", "--extractor", "nanohtml2text", &url])
        .current_dir(workspace.path())
        .output()?;
    serve.join().map_err(|_| "HTTP test server panicked")??;
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("exceeds 64 byte limit"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!workspace.path().join("out").exists());
    Ok(())
}

#[test]
fn http_acquisition_decodes_declared_non_utf8_charset() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    let workspace = tempfile::tempdir()?;
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let url = format!("http://{}/latin1", listener.local_addr()?);
    let serve = thread::spawn(move || -> std::io::Result<()> {
        let (mut socket, _) = listener.accept()?;
        let mut request = [0_u8; 1024];
        let _ = socket.read(&mut request)?;
        let body = b"<html><body><h1>caf\xe9</h1></body></html>";
        write!(
            socket,
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=iso-8859-1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )?;
        socket.write_all(body)?;
        Ok(())
    });
    let output = Command::new(env!("CARGO_BIN_EXE_html-to-text-comparison"))
        .args(["--acquire", "http", "--extractor", "nanohtml2text", &url])
        .current_dir(workspace.path())
        .output()?;
    serve
        .join()
        .map_err(|_| "charset fixture server panicked")??;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result = fs::read_to_string(workspace.path().join("out/nanohtml2text.txt"))?;
    assert!(
        result.contains("café"),
        "incorrect charset decode: {result}"
    );
    Ok(())
}

#[test]
fn oversized_native_and_external_outputs_are_explicit_failures(
) -> Result<(), Box<dyn std::error::Error>> {
    let workspace = tempfile::tempdir()?;
    let source = workspace.path().join("long.html");
    fs::write(
        &source,
        format!(
            "<html><body><p>{}</p></body></html>",
            "testing output boundaries ".repeat(200)
        ),
    )?;
    for extractor in ["nanohtml2text", "node-html-markdown"] {
        let output = Command::new(env!("CARGO_BIN_EXE_html-to-text-comparison"))
            .args(["--extractor", extractor])
            .arg(&source)
            .env("HTML_TO_TEXT_MAX_OUTPUT_BYTES", "128")
            .current_dir(workspace.path())
            .output()?;
        assert!(
            output.status.success(),
            "cli failed for {extractor}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let captured = fs::read_to_string(
            workspace
                .path()
                .join("out")
                .join(format!("{extractor}.txt")),
        )?;
        assert!(captured.contains("[ERROR]"), "{extractor}: {captured}");
        assert!(
            captured.contains("128 byte limit"),
            "{extractor}: {captured}"
        );
        assert!(captured.len() < 512, "huge output escaped the cap");
    }
    Ok(())
}

#[test]
fn offline_snapshot_accepts_original_url_and_rejects_invalid_scheme(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let source = root.path().join("snapshot.html");
    fs::write(
        &source,
        "<html><head><title>Reference</title></head><body><main><article><h1>Link resolution validation</h1><p>Documentation explains a transfer method from this captured page.</p><p><a href=\"/relative-resource-identifier\">Read documentation</a> with all the details.</p></article></main></body></html>",
    )?;
    let cli = env!("CARGO_BIN_EXE_html-to-text-comparison");
    let output = Command::new(cli)
        .args([
            "--extractor",
            "nanohtml2text",
            "--base-url",
            "https://original.example/test/",
        ])
        .arg(&source)
        .current_dir(root.path())
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let extracted = fs::read_to_string(root.path().join("out/webforai.txt"))?;
    assert!(
        extracted.contains("](https://original.example/relative-resource-identifier)"),
        "relative link did not resolve against captured URL: {extracted}"
    );
    assert_eq!(
        fs::read_to_string(root.path().join("out/source.html"))?,
        fs::read_to_string(&source)?
    );
    let rejected = Command::new(cli)
        .args(["--extractor", "nanohtml2text", "--base-url", "file:///tmp/"])
        .arg(source)
        .current_dir(root.path())
        .output()?;
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("http or https"));
    let url_input = Command::new(cli)
        .args([
            "--extractor",
            "nanohtml2text",
            "--base-url",
            "https://example.com/source",
            "https://example.invalid/no-fetch",
        ])
        .current_dir(root.path())
        .output()?;
    assert!(!url_input.status.success());
    assert!(String::from_utf8_lossy(&url_input.stderr)
        .contains("only applies to a local HTML snapshot"));
    Ok(())
}
