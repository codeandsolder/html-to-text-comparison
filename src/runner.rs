use comfy_table::{CellAlignment, Table};
use std::panic::AssertUnwindSafe;
use std::sync::mpsc;
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

#[derive(Debug, Clone, Default)]
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

    pub fn run(
        &mut self,
        name: impl Into<String>,
        extractor: impl FnOnce(&str) -> String + Send + 'static,
    ) {
        let name = name.into();
        let output_file = self.out_dir.join(format!("{name}.txt"));
        let html = self.html.clone();
        let timeout = self.timeout;
        eprintln!(
            "[runner] starting extractor={name} timeout_ms={} output_file={}",
            timeout.as_millis(),
            output_file.display()
        );

        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let start = Instant::now();
                let result = extractor(&html);
                let time_micros = u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX);
                (result, time_micros)
            }));
            let _ = tx.send(result);
        });

        let (output, time_micros, status, error) = match rx.recv_timeout(timeout) {
            Ok(result) => match result {
                Ok((output, time_micros)) => {
                    eprintln!(
                        "[runner] finished extractor={name} time_us={time_micros} output_bytes={}",
                        output.len()
                    );
                    let (status, error) = output.strip_prefix("[ERROR]").map_or_else(
                        || {
                            if output.is_empty() {
                                (
                                    ExtractionStatus::NoContent,
                                    "extractor returned empty content".to_string(),
                                )
                            } else {
                                (ExtractionStatus::Success, String::new())
                            }
                        },
                        |message| (ExtractionStatus::Failed, message.trim().to_string()),
                    );
                    (output, time_micros, status, error)
                }
                Err(e) => {
                    let msg = e.downcast_ref::<&str>().map_or_else(
                        || {
                            e.downcast_ref::<String>()
                                .map_or_else(|| "Unknown panic".to_string(), Clone::clone)
                        },
                        ToString::to_string,
                    );
                    eprintln!("[runner] panic extractor={name} error={msg}");
                    (String::new(), 0, ExtractionStatus::Panicked, msg)
                }
            },
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let msg = format!("timed out after {:.1}s", timeout.as_secs_f64());
                eprintln!("[runner] timeout extractor={name} error={msg}");
                (String::new(), 0, ExtractionStatus::TimedOut, msg)
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let msg = "worker thread disconnected unexpectedly".to_string();
                eprintln!("[runner] disconnected extractor={name} error={msg}");
                (String::new(), 0, ExtractionStatus::Panicked, msg)
            }
        };

        let final_output = if status.is_success() {
            output
        } else {
            format!("[ERROR] {error}\n")
        };

        self.stats.push(Stats {
            name: name.clone(),
            time_micros,
            output_size: if status.is_success() {
                final_output.len()
            } else {
                0
            },
            status,
            error,
        });

        let _ = write(&output_file, &final_output);
        eprintln!(
            "[runner] wrote extractor={name} bytes={} path={}",
            final_output.len(),
            output_file.display()
        );
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
            "Panic",
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
                if stat.status == ExtractionStatus::Panicked {
                    "YES"
                } else {
                    "no"
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
    use std::time::Duration;

    #[test]
    fn extractor_diagnostics_are_not_counted_as_content() -> Result<(), Box<dyn std::error::Error>>
    {
        use super::ExtractionStatus;

        let out_dir =
            std::env::temp_dir().join(format!("runner_outcomes_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&out_dir)?;
        let mut runner = Runner::new(out_dir.clone(), "<p>Hello</p>".to_string());

        runner.run("success", |_| "Converted".to_string());
        runner.run("failed", |_| "[ERROR] tool missing".to_string());
        runner.run("empty", |_| String::new());

        let stats = runner.into_stats();
        assert_eq!(stats[0].status, ExtractionStatus::Success);
        assert_eq!(stats[0].output_size, "Converted".len());
        assert_eq!(stats[1].status, ExtractionStatus::Failed);
        assert_eq!(stats[1].output_size, 0);
        assert_eq!(stats[1].error, "tool missing");
        assert_eq!(stats[2].status, ExtractionStatus::NoContent);
        assert_eq!(stats[2].output_size, 0);
        assert_eq!(
            std::fs::read_to_string(out_dir.join("failed.txt"))?,
            "[ERROR] tool missing\n"
        );
        std::fs::remove_dir_all(out_dir)?;
        Ok(())
    }

    #[test]
    fn runner_times_out_stuck_extractor() -> Result<(), Box<dyn std::error::Error>> {
        let out_dir = std::env::temp_dir().join(format!("runner_timeout_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&out_dir)?;
        let mut runner = Runner::with_timeout(
            out_dir.clone(),
            "hello".to_string(),
            Duration::from_millis(50),
        );

        runner.run("stuck", |_html| {
            std::thread::sleep(Duration::from_millis(200));
            "late".to_string()
        });

        let stats = runner.into_stats();
        assert_eq!(stats.len(), 1);
        assert!(stats[0].error.contains("timed out"));
        assert_eq!(stats[0].status, super::ExtractionStatus::TimedOut);
        assert_eq!(stats[0].output_size, 0);
        let output = std::fs::read_to_string(out_dir.join("stuck.txt"))?;
        assert!(output.contains("timed out"));
        let _ = std::fs::remove_dir_all(out_dir);
        Ok(())
    }
}
