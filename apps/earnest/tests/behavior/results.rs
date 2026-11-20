use serde_json::{Value, json};

use crate::expected::{self, MODELS};
use crate::support::{Run, Sandbox, model, models, rule, without_timings};

fn is_a_time(field: &str) -> bool {
    field
        .strip_suffix(" ms")
        .is_some_and(|number| number.parse::<u64>().is_ok())
}

fn detail_lines(run: &Run) -> Vec<&str> {
    run.stdout
        .lines()
        .filter(|line| line.starts_with(char::is_whitespace))
        .collect()
}

fn verify_all(arguments: &[&str]) -> Run {
    let sandbox = Sandbox::with(&expected::files());
    let run = Run::of(sandbox.command().arg("verify").args(arguments));
    run.exits(1);
    run
}

#[test]
fn r15_one_result_line_per_model_with_its_state_count_and_time() {
    let run = Sandbox::with(&["counter.model.ts", "turnstile.model.ts"]).earnest(&["verify"]);
    run.exits(1);
    let lines = run.model_lines();
    assert_eq!(lines.len(), 2, "one result line for each model\n{run}");
    for (line, wanted) in lines.iter().zip([
        ["pass", "Counter", "4 states"],
        ["fail", "Turnstile", "4 states"],
    ]) {
        assert_eq!(line.len(), 4, "status, model, states and time\n{run}");
        let found: Vec<&str> = line.iter().take(3).map(String::as_str).collect();
        assert_eq!(found, wanted, "status, model and state count\n{run}");
        assert!(
            line.get(3).is_some_and(|field| is_a_time(field)),
            "the line ends with its time as `<n> ms`\n{run}"
        );
    }
    assert_eq!(
        run.summary_total(),
        2,
        "the model count of the summary line"
    );
    assert_eq!(run.summary_count("pass"), 1, "models that pass");
    assert_eq!(run.summary_count("fail"), 1, "models that fail");
    assert_eq!(
        run.stdout.lines().last().map(str::to_owned),
        Some(run.summary_line()),
        "the summary line is the last line\n{run}"
    );
}

#[test]
fn r15_the_exit_code_is_zero_only_when_every_model_passes() {
    let passing: [&[&str]; 2] = [
        &["counter.model.ts"],
        &[
            "counter.model.ts",
            "heater.model.ts",
            "switches.model.ts",
            "counter.rs",
        ],
    ];
    for fixtures in passing {
        Sandbox::with(fixtures).earnest(&["verify"]).exits(0);
        Sandbox::with(fixtures)
            .earnest(&["verify", "--json"])
            .exits(0);
    }
    let failing: [&[&str]; 6] = [
        &["counter.model.ts", "turnstile.model.ts"],
        &["counter.model.ts", "download.model.ts"],
        &["counter.model.ts", "tally.model.ts"],
        &["counter.model.ts", "basket.model.ts"],
        &["counter.model.ts", "unreadable/clock.model.ts"],
        &["counter.model.ts", "turnstile.rs"],
    ];
    for fixtures in failing {
        Sandbox::with(fixtures).earnest(&["verify"]).exits(1);
        Sandbox::with(fixtures)
            .earnest(&["verify", "--json"])
            .exits(1);
    }
    Sandbox::with(&["grid.model.ts"])
        .earnest(&["verify", "--max-states", "10"])
        .exits(1);
}

#[test]
fn r16_the_same_results_come_out_as_json() {
    let run =
        Sandbox::with(&["counter.model.ts", "turnstile.model.ts"]).earnest(&["verify", "--json"]);
    run.exits(1);
    let report = run.json();
    assert_eq!(
        report["summary"],
        json!({ "models": 2, "pass": 1, "fail": 1, "unfinished": 0, "unreadable": 0 }),
        "the summary"
    );
    assert_eq!(report["unreadable"], json!([]), "no file is unreadable");
    assert_eq!(
        report["ruleChanges"],
        json!([]),
        "no rule changes without `--base`"
    );

    let counter = model(&report, "Counter");
    assert_eq!(
        without_timings(counter),
        json!({
            "name": "Counter",
            "file": "models/counter.model.ts",
            "line": 3,
            "status": "pass",
            "states": 4,
            "reason": null,
            "rules": counter["rules"],
            "findings": [],
        }),
        "the fields of a model that passes"
    );
    assert!(
        counter["milliseconds"].is_u64(),
        "the time is a whole number: {counter}"
    );
    assert_eq!(counter["cached"], false, "a first run is not cached");
    let bounded = rule(counter, "bounded");
    assert_eq!(
        bounded["says"], "The count is never above three",
        "the sentence of a rule"
    );
    assert_eq!(
        bounded["verdict"], "holds",
        "the verdict of a rule that holds"
    );
    assert_eq!(
        rule(counter, "even")["verdict"],
        "holds",
        "the verdict of the second rule"
    );

    let turnstile = model(&report, "Turnstile");
    assert_eq!(
        turnstile["file"], "models/turnstile.model.ts",
        "the file of a model"
    );
    assert_eq!(turnstile["line"], 3, "the line the model is declared on");
    assert_eq!(
        turnstile["status"], "fail",
        "the status of a model with a broken rule"
    );
    assert_eq!(turnstile["states"], 4, "the distinct states");
    assert_eq!(
        turnstile["reason"],
        Value::Null,
        "a fully checked model has no reason"
    );
    assert_eq!(
        turnstile["findings"],
        json!([]),
        "a broken rule is not a finding"
    );
    let locked = rule(turnstile, "locked");
    assert_eq!(
        locked["says"], "The gate stays locked until the visitor pays",
        "the sentence of the broken rule"
    );
    assert_eq!(
        locked["verdict"], "broken",
        "the verdict of the broken rule"
    );
}

