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
    html: String,
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
    let mut runner = Runner::new(request.output_dir, request.html);
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
                guard.0.take();
                return Ok(GroupExit::Exited(status));
            }
            Ok(None) => {
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

pub fn execute_isolated(
    out_dir: &Path,
    html: &str,
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
            html: html.to_string(),
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
            .stderr(Stdio::inherit());
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
