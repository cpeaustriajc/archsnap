//! Measures how fast the release `earnest verify` is on a fixed set of models, and prints the
//! result as Markdown. It stands in for the speed targets of the spec until the first app's
//! models are available.

mod models;
mod report;
mod run;

use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::thread;
use std::time::Duration;

use models::{LARGEST, Language, R12_LARGE, SCALED};
use report::Row;
use run::{Job, run_once};

const DEFAULT_RUNS: usize = 5;
const USAGE: &str = "usage: benchmark [--runs <n>] [--binary <path>]";

#[derive(Debug)]
struct Options {
    runs: usize,
    binary: PathBuf,
}

#[derive(Clone, Debug)]
struct Measured {
    median: Duration,
    peak_kib: Option<u64>,
    states: u64,
}

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn parse_options(arguments: &[String]) -> Result<Options, String> {
    let mut runs = DEFAULT_RUNS;
    let mut binary = repository().join("target/release/earnest");
    let mut remaining = arguments.iter();
    while let Some(flag) = remaining.next() {
        let value = remaining.next();
        match (flag.as_str(), value) {
            ("--runs", Some(count)) => {
                runs = count
                    .parse()
                    .ok()
                    .filter(|count| *count > 0)
                    .ok_or_else(|| format!("--runs needs a whole number above 0. {USAGE}"))?;
            }
            ("--binary", Some(path)) => binary = PathBuf::from(path),
            _ => return Err(format!("{flag} is not understood. {USAGE}")),
        }
    }
    Ok(Options { runs, binary })
}

struct Bench<'a> {
    options: &'a Options,
    scratch: &'a Path,
    cache: HashMap<(Vec<PathBuf>, Option<usize>), Measured>,
}

impl Bench<'_> {
    fn measure(&mut self, files: &[PathBuf], workers: Option<usize>) -> Result<Measured, String> {
        let key = (files.to_vec(), workers);
        if let Some(known) = self.cache.get(&key) {
            return Ok(known.clone());
        }
        let job = Job {
            binary: &self.options.binary,
            files,
            workers,
            max_states: models::state_limit(files),
            scratch: self.scratch,
        };
        let names: Vec<String> = files
            .iter()
            .filter_map(|file| file.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .collect();
        write_progress(&format!(
            "measuring {} with {} worker(s)",
            names.join(" "),
            workers.map_or_else(|| "all".to_owned(), |count| count.to_string())
        ))
        .map_err(|error| error.to_string())?;
        run_once(&job)?;
        let mut walls = Vec::new();
        let mut peak_kib = None;
        let mut states = 0;
        for _ in 0..self.options.runs {
            let sample = run_once(&job)?;
            walls.push(sample.wall);
            peak_kib = peak_kib.max(sample.peak_kib);
            states = sample.models.iter().map(|seen| seen.states).sum();
        }
        let measured = Measured {
            median: report::median(&walls),
            peak_kib,
            states,
        };
        self.cache.insert(key, measured.clone());
        Ok(measured)
    }
}

fn write_progress(message: &str) -> io::Result<()> {
    writeln!(io::stderr().lock(), "{message}")
}

fn row(label: &str, language: &str, measured: &Measured) -> Row {
    Row {
        model: label.to_owned(),
        language: language.to_owned(),
        states: measured.states,
        median: measured.median,
        peak_kib: measured.peak_kib,
    }
}

