use crate::build::{Sketch, below, both, counter, is, is_not, not, plus, steps, texts, yes};
use earnest_check::{Limits, Outcome, Status, Verdict, explore};
use earnest_ir::{Expr, Model};
use std::time::Duration;

fn long_counter() -> Model {
    let (mut sketch, n) = counter(99);
    sketch.end("full", is(&n, 99));
    sketch.rule("neverFive", is_not(&n, 5));
    sketch.rule("neverFifty", is_not(&n, 50));
    sketch.model()
}

fn with_states(max_states: usize) -> Limits {
    Limits {
        max_states,
        ..Limits::default()
    }
}

fn with_jobs(jobs: usize) -> Limits {
    Limits {
        jobs,
        keep_graph: true,
        ..Limits::default()
    }
}

fn wide_level() -> Model {
    let mut sketch = Sketch::new();
    let every_start: Vec<i64> = (0..2000).collect();
    let n = sketch.number("n", 0, 1999, &every_start);
    let flipped = sketch.boolean("flipped", &[false]);
    sketch.action("flip", not(&flipped), &[(&flipped, yes())]);
    sketch.end("flipped", flipped.clone());
    sketch.rule("not1234", not(&both(flipped, is(&n, 1234))));
    sketch.model()
}

fn cube() -> Model {
    let mut sketch = Sketch::new();
    let axes: Vec<Expr> = ["x", "y", "z"]
        .iter()
        .map(|name| sketch.number(name, 0, 29, &[0]))
        .collect();
    for (name, axis) in ["moveX", "moveY", "moveZ"].iter().zip(&axes) {
        sketch.action(name, below(axis, 29), &[(axis, plus(axis, 1))]);
    }
    let at = |x: i64, y: i64, z: i64| both(is(&axes[0], x), both(is(&axes[1], y), is(&axes[2], z)));
    sketch.end("corner", at(29, 29, 29));
    sketch.rule("notThere", not(&at(10, 20, 5)));
    sketch.model()
}

fn unfinished_at(outcome: &Outcome, states: usize, reason: &str) {
    assert_eq!(outcome.status, Status::Unfinished, "{:?}", outcome.reason);
    assert_eq!(outcome.states, states, "the count so far");
    assert_eq!(outcome.reason.as_deref(), Some(reason), "why it stopped");
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn the_default_limits_are_five_million_states_no_time_limit_and_every_rule() {
    let limits = Limits::default();
    assert_eq!(limits.max_states, 5_000_000);
    assert_eq!(limits.time_limit, None);
    assert_eq!(limits.only_rule, None);
    assert!(!limits.keep_graph);
}

#[test]
fn the_state_limit_stops_the_search_with_the_count_so_far() {
    let outcome = explore(&long_counter(), &with_states(3));
    unfinished_at(&outcome, 3, "state limit of 3");
}

#[test]
fn a_rule_not_seen_broken_before_the_limit_is_unknown() {
    let outcome = explore(&long_counter(), &with_states(3));
    assert_eq!(outcome.rules[0].verdict, Verdict::Unknown);
    assert_eq!(outcome.rules[1].verdict, Verdict::Unknown);
    assert_eq!(outcome.rules[0].path, None);
}

#[test]
fn a_rule_seen_broken_before_the_limit_fails_the_model() {
    let outcome = explore(&long_counter(), &with_states(20));
    assert_eq!(outcome.status, Status::Fail);
    assert_eq!(outcome.states, 20);
    assert_eq!(outcome.reason.as_deref(), Some("state limit of 20"));
    assert_eq!(outcome.rules[0].verdict, Verdict::Broken);
    assert_eq!(outcome.rules[1].verdict, Verdict::Unknown);
    let path = outcome.rules[0]
        .path
        .as_ref()
        .expect("a broken rule has a path");
    assert_eq!(path.steps.len(), 5);
}

#[test]
fn a_state_limit_equal_to_the_number_of_states_does_not_stop_the_search() {
    let (mut sketch, n) = counter(3);
    sketch.end("full", is(&n, 3));
    let model = sketch.model();
    assert_eq!(explore(&model, &with_states(4)).status, Status::Pass);
    unfinished_at(&explore(&model, &with_states(3)), 3, "state limit of 3");
}

#[test]
fn the_state_limit_also_bounds_the_start_states() {
    unfinished_at(
        &explore(&wide_level(), &with_states(100)),
        100,
        "state limit of 100",
    );
}

#[test]
fn a_time_limit_that_has_already_passed_stops_the_search_at_the_start_states() {
    let limits = Limits {
        time_limit: Some(Duration::ZERO),
        ..Limits::default()
    };
    let outcome = explore(&long_counter(), &limits);
    unfinished_at(&outcome, 1, "time limit of 0 s");
    assert_eq!(outcome.rules[0].verdict, Verdict::Unknown);
}

#[test]
fn a_time_limit_that_has_already_passed_stops_the_worker_threads_too() {
    let limits = Limits {
        time_limit: Some(Duration::ZERO),
        jobs: 4,
        ..Limits::default()
    };
    unfinished_at(&explore(&wide_level(), &limits), 2000, "time limit of 0 s");
}

#[test]
fn a_time_limit_that_is_not_reached_changes_nothing() {
    let limits = Limits {
        time_limit: Some(Duration::from_hours(1)),
        ..Limits::default()
    };
    let outcome = explore(&long_counter(), &limits);
    assert_eq!(outcome.states, 100);
    assert_eq!(outcome.reason, None);
    assert_eq!(outcome.status, Status::Fail);
}

#[test]
fn a_level_of_two_thousand_states_gives_the_same_outcome_on_one_thread_and_on_four() {
    let model = wide_level();
    let single = explore(&model, &with_jobs(1));
    assert_eq!(single.states, 4000);
    assert_eq!(single.status, Status::Fail);
    let path = single.rules[0]
        .path
        .as_ref()
        .expect("a broken rule has a path");
    assert_eq!(texts(&path.start), ["n = 1234", "flipped = false"]);
    assert_eq!(steps(path), ["flip  flipped: false -> true"]);
    assert_eq!(explore(&model, &with_jobs(4)), single);
    assert_eq!(explore(&model, &with_jobs(3)), single);
    assert_eq!(explore(&model, &with_jobs(64)), single);
}

#[test]
fn a_search_with_many_wide_levels_gives_the_same_outcome_on_one_thread_and_on_four() {
    let model = cube();
    let single = explore(&model, &with_jobs(1));
    assert_eq!(single.states, 27_000);
    assert_eq!(single.status, Status::Fail);
    let path = single.rules[0]
        .path
        .as_ref()
        .expect("a broken rule has a path");
    assert_eq!(path.steps.len(), 35);
    assert!(single.findings.is_empty(), "{:?}", single.findings);
    assert_eq!(explore(&model, &with_jobs(4)), single);
}

#[test]
fn no_worker_threads_counts_as_one() {
    let model = wide_level();
    assert_eq!(
        explore(&model, &with_jobs(0)),
        explore(&model, &with_jobs(1))
    );
}

#[test]
fn the_state_limit_gives_the_same_outcome_on_one_thread_and_on_four() {
    let model = wide_level();
    let limited = |jobs: usize| Limits {
        max_states: 3000,
        ..with_jobs(jobs)
    };
    let single = explore(&model, &limited(1));
    assert_eq!(single.states, 3000);
    assert_eq!(single.reason.as_deref(), Some("state limit of 3000"));
    assert_eq!(explore(&model, &limited(4)), single);
}
