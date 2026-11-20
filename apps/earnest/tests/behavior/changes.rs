use std::process::Command;

use serde_json::{Value, json};

use crate::support::{Run, Sandbox, fields, model, path_under, rule};

const MODEL_FILE: &str = "models/tank.model.ts";
const HEADER: &str = "rule changes since HEAD";

fn git(sandbox: &Sandbox, arguments: &[&str]) {
    let output = Command::new("git")
        .current_dir(sandbox.path())
        .args(["-c", "user.name=Conformance"])
        .args(["-c", "user.email=conformance@earnest.invalid"])
        .args(["-c", "commit.gpgsign=false"])
        .args(["-c", "init.defaultBranch=main"])
        .args(arguments)
        .output()
        .expect("`git` is on the `PATH`");
    assert!(
        output.status.success(),
        "git {arguments:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn changed_tank() -> Sandbox {
    let sandbox = Sandbox::empty();
    sandbox.copy("changes/before/tank.model.ts", MODEL_FILE);
    git(&sandbox, &["init", "--quiet"]);
    git(&sandbox, &["add", MODEL_FILE]);
    git(
        &sandbox,
        &["commit", "--quiet", "--message", "Model the tank"],
    );
    sandbox.copy("changes/after/tank.model.ts", MODEL_FILE);
    sandbox
}

fn change_section(run: &Run) -> Vec<Vec<String>> {
    run.stdout
        .lines()
        .skip_while(|line| line.trim() != HEADER)
        .skip(1)
        .take_while(|line| line.starts_with(char::is_whitespace))
        .map(fields)
        .collect()
}

fn position(section: &[Vec<String>], change: &str, rule: &str) -> usize {
    section
        .iter()
        .position(|line| {
            line.first().is_some_and(|word| word == change)
                && line.get(1).is_some_and(|name| name == rule)
        })
        .unwrap_or_else(|| panic!("no `{change}  {rule}` line among {section:?}"))
}

fn assert_was_and_now(
    section: &[Vec<String>],
    change: &str,
    rule: &str,
    was: [&str; 2],
    now: [&str; 2],
) {
    let head = position(section, change, rule);
    assert_eq!(
        section.get(head + 1).map(Vec::as_slice),
        Some(["was", was[0], was[1]].map(String::from).as_slice()),
        "the old text of {rule}"
    );
    assert_eq!(
        section.get(head + 2).map(Vec::as_slice),
        Some(["now", now[0], now[1]].map(String::from).as_slice()),
        "the new text of {rule}"
    );
}

fn change_of<'a>(report: &'a Value, id: &str) -> &'a Value {
    report["ruleChanges"]
        .as_array()
        .unwrap_or_else(|| panic!("the report has no `ruleChanges` array: {report}"))
        .iter()
        .find(|change| change["id"] == id)
        .unwrap_or_else(|| panic!("no change of rule {id} in {}", report["ruleChanges"]))
}

#[test]
fn r20_the_report_lists_each_rule_the_change_added_removed_or_loosened() {
    let sandbox = changed_tank();
    let run = sandbox.earnest(&["verify", "--base", "HEAD"]);
    run.exits(1);
    let section = change_section(&run);
    assert!(
        !section.is_empty(),
        "a `{HEADER}` section lists the changes\n{run}"
    );

    let added = position(&section, "added", "Tank.fresh");
    assert_eq!(
        section
            .get(added)
            .and_then(|line| line.get(2))
            .map(String::as_str),
        Some("\"The level is never seven\""),
        "an added rule is listed with its sentence\n{run}"
    );
    let removed = position(&section, "removed", "Tank.gone");
    assert_eq!(
        section
            .get(removed)
            .and_then(|line| line.get(2))
            .map(String::as_str),
        Some("\"The level is never five\""),
        "a removed rule is listed with its sentence\n{run}"
    );
    assert_was_and_now(
        &section,
        "loosened",
        "Tank.capped",
        ["\"The level is at most two\"", "s.level <= 2"],
        ["\"The level is at most three\"", "s.level <= 3"],
    );
    assert_was_and_now(
        &section,
        "tightened",
        "Tank.floor",
        ["\"The level is at least zero\"", "s.level >= 0"],
        ["\"The level is at least one\"", "s.level >= 1"],
    );
    position(&section, "changed", "Tank.steady");
    position(&section, "reworded", "Tank.named");
    assert!(
        !run.stdout.contains("Tank.kept"),
        "a rule whose sentence and condition are unchanged is not listed\n{run}"
    );
}

#[test]
fn r20_the_rule_changes_come_before_the_summary_and_after_the_results() {
    let run = changed_tank().earnest(&["verify", "--base", "HEAD"]);
    run.exits(1);
    let lines: Vec<&str> = run.stdout.lines().collect();
    let header = lines
        .iter()
        .position(|line| line.trim() == HEADER)
        .unwrap_or_else(|| panic!("no `{HEADER}` line\n{run}"));
    let summary = run.summary_line();
    let summary_at = lines
        .iter()
        .position(|line| *line == summary)
        .expect("the summary line is one of the lines");
    let model_at = lines
        .iter()
        .position(|line| fields(line).first().is_some_and(|word| word == "fail"))
        .unwrap_or_else(|| panic!("no result line for the tank\n{run}"));
    assert!(
        model_at < header,
        "the results come before the rule changes\n{run}"
    );
    assert!(
        header < summary_at,
        "the rule changes come before the summary\n{run}"
    );

    let floor = path_under(&run.detail("Tank"), &["broken", "floor"]);
    assert!(
        floor.start.contains("level = 0"),
        "the tightened rule breaks at the start\n{run}"
    );
    assert!(
        floor.steps.is_empty(),
        "a rule broken at the start has a path of no steps\n{run}"
    );
}

