use serde_json::{Value, json};

use crate::expected::{self, Expected, MODELS, Shortest};
use crate::support::{
    Run, Sandbox, actions, fields, finding, model, models, named, path_under, rule, steps,
};

const STUCK_SENTENCE: &str = "no action can happen here, and the model does not mark it as an end";

fn verify_text(fixtures: &[&str]) -> Run {
    Sandbox::with(fixtures).earnest(&["verify"])
}

fn verify_json(fixtures: &[&str]) -> Value {
    Sandbox::with(fixtures)
        .earnest(&["verify", "--json"])
        .json()
}

fn assert_shortest(path: &Value, shortest: &Shortest, what: &str) {
    match shortest {
        Shortest::Steps(expected) => {
            assert_eq!(actions(path), *expected, "the shortest path to {what}");
        }
        Shortest::Length(expected) => {
            assert_eq!(
                steps(path).len(),
                *expected,
                "the length of the shortest path to {what}"
            );
        }
    }
}

fn value_of<'a>(state: &'a Value, field: &str) -> &'a Value {
    state
        .as_array()
        .unwrap_or_else(|| panic!("a state is an array of fields: {state}"))
        .iter()
        .find(|entry| entry["name"] == field)
        .map_or(&Value::Null, |entry| &entry["value"])
}

fn assert_steps_agree_with_their_changes(path: &Value, what: &str) {
    let mut before = &path["start"];
    for step in steps(path) {
        let after = &step["state"];
        let entries = after
            .as_array()
            .unwrap_or_else(|| panic!("a step of {what} has no `state` array: {step}"));
        let expected: Vec<Value> = entries
            .iter()
            .filter(|entry| {
                let name = entry["name"].as_str().unwrap_or_default();
                value_of(before, name) != &entry["value"]
            })
            .map(|entry| {
                let name = entry["name"].as_str().unwrap_or_default();
                json!({
                    "field": name,
                    "before": value_of(before, name),
                    "after": entry["value"],
                })
            })
            .collect();
        assert_eq!(
            step["changes"],
            Value::Array(expected),
            "the changes of a step of {what} are the fields that differ between its states"
        );
        before = after;
    }
}

fn assert_model_matches(found: &Value, expected: &Expected) {
    let name = expected.model;
    assert_eq!(found["status"], expected.status, "the status of {name}");
    assert_eq!(
        found["states"], expected.states,
        "the distinct states of {name}"
    );
    for id in expected.holds {
        assert_eq!(rule(found, id)["verdict"], "holds", "rule {name}.{id}");
    }
    for id in expected.unknown {
        assert_eq!(rule(found, id)["verdict"], "unknown", "rule {name}.{id}");
    }
    for (id, _) in expected.broken {
        assert_eq!(rule(found, id)["verdict"], "broken", "rule {name}.{id}");
    }
    let rule_count = found["rules"].as_array().map_or(0, Vec::len);
    assert_eq!(
        rule_count,
        expected.holds.len() + expected.unknown.len() + expected.broken.len(),
        "the number of rules of {name}"
    );
    let kinds: Vec<&str> = found["findings"]
        .as_array()
        .unwrap_or_else(|| panic!("model {name} has no `findings` array"))
        .iter()
        .filter_map(|finding| finding["kind"].as_str())
        .collect();
    let expected_kinds: Vec<&str> = expected.findings.iter().map(|(kind, _)| *kind).collect();
    assert_eq!(kinds, expected_kinds, "the findings of {name}");
}

#[test]
fn r03_every_model_reports_its_state_count_and_each_rule_verdict() {
    let run = Sandbox::with(&expected::files()).earnest(&["verify", "--json"]);
    run.exits(1);
    let report = run.json();
    for expected in MODELS {
        assert_model_matches(model(&report, expected.model), expected);
    }
    assert_eq!(
        models(&report).len(),
        MODELS.len(),
        "one result for each model in the folder"
    );
}

