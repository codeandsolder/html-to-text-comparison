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
            Self::Lightpanda => {
                let output = std::process::Command::new("docker")
                    .args([
                        "exec",
                        "lightpanda",
                        "lightpanda",
                        "fetch",
                        "--dump",
                        "html",
                    ])
                    .arg(url.as_str())
                    .output()
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
