//! Runs `earnest verify` once on some files and measures its wall time and peak memory.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::models::expected_states;

const POLL: Duration = Duration::from_millis(1);

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Seen {
    pub(crate) name: String,
    pub(crate) states: u64,
}

#[derive(Debug)]
pub(crate) struct Sample {
    pub(crate) wall: Duration,
    pub(crate) peak_kib: Option<u64>,
    pub(crate) models: Vec<Seen>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Job<'a> {
    pub(crate) binary: &'a Path,
    pub(crate) files: &'a [PathBuf],
    pub(crate) workers: Option<usize>,
    pub(crate) max_states: Option<u64>,
    pub(crate) scratch: &'a Path,
}

pub(crate) fn peak_from_status(status: &str) -> Option<u64> {
    status
        .lines()
        .find_map(|line| line.strip_prefix("VmHWM:"))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|number| number.parse().ok())
}

fn peak_of(process: u32) -> Option<u64> {
    let status = fs::read_to_string(format!("/proc/{process}/status")).ok()?;
    peak_from_status(&status)
}

pub(crate) fn parse_results(json: &str) -> Result<Vec<Seen>, String> {
    let value: Value =
        serde_json::from_str(json).map_err(|error| format!("the output is not JSON: {error}"))?;
    let models = value["models"]
        .as_array()
        .ok_or("the JSON has no list of models")?;
    if value["unreadable"]
        .as_array()
        .is_some_and(|list| !list.is_empty())
    {
        return Err("a benchmark model is unreadable".to_owned());
    }
    models
        .iter()
        .map(|model| {
            let name = model["name"].as_str().ok_or("a model has no name")?;
            let status = model["status"].as_str().unwrap_or("missing");
            if status != "pass" {
                return Err(format!("{name} is {status}, not pass"));
            }
            let counted = model["states"]
                .as_u64()
                .ok_or_else(|| format!("{name} has no state count"))?;
            Ok(Seen {
                name: name.to_owned(),
                states: counted,
            })
        })
        .collect()
}

pub(crate) fn check_counts(models: &[Seen]) -> Result<(), String> {
    for seen in models {
        let expected = expected_states(&seen.name)
            .ok_or_else(|| format!("{} is not a benchmark model", seen.name))?;
        if seen.states != expected {
            return Err(format!(
                "{}: the checker counted {} states, but README.md derives {expected}",
                seen.name, seen.states
            ));
        }
    }
    Ok(())
}

pub(crate) fn run_once(job: &Job<'_>) -> Result<Sample, String> {
    let mut command = Command::new(job.binary);
    command.args(["verify", "--no-cache", "--json"]);
    if let Some(workers) = job.workers {
        command.args(["--jobs", &workers.to_string()]);
    }
    if let Some(limit) = job.max_states {
        command.args(["--max-states", &limit.to_string()]);
    }
    let started = Instant::now();
    let child = command
        .args(job.files)
        .current_dir(job.scratch)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("{} did not start: {error}", job.binary.display()))?;
    let process = child.id();
    let waiting = thread::spawn(move || child.wait_with_output());
    let mut peak_kib = None;
    while !waiting.is_finished() {
        peak_kib = peak_kib.max(peak_of(process));
        thread::sleep(POLL);
    }
    let wall = started.elapsed();
    let output = waiting
        .join()
        .map_err(|_| "the thread that waits for the checker panicked".to_owned())?
        .map_err(|error| format!("waiting for the checker failed: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        return Err(format!(
            "the checker exited with {}: {stdout}{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let models = parse_results(&stdout)?;
    check_counts(&models)?;
    Ok(Sample {
        wall,
        peak_kib,
        models,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peak_is_read_from_the_status_file() {
        let status = "Name:\tearnest\nVmPeak:\t  900 kB\nVmHWM:\t  123456 kB\nVmRSS:\t  100 kB\n";
        assert_eq!(
            peak_from_status(status),
            Some(123_456),
            "the high-water mark"
        );
        assert_eq!(peak_from_status("Name:\tearnest\n"), None, "no mark");
    }

    #[test]
    fn results_give_each_models_name_and_states() {
        let json =
            r#"{"models":[{"name":"Counter","status":"pass","states":100}],"unreadable":[]}"#;
        assert_eq!(
            parse_results(json),
            Ok(vec![Seen {
                name: "Counter".to_owned(),
                states: 100
            }]),
            "one passing model"
        );
    }

    #[test]
    fn a_model_that_does_not_pass_is_an_error() {
        let json =
            r#"{"models":[{"name":"Counter","status":"fail","states":100}],"unreadable":[]}"#;
        assert_eq!(
            parse_results(json),
            Err("Counter is fail, not pass".to_owned()),
            "a failing model"
        );
        assert!(parse_results("not json").is_err(), "text is not JSON");
        let unreadable = r#"{"models":[],"unreadable":[{"file":"a"}]}"#;
        assert!(parse_results(unreadable).is_err(), "an unreadable file");
    }

    #[test]
    fn counts_must_equal_the_derived_ones() {
        let right = [Seen {
            name: "Counter".to_owned(),
            states: 100,
        }];
        assert_eq!(check_counts(&right), Ok(()), "the derived count");
        let wrong = [Seen {
            name: "Counter".to_owned(),
            states: 101,
        }];
        assert_eq!(
            check_counts(&wrong),
            Err("Counter: the checker counted 101 states, but README.md derives 100".to_owned()),
            "a different count fails"
        );
        let stranger = [Seen {
            name: "Stranger".to_owned(),
            states: 1,
        }];
        assert!(check_counts(&stranger).is_err(), "an unknown model fails");
    }
}
