use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

use command_group::{CommandGroup, GroupChild};
use serde::{Deserialize, Serialize};

use crate::{
    extractor_config::ExtractorStates,
    runner::{ExtractionStatus, Runner, Stats},
    scores::run_named_extractor,
};

#[derive(Serialize, Deserialize)]
struct Request {
    key: String,
    name: String,
    url: String,
    snapshot_path: PathBuf,
    states: ExtractorStates,
    output_dir: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct Reply {
    stat: Stats,
}

pub struct ExecutionError {
    pub status: ExtractionStatus,
    pub message: String,
}

impl ExecutionError {
    fn failed(message: impl Into<String>) -> Self {
        Self {
            status: ExtractionStatus::Failed,
            message: message.into(),
        }
    }
}

fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ' ')
    {
        return Err(format!("invalid extractor output name: {name:?}"));
    }
    Ok(())
}

/// Maximum accepted output of an individual extractor (not input HTML).
/// Enforced in both the native runner and external command capture.
pub fn max_output_bytes() -> Result<usize, String> {
    const DEFAULT: usize = 64 * 1024 * 1024;
    const UPPER: usize = 1024 * 1024 * 1024;
    let limit = std::env::var("HTML_TO_TEXT_MAX_OUTPUT_BYTES").map_or(Ok(DEFAULT), |value| {
        value
            .parse::<usize>()
            .map_err(|error| format!("invalid HTML_TO_TEXT_MAX_OUTPUT_BYTES: {error}"))
    })?;
    if !(1..=UPPER).contains(&limit) {
        return Err(format!("HTML_TO_TEXT_MAX_OUTPUT_BYTES must be 1..={UPPER}"));
    }
    Ok(limit)
}

/// Worker requests may only access the immutable snapshot owned by their
/// run directory. Reject missing files, symlinks escaping that directory,
/// and giant inputs before allocating the snapshot string.
fn read_snapshot(request: &Request) -> Result<String, String> {
    let run_dir = fs::canonicalize(&request.output_dir)
        .map_err(|error| format!("resolving run directory: {error}"))?;
    let expected = run_dir.join("source.html");
    let source = fs::canonicalize(&request.snapshot_path)
        .map_err(|error| format!("resolving snapshot: {error}"))?;
    if source != expected {
        return Err("worker snapshot is not the run's immutable source.html".to_string());
    }
    let limit = crate::acquisition::max_html_bytes()?;
    let size = fs::metadata(&source)
        .map_err(|error| format!("reading snapshot metadata: {error}"))?
        .len();
    if size > u64::try_from(limit).unwrap_or(u64::MAX) {
        return Err(format!("worker snapshot exceeds {limit} byte limit"));
    }
    let html =
        fs::read_to_string(&source).map_err(|error| format!("reading UTF-8 snapshot: {error}"))?;
    if html.len() > limit {
        return Err(format!("worker snapshot exceeds {limit} byte limit"));
    }
    Ok(html)
}

pub fn worker_main() -> Result<(), String> {
    let mut args = std::env::args_os().skip(2);
    let request_path = args.next().ok_or("missing worker request path")?;
    let reply_path = args.next().ok_or("missing worker reply path")?;
    if args.next().is_some() {
        return Err("too many worker arguments".to_string());
    }
    let request: Request = serde_json::from_slice(
        &fs::read(&request_path).map_err(|error| format!("reading worker request: {error}"))?,
    )
    .map_err(|error| format!("parsing worker request: {error}"))?;

    validate_name(&request.name)?;
    let known = request.states.states.contains_key(&request.key);
    #[cfg(feature = "reader-lm-api")]
    let known = known || request.key == "reader-lm-api";
    if !known {
        return Err(format!("unknown extractor: {}", request.key));
    }
    let parsed_url = url::Url::parse(&request.url).map_err(|error| error.to_string())?;
    let html = read_snapshot(&request)?;
    let mut runner = Runner::new(request.output_dir, html);
    run_named_extractor(
        &mut runner,
        &request.states,
        &parsed_url,
        &request.key,
        &request.name,
    );
    let mut stats = runner.into_stats();
    if stats.len() != 1 {
        return Err(format!(
            "worker generated {} results instead of exactly one",
            stats.len()
        ));
    }
    let stat = stats.remove(0);
    fs::write(
        reply_path,
        serde_json::to_vec(&Reply { stat }).map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("writing worker response: {error}"))
}

