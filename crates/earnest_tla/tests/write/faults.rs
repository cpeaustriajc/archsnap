use crate::build::{bind, body_of, both, choose, either, int, null, one, two, yes};
use crate::ops::{Stage, stage};
use earnest_ir::{Expr, Math, Op, Order, Shelf, Sweep};

fn asked_first(mut stage: Stage, guard: Expr) -> String {
    stage.sketch.action("act", guard, &[]);
    let module = stage.sketch.written().module;
    body_of(&module, "Action_act")
        .first()
        .cloned()
        .unwrap_or_else(|| panic!("the action has a guard line: {module}"))
}

fn present(stage: &Stage) -> Expr {
    one(Op::Unwrap, &stage.maybe)
}

fn above_zero(value: &Expr) -> Expr {
    two(Op::Order(Order::Greater), value, int(0))
}

fn halves(value: &Expr) -> Expr {
    two(Op::Math(Math::Remainder), value, int(2))
}

#[test]
fn a_guard_that_reads_an_optional_first_asks_that_it_is_present() {
    let stage = stage();
    let guard = above_zero(&present(&stage));
    assert_eq!(
        asked_first(stage, guard),
        "IF InRange /\\ (maybe # <<>>) /\\ (maybe[1] > 0)"
    );
}

#[test]
fn a_guard_that_takes_a_remainder_first_asks_that_it_has_one() {
    let stage = stage();
    let guard = two(Op::Equal, &halves(&stage.n), int(0));
    assert_eq!(
        asked_first(stage, guard),
        "IF InRange /\\ ((n >= 0) /\\ (2 >= 1)) /\\ (Remainder(n, 2) = 0)"
    );
}

#[test]
fn an_operation_asks_for_its_arguments_before_itself() {
    let first = stage();
    let sum = two(Op::Math(Math::Add), &present(&first), present(&first));
    assert_eq!(
        asked_first(first, above_zero(&sum)),
        "IF InRange /\\ ((maybe # <<>>) /\\ (maybe # <<>>)) /\\ ((maybe[1] + maybe[1]) > 0)"
    );
    let second = stage();
    let guard = two(Op::Equal, &halves(&present(&second)), int(0));
    assert_eq!(
        asked_first(second, guard),
        "IF InRange /\\ ((maybe # <<>>) /\\ ((maybe[1] >= 0) /\\ (2 >= 1))) /\\ \
         (Remainder(maybe[1], 2) = 0)"
    );
}

#[test]
fn the_second_side_of_an_or_is_asked_about_only_when_the_first_is_false() {
    let stage = stage();
    let guard = either(
        two(Op::Equal, &stage.maybe, null()),
        above_zero(&present(&stage)),
    );
    assert_eq!(
        asked_first(stage, guard),
        "IF InRange /\\ ((maybe = <<>>) \\/ (maybe # <<>>)) /\\ \
         ((maybe = <<>>) \\/ (maybe[1] > 0))"
    );
}

#[test]
fn the_second_side_of_an_and_is_asked_about_only_when_the_first_is_true() {
    let stage = stage();
    let guard = both(stage.flag.clone(), above_zero(&present(&stage)));
    assert_eq!(
        asked_first(stage, guard),
        "IF InRange /\\ ((~flag) \\/ (maybe # <<>>)) /\\ (flag /\\ (maybe[1] > 0))"
    );
}

#[test]
fn both_sides_are_asked_about_in_order_when_both_can_fault() {
    let stage = stage();
    let guard = both(
        above_zero(&present(&stage)),
        two(Op::Equal, &halves(&stage.n), int(0)),
    );
    assert_eq!(
        asked_first(stage, guard),
        "IF InRange /\\ ((maybe # <<>>) /\\ ((~(maybe[1] > 0)) \\/ ((n >= 0) /\\ (2 >= 1)))) /\\ \
         ((maybe[1] > 0) /\\ (Remainder(n, 2) = 0))"
    );
}

#[test]
fn only_the_chosen_branch_of_a_choice_is_asked_about() {
    let stage = stage();
    let chosen = choose(stage.flag.clone(), present(&stage), int(0));
    assert_eq!(
        asked_first(stage, above_zero(&chosen)),
        "IF InRange /\\ (IF flag THEN (maybe # <<>>) ELSE TRUE) /\\ \
         ((IF flag THEN maybe[1] ELSE 0) > 0)"
    );
}

