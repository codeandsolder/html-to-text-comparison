use serde::{Deserialize, Serialize};
use std::io::Read;
use std::time::Duration;
use url::Url;

use crate::runtime_config::ExecutionPolicy;

/// Read a UTF-8 snapshot with a hard byte bound, including files that grow after open.
pub fn read_html_file(path: &std::path::Path, max_bytes: usize) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let limit = u64::try_from(max_bytes).unwrap_or(u64::MAX);
    if file.metadata().map_err(|error| error.to_string())?.len() > limit {
        return Err(format!("input HTML exceeds {max_bytes} byte limit"));
    }
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > max_bytes {
        return Err(format!("input HTML exceeds {max_bytes} byte limit"));
    }
    String::from_utf8(bytes).map_err(|error| error.to_string())
}

/// Exactly one root-document acquisition per run. An extractor receives
/// the resulting immutable HTML, not the page URL as a fetch instruction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Acquisition {
    #[default]
    Http,
    Lightpanda,
    Cloakbrowser,
    File,
    SavedSnapshot,
}

impl Acquisition {
    pub fn acquire(self, url: &Url, policy: &ExecutionPolicy) -> Result<String, String> {
        if !matches!(url.scheme(), "http" | "https") {
            return Err("site acquisition only supports http(s) URLs".to_string());
        }
        match self {
            Self::File | Self::SavedSnapshot => {
                Err("snapshot acquisition modes do not perform network requests".to_string())
            }
            Self::Http => {
                let max_bytes = policy.max_html_bytes;
                let config = ureq::Agent::config_builder()
                    .timeout_global(Some(Duration::from_secs(45)))
                    .build();
                let agent: ureq::Agent = config.into();
                let response = agent
                    .get(url.as_str())
                    .call()
                    .map_err(|error| format!("HTTP acquisition failed: {error}"))?;
                // Decode the response's declared charset (including legacy
                // encodings) before handing the same UTF-8 snapshot to every
                // extractor. The byte limit also bounds expansion on decode.
                let mut bytes = Vec::new();
                response
                    .into_body()
                    .into_with_config()
                    .lossy_utf8(true)
                    .reader()
                    .take(u64::try_from(max_bytes).unwrap_or(u64::MAX) + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|error| format!("reading HTML: {error}"))?;
                if bytes.len() > max_bytes {
                    return Err(format!("acquired HTML exceeds {max_bytes} byte limit"));
                }
                String::from_utf8(bytes)
                    .map_err(|error| format!("decoded HTML was not UTF-8: {error}"))
            }
            Self::Cloakbrowser => {
                let script = policy.extractor_assets.join("cloakbrowser-acquire.mjs");
                let mut command = std::process::Command::new("node");
                command.arg(script).arg(url.as_str());
                let output = crate::worker::run_captured_command(
                    command,
                    policy.browser_timeout(),
                    policy.max_html_bytes,
                )
                .map_err(|error| format!("CloakBrowser acquisition failed: {error}"))?;
                if !output.status.success() {
                    return Err(format!(
                        "CloakBrowser acquisition exited {}: {}",
                        output.status,
                        String::from_utf8_lossy(&output.stderr).trim()
                    ));
                }
                String::from_utf8(output.stdout)
                    .map_err(|error| format!("CloakBrowser returned invalid UTF-8: {error}"))
            }
            Self::Lightpanda => {
                let mut command = std::process::Command::new("docker");
                command
                    .args([
                        "exec",
                        "lightpanda",
                        "lightpanda",
                        "fetch",
                        "--dump",
                        "html",
                    ])
                    .arg(url.as_str());
                let output = crate::worker::run_captured_command(
                    command,
                    policy.browser_timeout(),
                    policy.max_html_bytes,
                )
                .map_err(|error| format!("Lightpanda acquisition failed: {error}"))?;
                if !output.status.success() {
                    return Err(format!(
                        "Lightpanda acquisition exited {}: {}",
                        output.status,
                        String::from_utf8_lossy(&output.stderr).trim()
                    ));
                }
                String::from_utf8(output.stdout)
                    .map_err(|error| format!("Lightpanda returned invalid UTF-8: {error}"))
            }
        }
    }
}