fn worker_executable() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("HTML_TO_TEXT_WORKER_EXE") {
        return Ok(PathBuf::from(path));
    }
    let current = std::env::current_exe().map_err(|error| error.to_string())?;
    let is_test_harness = current.file_name().is_some_and(|name| {
        name.to_string_lossy()
            .starts_with("html_to_text_comparison-")
    });
    if !is_test_harness {
        return Ok(current);
    }

    // Cargo may place the unit-test harness under deps/ OR under
    // build/<crate>/<hash>/out when an ephemeral build wrapper is active.
    // Both layouts keep the profile (debug/release) in the ancestry.
    let binary = format!("html-to-text-comparison{}", std::env::consts::EXE_SUFFIX);
    let profile = current
        .ancestors()
        .find_map(|part| {
            part.file_name()
                .filter(|name| *name == "debug" || *name == "release")
                .map(std::ffi::OsStr::to_os_string)
        })
        .unwrap_or_else(|| "debug".into());
    let candidates = [
        std::env::var_os("CARGO_TARGET_DIR")
            .map(|path| PathBuf::from(path).join(&profile).join(&binary)),
        current
            .ancestors()
            .find(|part| part.file_name().is_some_and(|name| name == profile))
            .map(|path| path.join(&binary)),
        Some(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("target")
                .join(profile)
                .join(&binary),
        ),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|path| path.is_file())
        .ok_or_else(|| {
            "Cargo test harness cannot locate the built extractor worker binary".to_string()
        })
}

enum GroupExit {
    Exited(ExitStatus),
    TimedOut,
}

// On unexpected errors and unwinds, also kill the whole process tree and reap.
struct GroupGuard(Option<GroupChild>);

impl Drop for GroupGuard {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// One dedicated OS process group (Unix) or Job Object (Windows) per extraction.
/// Kill *and reap* the entire group when its deadline is reached.
fn wait_for_group(command: &mut Command, timeout: Duration) -> Result<GroupExit, String> {
    wait_for_group_with_capture(command, timeout, None)
}

fn wait_for_group_with_capture(
    command: &mut Command,
    timeout: Duration,
    cap: Option<&CaptureFiles<'_>>,
) -> Result<GroupExit, String> {
    let started = Instant::now();
    let mut guard = GroupGuard(Some(
        command
            .group_spawn()
            .map_err(|error| format!("starting process group: {error}"))?,
    ));
    loop {
        let child = guard.0.as_mut().ok_or("process group missing")?;
        match child.try_wait() {
            Ok(Some(status)) => {
                if let Some(capture) = cap {
                    capture.check()?;
                }
                return Ok(GroupExit::Exited(status));
            }
            Ok(None) => {
                if let Some(capture) = cap {
                    capture.check()?;
                }
                if started.elapsed() >= timeout {
                    child
                        .kill()
                        .map_err(|error| format!("killing timed-out process group: {error}"))?;
                    child
                        .wait()
                        .map_err(|error| format!("reaping timed-out process group: {error}"))?;
                    guard.0.take();
                    return Ok(GroupExit::TimedOut);
                }
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("monitoring process group: {error}"));
            }
        }
    }
}

/// Capture subprocess stdout and stderr into scoped temporary files; bound
/// their growth during execution, before loading bytes into memory.
///
/// Extractor children intentionally inherit their worker's OS process group
/// so that a parent-side timeout kills *all* descendants, including uv/node.
pub fn run_bounded_command(
    command: &mut Command,
    timeout: Duration,
    max_stdout: usize,
) -> Result<std::process::Output, String> {
    capture_command(command, timeout, max_stdout, false)
}