#[test]
fn a_choice_with_a_test_that_can_fault_asks_about_the_test_first() {
    let stage = stage();
    let chosen = choose(above_zero(&present(&stage)), int(1), halves(&stage.n));
    assert_eq!(
        asked_first(stage, above_zero(&chosen)),
        "IF InRange /\\ ((maybe # <<>>) /\\ \
         (IF (maybe[1] > 0) THEN TRUE ELSE ((n >= 0) /\\ (2 >= 1)))) /\\ \
         ((IF (maybe[1] > 0) THEN 1 ELSE Remainder(n, 2)) > 0)"
    );
}

#[test]
fn a_bound_value_is_asked_about_even_when_the_body_cannot_fault() {
    let stage = stage();
    let guard = bind(0, present(&stage), above_zero(&Expr::Local(0)));
    assert_eq!(
        asked_first(stage, guard),
        "IF InRange /\\ (maybe # <<>>) /\\ (LET local == maybe[1] IN (local > 0))"
    );
}

#[test]
fn a_body_that_can_fault_is_asked_about_with_the_value_bound() {
    let stage = stage();
    let guard = bind(
        0,
        stage.n.clone(),
        two(Op::Equal, &halves(&Expr::Local(0)), int(0)),
    );
    assert_eq!(
        asked_first(stage, guard),
        "IF InRange /\\ (LET local == n IN ((local >= 0) /\\ (2 >= 1))) /\\ \
         (LET local == n IN (Remainder(local, 2) = 0))"
    );
}

#[test]
fn a_test_over_a_collection_is_asked_about_for_every_item() {
    let stage = stage();
    let divides = two(
        Op::Equal,
        &two(Op::Math(Math::Remainder), &Expr::Local(0), stage.n.clone()),
        int(0),
    );
    let guard = Expr::Sweep {
        sweep: Sweep::Some,
        shelf: Shelf::Set,
        slot: 0,
        source: Box::new(stage.seen.clone()),
        body: Box::new(divides),
    };
    assert_eq!(
        asked_first(stage, guard),
        "IF InRange /\\ (\\A item \\in seen : ((item >= 0) /\\ (n >= 1))) /\\ \
         (\\E item \\in seen : (Remainder(item, n) = 0))"
    );
}

#[test]
fn the_parts_of_a_built_value_are_asked_about() {
    let mut stage = stage();
    let x = stage.sketch.sym("x");
    let record = Expr::Record(vec![(x, present(&stage))]);
    let read = Expr::Get(Box::new(record), x);
    let listed = Expr::SeqOf(vec![read, halves(&stage.n)]);
    let guard = two(Op::Equal, &listed, Expr::SeqOf(Vec::new()));
    assert_eq!(
        asked_first(stage, guard),
        "IF InRange /\\ ((maybe # <<>>) /\\ ((n >= 0) /\\ (2 >= 1))) /\\ \
         (<<[x |-> maybe[1]].x, Remainder(n, 2)>> = <<>>)"
    );
}

#[test]
fn an_update_that_can_fault_is_asked_about_after_the_guard() {
    let mut stage = stage();
    let n = stage.n.clone();
    let maybe = stage.maybe.clone();
    let present = present(&stage);
    stage.sketch.action(
        "take",
        above_zero(&n),
        &[(&n, present), (&maybe, one(Op::SomeOf, &halves(&n)))],
    );
    let module = stage.sketch.written().module;
    assert_eq!(
        body_of(&module, "Action_take"),
        [
            "IF InRange /\\ (n > 0) /\\ (maybe # <<>>) /\\ ((n >= 0) /\\ (2 >= 1))",
            "THEN /\\ n' = maybe[1]",
            "     /\\ maybe' = <<Remainder(n, 2)>>",
            "     /\\ UNCHANGED <<flag, seen, book, loose, line, gaps>>",
            "ELSE FALSE",
        ]
    );
}

#[test]
fn an_action_that_can_always_happen_still_asks_about_its_update() {
    let mut stage = stage();
    let n = stage.n.clone();
    let present = present(&stage);
    stage.sketch.action("take", yes(), &[(&n, present)]);
    let module = stage.sketch.written().module;
    assert_eq!(
        body_of(&module, "Action_take").first().map(String::as_str),
        Some("IF InRange /\\ (maybe # <<>>)")
    );
}

#[test]
fn an_end_that_cannot_be_read_does_not_stop_the_checker() {
    let mut stage = stage();
    let when = two(Op::Equal, &present(&stage), int(0));
    stage.sketch.end("drained", when);
    let module = stage.sketch.written().module;
    assert_eq!(
        body_of(&module, "End_drained"),
        ["((~(maybe # <<>>)) \\/ (maybe[1] = 0))"]
    );
}

#[test]
fn a_rule_that_cannot_be_read_is_left_to_stop_the_checker() {
    let stage = stage();
    let holds = above_zero(&present(&stage));
    assert_eq!(stage.sketch.shown(holds), "(maybe[1] > 0)");
}
