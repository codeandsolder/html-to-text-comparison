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
