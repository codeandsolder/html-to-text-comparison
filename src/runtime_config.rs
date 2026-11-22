//! Explicit, validated process configuration, shared across the worker boundary.

use serde::{Deserialize, Serialize};
use std::{
    net::{Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    time::Duration,
};

const DEFAULT_BYTE_LIMIT: usize = 64 * 1024 * 1024;
const MAX_BYTE_LIMIT: usize = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExecutionPolicy {
    pub max_html_bytes: usize,
    pub max_output_bytes: usize,
    pub browser_timeout_ms: u64,
    pub extractor_assets: PathBuf,
    pub webclaw_bin: PathBuf,
    pub html_to_markdown_go_bin: PathBuf,
}

impl Default for ExecutionPolicy {
    fn default() -> Self {
        Self {
            max_html_bytes: DEFAULT_BYTE_LIMIT,
            max_output_bytes: DEFAULT_BYTE_LIMIT,
            browser_timeout_ms: 120_000,
            extractor_assets: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tools/extractors"),
            webclaw_bin: "webclaw".into(),
            html_to_markdown_go_bin: "html2markdown".into(),
        }
    }
}

impl ExecutionPolicy {
    pub fn validate(&self) -> Result<(), String> {
        for (field, value) in [
            ("max_html_bytes", self.max_html_bytes),
            ("max_output_bytes", self.max_output_bytes),
        ] {
            if !(1..=MAX_BYTE_LIMIT).contains(&value) {
                return Err(format!("{field} must be 1..={MAX_BYTE_LIMIT}"));
            }
        }
        if !(1_000..=900_000).contains(&self.browser_timeout_ms) {
            return Err("browser_timeout_ms must be 1,000..=900,000".to_owned());
        }
        for (field, path) in [
            ("extractor_assets", &self.extractor_assets),
            ("webclaw_bin", &self.webclaw_bin),
            ("html_to_markdown_go_bin", &self.html_to_markdown_go_bin),
        ] {
            if path.as_os_str().is_empty() {
                return Err(format!("{field} must not be empty"));
            }
        }
        Ok(())
    }

    pub const fn browser_timeout(&self) -> Duration {
        Duration::from_millis(self.browser_timeout_ms)
    }
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerConfig {
    pub listen: SocketAddr,
    pub data_dir: PathBuf,
    pub max_saved_runs: usize,
    pub max_concurrent_runs: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen: SocketAddr::from((Ipv4Addr::LOCALHOST, 3000)),
            data_dir: "data".into(),
            max_saved_runs: 100,
            max_concurrent_runs: 2,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TunerConfig {
    pub listen: SocketAddr,
    pub corpus_dir: PathBuf,
    pub state_dir: PathBuf,
    pub max_concurrent_previews: usize,
}

impl TunerConfig {
    fn validate(&self) -> Result<(), String> {
        if !self.listen.ip().is_loopback() {
            return Err("tuner listen address must be loopback".into());
        }
        if self.corpus_dir.as_os_str().is_empty() || self.state_dir.as_os_str().is_empty() {
            return Err("tuner corpus_dir and state_dir must not be empty".into());
        }
        if !(1..=64).contains(&self.max_concurrent_previews) {
            return Err("tuner max_concurrent_previews must be 1..=64".into());
        }
        Ok(())
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RuntimeConfig {
    pub execution: ExecutionPolicy,
    pub server: ServerConfig,
    pub tuner: Option<TunerConfig>,
}

impl RuntimeConfig {
    pub fn load(path: Option<&Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let config = if let Some(path) = path {
            serde_json::from_slice(&std::fs::read(path)?)?
        } else {
            Self::default()
        };
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), String> {
        self.execution.validate()?;
        if let Some(tuner) = &self.tuner {
            tuner.validate()?;
        }
        if self.server.data_dir.as_os_str().is_empty() {
            return Err("data_dir must not be empty".to_owned());
        }
        if self.server.max_saved_runs == 0 {
            return Err("max_saved_runs must be positive".to_owned());
        }
        if !(1..=64).contains(&self.server.max_concurrent_runs) {
            return Err("max_concurrent_runs must be 1..=64".to_owned());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_configuration_has_one_source_of_defaults() -> Result<(), Box<dyn std::error::Error>>
    {
        let config: RuntimeConfig = serde_json::from_str(
            r#"{"execution":{"max_output_bytes":128},"server":{"listen":"[::1]:8080"}}"#,
        )?;
        config.validate()?;
        assert_eq!(config.execution.max_output_bytes, 128);
        assert_eq!(config.execution.max_html_bytes, DEFAULT_BYTE_LIMIT);
        assert_eq!(config.execution.browser_timeout(), Duration::from_secs(120));
        assert_eq!(config.server.listen.to_string(), "[::1]:8080");
        assert_eq!(config.server.max_saved_runs, 100);
        assert_eq!(config.server.max_concurrent_runs, 2);
        Ok(())
    }

    #[test]
    fn tuner_configuration_is_explicit_and_loopback_only() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut config: RuntimeConfig = serde_json::from_str(
            r#"{"execution":{"extractor_assets":"/assets"},"tuner":{"listen":"127.0.0.1:18580","corpus_dir":"/corpus","state_dir":"/state","max_concurrent_previews":3}}"#,
        )?;
        config.validate()?;
        assert_eq!(config.execution.extractor_assets, PathBuf::from("/assets"));
        let tuner = config.tuner.as_mut().ok_or("missing tuner configuration")?;
        assert_eq!(tuner.max_concurrent_previews, 3);
        tuner.listen = "0.0.0.0:18580".parse()?;
        assert!(config.validate().is_err());
        Ok(())
    }

    #[test]
    fn malformed_and_out_of_range_configuration_is_rejected() {
        for input in [
            r#"{"execution":{"max_html_bytes":0}}"#,
            r#"{"execution":{"max_output_bytes":1073741825}}"#,
            r#"{"execution":{"browser_timeout_ms":999}}"#,
            r#"{"execution":{"webclaw_bin":""}}"#,
            r#"{"execution":{"extractor_assets":""}}"#,
            r#"{"execution":{"node_modules":"/unrelated/modules"}}"#,
            r#"{"tuner":{"listen":"127.0.0.1:18580","corpus_dir":"/corpus","state_dir":"/state","max_concurrent_previews":0}}"#,
            r#"{"server":{"max_concurrent_runs":65}}"#,
            r#"{"server":{"max_saved_runs":0}}"#,
            r#"{"server":{"data_dir":""}}"#,
            r#"{"server":{"listen":"not an address"}}"#,
            r#"{"execution":{"max_ouput_bytes":128}}"#,
        ] {
            assert!(
                serde_json::from_str::<RuntimeConfig>(input)
                    .map_err(|error| error.to_string())
                    .and_then(|config| config.validate())
                    .is_err(),
                "{input}"
            );
        }
    }
}