#[test]
fn r16_json_and_text_agree_on_every_model() {
    let text = verify_all(&[]);
    let report = verify_all(&["--json"]).json();
    let lines = text.model_lines();
    assert_eq!(
        lines.len(),
        models(&report).len(),
        "as many lines as JSON results"
    );
    for (line, found) in lines.iter().zip(models(&report)) {
        let states = format!("{} states", found["states"]);
        let from_json = [
            found["status"].as_str().unwrap_or_default(),
            found["name"].as_str().unwrap_or_default(),
            states.as_str(),
        ];
        let from_text: Vec<&str> = line.iter().take(3).map(String::as_str).collect();
        assert_eq!(from_text, from_json, "a text line and its JSON result");
    }
    assert_eq!(
        report["summary"]["models"],
        MODELS.len(),
        "models in the summary"
    );
    assert_eq!(
        report["summary"]["pass"],
        expected::count("pass"),
        "models that pass"
    );
    assert_eq!(
        report["summary"]["fail"],
        expected::count("fail"),
        "models that fail"
    );
}

#[test]
fn r12_one_worker_and_four_workers_give_the_same_results() {
    let one = verify_all(&["--json", "--no-cache", "--jobs", "1"]).json();
    let four = verify_all(&["--json", "--no-cache", "--jobs", "4"]).json();
    assert_eq!(
        without_timings(&four),
        without_timings(&one),
        "the JSON apart from `milliseconds` and `cached`"
    );
    assert_eq!(
        models(&four).len(),
        MODELS.len(),
        "every model is checked in one process"
    );
}

#[test]
fn r12_a_large_model_has_the_same_count_on_every_number_of_workers() {
    for jobs in ["1", "2", "4", "8"] {
        let run = Sandbox::with(&["grid.model.ts"]).earnest(&["verify", "--json", "--jobs", jobs]);
        run.exits(0);
        let report = run.json();
        let grid = model(&report, "Grid");
        assert_eq!(
            grid["states"], 27_000,
            "the states of the grid with {jobs} workers"
        );
        assert_eq!(grid["status"], "pass", "the grid with {jobs} workers");
    }
}

#[test]
fn r17_two_runs_give_the_same_json_in_the_same_order() {
    let first = verify_all(&["--json"]).json();
    let second = verify_all(&["--json"]).json();
    assert_eq!(
        without_timings(&second),
        without_timings(&first),
        "the JSON apart from `milliseconds` and `cached`"
    );
    let names: Vec<&str> = models(&first)
        .iter()
        .filter_map(|found| found["name"].as_str())
        .collect();
    let wanted: Vec<&str> = MODELS.iter().map(|expected| expected.model).collect();
    assert_eq!(
        names, wanted,
        "models come in order of file, then of position in the file"
    );
}

#[test]
fn r17_two_runs_print_the_same_path_for_each_broken_rule() {
    let first = verify_all(&[]);
    let second = verify_all(&["--jobs", "3"]);
    assert!(
        !detail_lines(&first).is_empty(),
        "the failing models print their paths\n{first}"
    );
    assert_eq!(
        detail_lines(&second),
        detail_lines(&first),
        "the detail lines of two runs"
    );
    let models_of = |run: &Run| -> Vec<Vec<String>> {
        run.model_lines()
            .into_iter()
            .map(|line| line.into_iter().take(3).collect())
            .collect()
    };
    assert_eq!(
        models_of(&second),
        models_of(&first),
        "the result lines of two runs"
    );
}

#[test]
fn r17_a_path_among_many_of_the_same_length_is_the_same_on_every_run() {
    let path_of = |jobs: &str| -> Value {
        let run = Sandbox::with(&["dial.model.ts"]).earnest(&["verify", "--json", "--jobs", jobs]);
        run.exits(1);
        rule(model(&run.json(), "Dial"), "near")["path"].clone()
    };
    let first = path_of("1");
    assert_eq!(path_of("1"), first, "the same path on a second run");
    assert_eq!(path_of("4"), first, "the same path with four workers");
}
