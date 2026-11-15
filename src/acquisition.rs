use serde::{Deserialize, Serialize};
use std::io::Read;
use std::time::Duration;
use url::Url;

/// Bound acquisition memory independently of extractor timeouts; HTML is
/// copied once per isolated worker and compared across every enabled engine.
pub fn max_html_bytes() -> Result<usize, String> {
    const DEFAULT: usize = 64 * 1024 * 1024;
    const MAXIMUM: usize = 1024 * 1024 * 1024;
    let limit = std::env::var("HTML_TO_TEXT_MAX_HTML_BYTES").map_or(Ok(DEFAULT), |raw| {
        raw.parse::<usize>()
            .map_err(|error| format!("invalid HTML_TO_TEXT_MAX_HTML_BYTES: {error}"))
    })?;
    if !(1..=MAXIMUM).contains(&limit) {
        return Err(format!("HTML_TO_TEXT_MAX_HTML_BYTES must be 1..={MAXIMUM}"));
    }
    Ok(limit)
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
    pub fn acquire(self, url: &Url) -> Result<String, String> {
        if !matches!(url.scheme(), "http" | "https") {
            return Err("site acquisition only supports http(s) URLs".to_string());
        }
        match self {
            Self::File | Self::SavedSnapshot => {
                Err("snapshot acquisition modes do not perform network requests".to_string())
            }
            Self::Http => {
                let max_bytes = max_html_bytes()?;
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
                let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tools/extractors/cloakbrowser-acquire.mjs");
                let mut command = std::process::Command::new("node");
                command.arg(script).arg(url.as_str());
                let output = crate::worker::run_captured_command(
                    command,
                    acquisition_timeout()?,
                    max_html_bytes()?,
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
                    acquisition_timeout()?,
                    max_html_bytes()?,
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
