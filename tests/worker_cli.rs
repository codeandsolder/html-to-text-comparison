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