#[test]
fn r03_text_has_one_line_per_model_in_file_then_position_order() {
    let run = verify_text(&expected::files());
    run.exits(1);
    let lines = run.model_lines();
    let found: Vec<(&str, &str, &str)> = lines
        .iter()
        .map(|line| {
            (
                line.first().map_or("", String::as_str),
                line.get(1).map_or("", String::as_str),
                line.get(2).map_or("", String::as_str),
            )
        })
        .collect();
    let state_counts: Vec<String> = MODELS
        .iter()
        .map(|expected| format!("{} states", expected.states))
        .collect();
    let wanted: Vec<(&str, &str, &str)> = MODELS
        .iter()
        .zip(&state_counts)
        .map(|(expected, states)| (expected.status, expected.model, states.as_str()))
        .collect();
    assert_eq!(
        found, wanted,
        "status, model and state count of each line, in order"
    );
    assert_eq!(
        run.summary_count("pass"),
        expected::count("pass"),
        "models that pass"
    );
    assert_eq!(
        run.summary_count("fail"),
        expected::count("fail"),
        "models that fail"
    );
    assert_eq!(run.summary_count("unfinished"), 0, "unfinished models");
    assert_eq!(run.summary_count("unreadable"), 0, "unreadable files");
    assert_eq!(
        run.summary_total(),
        expected::count("pass") + expected::count("fail"),
        "the model count of the summary line"
    );
}

#[test]
fn r03_only_model_files_outside_skipped_folders_are_checked() {
    let sandbox = Sandbox::with(&["counter.model.ts"]);
    sandbox.copy("heater.model.ts", "models/heater.model.mts");
    for folder in ["node_modules/pkg", "target", "dist", "build", ".hidden"] {
        sandbox.copy(
            "turnstile.model.ts",
            &format!("{folder}/turnstile.model.ts"),
        );
        sandbox.copy("turnstile.rs", &format!("{folder}/turnstile.rs"));
    }
    sandbox.write(
        "models/prices.ts",
        "export const quote = () => fetch(\"https://prices.invalid/quote\");\n",
    );
    sandbox.write("models/plain.rs", "pub fn answer() -> u8 {\n    42\n}\n");
    let run = sandbox.earnest(&["verify", "--json"]);
    run.exits(0);
    let report = run.json();
    let names: Vec<&str> = models(&report)
        .iter()
        .filter_map(|found| found["name"].as_str())
        .collect();
    assert_eq!(names, ["Counter", "Heater"], "the models that were checked");
    assert_eq!(
        report["unreadable"],
        json!([]),
        "a file that is not a model is not read"
    );
}

#[test]
fn r04_shortest_path_to_a_broken_rule() {
    let run = verify_text(&["turnstile.model.ts"]);
    run.exits(1);
    let detail = run.detail("Turnstile");
    let head = [
        "broken",
        "locked",
        "\"The gate stays locked until the visitor pays\"",
    ];
    assert!(
        detail.iter().any(|line| fields(line) == head),
        "a `broken` line names the rule and its sentence\n{run}"
    );
    let path = path_under(&detail, &["broken", "locked"]);
    assert!(
        path.start.contains("gate = \"Locked\"") && path.start.contains("paid = false"),
        "the start line has every field: {}",
        path.start
    );
    assert_eq!(
        path.actions(),
        ["push"],
        "one numbered step per action\n{run}"
    );
    let step = path.steps.first().expect("the path has one step");
    assert!(
        step.contains("gate: \"Locked\" -> \"Open\""),
        "the step shows the value that changed: {step}"
    );
    assert!(
        !step.contains("paid"),
        "the step leaves out the value that did not change: {step}"
    );
}

#[test]
fn r04_shortest_path_to_a_broken_rule_as_json() {
    let report = verify_json(&["turnstile.model.ts"]);
    let locked = rule(model(&report, "Turnstile"), "locked");
    assert_eq!(
        locked["verdict"], "broken",
        "the verdict of the broken rule"
    );
    assert_eq!(
        locked["path"],
        json!({
            "start": [
                { "name": "gate", "value": "Locked" },
                { "name": "paid", "value": false },
            ],
            "steps": [{
                "action": "push",
                "label": "the visitor pushes",
                "input": [],
                "changes": [{ "field": "gate", "before": "Locked", "after": "Open" }],
                "state": [
                    { "name": "gate", "value": "Open" },
                    { "name": "paid", "value": false },
                ],
            }],
        }),
        "the path to the first state that breaks the rule"
    );
}

