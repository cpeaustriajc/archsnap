use serde_json::{Value, json};

use crate::support::{Run, Sandbox, fields, find_model, model, models};

struct Outside {
    fixture: &'static str,
    line: u64,
    feature: &'static str,
    feature_is_exact: Exactness,
}

enum Exactness {
    Whole,
    Part,
}

const OUTSIDE: &[Outside] = &[
    Outside {
        fixture: "unreadable/network.model.ts",
        line: 13,
        feature: "fetch",
        feature_is_exact: Exactness::Whole,
    },
    Outside {
        fixture: "unreadable/clock.model.ts",
        line: 8,
        feature: "Date.now",
        feature_is_exact: Exactness::Whole,
    },
    Outside {
        fixture: "unreadable/dice.model.ts",
        line: 10,
        feature: "Math.random",
        feature_is_exact: Exactness::Whole,
    },
    Outside {
        fixture: "unreadable/loop.model.ts",
        line: 13,
        feature: "for",
        feature_is_exact: Exactness::Part,
    },
    Outside {
        fixture: "unreadable/variable.model.ts",
        line: 11,
        feature: "let",
        feature_is_exact: Exactness::Part,
    },
    Outside {
        fixture: "unreadable/undefined.model.ts",
        line: 9,
        feature: "surcharge",
        feature_is_exact: Exactness::Part,
    },
];

fn names_the_feature(found: &str, outside: &Outside) -> bool {
    let Some(inner) = found
        .strip_prefix('`')
        .and_then(|rest| rest.strip_suffix('`'))
    else {
        return false;
    };
    match outside.feature_is_exact {
        Exactness::Whole => inner == outside.feature,
        Exactness::Part => inner.contains(outside.feature),
    }
}

fn is_problem_line(line: &str, file: &str, outside: &Outside) -> bool {
    let found = fields(line);
    let place = format!("{file}:{}:", outside.line);
    let has_place = found.first().is_some_and(|field| {
        field
            .strip_prefix(&place)
            .is_some_and(|column| column.parse::<u64>().is_ok_and(|column| column >= 1))
    });
    let has_feature = found
        .get(1)
        .is_some_and(|field| names_the_feature(field, outside));
    let has_sentence = found.get(2).is_some_and(|field| !field.is_empty());
    has_place && has_feature && has_sentence
}

fn assert_text_names_the_problem(run: &Run, file: &str, outside: &Outside) {
    run.exits(1);
    let mut lines = run
        .stdout
        .lines()
        .skip_while(|line| fields(line) != ["unreadable", file]);
    assert!(
        lines.next().is_some(),
        "an `unreadable` line names the file {file}\n{run}"
    );
    let problems: Vec<&str> = lines
        .take_while(|line| line.starts_with(char::is_whitespace))
        .collect();
    assert!(
        problems
            .iter()
            .any(|line| is_problem_line(line, file, outside)),
        "a problem line names {file}:{}, the feature `{}` and a sentence\n{run}",
        outside.line,
        outside.feature
    );
    assert!(
        run.model_lines().is_empty(),
        "no model in an unreadable file is checked\n{run}"
    );
    assert_eq!(
        run.summary_count("unreadable"),
        1,
        "unreadable files in the summary"
    );
}

fn is_problem_entry(entry: &Value, file: &str, outside: &Outside) -> bool {
    entry["file"] == file
        && entry["line"] == outside.line
        && entry["column"].as_u64().is_some_and(|column| column >= 1)
        && entry["feature"]
            .as_str()
            .is_some_and(|feature| names_the_feature(feature, outside))
        && entry["message"]
            .as_str()
            .is_some_and(|message| !message.is_empty())
}

fn assert_json_names_the_problem(run: &Run, file: &str, outside: &Outside) {
    run.exits(1);
    let report = run.json();
    let problems = report["unreadable"]
        .as_array()
        .unwrap_or_else(|| panic!("the report has no `unreadable` array\n{run}"));
    assert!(
        problems
            .iter()
            .any(|entry| is_problem_entry(entry, file, outside)),
        "an `unreadable` entry names {file}:{} and the feature `{}`\n{run}",
        outside.line,
        outside.feature
    );
    assert!(
        problems.iter().all(|entry| entry["file"] == file),
        "every problem is in {file}\n{run}"
    );
    assert_eq!(
        report["models"],
        json!([]),
        "no model in the file is checked"
    );
    assert_eq!(
        report["summary"]["unreadable"], 1,
        "unreadable files in the summary"
    );
    assert_eq!(report["summary"]["pass"], 0, "models that pass");
}

#[test]
fn r07_a_feature_outside_the_library_is_named_with_its_file_and_line() {
    for outside in OUTSIDE {
        let file = format!("models/{}", outside.fixture);
        let run = Sandbox::with(&[outside.fixture]).earnest(&["verify"]);
        assert_text_names_the_problem(&run, &file, outside);
    }
}

#[test]
fn r07_a_feature_outside_the_library_is_named_in_json() {
    for outside in OUTSIDE {
        let file = format!("models/{}", outside.fixture);
        let run = Sandbox::with(&[outside.fixture]).earnest(&["verify", "--json"]);
        assert_json_names_the_problem(&run, &file, outside);
    }
}

#[test]
fn r07_an_unreadable_file_checks_nothing_in_it_and_the_other_models_still_run() {
    let fixtures = ["counter.model.ts", "unreadable/clock.model.ts"];
    let run = Sandbox::with(&fixtures).earnest(&["verify"]);
    run.exits(1);
    assert_eq!(
        run.model_line("Counter").get(..3),
        Some(["pass", "Counter", "4 states"].map(String::from).as_slice()),
        "the readable model is still checked\n{run}"
    );
    assert!(
        run.find_model_line("Stamp").is_none(),
        "the model in the unreadable file is not checked\n{run}"
    );
    assert_eq!(run.summary_count("pass"), 1, "models that pass");
    assert_eq!(run.summary_count("unreadable"), 1, "unreadable files");

    let json_run = Sandbox::with(&fixtures).earnest(&["verify", "--json"]);
    json_run.exits(1);
    let report = json_run.json();
    assert_eq!(
        models(&report).len(),
        1,
        "only the readable model has a result"
    );
    assert_eq!(
        model(&report, "Counter")["status"],
        "pass",
        "the readable model passes"
    );
    assert!(
        find_model(&report, "Stamp").is_none(),
        "the model in the unreadable file has no result"
    );
}

#[test]
fn the_fixtures_have_each_feature_once_on_the_line_the_tests_name() {
    for outside in OUTSIDE {
        let file = format!("models/{}", outside.fixture);
        let source = Sandbox::with(&[outside.fixture]).read(&file);
        let line = usize::try_from(outside.line)
            .ok()
            .and_then(|number| source.lines().nth(number.saturating_sub(1)))
            .unwrap_or_default();
        assert!(
            line.contains(outside.feature),
            "line {} of {} has `{}`, not: {line}",
            outside.line,
            outside.fixture,
            outside.feature
        );
        let uses = source
            .lines()
            .filter(|line| line.contains(outside.feature))
            .count();
        assert_eq!(
            uses, 1,
            "{} has `{}` on one line",
            outside.fixture, outside.feature
        );
    }
}
