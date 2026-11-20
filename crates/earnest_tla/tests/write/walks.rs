use crate::build::{Sketch, body_of, int, two};
use earnest_ir::{Expr, Math, Op, Shelf, Sweep};
use earnest_value::{Domain, Value};
use std::collections::BTreeSet;

fn sweeping_in(mut sketch: Sketch, items: Domain, start: Value, reach: Sweep) -> String {
    let n = sketch.number("n", 0, 1, 0);
    let seen = sketch.field("seen", Domain::Set(Box::new(items)), vec![start]);
    let divides = two(
        Op::Equal,
        &two(Op::Math(Math::Remainder), &int(6), n),
        int(0),
    );
    sketch.rule(
        "every",
        Expr::Sweep {
            sweep: reach,
            shelf: Shelf::Set,
            slot: 0,
            source: Box::new(seen),
            body: Box::new(divides),
        },
    );
    sketch.written().module
}

fn sweeping(items: Domain, start: Value, reach: Sweep) -> String {
    sweeping_in(Sketch::new("Walk"), items, start, reach)
}

fn texts(sketch: &mut Sketch, listed: &[&str]) -> (Domain, Value) {
    let domain = sketch.one_of(listed);
    let items: BTreeSet<Value> = sketch.texts(listed).into_iter().collect();
    (domain, Value::set(items))
}

#[test]
fn a_test_over_a_set_of_texts_is_asked_about_in_the_order_of_the_texts() {
    let mut sketch = Sketch::new("Walk");
    let (domain, start) = texts(&mut sketch, &["pear", "apple", "fig"]);
    let module = sweeping_in(sketch, domain, start, Sweep::Some);
    assert!(
        module.contains("(TextRank(before) < TextRank(at))"),
        "the items are compared by the place of their text: {module}"
    );
}

#[test]
fn the_rank_of_a_text_is_its_place_among_every_text_of_the_model_by_code_point() {
    let mut sketch = Sketch::new("Walk");
    let (domain, start) = texts(&mut sketch, &["pear", "apple", "Fig"]);
    let module = sweeping_in(sketch, domain, start, Sweep::Some);
    let rank = module
        .lines()
        .find(|line| line.starts_with("TextRank("))
        .unwrap_or_else(|| panic!("the module defines the rank: {module}"));
    assert!(
        rank.contains("<<\"Fig\", \"apple\", \"n\", \"pear\", \"seen\">>[index] = text"),
        "a capital comes before a small letter: {rank}"
    );
}

#[test]
fn a_set_of_numbers_needs_no_rank_and_a_set_of_texts_asks_for_one() {
    let numbers = sweeping(
        Domain::Int { lo: 0, hi: 3 },
        Value::set(BTreeSet::from([Value::Int(1), Value::Int(2)])),
        Sweep::Some,
    );
    assert!(!numbers.contains("TextRank"), "{numbers}");
    assert!(numbers.contains("(before < at)"), "{numbers}");
}

#[test]
fn a_set_of_optional_numbers_is_walked_with_none_first() {
    let module = sweeping(
        Domain::Optional(Box::new(Domain::Int { lo: 0, hi: 3 })),
        Value::set(BTreeSet::from([Value::Null, Value::Int(2)])),
        Sweep::Every,
    );
    assert!(
        module.contains("((before = <<>>) /\\ (at # <<>>))"),
        "an absent value comes before a present one: {module}"
    );
    assert!(module.contains("(before[1] < at[1])"), "{module}");
}

#[test]
fn a_value_that_can_fault_is_judged_only_where_it_has_one_whatever_order_tlc_takes() {
    let module = sweeping(
        Domain::Bool,
        Value::set(BTreeSet::from([Value::Bool(false), Value::Bool(true)])),
        Sweep::Some,
    );
    let rule = body_of(&module, "Rule_every");
    let [line] = rule.as_slice() else {
        panic!("a rule is one line: {module}");
    };
    assert!(
        line.contains("\\E item \\in seen : (((6 >= 0) /\\ (n >= 1)) /\\"),
        "an item is tested only where the test has a value: {line}"
    );
}