#[test]
fn r04_a_run_with_several_broken_rules_gives_a_path_for_each() {
    let run = verify_text(&["oven.model.ts"]);
    run.exits(1);
    let detail = run.detail("Oven");
    let safe = path_under(&detail, &["broken", "safe"]);
    assert_eq!(
        safe.actions(),
        ["start", "open"],
        "the path to `safe`\n{run}"
    );
    assert!(
        safe.steps
            .first()
            .is_some_and(|step| step.contains("heat: \"Off\" -> \"On\"")),
        "the first step to `safe` turns the heat on\n{run}"
    );
    assert!(
        safe.steps
            .get(1)
            .is_some_and(|step| step.contains("door: \"Closed\" -> \"Open\"")),
        "the second step to `safe` opens the door\n{run}"
    );
    let closed = path_under(&detail, &["broken", "closed"]);
    assert_eq!(closed.actions(), ["open"], "the path to `closed`\n{run}");

    let report = verify_json(&["oven.model.ts"]);
    let oven = model(&report, "Oven");
    assert_eq!(
        actions(&rule(oven, "safe")["path"]),
        ["start", "open"],
        "`safe` as JSON"
    );
    assert_eq!(
        actions(&rule(oven, "closed")["path"]),
        ["open"],
        "`closed` as JSON"
    );
}

#[test]
fn r04_a_step_shows_its_input_and_the_values_that_changed() {
    let run = verify_text(&["mailbox.model.ts", "stock.model.ts", "trail.model.ts"]);
    run.exits(1);

    let bills = path_under(&run.detail("Mailbox"), &["broken", "bills"]);
    assert_eq!(
        bills.actions(),
        ["deliver", "deliver"],
        "the path to `bills`\n{run}"
    );
    let first = bills.steps.first().expect("the path has a first step");
    assert!(
        first.contains("letter = \"Bill\"") && first.contains("inbox: [] -> [\"Bill\"]"),
        "a step shows its input and a queue before and after: {first}"
    );
    let second = bills.steps.get(1).expect("the path has a second step");
    assert!(
        second.contains("inbox: [\"Bill\"] -> [\"Bill\", \"Bill\"]"),
        "a queue prints its letters in order: {second}"
    );

    let single = path_under(&run.detail("Stock"), &["broken", "single"]);
    let restock = single.steps.get(1).expect("the path has a second step");
    assert!(
        restock.contains("item = \"Pen\"")
            && restock.contains("shelf: {\"Pen\" -> 1} -> {\"Pen\" -> 2}"),
        "a map prints as keys and values: {restock}"
    );

    let ordered = path_under(&run.detail("Trail"), &["broken", "ordered"]);
    let north = ordered.steps.get(1).expect("the path has a second step");
    assert!(
        north.contains("stop = \"North\"")
            && north.contains("visited: [\"South\"] -> [\"South\", \"North\"]"),
        "a sequence prints its stops in order: {north}"
    );
}

#[test]
fn r04_a_step_carries_its_input_and_changes_as_json() {
    let report = verify_json(&["mailbox.model.ts", "party.model.ts", "stock.model.ts"]);

    let bills = &rule(model(&report, "Mailbox"), "bills")["path"];
    let first = steps(bills).first().expect("the path has a first step");
    assert_eq!(first["action"], "deliver", "the id of the action");
    assert_eq!(
        first["label"], "the postman delivers a letter",
        "the sentence of the action"
    );
    assert_eq!(
        first["input"],
        named(&[("letter", json!("Bill"))]),
        "the input of the step"
    );
    assert_eq!(
        first["changes"],
        json!([{ "field": "inbox", "before": [], "after": ["Bill"] }]),
        "a queue is an array"
    );
    assert_eq!(
        first["state"],
        named(&[("inbox", json!(["Bill"]))]),
        "the state after the step"
    );

    let single = &rule(model(&report, "Stock"), "single")["path"];
    let restock = steps(single).get(1).expect("the path has a second step");
    assert_eq!(
        restock["changes"],
        json!([{ "field": "shelf", "before": [["Pen", 1]], "after": [["Pen", 2]] }]),
        "a map is an array of key and value pairs"
    );

    let alone = &rule(model(&report, "Party"), "alone")["path"];
    assert_eq!(
        alone["start"],
        named(&[("inside", json!([]))]),
        "a set starts empty"
    );
    let ann = steps(alone).first().expect("the path has a first step");
    assert_eq!(
        ann["input"],
        named(&[("guest", json!("Ann"))]),
        "Ann arrives first"
    );
    assert_eq!(
        ann["state"],
        named(&[("inside", json!(["Ann"]))]),
        "a set is an array"
    );
    let both = steps(alone).get(1).expect("the path has a second step");
    let mut inside: Vec<&str> = both["state"][0]["value"]
        .as_array()
        .expect("the set is an array")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    inside.sort_unstable();
    assert_eq!(inside, ["Ann", "Bob"], "both guests are inside at the end");
}