#[test]
fn r20_the_rule_changes_come_out_as_json() {
    let run = changed_tank().earnest(&["verify", "--base", "HEAD", "--json"]);
    run.exits(1);
    let report = run.json();
    assert_eq!(
        change_of(&report, "capped"),
        &json!({
            "file": MODEL_FILE,
            "model": "Tank",
            "id": "capped",
            "change": "loosened",
            "was": { "says": "The level is at most two", "holds": "s.level <= 2" },
            "now": { "says": "The level is at most three", "holds": "s.level <= 3" },
        }),
        "a rule that a reachable state now passes and used to break is loosened"
    );
    assert_eq!(
        change_of(&report, "floor"),
        &json!({
            "file": MODEL_FILE,
            "model": "Tank",
            "id": "floor",
            "change": "tightened",
            "was": { "says": "The level is at least zero", "holds": "s.level >= 0" },
            "now": { "says": "The level is at least one", "holds": "s.level >= 1" },
        }),
        "a rule that a reachable state now breaks and used to pass is tightened"
    );
    assert_eq!(
        change_of(&report, "steady"),
        &json!({
            "file": MODEL_FILE,
            "model": "Tank",
            "id": "steady",
            "change": "changed",
            "was": { "says": "The level stays within the tank", "holds": "s.level <= 3" },
            "now": { "says": "The level stays within the tank", "holds": "s.level < 4" },
        }),
        "a condition that no reachable state tells from the old one is changed"
    );
    assert_eq!(
        change_of(&report, "named"),
        &json!({
            "file": MODEL_FILE,
            "model": "Tank",
            "id": "named",
            "change": "reworded",
            "was": { "says": "The level is small", "holds": "s.level < 4" },
            "now": { "says": "The level is tiny", "holds": "s.level < 4" },
        }),
        "a rule with only a new sentence is reworded"
    );

    let fresh = change_of(&report, "fresh");
    assert_eq!(fresh["model"], "Tank", "the model of the added rule");
    assert_eq!(fresh["change"], "added", "a rule only the new file has");
    assert_eq!(
        fresh["now"],
        json!({ "says": "The level is never seven", "holds": "s.level !== 7" }),
        "the text of the added rule"
    );
    let gone = change_of(&report, "gone");
    assert_eq!(gone["model"], "Tank", "the model of the removed rule");
    assert_eq!(gone["change"], "removed", "a rule only the old file has");
    assert_eq!(
        gone["was"],
        json!({ "says": "The level is never five", "holds": "s.level !== 5" }),
        "the text of the removed rule"
    );

    let changes = report["ruleChanges"].as_array().map_or(0, Vec::len);
    assert_eq!(
        changes, 6,
        "the unchanged rule is not listed: {}",
        report["ruleChanges"]
    );
    let tank = model(&report, "Tank");
    assert_eq!(
        tank["states"], 4,
        "the working tree is the model that is checked"
    );
    assert_eq!(
        rule(tank, "floor")["verdict"],
        "broken",
        "the tightened rule breaks"
    );
    assert_eq!(
        rule(tank, "capped")["verdict"],
        "holds",
        "the loosened rule holds"
    );
}

#[test]
fn r20_a_model_that_did_not_change_has_no_rule_changes() {
    let sandbox = Sandbox::empty();
    sandbox.copy("changes/before/tank.model.ts", MODEL_FILE);
    git(&sandbox, &["init", "--quiet"]);
    git(&sandbox, &["add", MODEL_FILE]);
    git(
        &sandbox,
        &["commit", "--quiet", "--message", "Model the tank"],
    );
    let run = sandbox.earnest(&["verify", "--base", "HEAD", "--json"]);
    run.exits(1);
    let report = run.json();
    assert_eq!(report["ruleChanges"], json!([]), "no rule changed");
    assert_eq!(
        rule(model(&report, "Tank"), "capped")["verdict"],
        "broken",
        "the committed model is checked as it is"
    );
}

#[test]
fn r20_the_pr_report_lists_the_rule_changes() {
    let run = changed_tank().earnest(&["report", "--base", "HEAD"]);
    for token in [
        "loosened",
        "Tank.capped",
        "\"The level is at most two\"",
        "s.level <= 2",
        "\"The level is at most three\"",
        "s.level <= 3",
        "tightened",
        "Tank.floor",
        "added",
        "Tank.fresh",
        "removed",
        "Tank.gone",
    ] {
        assert!(
            run.stdout.contains(token),
            "the report for a PR body has `{token}` among its rule changes\n{run}"
        );
    }
}
