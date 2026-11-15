use serde::{Deserialize, Serialize};
use std::io::Read;
use url::Url;

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
    pub fn acquire(self, url: &Url) -> Result<String, String> {
        if !matches!(url.scheme(), "http" | "https") {
            return Err("site acquisition only supports http(s) URLs".to_string());
        }
        match self {
            Self::File | Self::SavedSnapshot => {
                Err("snapshot acquisition modes do not perform network requests".to_string())
            }
            Self::Http => {
                let response = ureq::get(url.as_str())
                    .call()
                    .map_err(|error| format!("HTTP acquisition failed: {error}"))?;
                let mut html = String::new();
                response
                    .into_body()
                    .into_reader()
                    .read_to_string(&mut html)
                    .map_err(|error| format!("reading HTML: {error}"))?;
                Ok(html)
            }
            Self::Cloakbrowser => {
                let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tools/extractors/cloakbrowser-acquire.mjs");
                let mut command = std::process::Command::new("node");
                command.arg(script).arg(url.as_str());
                let output =
                    crate::worker::run_captured_command(command, acquisition_timeout()?)
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
                let output = crate::worker::run_captured_command(command, acquisition_timeout()?)
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

/// Browser navigation limits apply to the entire process group (including
/// the Chromium descendants), and can be lengthened for cold first launches.
fn acquisition_timeout() -> Result<std::time::Duration, String> {
    let millis = std::env::var("HTML_TO_TEXT_ACQUIRE_TIMEOUT_MS")
        .unwrap_or_else(|_| "120000".to_string())
        .parse::<u64>()
        .map_err(|error| format!("invalid acquisition deadline: {error}"))?;
    if !(1000..=900_000).contains(&millis) {
        return Err("acquisition deadline must be 1,000–900,000 ms".to_string());
    }
    Ok(std::time::Duration::from_millis(millis))
}
