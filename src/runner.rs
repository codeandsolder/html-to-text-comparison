use comfy_table::{CellAlignment, Table};
use std::panic::AssertUnwindSafe;
use std::time::{Duration, Instant};
use std::{fs::write, path::PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionStatus {
    #[default]
    Success,
    NoContent,
    Failed,
    TimedOut,
    Panicked,
}

impl ExtractionStatus {
    pub const fn is_success(self) -> bool {
        matches!(self, Self::Success)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Stats {
    pub name: String,
    pub time_micros: u64,
    pub output_size: usize,
    pub status: ExtractionStatus,
    pub error: String,
}

pub struct Runner {
    out_dir: PathBuf,
    html: String,
    stats: Vec<Stats>,
    timeout: Duration,
}

impl Runner {
    pub const fn new(out_dir: PathBuf, html: String) -> Self {
        Self::with_timeout(out_dir, html, Duration::from_secs(10))
    }

    pub const fn with_timeout(out_dir: PathBuf, html: String, timeout: Duration) -> Self {
        Self {
            out_dir,
            html,
            stats: Vec::new(),
            timeout,
        }
    }

    // Runs only inside the isolated worker, or in a focused in-process unit test.
    // A caller requiring a deadline must use run_isolated: Rust threads cannot
    // be forcibly cancelled, so a receive timeout must not masquerade as one.
    pub fn run(
        &mut self,
        name: impl Into<String>,
        extractor: impl FnOnce(&str) -> Result<String, String>,
    ) {
        let name = name.into();
        let output_file = self.out_dir.join(format!("{name}.txt"));
        let started = Instant::now();
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| extractor(&self.html)));
        let elapsed = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
        let (output, status, error) = match result {
            Ok(Ok(output)) if !output.is_empty() => {
                (output, ExtractionStatus::Success, String::new())
            }
            Ok(Ok(_)) => (
                String::new(),
                ExtractionStatus::NoContent,
                "extractor returned empty content".to_string(),
            ),
            Ok(Err(error)) => (String::new(), ExtractionStatus::Failed, error),
            Err(payload) => {
                let error = payload.downcast_ref::<&str>().map_or_else(
                    || {
                        payload
                            .downcast_ref::<String>()
                            .map_or_else(|| "Unknown panic".to_string(), Clone::clone)
                    },
                    ToString::to_string,
                );
                (String::new(), ExtractionStatus::Panicked, error)
            }
        };
        let final_output = if status.is_success() {
            output
        } else {
            format!("[ERROR] {error}\n")
        };
        let output_size = if status.is_success() {
            final_output.len()
        } else {
            0
        };
        let stat = Stats {
            name,
            time_micros: elapsed,
            output_size,
            status,
            error,
        };
        if let Err(error) = write(&output_file, &final_output) {
            self.stats.push(Stats {
                status: ExtractionStatus::Failed,
                output_size: 0,
                error: format!("cannot write {}: {error}", output_file.display()),
                ..stat
            });
        } else {
            self.stats.push(stat);
        }
    }

    pub fn run_isolated(
        &mut self,
        extractor_key: &str,
        output_name: &str,
        url: &str,
        states: &crate::extractor_config::ExtractorStates,
    ) {
        let outcome = crate::worker::execute_isolated(
            &self.out_dir,
            &self.html,
            extractor_key,
            output_name,
            url,
            states,
            self.timeout,
        );
        let stat = outcome.unwrap_or_else(|error| Stats {
            name: output_name.to_string(),
            status: error.status,
            error: error.message,
            ..Stats::default()
        });
        if !stat.status.is_success() {
            let diagnostic = format!("[ERROR] {}\n", stat.error);
            let output_file = self.out_dir.join(format!("{output_name}.txt"));
            if let Err(error) = write(&output_file, diagnostic) {
                eprintln!("Cannot write diagnostic {}: {error}", output_file.display());
            }
        }
        self.stats.push(stat);
    }

    pub fn into_stats(self) -> Vec<Stats> {
        self.stats
    }

    pub fn into_table(self) -> Table {
        let mut stats = self.stats;
        stats.sort_by_key(|s| s.name.clone());

        let mut table = Table::new();
        table.set_header(vec![
            "Name",
            "Time (microseconds)",
            "Output Size (bytes)",
            "% Reduction",
            "Status",
            "Error",
            "Output File",
        ]);
        let numeric_columns = 1..=2;
        for column in numeric_columns {
            if let Some(column) = table.column_mut(column) {
                column.set_cell_alignment(CellAlignment::Right);
            }
        }

        for stat in &stats {
            table.add_row(vec![
                stat.name.as_str(),
                &format!("{}", stat.time_micros),
                &format!("{}", stat.output_size),
                &if stat.status.is_success() {
                    format!(
                        "{:.2}%",
                        crate::metrics::reduction_percent(stat.output_size, self.html.len())
                    )
                } else {
                    "—".to_string()
                },
                match stat.status {
                    ExtractionStatus::Success => "success",
                    ExtractionStatus::NoContent => "no_content",
                    ExtractionStatus::Failed => "failed",
                    ExtractionStatus::TimedOut => "timed_out",
                    ExtractionStatus::Panicked => "panicked",
                },
                if stat.error.is_empty() {
                    "-"
                } else {
                    &stat.error
                },
                &format!(
                    "{}",
                    self.out_dir.join(format!("{}.txt", stat.name)).display()
                ),
            ]);
        }
        table
    }
}

#[cfg(test)]
mod tests {
    use super::Runner;

    #[test]
    fn typed_extractor_outcomes_preserve_legitimate_text() -> Result<(), Box<dyn std::error::Error>>
    {
        use super::ExtractionStatus;

        let out_dir =
            std::env::temp_dir().join(format!("runner_outcomes_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&out_dir)?;
        let mut runner = Runner::new(out_dir.clone(), "<p>Hello</p>".to_string());

        runner.run("success", |_| Ok("Converted".to_string()));
        runner.run("failed", |_| Err("tool missing".to_string()));
        runner.run("empty", |_| Ok(String::new()));
        runner.run("prefix", |_| {
            Ok("[ERROR] legitimate document text".to_string())
        });

        let stats = runner.into_stats();
        assert_eq!(stats[0].status, ExtractionStatus::Success);
        assert_eq!(stats[0].output_size, "Converted".len());
        assert_eq!(stats[1].status, ExtractionStatus::Failed);
        assert_eq!(stats[1].output_size, 0);
        assert_eq!(stats[1].error, "tool missing");
        assert_eq!(stats[2].status, ExtractionStatus::NoContent);
        assert_eq!(stats[2].output_size, 0);
        assert_eq!(stats[3].status, ExtractionStatus::Success);
        assert_eq!(
            stats[3].output_size,
            "[ERROR] legitimate document text".len()
        );
        assert_eq!(
            std::fs::read_to_string(out_dir.join("failed.txt"))?,
            "[ERROR] tool missing\n"
        );
        std::fs::remove_dir_all(out_dir)?;
        Ok(())
    }
}