/// Browser acquisition is not inside an extractor worker. It needs a
/// dedicated process group of its own for complete teardown on timeout.
pub fn run_captured_command(
    mut command: Command,
    timeout: Duration,
    max_stdout: usize,
) -> Result<std::process::Output, String> {
    command.stdin(Stdio::null());
    capture_command(&mut command, timeout, max_stdout, true)
}

struct CaptureFiles<'a> {
    stdout: &'a Path,
    stderr: &'a Path,
    max_stdout: u64,
    max_stderr: u64,
}

impl CaptureFiles<'_> {
    fn check(&self) -> Result<(), String> {
        for (path, bound, description) in [
            (self.stdout, self.max_stdout, "stdout"),
            (self.stderr, self.max_stderr, "stderr"),
        ] {
            let length = fs::metadata(path)
                .map_err(|error| format!("checking captured {description}: {error}"))?
                .len();
            if length > bound {
                return Err(format!("{description} exceeds {bound} byte limit"));
            }
        }
        Ok(())
    }
}

fn capture_command(
    command: &mut Command,
    timeout: Duration,
    max_stdout: usize,
    own_group: bool,
) -> Result<std::process::Output, String> {
    const STDERR_LIMIT: u64 = 1024 * 1024;
    let scratch = tempfile::tempdir().map_err(|error| error.to_string())?;
    let stdout_path = scratch.path().join("stdout");
    let stderr_path = scratch.path().join("stderr");
    command
        .stdout(Stdio::from(
            fs::File::create(&stdout_path).map_err(|error| error.to_string())?,
        ))
        .stderr(Stdio::from(
            fs::File::create(&stderr_path).map_err(|error| error.to_string())?,
        ))
        .env("TMPDIR", scratch.path())
        .env("TMP", scratch.path())
        .env("TEMP", scratch.path());
    let cap = CaptureFiles {
        stdout: &stdout_path,
        stderr: &stderr_path,
        max_stdout: u64::try_from(max_stdout).unwrap_or(u64::MAX),
        max_stderr: STDERR_LIMIT,
    };
    let exit = if own_group {
        wait_for_group_with_capture(command, timeout, Some(&cap))?
    } else {
        wait_for_child_with_capture(command, timeout, &cap)?
    };
    let status = match exit {
        GroupExit::TimedOut => {
            return Err(format!(
                "subprocess exceeded its {}s deadline",
                timeout.as_secs_f64()
            ));
        }
        GroupExit::Exited(status) => status,
    };
    cap.check()?;
    Ok(std::process::Output {
        status,
        stdout: fs::read(stdout_path).map_err(|error| error.to_string())?,
        stderr: fs::read(stderr_path).map_err(|error| error.to_string())?,
    })
}

fn wait_for_child_with_capture(
    command: &mut Command,
    timeout: Duration,
    cap: &CaptureFiles<'_>,
) -> Result<GroupExit, String> {
    let started = Instant::now();
    let mut child = command
        .spawn()
        .map_err(|error| format!("starting subprocess: {error}"))?;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                cap.check()?;
                return Ok(GroupExit::Exited(status));
            }
            Ok(None) => {
                if let Err(error) = cap.check() {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(error);
                }
                if started.elapsed() >= timeout {
                    let _ = child.kill();
                    child
                        .wait()
                        .map_err(|error| format!("reaping subprocess: {error}"))?;
                    return Ok(GroupExit::TimedOut);
                }
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("monitoring subprocess: {error}"));
            }
        }
    }
}