#[test]
fn r04_the_path_starts_at_the_start_state_nearest_to_the_broken_one() {
    let run = verify_text(&["lamp.model.ts"]);
    run.exits(1);
    let dim = path_under(&run.detail("Lamp"), &["broken", "dim"]);
    assert!(
        dim.start.contains("power = true") && dim.start.contains("level = 0"),
        "the path starts with the lamp on: {}",
        dim.start
    );
    assert_eq!(
        dim.actions(),
        ["brighten", "brighten", "toggle"],
        "the path to `dim`\n{run}"
    );
    assert!(
        dim.steps
            .get(2)
            .is_some_and(|step| step.contains("power: true -> false")),
        "the last step turns the lamp off\n{run}"
    );

    let report = verify_json(&["lamp.model.ts"]);
    let path = &rule(model(&report, "Lamp"), "dim")["path"];
    assert_eq!(
        path["start"],
        named(&[("power", json!(true)), ("level", json!(0))]),
        "the start state of the path"
    );
    assert_eq!(
        model(&report, "Lamp")["states"],
        6,
        "both start states are searched"
    );
}

#[test]
fn r04_every_path_in_the_table_is_the_shortest() {
    let report = verify_json(&expected::files());
    for expected in MODELS {
        let found = model(&report, expected.model);
        for (id, shortest) in expected.broken {
            let what = format!("{}.{id}", expected.model);
            let path = &rule(found, id)["path"];
            assert_shortest(path, shortest, &what);
            assert_steps_agree_with_their_changes(path, &what);
        }
        for (kind, shortest) in expected.findings {
            let what = format!("the {kind} state of {}", expected.model);
            let path = &finding(found, kind)["path"];
            assert_shortest(path, shortest, &what);
            assert_steps_agree_with_their_changes(path, &what);
        }
    }
}

#[test]
fn r05_a_stuck_state_is_named_with_the_path_to_it() {
    let run = verify_text(&["download.model.ts"]);
    run.exits(1);
    assert_eq!(
        run.model_line("Download").get(..3),
        Some(
            ["fail", "Download", "4 states"]
                .map(String::from)
                .as_slice()
        ),
        "a stuck model fails\n{run}"
    );
    let detail = run.detail("Download");
    assert!(
        detail
            .iter()
            .any(|line| fields(line) == ["stuck", STUCK_SENTENCE]),
        "a `stuck` line says what is wrong\n{run}"
    );
    let path = path_under(&detail, &["stuck"]);
    assert!(
        path.start.contains("phase = \"Idle\"") && path.start.contains("saved = false"),
        "the start line has every field: {}",
        path.start
    );
    assert_eq!(
        path.actions(),
        ["start", "fail"],
        "the path to the stuck state\n{run}"
    );
    assert!(
        path.steps
            .get(1)
            .is_some_and(|step| step.contains("phase: \"Loading\" -> \"Failed\"")),
        "the last step is the one into the stuck state\n{run}"
    );
}

