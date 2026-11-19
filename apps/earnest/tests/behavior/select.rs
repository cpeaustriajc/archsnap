use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use serde_json::Value;

use crate::expected;
use crate::support::{Sandbox, find_path_under, find_rule, model, models, rule};

fn is_not_broken(found: Option<&Value>) -> bool {
    found.is_none_or(|rule| rule["verdict"] != "broken")
}

#[test]
fn r14_one_model_runs_by_name() {
    let sandbox = Sandbox::with(&expected::files());
    let run = sandbox.earnest(&["verify", "--model", "Counter"]);
    run.exits(0);
    let names: Vec<String> = run
        .model_lines()
        .iter()
        .filter_map(|line| line.get(1).cloned())
        .collect();
    assert_eq!(names, ["Counter"], "only the named model is checked\n{run}");
    assert_eq!(run.summary_count("pass"), 1, "models that pass");
    assert_eq!(run.summary_count("fail"), 0, "models that fail");

    let as_json =
        Sandbox::with(&expected::files()).earnest(&["verify", "--model", "Hall", "--json"]);
    as_json.exits(0);
    let report = as_json.json();
    assert_eq!(
        models(&report).len(),
        1,
        "only the named model has a result"
    );
    assert_eq!(
        model(&report, "Hall")["states"],
        3,
        "the states of the named model"
    );
}

#[test]
fn r14_one_model_runs_by_name_from_one_file() {
    let sandbox = Sandbox::with(&expected::files());
    let run = sandbox.earnest(&["verify", "models/switches.model.ts", "--json"]);
    run.exits(0);
    let report = run.json();
    let names: Vec<&str> = models(&report)
        .iter()
        .filter_map(|found| found["name"].as_str())
        .collect();
    assert_eq!(
        names,
        ["Porch", "Hall"],
        "the models of the one file, in file order"
    );
}

#[test]
fn r14_one_rule_of_one_model_runs_by_name() {
    let run = Sandbox::with(&["oven.model.ts"])
        .earnest(&["verify", "--model", "Oven", "--rule", "closed"]);
    run.exits(1);
    let detail = run.detail("Oven");
    let closed = find_path_under(&detail, &["broken", "closed"]);
    assert_eq!(
        closed.map(|path| path.actions()),
        Some(vec!["open".to_owned()]),
        "the named rule is checked\n{run}"
    );
    assert!(
        find_path_under(&detail, &["broken", "safe"]).is_none(),
        "the other rule is not checked\n{run}"
    );

    let as_json = Sandbox::with(&["oven.model.ts"])
        .earnest(&["verify", "--model", "Oven", "--rule", "safe", "--json"]);
    as_json.exits(1);
    let report = as_json.json();
    let oven = model(&report, "Oven");
    assert_eq!(
        rule(oven, "safe")["verdict"],
        "broken",
        "the named rule is checked"
    );
    assert!(
        is_not_broken(find_rule(oven, "closed")),
        "the other rule is not reported broken: {oven}"
    );
}

#[test]
fn r14_a_model_passes_when_its_one_named_rule_holds() {
    let run = Sandbox::with(&["party.model.ts"])
        .earnest(&["verify", "--model", "Party", "--rule", "algebra", "--json"]);
    run.exits(0);
    let report = run.json();
    let party = model(&report, "Party");
    assert_eq!(party["status"], "pass", "the broken rule was not asked for");
    assert_eq!(
        rule(party, "algebra")["verdict"],
        "holds",
        "the named rule holds"
    );
    assert!(
        is_not_broken(find_rule(party, "alone")),
        "the rule that was not asked for is not reported broken: {party}"
    );
}

#[test]
fn r14_a_rule_id_that_one_model_has_runs_without_the_model_name() {
    let run = Sandbox::with(&["counter.model.ts", "oven.model.ts"])
        .earnest(&["verify", "--rule", "even"]);
    run.exits(0);
    assert!(
        run.find_model_line("Counter").is_some(),
        "the model with the rule is checked\n{run}"
    );
    assert!(
        run.find_model_line("Oven").is_none(),
        "a model without the rule is not checked\n{run}"
    );
}

#[test]
fn r14_a_rule_id_that_two_models_share_needs_the_model_name() {
    let fixtures = ["counter.model.ts", "tally.model.ts"];
    Sandbox::with(&fixtures)
        .command()
        .args(["verify", "--rule", "bounded"])
        .assert()
        .code(2)
        .stdout(contains("4 states").not());
    Sandbox::with(&fixtures)
        .earnest(&["verify", "--model", "Counter", "--rule", "bounded"])
        .exits(0);
}

#[test]
fn exit_code_two_for_bad_flags() {
    let bad: [&[&str]; 6] = [
        &["verify", "--frobnicate"],
        &["verify", "--jobs", "many"],
        &["verify", "--max-states", "lots"],
        &["verify", "--time-limit", "soon"],
        &["verify", "--model"],
        &["verify", "--rule"],
    ];
    for arguments in bad {
        Sandbox::with(&["counter.model.ts"])
            .command()
            .args(arguments)
            .assert()
            .code(2)
            .stdout(contains("4 states").not());
    }
}

#[test]
fn exit_code_two_when_no_model_matches() {
    let sandbox = Sandbox::with(&["counter.model.ts", "turnstile.model.ts"]);
    sandbox
        .command()
        .args(["verify", "--model", "Carousel"])
        .assert()
        .code(2)
        .stdout(contains("4 states").not());
    sandbox
        .command()
        .args(["verify", "--model", "Carousel", "--json"])
        .assert()
        .code(2);
}

#[test]
fn exit_code_two_when_no_rule_matches() {
    let sandbox = Sandbox::with(&["counter.model.ts", "turnstile.model.ts"]);
    sandbox
        .command()
        .args(["verify", "--rule", "missing"])
        .assert()
        .code(2);
    sandbox
        .command()
        .args(["verify", "--model", "Counter", "--rule", "locked"])
        .assert()
        .code(2);
}
