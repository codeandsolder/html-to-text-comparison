use comfy_table::{CellAlignment, Table};
use std::panic::AssertUnwindSafe;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use std::{fs::write, path::PathBuf};

#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub name: String,
    pub time_micros: u64,
    pub output_size: usize,
    pub panicked: bool,
    pub error: String,
}

pub struct Runner {
    out_dir: PathBuf,
    html: String,
    stats: Vec<Stats>,
    timeout: Duration,
}

impl Runner {
    pub fn new(out_dir: PathBuf, html: String) -> Self {
        Self::with_timeout(out_dir, html, Duration::from_secs(10))
    }

    pub fn with_timeout(out_dir: PathBuf, html: String, timeout: Duration) -> Self {
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
        let output_file = self.out_dir.join(format!("{}.txt", name));
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
                let time_micros = start.elapsed().as_micros() as u64;
                (result, time_micros)
            }));
            let _ = tx.send(result);
        });

        let (output, time_micros, panicked, error) = match rx.recv_timeout(timeout) {
            Ok(result) => match result {
                Ok((output, time_micros)) => {
                    eprintln!(
                        "[runner] finished extractor={name} time_us={} output_bytes={}",
                        time_micros,
                        output.len()
                    );
                    (output, time_micros, false, String::new())
                }
                Err(e) => {
                    let msg = if let Some(s) = e.downcast_ref::<&str>() {
                        s.to_string()
                    } else if let Some(s) = e.downcast_ref::<String>() {
                        s.clone()
                    } else {
                        "Unknown panic".to_string()
                    };
                    eprintln!("[runner] panic extractor={name} error={msg}");
                    (String::new(), 0, true, msg)
                }
            },
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let msg = format!("timed out after {:.1}s", timeout.as_secs_f64());
                eprintln!("[runner] timeout extractor={name} error={msg}");
                (String::new(), 0, false, msg)
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let msg = "worker thread disconnected unexpectedly".to_string();
                eprintln!("[runner] disconnected extractor={name} error={msg}");
                (String::new(), 0, true, msg)
            }
        };

        let final_output = if !error.is_empty() {
            format!("[ERROR] {}\n", error)
        } else if output.is_empty() {
            "[ERROR] empty output — tool may not be installed, crashed, or returned no content]\n"
                .to_string()
        } else {
            output.clone()
        };

        self.stats.push(Stats {
            name: name.clone(),
            time_micros,
            output_size: final_output.len(),
            panicked,
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
        let mut stats = self.stats.clone();
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
            table
                .column_mut(column)
                .unwrap()
                .set_cell_alignment(CellAlignment::Right);
        }

        for stat in &stats {
            table.add_row(vec![
                stat.name.as_str(),
                &format!("{}", stat.time_micros),
                &format!("{}", stat.output_size),
                &format!(
                    "{:.2}%",
                    100.0 - (stat.output_size as f64 / self.html.len() as f64) * 100.0
                ),
                if stat.panicked { "YES" } else { "no" },
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
    fn runner_times_out_stuck_extractor() {
        let out_dir = std::env::temp_dir().join(format!("runner_timeout_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&out_dir).unwrap();
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
        let output = std::fs::read_to_string(out_dir.join("stuck.txt")).unwrap();
        assert!(output.contains("timed out"));
        let _ = std::fs::remove_dir_all(out_dir);
    }
}
