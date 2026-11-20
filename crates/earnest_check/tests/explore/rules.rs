use crate::build::{Sketch, below, counter, int, is, is_not, steps, texts};
use earnest_check::{Limits, Outcome, Status, Verdict, explore};
use earnest_ir::{Expr, Math, Op};
use earnest_value::Value;
use serde_json::json;

fn jumping_counter() -> Sketch {
    let (mut sketch, n) = counter(3);
    sketch.action("jump", is(&n, 0), &[(&n, int(2))]);
    sketch.end("full", is(&n, 3));
    sketch.rule("neverThree", is_not(&n, 3));
    sketch
}

fn two_broken_rules() -> Sketch {
    let (mut sketch, n) = counter(3);
    sketch.end("full", is(&n, 3));
    sketch.rule("neverOne", is_not(&n, 1));
    sketch.rule("small", below(&n, 4));
    sketch.rule("neverThree", is_not(&n, 3));
    sketch
}

fn only(outcome: &Outcome, rule: &str) -> Vec<String> {
    let found = outcome
        .rules
        .iter()
        .find(|each| each.id == rule)
        .expect("the outcome lists the rule");
    steps(found.path.as_ref().expect("the rule has a path"))
}

#[test]
fn a_broken_rule_fails_the_model_and_every_state_is_still_counted() {
    let outcome = jumping_counter().explored();
    assert_eq!(outcome.status, Status::Fail);
    assert_eq!(outcome.states, 4);
    assert_eq!(outcome.rules[0].verdict, Verdict::Broken);
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
    assert_eq!(outcome.reason, None);
}

#[test]
fn the_path_to_a_broken_rule_is_the_shortest_one() {
    let outcome = jumping_counter().explored();
    let path = outcome.rules[0]
        .path
        .as_ref()
        .expect("a broken rule has a path");
    assert_eq!(texts(&path.start), ["n = 0"]);
    assert_eq!(steps(path), ["jump  n: 0 -> 2", "inc  n: 2 -> 3"]);
}

#[test]
fn each_step_of_a_path_names_its_action_and_the_state_it_leads_to() {
    let outcome = jumping_counter().explored();
    let path = outcome.rules[0]
        .path
        .as_ref()
        .expect("a broken rule has a path");
    let first = &path.steps[0];
    assert_eq!(first.action, "jump");
    assert_eq!(first.label, "does jump");
    assert!(first.input.is_empty());
    assert_eq!(texts(&first.state), ["n = 2"]);
    assert_eq!(texts(&path.steps[1].state), ["n = 3"]);
    assert_eq!(first.changes[0].before.json, json!(0));
    assert_eq!(first.changes[0].after.json, json!(2));
}

#[test]
fn a_rule_broken_in_a_start_state_has_a_path_without_steps() {
    let (mut sketch, n) = counter(1);
    sketch.end("full", is(&n, 1));
    sketch.rule("neverZero", is_not(&n, 0));
    let outcome = sketch.explored();
    let path = outcome.rules[0]
        .path
        .as_ref()
        .expect("a broken rule has a path");
    assert_eq!(texts(&path.start), ["n = 0"]);
    assert!(path.steps.is_empty());
}

#[test]
fn each_broken_rule_has_its_own_path_and_a_rule_that_holds_has_none() {
    let outcome = two_broken_rules().explored();
    let verdicts: Vec<(&str, Verdict)> = outcome
        .rules
        .iter()
        .map(|rule| (rule.id.as_str(), rule.verdict))
        .collect();
    assert_eq!(
        verdicts,
        [
            ("neverOne", Verdict::Broken),
            ("small", Verdict::Holds),
            ("neverThree", Verdict::Broken),
        ]
    );
    assert_eq!(only(&outcome, "neverOne"), ["inc  n: 0 -> 1"]);
    assert_eq!(
        only(&outcome, "neverThree"),
        ["inc  n: 0 -> 1", "inc  n: 1 -> 2", "inc  n: 2 -> 3"]
    );
    assert_eq!(outcome.rules[1].path, None);
    assert_eq!(outcome.status, Status::Fail);
}

#[test]
fn a_step_shows_the_input_the_action_was_taken_with() {
    let mut sketch = Sketch::new();
    let n = sketch.number("n", 0, 2, &[0]);
    let amounts = vec![Value::Int(1), Value::Int(2)];
    sketch.action_with(
        "set",
        &[("amount", amounts)],
        is(&n, 0),
        &[(&n, Expr::Local(0))],
    );
    sketch.end("set", is_not(&n, 0));
    sketch.rule("neverTwo", is_not(&n, 2));
    let outcome = sketch.explored();
    assert_eq!(outcome.states, 3);
    assert_eq!(only(&outcome, "neverTwo"), ["set  amount = 2  n: 0 -> 2"]);
}

#[test]
fn an_action_with_two_inputs_is_tried_with_every_pair() {
    let mut sketch = Sketch::new();
    let n = sketch.number("n", 0, 99, &[0]);
    let tens = vec![Value::Int(10), Value::Int(20), Value::Int(30)];
    let ones = vec![Value::Int(1), Value::Int(2)];
    let sum = Expr::Apply(Op::Math(Math::Add), vec![Expr::Local(0), Expr::Local(1)]);
    sketch.action_with(
        "pick",
        &[("ten", tens), ("one", ones)],
        is(&n, 0),
        &[(&n, sum)],
    );
    sketch.end("picked", is_not(&n, 0));
    sketch.rule("never32", is_not(&n, 32));
    let outcome = sketch.explored();
    assert_eq!(outcome.states, 7);
    assert_eq!(
        only(&outcome, "never32"),
        ["pick  ten = 30  one = 2  n: 0 -> 32"]
    );
}

#[test]
fn checking_only_one_rule_leaves_the_others_out() {
    let limits = Limits {
        only_rule: Some("neverThree".to_owned()),
        ..Limits::default()
    };
    let outcome = explore(&two_broken_rules().model(), &limits);
    assert_eq!(outcome.rules.len(), 1);
    assert_eq!(outcome.rules[0].id, "neverThree");
    assert_eq!(outcome.rules[0].verdict, Verdict::Broken);
    assert_eq!(outcome.status, Status::Fail);
    assert_eq!(outcome.states, 4);
}

#[test]
fn checking_only_a_rule_that_holds_passes_whatever_the_others_would_say() {
    let limits = Limits {
        only_rule: Some("small".to_owned()),
        ..Limits::default()
    };
    let outcome = explore(&two_broken_rules().model(), &limits);
    assert_eq!(outcome.rules.len(), 1);
    assert_eq!(outcome.rules[0].verdict, Verdict::Holds);
    assert_eq!(outcome.status, Status::Pass);
}

#[test]
fn checking_only_a_rule_the_model_does_not_have_checks_no_rule() {
    let limits = Limits {
        only_rule: Some("absent".to_owned()),
        ..Limits::default()
    };
    let outcome = explore(&two_broken_rules().model(), &limits);
    assert!(outcome.rules.is_empty());
    assert_eq!(outcome.status, Status::Pass);
    assert_eq!(outcome.states, 4);
}

#[test]
fn a_rule_that_is_left_out_is_not_evaluated() {
    let (mut sketch, n) = counter(2);
    sketch.end("full", is(&n, 2));
    sketch.rule("faults", Expr::Field(9));
    sketch.rule("small", below(&n, 3));
    let limits = Limits {
        only_rule: Some("small".to_owned()),
        ..Limits::default()
    };
    let outcome = explore(&sketch.model(), &limits);
    assert_eq!(outcome.status, Status::Pass);
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}