pub fn execute_isolated(
    out_dir: &Path,
    snapshot_path: &Path,
    key: &str,
    name: &str,
    url: &str,
    states: &ExtractorStates,
    timeout: Duration,
) -> Result<Stats, ExecutionError> {
    let execute = || -> Result<Stats, String> {
        validate_name(name)?;
        let exchange = tempfile::tempdir_in(out_dir)
            .map_err(|error| format!("creating worker IPC directory: {error}"))?;
        let request_path = exchange.path().join("request.json");
        let reply_path = exchange.path().join("reply.json");
        let request = Request {
            key: key.to_string(),
            name: name.to_string(),
            url: url.to_string(),
            snapshot_path: snapshot_path.to_path_buf(),
            states: states.clone(),
            output_dir: out_dir.to_path_buf(),
        };
        fs::write(
            &request_path,
            serde_json::to_vec(&request).map_err(|error| error.to_string())?,
        )
        .map_err(|error| format!("writing worker request: {error}"))?;

        let mut command = Command::new(worker_executable()?);
        command
            .arg("--extract-worker")
            .arg(&request_path)
            .arg(&reply_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            // All subprocess scratch paths reside below the parent-owned
            // exchange directory, which is deleted even after SIGKILL.
            .env("TMPDIR", exchange.path())
            .env("TMP", exchange.path())
            .env("TEMP", exchange.path());
        match wait_for_group(&mut command, timeout)? {
            GroupExit::TimedOut => Err(format!("__timeout__ {}", timeout.as_secs_f64())),
            GroupExit::Exited(exit) => {
                if !exit.success() {
                    return Err(format!("extractor worker exited {exit}"));
                }
                let reply: Reply = serde_json::from_slice(
                    &fs::read(&reply_path)
                        .map_err(|error| format!("worker did not return metadata: {error}"))?,
                )
                .map_err(|error| format!("invalid worker response: {error}"))?;
                if reply.stat.name != name {
                    return Err("worker output name differs from request".to_string());
                }
                let output_path = out_dir.join(format!("{name}.txt"));
                let actual_size = fs::metadata(&output_path)
                    .map_err(|error| format!("worker did not write output: {error}"))?
                    .len();
                if reply.stat.status.is_success()
                    && actual_size > u64::try_from(max_output_bytes()?).unwrap_or(u64::MAX)
                {
                    return Err("worker output exceeds configured byte limit".to_string());
                }
                if reply.stat.status.is_success()
                    && actual_size != u64::try_from(reply.stat.output_size).unwrap_or(u64::MAX)
                {
                    return Err("worker output length disagrees with metadata".to_string());
                }
                Ok(reply.stat)
            }
        }
    };

    execute().map_err(|error| {
        error.strip_prefix("__timeout__ ").map_or_else(
            || ExecutionError::failed(error.clone()),
            |seconds| ExecutionError {
                status: ExtractionStatus::TimedOut,
                message: format!("timed out after {seconds}s; terminated process group"),
            },
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_ipc_references_one_shared_snapshot_without_embedding_html(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let source = directory.path().join("source.html");
        let html = format!("<html><p>{}</p></html>", "PAYLOAD_SENTINEL_".repeat(65536));
        fs::write(&source, &html)?;
        let request = Request {
            key: "nanohtml2text".to_string(),
            name: "nanohtml2text".to_string(),
            url: "https://example.com/shared".to_string(),
            snapshot_path: source,
            states: ExtractorStates::default(),
            output_dir: directory.path().to_path_buf(),
        };
        let ipc = serde_json::to_vec(&request)?;
        assert!(
            ipc.len() < html.len() / 8,
            "per-worker IPC unexpectedly scaled with HTML payload size"
        );
        assert!(
            !String::from_utf8_lossy(&ipc).contains("PAYLOAD_SENTINEL_"),
            "per-worker request embedded source HTML"
        );
        let decoded: Request = serde_json::from_slice(&ipc)?;
        assert_eq!(read_snapshot(&decoded)?, html);
        Ok(())
    }

    #[test]
    fn worker_snapshot_path_cannot_escape_owned_run_directory(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let owned = tempfile::tempdir()?;
        let other = tempfile::tempdir()?;
        let other_source = other.path().join("source.html");
        fs::write(&other_source, "<p>escape attempt</p>")?;
        let request = Request {
            key: "nanohtml2text".to_string(),
            name: "nanohtml2text".to_string(),
            url: "https://example.com/".to_string(),
            snapshot_path: other_source,
            states: ExtractorStates::default(),
            output_dir: owned.path().to_path_buf(),
        };
        assert!(read_snapshot(&request).is_err());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn worker_snapshot_cannot_be_a_symlink_outside_run() -> Result<(), Box<dyn std::error::Error>> {
        let owned = tempfile::tempdir()?;
        let other = tempfile::tempdir()?;
        let outside = other.path().join("source.html");
        fs::write(&outside, "<p>escape attempt</p>")?;
        let symlink = owned.path().join("source.html");
        std::os::unix::fs::symlink(outside, &symlink)?;
        let request = Request {
            key: "nanohtml2text".to_string(),
            name: "nanohtml2text".to_string(),
            url: "https://example.com/".to_string(),
            snapshot_path: symlink,
            states: ExtractorStates::default(),
            output_dir: owned.path().to_path_buf(),
        };
        assert!(read_snapshot(&request).is_err());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn browser_stdout_is_bounded_before_loading() -> Result<(), Box<dyn std::error::Error>> {
        let mut command = Command::new("sh");
        command.args(["-c", "printf 123456789"]);
        let error = run_captured_command(command, Duration::from_secs(2), 5);
        assert!(error.is_err_and(|message| message.contains("byte limit")));

        let mut command = Command::new("sh");
        command.args(["-c", "printf 12345"]);
        let output = run_captured_command(command, Duration::from_secs(2), 5)?;
        assert!(output.status.success());
        assert_eq!(output.stdout, b"12345");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn external_capture_stops_stdout_growth_before_completion() {
        let mut command = Command::new("sh");
        command.args(["-c", "yes text | head -c 300000; sleep 2"]);
        let started = Instant::now();
        let error = run_bounded_command(&mut command, Duration::from_secs(3), 128);
        assert!(error.is_err_and(|message| message.contains("stdout exceeds 128")));
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "output limiter waited for an otherwise running process"
        );
    }

    #[cfg(unix)]
    #[test]
    fn bounded_capture_preserves_explicit_stdin() -> Result<(), Box<dyn std::error::Error>> {
        use std::io::{Seek, Write};
        let mut input = tempfile::tempfile()?;
        input.write_all(b"stream from a preopened file")?;
        input.rewind()?;
        let mut command = Command::new("cat");
        command.stdin(Stdio::from(input));
        // Preserve stdin behavior under heavily loaded shared CI runners.
        let output = run_bounded_command(&mut command, Duration::from_secs(10), 100)?;
        assert!(output.status.success());
        assert_eq!(output.stdout, b"stream from a preopened file");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn external_capture_stops_unbounded_stderr() {
        let mut command = Command::new("sh");
        command.args(["-c", "yes err | head -c 3000000 >&2; sleep 2"]);
        let error = run_bounded_command(&mut command, Duration::from_secs(3), 128);
        assert!(error.is_err_and(|message| message.contains("stderr exceeds")));
    }

    #[cfg(unix)]
    #[test]
    fn timeout_terminates_process_tree() -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let heartbeat = root.path().join("heartbeat");
        let mut cmd = Command::new("sh");
        cmd.args([
            "-c",
            "(while :; do echo x >> \"$1\"; sleep 0.01; done) & wait",
            "sh",
        ])
        .arg(&heartbeat)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
        assert!(matches!(
            wait_for_group(&mut cmd, Duration::from_millis(200))?,
            GroupExit::TimedOut
        ));
        let before = fs::read(&heartbeat)?.len();
        thread::sleep(Duration::from_millis(100));
        let after = fs::read(&heartbeat)?.len();
        assert_eq!(before, after, "descendant kept writing after group timeout");
        Ok(())
    }
}
