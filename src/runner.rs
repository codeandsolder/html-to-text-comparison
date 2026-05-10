use comfy_table::{CellAlignment, Table};
use std::panic::AssertUnwindSafe;
use std::time::Instant;
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
}

impl Runner {
    pub fn new(out_dir: PathBuf, html: String) -> Self {
        Self {
            out_dir,
            html,
            stats: Vec::new(),
        }
    }

    pub fn run(&mut self, name: impl Into<String>, extractor: impl Fn(&str) -> String) {
        let name = name.into();
        let (output, time_micros, panicked, error) =
            match std::panic::catch_unwind(AssertUnwindSafe(|| {
                let start = Instant::now();
                let result = extractor(&self.html);
                let time_micros = start.elapsed().as_micros() as u64;
                (result, time_micros)
            })) {
                Ok((output, time_micros)) => (output, time_micros, false, String::new()),
                Err(e) => {
                    let msg = if let Some(s) = e.downcast_ref::<&str>() {
                        s.to_string()
                    } else if let Some(s) = e.downcast_ref::<String>() {
                        s.clone()
                    } else {
                        "Unknown panic".to_string()
                    };
                    (String::new(), 0, true, msg)
                }
            };

        let final_output = if !error.is_empty() {
            format!("[ERROR] {}\n", error)
        } else if output.is_empty() {
            "[ERROR] empty output — tool may not be installed, crashed, or returned no content]\n".to_string()
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

        let output_file = self.out_dir.join(format!("{}.txt", name));
        let _ = write(&output_file, &final_output);
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
                if stat.error.is_empty() { "-" } else { &stat.error },
                &format!(
                    "{}",
                    self.out_dir.join(format!("{}.txt", stat.name)).display()
                ),
            ]);
        }
        table
    }
}