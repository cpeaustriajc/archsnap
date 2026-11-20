use crate::build::{Sketch, apply, below, both, counter, is, not, plus, yes};
use earnest_check::{Outcome, Status, Verdict};
use earnest_ir::{Expr, Op, Order, SeqOp};
use earnest_value::{Domain, Value};

fn size_of(queue: &Expr) -> Expr {
    Expr::Apply(Op::Seq(SeqOp::Size), vec![queue.clone()])
}

fn queue_of_two_messages() -> (Sketch, Expr) {
    let mut sketch = Sketch::new();
    let messages = vec![sketch.text("a"), sketch.text("b")];
    let domain = Domain::Seq {
        item: Box::new(Domain::OneOf(messages.clone())),
        max: 2,
    };
    let queue = sketch.field("queue", domain, vec![Value::seq(Vec::new())]);
    let has_room = apply(
        Op::Order(Order::Less),
        &size_of(&queue),
        Expr::Const(Value::Int(2)),
    );
    let sent = apply(Op::Seq(SeqOp::Push), &queue, Expr::Local(0));
    sketch.action_with(
        "send",
        &[("message", messages)],
        has_room,
        &[(&queue, sent)],
    );
    (sketch, queue)
}

fn passes_with(outcome: &Outcome, states: usize) {
    assert_eq!(outcome.status, Status::Pass, "{outcome:?}");
    assert_eq!(outcome.states, states, "the number of distinct states");
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
    assert_eq!(outcome.reason, None, "the search visited every state");
}

#[test]
fn a_counter_from_zero_to_three_has_four_states() {
    let (mut sketch, n) = counter(3);
    sketch.end("full", is(&n, 3));
    sketch.rule("inRange", below(&n, 4));
    let outcome = sketch.explored();
    passes_with(&outcome, 4);
    assert_eq!(outcome.rules.len(), 1);
    assert_eq!(outcome.rules[0].id, "inRange");
    assert_eq!(outcome.rules[0].says, "inRange always");
    assert_eq!(outcome.rules[0].verdict, Verdict::Holds);
    assert_eq!(outcome.rules[0].path, None);
    assert_eq!(outcome.graph, None);
}

#[test]
fn two_independent_booleans_have_four_states() {
    let mut sketch = Sketch::new();
    let left = sketch.boolean("left", &[false]);
    let right = sketch.boolean("right", &[false]);
    sketch.action("flipLeft", yes(), &[(&left, not(&left))]);
    sketch.action("flipRight", yes(), &[(&right, not(&right))]);
    passes_with(&sketch.explored(), 4);
}

#[test]
fn every_combination_of_start_values_is_a_start_state() {
    let mut sketch = Sketch::new();
    sketch.boolean("left", &[false, true]);
    sketch.number("level", 0, 2, &[0, 1, 2]);
    sketch.end("anywhere", yes());
    passes_with(&sketch.explored(), 6);
}

#[test]
fn a_queue_of_capacity_two_over_two_messages_has_seven_contents() {
    let (mut sketch, queue) = queue_of_two_messages();
    sketch.end(
        "full",
        apply(Op::Equal, &size_of(&queue), Expr::Const(Value::Int(2))),
    );
    passes_with(&sketch.explored(), 7);
}

#[test]
fn receiving_from_the_queue_reaches_no_new_contents() {
    let (mut sketch, queue) = queue_of_two_messages();
    let not_empty = apply(
        Op::Order(Order::Greater),
        &size_of(&queue),
        Expr::Const(Value::Int(0)),
    );
    let received = Expr::Apply(Op::Seq(SeqOp::Shift), vec![queue.clone()]);
    sketch.action("receive", not_empty, &[(&queue, received)]);
    passes_with(&sketch.explored(), 7);
}

#[test]
fn states_reached_twice_are_counted_once() {
    let mut sketch = Sketch::new();
    let x = sketch.number("x", 0, 2, &[0]);
    let y = sketch.number("y", 0, 2, &[0]);
    sketch.action("right", below(&x, 2), &[(&x, plus(&x, 1))]);
    sketch.action("up", below(&y, 2), &[(&y, plus(&y, 1))]);
    sketch.end("corner", both(is(&x, 2), is(&y, 2)));
    passes_with(&sketch.explored(), 9);
}

#[test]
fn a_model_without_fields_has_one_state() {
    let mut sketch = Sketch::new();
    sketch.end("done", yes());
    passes_with(&sketch.explored(), 1);
}

#[test]
fn a_field_without_start_values_leaves_nothing_to_search() {
    let mut sketch = Sketch::new();
    sketch.number("n", 0, 3, &[]);
    passes_with(&sketch.explored(), 0);
}
