use std::time::Duration;

use crate::support::{Run, Sandbox, model, rule};

const TIME_LIMIT_GUARD: Duration = Duration::from_mins(2);

fn first_three(line: &[String]) -> Vec<&str> {
    line.iter().take(3).map(String::as_str).collect()
}

fn has_a_time(line: &[String]) -> bool {
    line.iter().any(|field| {
        field
            .strip_suffix(" ms")
            .is_some_and(|number| number.parse::<u64>().is_ok())
    })
}

#[test]
fn r08_a_state_limit_reports_the_model_as_not_fully_checked_with_the_states_so_far() {
    let sandbox = Sandbox::with(&["counter.model.ts", "grid.model.ts"]);
    let run = sandbox.earnest(&["verify", "--max-states", "1000"]);
    run.exits(1);
    let grid = run.model_line("Grid");
    assert_eq!(
        first_three(&grid),
        ["unfinished", "Grid", "1000 states so far"],
        "the line of the model that reached the limit\n{run}"
    );
    assert!(
        has_a_time(&grid),
        "an unfinished line still has its time\n{run}"
    );
    assert!(
        grid.iter().any(|field| field == "(state limit of 1000)"),
        "the line names the limit that stopped the model\n{run}"
    );
    assert_eq!(
        first_three(&run.model_line("Counter")),
        ["pass", "Counter", "4 states"],
        "a model below the limit is fully checked\n{run}"
    );
    assert_eq!(run.summary_count("pass"), 1, "models that pass");
    assert_eq!(run.summary_count("unfinished"), 1, "unfinished models");
}

#[test]
fn r08_a_state_limit_in_json_leaves_unbroken_rules_unknown() {
    let sandbox = Sandbox::with(&["counter.model.ts", "grid.model.ts"]);
    let run = sandbox.earnest(&["verify", "--max-states", "1000", "--json"]);
    run.exits(1);
    let report = run.json();
    let grid = model(&report, "Grid");
    assert_eq!(
        grid["status"], "unfinished",
        "the status of the model at the limit"
    );
    assert_eq!(grid["states"], 1000, "the states found so far");
    assert_eq!(
        rule(grid, "inside")["verdict"],
        "unknown",
        "a rule not seen broken"
    );
    assert!(
        grid["reason"].is_string(),
        "an unfinished model says why: {grid}"
    );
    assert_eq!(
        model(&report, "Counter")["status"],
        "pass",
        "a model below the limit"
    );
    assert_eq!(
        report["summary"]["unfinished"], 1,
        "unfinished models in the summary"
    );
}

#[test]
fn r08_a_limit_above_the_state_count_changes_nothing() {
    let sandbox = Sandbox::with(&["counter.model.ts"]);
    let run = sandbox.earnest(&["verify", "--max-states", "4"]);
    run.exits(0);
    assert_eq!(
        first_three(&run.model_line("Counter")),
        ["pass", "Counter", "4 states"],
        "a model with exactly as many states as the limit is fully checked\n{run}"
    );
}

fn verify_with_a_time_limit(arguments: &[&str]) -> Run {
    let sandbox = Sandbox::with(&["counter.model.ts", "limits/slow.model.ts"]);
    Run::of(
        sandbox
            .command()
            .args(["verify", "--time-limit", "1"])
            .args(arguments)
            .timeout(TIME_LIMIT_GUARD),
    )
}

#[test]
fn r19_a_time_limit_stops_one_model_and_the_others_still_finish() {
    let run = verify_with_a_time_limit(&[]);
    run.exits(1);
    let slow = run.model_line("Slow");
    assert_eq!(
        slow.first().map(String::as_str),
        Some("unfinished"),
        "the slow model\n{run}"
    );
    assert!(
        slow.get(2).is_some_and(|field| {
            field
                .strip_suffix(" states so far")
                .is_some_and(|number| number.parse::<u64>().is_ok())
        }),
        "the slow model reports its states so far\n{run}"
    );
    assert_eq!(
        first_three(&run.model_line("Counter")),
        ["pass", "Counter", "4 states"],
        "the small model still finishes\n{run}"
    );
    assert_eq!(run.summary_count("pass"), 1, "models that pass");
    assert_eq!(run.summary_count("unfinished"), 1, "unfinished models");
}

#[test]
fn r19_a_time_limit_in_json_reports_the_states_so_far() {
    let run = verify_with_a_time_limit(&["--json"]);
    run.exits(1);
    let report = run.json();
    let slow = model(&report, "Slow");
    assert_eq!(slow["status"], "unfinished", "the status of the slow model");
    assert!(
        slow["states"].as_u64().is_some_and(|states| states < 1000),
        "the slow model stopped before its last state: {slow}"
    );
    assert_eq!(
        rule(slow, "heavy")["verdict"],
        "unknown",
        "a rule not seen broken"
    );
    assert!(
        slow["reason"].is_string(),
        "an unfinished model says why: {slow}"
    );
    assert_eq!(
        model(&report, "Counter")["status"],
        "pass",
        "the small model"
    );
    assert_eq!(
        model(&report, "Counter")["states"],
        4,
        "the states of the small model"
    );
}