#[test]
fn r05_a_stuck_state_is_a_finding_in_json_and_a_marked_end_is_not() {
    let report = verify_json(&["counter.model.ts", "download.model.ts"]);
    let download = model(&report, "Download");
    assert_eq!(download["status"], "fail", "a stuck model fails");
    assert_eq!(
        rule(download, "whole")["verdict"],
        "holds",
        "its rule still holds"
    );
    let stuck = finding(download, "stuck");
    assert!(
        stuck["message"].is_string(),
        "a finding has a message: {stuck}"
    );
    assert_eq!(
        actions(&stuck["path"]),
        ["start", "fail"],
        "the path to the stuck state"
    );
    let last = steps(&stuck["path"]).last().expect("the path has steps");
    assert_eq!(
        last["state"],
        named(&[("phase", json!("Failed")), ("saved", json!(false))]),
        "the stuck state is the failed download, not the finished one"
    );
    let counter = model(&report, "Counter");
    assert_eq!(
        counter["status"], "pass",
        "a model that stops at a marked end passes"
    );
    assert_eq!(
        counter["findings"],
        json!([]),
        "a marked end is not a finding"
    );
}

#[test]
fn readme_an_out_of_range_state_is_reported_with_the_path_to_it() {
    let run = verify_text(&["tally.model.ts"]);
    run.exits(1);
    let detail = run.detail("Tally");
    let path = path_under(&detail, &["out of range"]);
    assert_eq!(
        path.actions(),
        ["add", "add", "add", "add"],
        "the path out of range\n{run}"
    );
    assert!(
        path.steps
            .last()
            .is_some_and(|step| step.contains("count: 3 -> 4")),
        "the last step gives the field a value outside its range\n{run}"
    );

    let report = verify_json(&["tally.model.ts"]);
    let tally = model(&report, "Tally");
    assert_eq!(tally["status"], "fail", "an out of range model fails");
    assert_eq!(
        tally["states"], 5,
        "the out of range state is one of the states found"
    );
    assert_eq!(
        rule(tally, "bounded")["verdict"],
        "unknown",
        "its rule is not judged in the state out of range, so it is not known to hold"
    );
    let last = steps(&finding(tally, "out-of-range")["path"])
        .last()
        .expect("the path has steps");
    assert_eq!(
        last["state"],
        named(&[("count", json!(4))]),
        "the state out of range"
    );
}

#[test]
fn readme_a_read_of_a_field_of_null_is_a_fault_with_the_path_to_it() {
    let run = verify_text(&["basket.model.ts"]);
    run.exits(1);
    let path = path_under(&run.detail("Basket"), &["fault"]);
    assert!(
        path.start.contains("chosen = {qty: 1}") && path.start.contains("total = 0"),
        "a record prints as names and values: {}",
        path.start
    );
    assert_eq!(
        path.actions(),
        ["clear"],
        "the path to the state with the fault\n{run}"
    );
    assert!(
        path.steps
            .first()
            .is_some_and(|step| step.contains("chosen: {qty: 1} -> null")),
        "the step clears the record\n{run}"
    );

    let report = verify_json(&["basket.model.ts"]);
    let basket = model(&report, "Basket");
    assert_eq!(basket["status"], "fail", "a model with a fault fails");
    assert_eq!(basket["states"], 4, "the search goes on past the fault");
    let clear = steps(&finding(basket, "fault")["path"])
        .first()
        .expect("the path has a step");
    assert_eq!(
        clear["changes"],
        json!([{ "field": "chosen", "before": { "qty": 1 }, "after": null }]),
        "a record is an object and an empty optional is null"
    );
}

#[test]
fn readme_a_remainder_of_a_negative_number_is_a_fault() {
    let run = verify_text(&["meter.model.ts"]);
    run.exits(1);
    let path = path_under(&run.detail("Meter"), &["fault"]);
    assert_eq!(
        path.actions(),
        ["drain", "drain"],
        "the path to the negative level\n{run}"
    );
    assert!(
        path.steps
            .get(1)
            .is_some_and(|step| step.contains("level: 0 -> -1")),
        "the second step takes the level below zero\n{run}"
    );
    let report = verify_json(&["meter.model.ts"]);
    assert_eq!(
        model(&report, "Meter")["states"],
        3,
        "the states of the meter"
    );
}
