use crate::build::{Sketch, apply, below, counter, int, is, plus, steps, texts, yes};
use earnest_check::{Finding, FindingKind, Outcome, Status, Verdict};
use earnest_ir::{Expr, Math, Op, Order, SeqOp};
use earnest_value::{Domain, IntType, Value};

const STUCK: &str = "no action can happen here, and the model does not mark it as an end";

fn the_finding(outcome: &Outcome) -> &Finding {
    assert_eq!(outcome.findings.len(), 1, "{:?}", outcome.findings);
    assert_eq!(outcome.status, Status::Fail, "a finding fails the model");
    &outcome.findings[0]
}

fn optional_number(sketch: &mut Sketch) -> Expr {
    let domain = Domain::Optional(Box::new(Domain::Int { lo: 0, hi: 1 }));
    sketch.field("picked", domain, vec![Value::Null])
}

fn unwrapped(optional: &Expr) -> Expr {
    Expr::Apply(Op::Unwrap, vec![optional.clone()])
}

#[test]
fn a_state_without_an_action_or_an_end_is_stuck() {
    let (sketch, _) = counter(3);
    let outcome = sketch.explored();
    assert_eq!(outcome.states, 4);
    let finding = the_finding(&outcome);
    assert_eq!(finding.kind, FindingKind::Stuck);
    assert_eq!(finding.message, STUCK);
    assert_eq!(texts(&finding.path.start), ["n = 0"]);
    assert_eq!(
        steps(&finding.path),
        ["inc  n: 0 -> 1", "inc  n: 1 -> 2", "inc  n: 2 -> 3"]
    );
}

