use crate::build::{Sketch, apply, below, plus};
use earnest_check::{FindingKind, Limits, Outcome, Status, Verdict, explore};
use earnest_ir::{Expr, Model, Op, Order, SeqOp};
use earnest_value::{Domain, Value};

fn size_of(queue: &Expr) -> Expr {
    Expr::Apply(Op::Seq(SeqOp::Size), vec![queue.clone()])
}

fn queue_of_up_to_three(max: usize) -> Model {
    let mut sketch = Sketch::new();
    let messages = vec![sketch.text("a"), sketch.text("b")];
    let domain = Domain::Seq {
        item: Box::new(Domain::OneOf(messages.clone())),
        max,
    };
    let queue = sketch.field("queue", domain, vec![Value::seq(Vec::new())]);
    let below_three = apply(
        Op::Order(Order::Less),
        &size_of(&queue),
        Expr::Const(Value::Int(3)),
    );
    let sent = apply(Op::Seq(SeqOp::Push), &queue, Expr::Local(0));
    sketch.action_with(
        "send",
        &[("message", messages)],
        below_three.clone(),
        &[(&queue, sent)],
    );
    sketch.rule("neverThree", below_three);
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
    sketch.model()
}

fn with_graph(jobs: usize) -> Limits {
    Limits {
        jobs,
        keep_graph: true,
        ..Limits::default()
    }
}

fn broken_after(outcome: &Outcome, steps: usize) {
    let rule = &outcome.rules[0];
    assert_eq!(rule.verdict, Verdict::Broken, "{outcome:?}");
    let path = rule.path.as_ref().expect("a broken rule has a path");
    assert_eq!(path.steps.len(), steps, "the shortest path");
}

#[test]
fn a_field_with_too_many_values_to_list_is_searched_like_one_with_few() {
    for limits in [Limits::default(), with_graph(1), with_graph(4)] {
        let listed = explore(&queue_of_up_to_three(3), &limits);
        let grown = explore(&queue_of_up_to_three(20), &limits);
        assert_eq!(listed.states, 15, "queues of up to three messages");
        broken_after(&listed, 3);
        assert_eq!(grown, listed, "the same states, verdicts and paths");
    }
}

#[test]
fn a_value_that_leaves_its_domain_is_kept_and_shown_on_its_path() {
    let outcome = explore(&queue_of_up_to_three(2), &Limits::default());
    assert_eq!(outcome.status, Status::Fail);
    assert_eq!(outcome.states, 15, "the queues of three are counted");
    assert_eq!(
        outcome.rules[0].verdict,
        Verdict::Unknown,
        "a state out of range is not judged"
    );
    let finding = &outcome.findings[0];
    assert_eq!(finding.kind, FindingKind::OutOfRange);
    assert_eq!(finding.path.steps.len(), 3, "the shortest path");
}

#[test]
fn a_limit_above_four_billion_states_changes_nothing() {
    let model = cube();
    for jobs in [1, 4] {
        let usual = explore(&model, &with_graph(jobs));
        let huge = explore(
            &model,
            &Limits {
                max_states: usize::MAX,
                ..with_graph(jobs)
            },
        );
        assert_eq!(usual.states, 27_000);
        assert_eq!(huge, usual, "{jobs} jobs");
    }
}