fn benchmark(options: &Options) -> Result<String, String> {
    if !options.binary.is_file() {
        return Err(format!(
            "{} does not exist; build it with `cargo build --release -p earnest-cli`",
            options.binary.display()
        ));
    }
    let scratch = tempfile::tempdir().map_err(|error| error.to_string())?;
    let repository = repository();
    let mut bench = Bench {
        options,
        scratch: scratch.path(),
        cache: HashMap::new(),
    };

    let mut scaled = Vec::new();
    for entry in SCALED {
        for language in [Language::TypeScript, Language::Rust] {
            let file = models::scaled_path(&repository, entry.file, language);
            let measured = bench.measure(&[file], None)?;
            scaled.push(row(entry.model, language.label(), &measured));
        }
    }

    let mixed = models::mixed_files(&repository);
    let together = bench.measure(&mixed, None)?;
    let mut alone = Vec::new();
    for file in &mixed {
        let measured = bench.measure(std::slice::from_ref(file), None)?;
        let label = file
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map_or("?", |stem| stem.trim_end_matches(".model"));
        alone.push(row(label, Language::TypeScript.label(), &measured));
    }
    let slowest = alone
        .iter()
        .map(|each| each.median)
        .max()
        .unwrap_or(Duration::ZERO);

    let largest = models::scaled_path(&repository, LARGEST, Language::TypeScript);
    let cores = thread::available_parallelism().map_or(1, usize::from);
    let all = bench.measure(std::slice::from_ref(&largest), None)?;
    let one = bench.measure(std::slice::from_ref(&largest), Some(1))?;
    let target = bench.measure(std::slice::from_ref(&largest), Some(report::TARGET_WORKERS))?;

    let r12_large = models::scaled_path(&repository, R12_LARGE, Language::TypeScript);
    let r12_alone = bench.measure(std::slice::from_ref(&r12_large), None)?;
    let mut with_r12_large = mixed.clone();
    with_r12_large.push(r12_large);
    let everything = bench.measure(&with_r12_large, None)?;
    let slowest_row = alone.iter().max_by_key(|each| each.median);
    let r12 = report::r12_lines(
        everything.median,
        with_r12_large.len(),
        report::Alone {
            model: R12_LARGE,
            median: r12_alone.median,
        },
        report::Alone {
            model: slowest_row.map_or("?", |each| each.model.as_str()),
            median: slowest,
        },
    );

    let spec = report::spec_lines(together.median, slowest);
    let together_line = format!(
        "the 12 models in that one run: {} states, peak memory {} MiB",
        report::with_separators(together.states),
        report::memory(together.peak_kib)
    );
    let target_line = report::target_line(LARGEST, target.median, target.peak_kib, cores);
    let speedup = report::speedup_line(LARGEST, one.median, all.median, cores);
    let text = [
        format!(
            "Cores: {cores}. Runs per model: {} after one unmeasured run. Wall time includes starting the process and reading the files; peak memory is the largest resident set of any run.\n",
            options.runs
        ),
        "## Scaled models\n".to_owned(),
        report::table(&scaled),
        "## The 12 mixed models, each alone\n".to_owned(),
        report::table(&alone),
        "## Spec targets\n".to_owned(),
        format!(
            "- {}\n- {}\n- {together_line}\n- {target_line}\n",
            spec[0], spec[1]
        ),
        "## One large model on one worker and on all cores\n".to_owned(),
        format!("- {speedup}\n"),
        "## R12: many models and one large model in one run\n".to_owned(),
        format!(
            "- {}\n- {}\n- peak memory {} MiB\n",
            r12[0],
            r12[1],
            report::memory(everything.peak_kib)
        ),
    ]
    .join("\n");
    Ok(text)
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let outcome = parse_options(&arguments).and_then(|options| benchmark(&options));
    match outcome {
        Ok(text) => match io::stdout().lock().write_all(text.as_bytes()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::FAILURE,
        },
        Err(message) => {
            let _ = writeln!(io::stderr().lock(), "benchmark: {message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn readme() -> String {
        fs::read_to_string(repository().join("tasks/benchmark/README.md"))
            .expect("the benchmark README exists")
    }

    #[test]
    fn options_default_and_parse() {
        let defaults = parse_options(&[]).expect("no arguments is valid");
        assert_eq!(defaults.runs, DEFAULT_RUNS, "the default number of runs");
        let given = parse_options(&[
            "--runs".to_owned(),
            "3".to_owned(),
            "--binary".to_owned(),
            "x".to_owned(),
        ])
        .expect("both flags are valid");
        assert_eq!(
            (given.runs, given.binary),
            (3, PathBuf::from("x")),
            "both flags are read"
        );
    }

    #[test]
    fn options_refuse_bad_flags() {
        for bad in [
            vec!["--runs"],
            vec!["--runs", "0"],
            vec!["--runs", "many"],
            vec!["--fast", "1"],
        ] {
            let arguments: Vec<String> = bad.iter().map(|word| (*word).to_owned()).collect();
            assert!(parse_options(&arguments).is_err(), "{bad:?} is refused");
        }
    }

    #[test]
    fn every_benchmark_file_exists_in_both_languages_where_scaled() {
        let repository = repository();
        for entry in SCALED {
            for language in [Language::TypeScript, Language::Rust] {
                let path = models::scaled_path(&repository, entry.file, language);
                assert!(path.is_file(), "{} exists", path.display());
            }
        }
        let mixed = models::mixed_files(&repository);
        assert_eq!(mixed.len(), 12, "the stand-in set has 12 models");
        for path in mixed {
            assert!(path.is_file(), "{} exists", path.display());
        }
    }

    #[test]
    fn the_readme_derives_every_expected_count() {
        let readme = readme();
        for (name, states) in models::EXPECTED_STATES {
            let line = readme
                .lines()
                .find(|line| line.contains(&format!("`{name}`")))
                .unwrap_or_else(|| panic!("README.md has a row for {name}"));
            assert!(
                line.contains(&report::with_separators(states)),
                "the row for {name} states {states} states: {line}"
            );
        }
    }
}