#[test]
fn a_state_without_an_action_that_an_end_marks_is_not_stuck() {
    let (mut sketch, n) = counter(3);
    sketch.end("full", is(&n, 3));
    let outcome = sketch.explored();
    assert_eq!(outcome.status, Status::Pass);
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn an_end_that_marks_another_state_does_not_help_the_stuck_one() {
    let (mut sketch, n) = counter(3);
    sketch.end("halfway", is(&n, 2));
    let outcome = sketch.explored();
    assert_eq!(the_finding(&outcome).kind, FindingKind::Stuck);
    assert_eq!(steps(&the_finding(&outcome).path).len(), 3);
}

#[test]
fn one_of_several_ends_is_enough() {
    let (mut sketch, n) = counter(3);
    sketch.end("halfway", is(&n, 2));
    sketch.end("full", is(&n, 3));
    assert_eq!(sketch.explored().status, Status::Pass);
}

#[test]
fn a_stuck_start_state_has_a_path_without_steps() {
    let mut sketch = Sketch::new();
    sketch.number("n", 0, 3, &[2]);
    let outcome = sketch.explored();
    let finding = the_finding(&outcome);
    assert_eq!(finding.kind, FindingKind::Stuck);
    assert_eq!(texts(&finding.path.start), ["n = 2"]);
    assert!(finding.path.steps.is_empty());
}

#[test]
fn only_the_first_stuck_state_is_reported() {
    let mut sketch = Sketch::new();
    let n = sketch.number("n", 0, 2, &[0]);
    sketch.action("left", is(&n, 0), &[(&n, int(1))]);
    sketch.action("right", is(&n, 0), &[(&n, int(2))]);
    let outcome = sketch.explored();
    assert_eq!(outcome.states, 3);
    assert_eq!(steps(&the_finding(&outcome).path), ["left  n: 0 -> 1"]);
}

#[test]
fn counting_past_a_range_is_out_of_range_with_the_path_to_it() {
    let mut sketch = Sketch::new();
    let n = sketch.number("n", 0, 3, &[0]);
    sketch.action("inc", yes(), &[(&n, plus(&n, 1))]);
    let outcome = sketch.explored();
    assert_eq!(outcome.states, 5);
    let finding = the_finding(&outcome);
    assert_eq!(finding.kind, FindingKind::OutOfRange);
    assert_eq!(
        finding.message,
        "the field `n` leaves the values it declares"
    );
    assert_eq!(
        steps(&finding.path),
        [
            "inc  n: 0 -> 1",
            "inc  n: 1 -> 2",
            "inc  n: 2 -> 3",
            "inc  n: 3 -> 4"
        ]
    );
}

#[test]
fn a_state_out_of_range_is_not_searched_further_and_its_rules_are_not_judged() {
    let mut sketch = Sketch::new();
    let n = sketch.number("n", 0, 3, &[0]);
    sketch.action("inc", yes(), &[(&n, plus(&n, 1))]);
    sketch.rule("small", below(&n, 4));
    let outcome = sketch.explored();
    assert_eq!(outcome.states, 5);
    assert_eq!(outcome.rules[0].verdict, Verdict::Holds);
    assert_eq!(the_finding(&outcome).kind, FindingKind::OutOfRange);
}

#[test]
fn sending_to_a_full_queue_is_out_of_range() {
    let mut sketch = Sketch::new();
    let domain = Domain::Seq {
        item: Box::new(Domain::Bool),
        max: 1,
    };
    let queue = sketch.field("queue", domain, vec![Value::seq(Vec::new())]);
    let sent = apply(Op::Seq(SeqOp::Push), &queue, yes());
    sketch.action("send", yes(), &[(&queue, sent)]);
    let outcome = sketch.explored();
    assert_eq!(outcome.states, 3);
    let finding = the_finding(&outcome);
    assert_eq!(finding.kind, FindingKind::OutOfRange);
    assert_eq!(
        steps(&finding.path),
        [
            "send  queue: [] -> [true]",
            "send  queue: [true] -> [true, true]"
        ]
    );
}

#[test]
fn a_start_value_outside_its_field_is_out_of_range_at_the_start() {
    let mut sketch = Sketch::new();
    sketch.number("n", 0, 3, &[7]);
    let outcome = sketch.explored();
    let finding = the_finding(&outcome);
    assert_eq!(finding.kind, FindingKind::OutOfRange);
    assert!(finding.path.steps.is_empty());
}

#[test]
fn a_rule_that_reads_null_is_a_fault() {
    let mut sketch = Sketch::new();
    let picked = optional_number(&mut sketch);
    sketch.end("anywhere", yes());
    let positive = apply(Op::Order(Order::Greater), &unwrapped(&picked), int(-1));
    sketch.rule("positive", positive);
    let outcome = sketch.explored();
    let finding = the_finding(&outcome);
    assert_eq!(finding.kind, FindingKind::Fault);
    assert_eq!(
        finding.message,
        "the rule `positive` reads a value that is null in this state"
    );
    assert_eq!(texts(&finding.path.start), ["picked = null"]);
    assert!(finding.path.steps.is_empty());
}

#[test]
fn a_rule_that_can_not_be_evaluated_in_a_state_neither_holds_nor_is_broken() {
    let mut sketch = Sketch::new();
    let picked = optional_number(&mut sketch);
    sketch.end("anywhere", yes());
    let positive = apply(Op::Order(Order::Greater), &unwrapped(&picked), int(-1));
    sketch.rule("positive", positive);
    sketch.rule("fine", yes());
    let outcome = sketch.explored();
    assert_eq!(outcome.rules[0].verdict, Verdict::Unknown);
    assert_eq!(outcome.rules[0].path, None);
    assert_eq!(outcome.rules[1].verdict, Verdict::Holds);
    assert_eq!(outcome.reason, None);
}

#[test]
fn a_guard_that_reads_null_is_a_fault_and_the_state_is_not_called_stuck() {
    let mut sketch = Sketch::new();
    let picked = optional_number(&mut sketch);
    let guard = apply(Op::Equal, &unwrapped(&picked), int(1));
    sketch.action("go", guard, &[(&picked, int(0))]);
    let outcome = sketch.explored();
    let finding = the_finding(&outcome);
    assert_eq!(finding.kind, FindingKind::Fault);
    assert_eq!(
        finding.message,
        "the `when` of the action `go` reads a value that is null in this state"
    );
}

#[test]
fn a_result_too_large_to_hold_is_a_fault() {
    let mut sketch = Sketch::new();
    let n = sketch.number("n", 0, i64::MAX, &[i64::MAX]);
    sketch.action("inc", yes(), &[(&n, plus(&n, 1))]);
    let outcome = sketch.explored();
    assert_eq!(outcome.states, 1);
    let finding = the_finding(&outcome);
    assert_eq!(finding.kind, FindingKind::Fault);
    assert_eq!(
        finding.message,
        "the `then` of the action `inc` computes a number too large to hold in this state"
    );
}

#[test]
fn a_remainder_of_a_negative_number_is_a_fault_with_the_path_to_it() {
    let mut sketch = Sketch::new();
    let n = sketch.number("n", -1, 1, &[1]);
    sketch.action("dec", is(&n, 1), &[(&n, plus(&n, -1))]);
    sketch.action("drop", is(&n, 0), &[(&n, plus(&n, -1))]);
    sketch.end("bottom", is(&n, -1));
    let remainder = apply(Op::Math(Math::Remainder), &n, int(2));
    sketch.rule("even", apply(Op::Order(Order::Less), &remainder, int(2)));
    let outcome = sketch.explored();
    assert_eq!(outcome.states, 3);
    let finding = the_finding(&outcome);
    assert_eq!(finding.kind, FindingKind::Fault);
    assert_eq!(
        finding.message,
        "the rule `even` takes a remainder with a negative number or a divisor below 1 in this \
         state"
    );
    assert_eq!(steps(&finding.path), ["dec  n: 1 -> 0", "drop  n: 0 -> -1"]);
}

#[test]
fn a_number_outside_its_integer_type_is_a_fault_that_names_the_type() {
    let (mut sketch, n) = counter(3);
    sketch.end("full", is(&n, 3));
    let one_below = Expr::Apply(Op::Fit(IntType::U8), vec![plus(&n, -1)]);
    sketch.rule(
        "below",
        apply(Op::Order(Order::LessOrEqual), &one_below, int(2)),
    );
    let outcome = sketch.explored();
    assert_eq!(outcome.states, 4, "the count goes 0, 1, 2, 3");
    let finding = the_finding(&outcome);
    assert_eq!(finding.kind, FindingKind::Fault);
    assert_eq!(
        finding.message,
        "the rule `below` computes a number outside `u8` in this state"
    );
    assert!(finding.path.steps.is_empty(), "0 - 1 in the start state");
    assert_eq!(
        outcome.rules[0].verdict,
        Verdict::Unknown,
        "the rule holds at 1, 2 and 3 and has no value at 0"
    );
}

#[test]
fn an_end_that_can_not_be_evaluated_is_a_fault_and_the_state_is_not_called_stuck() {
    let mut sketch = Sketch::new();
    let picked = optional_number(&mut sketch);
    sketch.end("done", apply(Op::Equal, &unwrapped(&picked), int(1)));
    let outcome = sketch.explored();
    let finding = the_finding(&outcome);
    assert_eq!(finding.kind, FindingKind::Fault);
    assert_eq!(
        finding.message,
        "the end `done` reads a value that is null in this state"
    );
}

#[test]
fn a_stuck_state_a_value_out_of_range_and_a_fault_are_each_reported_once() {
    let mut sketch = Sketch::new();
    let n = sketch.number("n", 0, 2, &[0]);
    let picked = optional_number(&mut sketch);
    sketch.action("left", is(&n, 0), &[(&n, int(1))]);
    sketch.action("right", is(&n, 0), &[(&n, int(3))]);
    let reads_null = apply(Op::Order(Order::Less), &unwrapped(&picked), int(5));
    sketch.rule(
        "smallPick",
        Expr::Or(Box::new(is(&n, 0)), Box::new(reads_null)),
    );
    let outcome = sketch.explored();
    assert_eq!(outcome.status, Status::Fail);
    assert_eq!(outcome.states, 3);
    let kinds: Vec<FindingKind> = outcome
        .findings
        .iter()
        .map(|finding| finding.kind)
        .collect();
    assert_eq!(
        kinds,
        [
            FindingKind::Stuck,
            FindingKind::OutOfRange,
            FindingKind::Fault
        ]
    );
    assert_eq!(steps(&outcome.findings[0].path), ["left  n: 0 -> 1"]);
    assert_eq!(steps(&outcome.findings[1].path), ["right  n: 0 -> 3"]);
    assert_eq!(steps(&outcome.findings[2].path), ["left  n: 0 -> 1"]);
}
