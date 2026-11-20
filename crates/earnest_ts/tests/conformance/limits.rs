use crate::search;
use earnest_check::{Limits, Status, Verdict};
use std::time::Duration;

#[test]
fn the_grid_is_unfinished_at_a_thousand_states() {
    let limits = Limits {
        max_states: 1000,
        ..Limits::default()
    };
    let outcome = search("grid.model.ts", "Grid", &limits);
    assert_eq!(outcome.status, Status::Unfinished, "{:?}", outcome.reason);
    assert_eq!(outcome.states, 1000, "states so far");
    let verdicts: Vec<(&str, Verdict)> = outcome
        .rules
        .iter()
        .map(|rule| (rule.id.as_str(), rule.verdict))
        .collect();
    assert_eq!(
        verdicts,
        [("inside", Verdict::Unknown)],
        "the rule is unknown"
    );
}

#[test]
fn the_slow_model_is_unfinished_after_a_second() {
    let limits = Limits {
        time_limit: Some(Duration::from_secs(1)),
        ..Limits::default()
    };
    let outcome = search("limits/slow.model.ts", "Slow", &limits);
    assert_eq!(outcome.status, Status::Unfinished, "{:?}", outcome.reason);
    assert!(
        outcome.states < 1000,
        "fewer than the 1000 states of a full run: {}",
        outcome.states
    );
    let verdicts: Vec<(&str, Verdict)> = outcome
        .rules
        .iter()
        .map(|rule| (rule.id.as_str(), rule.verdict))
        .collect();
    assert_eq!(
        verdicts,
        [("heavy", Verdict::Unknown)],
        "the rule is unknown"
    );
}
