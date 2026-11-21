//! Every operation of the expression language gives the value its definition says, and every
//! fault is reported as a fault. Each case is evaluated by the reference walk and by the
//! prepared [`Code`], which must agree on the value, the fault and the effects on the frame.

use earnest_ir::{
    Action, Code, Expr, Fault, Frame, Input, MapOp, Math, Op, Order, SeqOp, SetOp, Shelf, Sweep,
    eval, truth,
};
use earnest_value::{Domain, IntType, MOST_SAFE_INTEGER, Sym, Symbols, Value};
use proptest::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

const SLOTS: usize = 4;

struct Ran<T> {
    answer: Result<T, Fault>,
    beyond_32_bits: bool,
    locals: Vec<Result<Value, Fault>>,
}

fn ran<T>(
    state: &[Value],
    locals: &[Value],
    evaluate: impl FnOnce(&mut Frame<'_>) -> Result<T, Fault>,
) -> Ran<T> {
    let mut frame = Frame::new(state, SLOTS);
    for (slot, value) in locals.iter().enumerate() {
        frame.put(slot, value.clone());
    }
    let answer = evaluate(&mut frame);
    Ran {
        answer,
        beyond_32_bits: frame.computed_beyond_32_bits(),
        locals: (0..SLOTS)
            .map(|slot| eval(&Expr::Local(slot), &mut frame))
            .collect(),
    }
}

fn agreed<T: PartialEq + std::fmt::Debug>(reference: Ran<T>, prepared: &Ran<T>) -> Ran<T> {
    assert_eq!(reference.answer, prepared.answer, "the same value or fault");
    assert_eq!(
        reference.beyond_32_bits, prepared.beyond_32_bits,
        "the same mark for numbers beyond 32 bits"
    );
    assert_eq!(
        reference.locals, prepared.locals,
        "the same slots afterwards"
    );
    reference
}

fn both_values(state: &[Value], locals: &[Value], expr: &Expr) -> Ran<Value> {
    let code = Code::new(expr.clone());
    agreed(
        ran(state, locals, |frame| eval(expr, frame)),
        &ran(state, locals, |frame| code.eval(frame)),
    )
}

fn both_truths(state: &[Value], locals: &[Value], expr: &Expr) -> Ran<bool> {
    let code = Code::new(expr.clone());
    agreed(
        ran(state, locals, |frame| truth(expr, frame)),
        &ran(state, locals, |frame| code.truth(frame)),
    )
}

fn value_in(state: &[Value], expr: &Expr) -> Result<Value, Fault> {
    both_values(state, &[], expr).answer
}

fn condition(expr: &Expr) -> Result<bool, Fault> {
    both_truths(&[], &[], expr).answer
}

fn value_of(expr: &Expr) -> Result<Value, Fault> {
    value_in(&[], expr)
}

fn int(number: i64) -> Expr {
    Expr::Const(Value::Int(number))
}

fn yes() -> Expr {
    Expr::Const(Value::Bool(true))
}

fn no() -> Expr {
    Expr::Const(Value::Bool(false))
}

fn null() -> Expr {
    Expr::Const(Value::Null)
}

fn apply(op: Op, arguments: &[Expr]) -> Expr {
    Expr::Apply(op, arguments.to_vec())
}

fn ints(numbers: &[i64]) -> Vec<Value> {
    numbers.iter().copied().map(Value::Int).collect()
}

fn set_value(numbers: &[i64]) -> Value {
    Value::set(ints(numbers).into_iter().collect())
}

fn seq_value(numbers: &[i64]) -> Value {
    Value::seq(ints(numbers))
}

fn map_value(pairs: &[(i64, i64)]) -> Value {
    Value::map(
        pairs
            .iter()
            .map(|(key, value)| (Value::Int(*key), Value::Int(*value)))
            .collect(),
    )
}

fn set(numbers: &[i64]) -> Expr {
    Expr::Const(set_value(numbers))
}

fn seq(numbers: &[i64]) -> Expr {
    Expr::Const(seq_value(numbers))
}

fn map(pairs: &[(i64, i64)]) -> Expr {
    Expr::Const(map_value(pairs))
}

fn faulting() -> Expr {
    apply(Op::Unwrap, &[null()])
}

fn math(op: Math, left: i64, right: i64) -> Result<Value, Fault> {
    value_of(&apply(Op::Math(op), &[int(left), int(right)]))
}

fn order(op: Order, left: i64, right: i64) -> Result<Value, Fault> {
    value_of(&apply(Op::Order(op), &[int(left), int(right)]))
}

fn on_set(op: SetOp, arguments: &[Expr]) -> Result<Value, Fault> {
    value_of(&apply(Op::Set(op), arguments))
}

fn on_map(op: MapOp, arguments: &[Expr]) -> Result<Value, Fault> {
    value_of(&apply(Op::Map(op), arguments))
}

fn on_seq(op: SeqOp, arguments: &[Expr]) -> Result<Value, Fault> {
    value_of(&apply(Op::Seq(op), arguments))
}

fn item_above(limit: i64) -> Expr {
    apply(Op::Order(Order::Greater), &[Expr::Local(0), int(limit)])
}

fn sweep(sweep: Sweep, shelf: Shelf, source: Expr, body: Expr) -> Result<Value, Fault> {
    value_of(&Expr::Sweep {
        sweep,
        shelf,
        slot: 0,
        source: Box::new(source),
        body: Box::new(body),
    })
}

const YES: Result<Value, Fault> = Ok(Value::Bool(true));
const NO: Result<Value, Fault> = Ok(Value::Bool(false));

#[test]
fn a_constant_is_its_value() {
    assert_eq!(value_of(&int(7)), Ok(Value::Int(7)));
    assert_eq!(value_of(&null()), Ok(Value::Null));
}

#[test]
fn a_field_is_read_from_the_state_by_position() {
    let state = ints(&[10, 20]);
    assert_eq!(value_in(&state, &Expr::Field(1)), Ok(Value::Int(20)));
    assert_eq!(value_in(&state, &Expr::Field(2)), Err(Fault::WrongKind));
}

#[test]
fn a_local_is_read_from_its_slot() {
    let mut frame = Frame::new(&[], 2);
    frame.put(1, Value::Int(5));
    assert_eq!(eval(&Expr::Local(1), &mut frame), Ok(Value::Int(5)));
    assert_eq!(eval(&Expr::Local(0), &mut frame), Ok(Value::Null));
}

#[test]
fn a_slot_the_frame_does_not_have_is_never_written_and_can_not_be_read() {
    let mut frame = Frame::new(&[], 1);
    frame.put(3, Value::Int(5));
    assert_eq!(eval(&Expr::Local(3), &mut frame), Err(Fault::WrongKind));
}

#[test]
fn not_gives_the_opposite_boolean() {
    assert_eq!(value_of(&apply(Op::Not, &[yes()])), NO);
    assert_eq!(value_of(&apply(Op::Not, &[no()])), YES);
    assert_eq!(value_of(&apply(Op::Not, &[int(1)])), Err(Fault::WrongKind));
    assert_eq!(value_of(&apply(Op::Not, &[null()])), Err(Fault::NullRead));
}

#[test]
fn negate_flips_the_sign_and_overflows_on_the_smallest_number() {
    assert_eq!(value_of(&apply(Op::Negate, &[int(5)])), Ok(Value::Int(-5)));
    assert_eq!(value_of(&apply(Op::Negate, &[int(-5)])), Ok(Value::Int(5)));
    assert_eq!(
        value_of(&apply(Op::Negate, &[int(i64::MIN)])),
        Err(Fault::Overflow)
    );
}

#[test]
fn a_present_optional_is_the_plain_value() {
    assert_eq!(value_of(&apply(Op::SomeOf, &[int(3)])), Ok(Value::Int(3)));
    assert_eq!(value_of(&apply(Op::Unwrap, &[int(3)])), Ok(Value::Int(3)));
}

#[test]
fn reading_through_null_is_a_fault() {
    assert_eq!(
        value_of(&apply(Op::Unwrap, &[null()])),
        Err(Fault::NullRead)
    );
}

#[test]
fn equal_and_differ_compare_whole_values() {
    assert_eq!(value_of(&apply(Op::Equal, &[int(1), int(1)])), YES);
    assert_eq!(value_of(&apply(Op::Equal, &[int(1), int(2)])), NO);
    assert_eq!(value_of(&apply(Op::Equal, &[int(1), yes()])), NO);
    assert_eq!(value_of(&apply(Op::Equal, &[null(), null()])), YES);
    assert_eq!(value_of(&apply(Op::Equal, &[null(), int(0)])), NO);
    assert_eq!(value_of(&apply(Op::Differ, &[int(1), int(2)])), YES);
    assert_eq!(value_of(&apply(Op::Differ, &[int(1), int(1)])), NO);
    assert_eq!(
        value_of(&apply(Op::Equal, &[set(&[1, 2]), set(&[2, 1])])),
        YES
    );
    assert_eq!(
        value_of(&apply(Op::Equal, &[seq(&[1, 2]), seq(&[2, 1])])),
        NO
    );
}

#[test]
fn an_operation_missing_an_argument_is_malformed() {
    assert_eq!(
        value_of(&apply(Op::Equal, &[int(1)])),
        Err(Fault::WrongKind)
    );
    assert_eq!(value_of(&apply(Op::Not, &[])), Err(Fault::WrongKind));
    assert_eq!(
        value_of(&apply(Op::Math(Math::Add), &[int(1)])),
        Err(Fault::WrongKind)
    );
    assert_eq!(on_set(SetOp::Has, &[set(&[1])]), Err(Fault::WrongKind));
    assert_eq!(
        on_map(MapOp::Set, &[map(&[]), int(1)]),
        Err(Fault::WrongKind)
    );
    assert_eq!(on_seq(SeqOp::Push, &[seq(&[])]), Err(Fault::WrongKind));
}

#[test]
fn numbers_compare_in_all_four_orders() {
    assert_eq!(order(Order::Less, 1, 2), YES);
    assert_eq!(order(Order::Less, 2, 2), NO);
    assert_eq!(order(Order::LessOrEqual, 2, 2), YES);
    assert_eq!(order(Order::LessOrEqual, 3, 2), NO);
    assert_eq!(order(Order::Greater, 3, 2), YES);
    assert_eq!(order(Order::Greater, 2, 2), NO);
    assert_eq!(order(Order::GreaterOrEqual, 2, 2), YES);
    assert_eq!(order(Order::GreaterOrEqual, 1, 2), NO);
    assert_eq!(order(Order::Less, -3, -2), YES);
}

#[test]
fn comparing_null_with_a_number_is_a_null_read() {
    let below = apply(Op::Order(Order::Less), &[null(), int(1)]);
    assert_eq!(value_of(&below), Err(Fault::NullRead));
    let above = apply(Op::Order(Order::Greater), &[int(1), null()]);
    assert_eq!(value_of(&above), Err(Fault::NullRead));
    let wrong = apply(Op::Order(Order::Less), &[yes(), int(1)]);
    assert_eq!(value_of(&wrong), Err(Fault::WrongKind));
}

#[test]
fn numbers_add_subtract_and_multiply() {
    assert_eq!(math(Math::Add, 2, 3), Ok(Value::Int(5)));
    assert_eq!(math(Math::Subtract, 2, 3), Ok(Value::Int(-1)));
    assert_eq!(math(Math::Multiply, -4, 3), Ok(Value::Int(-12)));
}

#[test]
fn a_number_too_large_to_hold_is_an_overflow() {
    assert_eq!(math(Math::Add, i64::MAX, 1), Err(Fault::Overflow));
    assert_eq!(math(Math::Subtract, i64::MIN, 1), Err(Fault::Overflow));
    assert_eq!(math(Math::Multiply, i64::MAX, 2), Err(Fault::Overflow));
    assert_eq!(math(Math::Add, i64::MAX, 0), Ok(Value::Int(i64::MAX)));
}

#[test]
fn a_remainder_of_numbers_that_are_not_negative_is_what_is_left_over() {
    assert_eq!(math(Math::Remainder, 7, 3), Ok(Value::Int(1)));
    assert_eq!(math(Math::Remainder, 6, 3), Ok(Value::Int(0)));
    assert_eq!(math(Math::Remainder, 0, 3), Ok(Value::Int(0)));
    assert_eq!(math(Math::Remainder, 2, 5), Ok(Value::Int(2)));
}

#[test]
fn a_remainder_of_a_negative_number_is_a_fault() {
    assert_eq!(math(Math::Remainder, -7, 3), Err(Fault::Remainder));
    assert_eq!(math(Math::Remainder, -1, 1), Err(Fault::Remainder));
}

#[test]
fn a_remainder_by_zero_or_by_a_negative_number_is_a_fault() {
    assert_eq!(math(Math::Remainder, 7, 0), Err(Fault::Remainder));
    assert_eq!(math(Math::Remainder, 7, -3), Err(Fault::Remainder));
}

#[test]
fn a_division_of_numbers_that_are_not_negative_drops_the_fraction() {
    assert_eq!(math(Math::Divide, 7, 2), Ok(Value::Int(3)));
    assert_eq!(math(Math::Divide, 6, 3), Ok(Value::Int(2)));
    assert_eq!(math(Math::Divide, 0, 3), Ok(Value::Int(0)));
    assert_eq!(math(Math::Divide, 2, 5), Ok(Value::Int(0)));
}

#[test]
fn a_division_of_a_negative_number_or_by_a_divisor_below_one_is_a_fault() {
    assert_eq!(math(Math::Divide, -7, 3), Err(Fault::Division));
    assert_eq!(math(Math::Divide, 7, 0), Err(Fault::Division));
    assert_eq!(math(Math::Divide, 7, -3), Err(Fault::Division));
    assert_eq!(math(Math::Divide, i64::MIN, -1), Err(Fault::Division));
}

#[test]
fn and_is_true_only_when_both_sides_are() {
    let and = |left: Expr, right: Expr| value_of(&Expr::And(Box::new(left), Box::new(right)));
    assert_eq!(and(yes(), yes()), YES);
    assert_eq!(and(yes(), no()), NO);
    assert_eq!(and(no(), yes()), NO);
    assert_eq!(and(no(), no()), NO);
}

#[test]
fn or_is_true_when_either_side_is() {
    let or = |left: Expr, right: Expr| value_of(&Expr::Or(Box::new(left), Box::new(right)));
    assert_eq!(or(yes(), yes()), YES);
    assert_eq!(or(yes(), no()), YES);
    assert_eq!(or(no(), yes()), YES);
    assert_eq!(or(no(), no()), NO);
}

#[test]
fn and_and_or_skip_the_right_side_when_the_left_decides() {
    let and = Expr::And(Box::new(no()), Box::new(faulting()));
    assert_eq!(value_of(&and), NO);
    let or = Expr::Or(Box::new(yes()), Box::new(faulting()));
    assert_eq!(value_of(&or), YES);
}

#[test]
fn and_and_or_evaluate_the_right_side_when_the_left_does_not_decide() {
    let and = Expr::And(Box::new(yes()), Box::new(faulting()));
    assert_eq!(value_of(&and), Err(Fault::NullRead));
    let or = Expr::Or(Box::new(no()), Box::new(faulting()));
    assert_eq!(value_of(&or), Err(Fault::NullRead));
}

#[test]
fn a_choice_evaluates_only_the_chosen_branch() {
    let first = Expr::Choose(Box::new(yes()), Box::new(int(1)), Box::new(faulting()));
    assert_eq!(value_of(&first), Ok(Value::Int(1)));
    let second = Expr::Choose(Box::new(no()), Box::new(faulting()), Box::new(int(2)));
    assert_eq!(value_of(&second), Ok(Value::Int(2)));
    let test_faults = Expr::Choose(Box::new(faulting()), Box::new(int(1)), Box::new(int(2)));
    assert_eq!(value_of(&test_faults), Err(Fault::NullRead));
}

#[test]
fn a_binding_is_visible_in_its_body() {
    let bound = Expr::Bind {
        slot: 2,
        value: Box::new(int(5)),
        body: Box::new(apply(Op::Math(Math::Add), &[Expr::Local(2), int(1)])),
    };
    assert_eq!(value_of(&bound), Ok(Value::Int(6)));
}

#[test]
fn a_record_is_built_read_and_changed_by_field_name() {
    let mut symbols = Symbols::default();
    let a = symbols.intern("a");
    let b = symbols.intern("b");
    let c = symbols.intern("c");
    let record = Expr::Record(vec![(b, int(2)), (a, int(1))]);
    let built = Value::record(vec![(a, Value::Int(1)), (b, Value::Int(2))]);
    assert_eq!(value_of(&record), Ok(built));
    assert_eq!(
        value_of(&Expr::Get(Box::new(record.clone()), b)),
        Ok(Value::Int(2))
    );
    let changed = Expr::With(Box::new(record), vec![(a, int(9)), (c, int(3))]);
    let expected = Value::record(vec![
        (a, Value::Int(9)),
        (b, Value::Int(2)),
        (c, Value::Int(3)),
    ]);
    assert_eq!(value_of(&changed), Ok(expected));
}

#[test]
fn reading_or_changing_a_field_of_null_is_a_null_read() {
    let mut symbols = Symbols::default();
    let a = symbols.intern("a");
    assert_eq!(
        value_of(&Expr::Get(Box::new(null()), a)),
        Err(Fault::NullRead)
    );
    let changed = Expr::With(Box::new(null()), vec![(a, int(1))]);
    assert_eq!(value_of(&changed), Err(Fault::NullRead));
}

#[test]
fn reading_a_field_a_value_does_not_have_is_malformed() {
    let mut symbols = Symbols::default();
    let a = symbols.intern("a");
    let b = symbols.intern("b");
    let record = Expr::Record(vec![(a, int(1))]);
    assert_eq!(
        value_of(&Expr::Get(Box::new(record), b)),
        Err(Fault::WrongKind)
    );
    assert_eq!(
        value_of(&Expr::Get(Box::new(int(1)), a)),
        Err(Fault::WrongKind)
    );
    let changed = Expr::With(Box::new(int(1)), vec![(a, int(1))]);
    assert_eq!(value_of(&changed), Err(Fault::WrongKind));
}

#[test]
fn collection_literals_build_a_set_a_sequence_and_a_map() {
    let items = vec![int(2), int(1), int(2)];
    assert_eq!(
        value_of(&Expr::SetOf(items.clone())),
        Ok(set_value(&[1, 2]))
    );
    assert_eq!(value_of(&Expr::SeqOf(items)), Ok(seq_value(&[2, 1, 2])));
    let entries = vec![(int(1), int(10)), (int(2), int(20)), (int(1), int(30))];
    assert_eq!(
        value_of(&Expr::MapOf(entries)),
        Ok(map_value(&[(1, 30), (2, 20)]))
    );
    assert_eq!(
        value_of(&Expr::SetOf(Vec::new())),
        Ok(Value::set(BTreeSet::new()))
    );
    assert_eq!(
        value_of(&Expr::MapOf(Vec::new())),
        Ok(Value::map(BTreeMap::new()))
    );
}

#[test]
fn a_set_answers_membership_and_size() {
    assert_eq!(on_set(SetOp::Has, &[set(&[1, 2]), int(2)]), YES);
    assert_eq!(on_set(SetOp::Has, &[set(&[1, 2]), int(3)]), NO);
    assert_eq!(on_set(SetOp::Size, &[set(&[1, 2, 5])]), Ok(Value::Int(3)));
    assert_eq!(on_set(SetOp::Size, &[set(&[])]), Ok(Value::Int(0)));
}

#[test]
fn a_set_gains_and_loses_items_without_duplicates() {
    assert_eq!(
        on_set(SetOp::Add, &[set(&[1]), int(2)]),
        Ok(set_value(&[1, 2]))
    );
    assert_eq!(
        on_set(SetOp::Add, &[set(&[1]), int(1)]),
        Ok(set_value(&[1]))
    );
    assert_eq!(
        on_set(SetOp::Delete, &[set(&[1, 2]), int(1)]),
        Ok(set_value(&[2]))
    );
    assert_eq!(
        on_set(SetOp::Delete, &[set(&[1, 2]), int(3)]),
        Ok(set_value(&[1, 2]))
    );
}

#[test]
fn two_sets_combine_by_union_intersection_and_difference() {
    let left = set(&[1, 2, 3]);
    let right = set(&[2, 3, 4]);
    assert_eq!(
        on_set(SetOp::Union, &[left.clone(), right.clone()]),
        Ok(set_value(&[1, 2, 3, 4]))
    );
    assert_eq!(
        on_set(SetOp::Intersect, &[left.clone(), right.clone()]),
        Ok(set_value(&[2, 3]))
    );
    assert_eq!(on_set(SetOp::Subtract, &[left, right]), Ok(set_value(&[1])));
}

#[test]
fn a_subset_has_no_item_the_other_set_lacks() {
    assert_eq!(
        on_set(SetOp::IsSubset, &[set(&[1, 2]), set(&[1, 2, 3])]),
        YES
    );
    assert_eq!(on_set(SetOp::IsSubset, &[set(&[1, 2]), set(&[1, 2])]), YES);
    assert_eq!(on_set(SetOp::IsSubset, &[set(&[]), set(&[])]), YES);
    assert_eq!(
        on_set(SetOp::IsSubset, &[set(&[1, 4]), set(&[1, 2, 3])]),
        NO
    );
}

#[test]
fn a_set_primitive_on_another_kind_of_value_is_a_fault() {
    assert_eq!(on_set(SetOp::Size, &[null()]), Err(Fault::NullRead));
    assert_eq!(on_set(SetOp::Size, &[seq(&[1])]), Err(Fault::WrongKind));
    assert_eq!(
        on_set(SetOp::Union, &[set(&[1]), null()]),
        Err(Fault::NullRead)
    );
}

#[test]
fn a_map_answers_lookups_and_size() {
    let scores = map(&[(1, 10), (2, 20)]);
    assert_eq!(on_map(MapOp::Has, &[scores.clone(), int(2)]), YES);
    assert_eq!(on_map(MapOp::Has, &[scores.clone(), int(3)]), NO);
    assert_eq!(
        on_map(MapOp::Get, &[scores.clone(), int(2)]),
        Ok(Value::Int(20))
    );
    assert_eq!(
        on_map(MapOp::Get, &[scores.clone(), int(3)]),
        Ok(Value::Null)
    );
    assert_eq!(
        on_map(MapOp::Size, std::slice::from_ref(&scores)),
        Ok(Value::Int(2))
    );
    assert_eq!(on_map(MapOp::Keys, &[scores]), Ok(set_value(&[1, 2])));
    assert_eq!(on_map(MapOp::Keys, &[map(&[])]), Ok(set_value(&[])));
}

#[test]
fn a_map_gains_replaces_and_loses_entries() {
    let scores = map(&[(1, 10)]);
    assert_eq!(
        on_map(MapOp::Set, &[scores.clone(), int(2), int(20)]),
        Ok(map_value(&[(1, 10), (2, 20)]))
    );
    assert_eq!(
        on_map(MapOp::Set, &[scores.clone(), int(1), int(11)]),
        Ok(map_value(&[(1, 11)]))
    );
    assert_eq!(
        on_map(MapOp::Delete, &[scores.clone(), int(1)]),
        Ok(map_value(&[]))
    );
    assert_eq!(
        on_map(MapOp::Delete, &[scores, int(5)]),
        Ok(map_value(&[(1, 10)]))
    );
}

#[test]
fn a_map_primitive_on_another_kind_of_value_is_a_fault() {
    assert_eq!(on_map(MapOp::Size, &[null()]), Err(Fault::NullRead));
    assert_eq!(on_map(MapOp::Size, &[set(&[1])]), Err(Fault::WrongKind));
}

#[test]
fn a_sequence_answers_size_and_membership() {
    assert_eq!(on_seq(SeqOp::Size, &[seq(&[4, 4, 5])]), Ok(Value::Int(3)));
    assert_eq!(on_seq(SeqOp::Size, &[seq(&[])]), Ok(Value::Int(0)));
    assert_eq!(on_seq(SeqOp::Includes, &[seq(&[4, 5]), int(5)]), YES);
    assert_eq!(on_seq(SeqOp::Includes, &[seq(&[4, 5]), int(6)]), NO);
}

#[test]
fn a_sequence_gives_items_by_position_counted_from_zero() {
    let items = seq(&[7, 8, 9]);
    assert_eq!(
        on_seq(SeqOp::Get, &[items.clone(), int(0)]),
        Ok(Value::Int(7))
    );
    assert_eq!(
        on_seq(SeqOp::Get, &[items.clone(), int(2)]),
        Ok(Value::Int(9))
    );
    assert_eq!(
        on_seq(SeqOp::First, std::slice::from_ref(&items)),
        Ok(Value::Int(7))
    );
    assert_eq!(on_seq(SeqOp::Last, &[items]), Ok(Value::Int(9)));
}

#[test]
fn a_position_a_sequence_does_not_have_gives_null() {
    let items = seq(&[7, 8, 9]);
    assert_eq!(
        on_seq(SeqOp::Get, &[items.clone(), int(3)]),
        Ok(Value::Null)
    );
    assert_eq!(on_seq(SeqOp::Get, &[items, int(-1)]), Ok(Value::Null));
    assert_eq!(on_seq(SeqOp::First, &[seq(&[])]), Ok(Value::Null));
    assert_eq!(on_seq(SeqOp::Last, &[seq(&[])]), Ok(Value::Null));
}

#[test]
fn a_position_that_is_not_a_number_is_a_fault() {
    assert_eq!(
        on_seq(SeqOp::Get, &[seq(&[7]), null()]),
        Err(Fault::NullRead)
    );
    assert_eq!(
        on_seq(SeqOp::Get, &[seq(&[7]), yes()]),
        Err(Fault::WrongKind)
    );
}

#[test]
fn a_sequence_grows_at_the_end_and_shrinks_at_either_end() {
    assert_eq!(
        on_seq(SeqOp::Push, &[seq(&[1, 2]), int(3)]),
        Ok(seq_value(&[1, 2, 3]))
    );
    assert_eq!(
        on_seq(SeqOp::Shift, &[seq(&[1, 2, 3])]),
        Ok(seq_value(&[2, 3]))
    );
    assert_eq!(
        on_seq(SeqOp::Pop, &[seq(&[1, 2, 3])]),
        Ok(seq_value(&[1, 2]))
    );
}

#[test]
fn an_empty_sequence_stays_empty_when_an_item_is_taken() {
    assert_eq!(on_seq(SeqOp::Shift, &[seq(&[])]), Ok(seq_value(&[])));
    assert_eq!(on_seq(SeqOp::Pop, &[seq(&[])]), Ok(seq_value(&[])));
}

#[test]
fn a_sequence_primitive_on_another_kind_of_value_is_a_fault() {
    assert_eq!(on_seq(SeqOp::First, &[null()]), Err(Fault::NullRead));
    assert_eq!(on_seq(SeqOp::First, &[set(&[1])]), Err(Fault::WrongKind));
}

#[test]
fn every_is_true_when_all_items_pass_and_for_no_items() {
    assert_eq!(
        sweep(Sweep::Every, Shelf::Set, set(&[2, 3]), item_above(1)),
        YES
    );
    assert_eq!(
        sweep(Sweep::Every, Shelf::Set, set(&[1, 3]), item_above(1)),
        NO
    );
    assert_eq!(
        sweep(Sweep::Every, Shelf::Seq, seq(&[]), item_above(1)),
        YES
    );
}

#[test]
fn some_is_true_when_one_item_passes_and_false_for_no_items() {
    assert_eq!(
        sweep(Sweep::Some, Shelf::Seq, seq(&[0, 3]), item_above(1)),
        YES
    );
    assert_eq!(
        sweep(Sweep::Some, Shelf::Seq, seq(&[0, 1]), item_above(1)),
        NO
    );
    assert_eq!(sweep(Sweep::Some, Shelf::Set, set(&[]), item_above(1)), NO);
}

#[test]
fn filter_keeps_the_passing_items_in_a_collection_of_the_same_kind() {
    assert_eq!(
        sweep(Sweep::Filter, Shelf::Set, set(&[1, 2, 3]), item_above(1)),
        Ok(set_value(&[2, 3]))
    );
    assert_eq!(
        sweep(Sweep::Filter, Shelf::Seq, seq(&[3, 1, 2, 3]), item_above(1)),
        Ok(seq_value(&[3, 2, 3]))
    );
    assert_eq!(
        sweep(Sweep::Filter, Shelf::Seq, seq(&[0]), item_above(1)),
        Ok(seq_value(&[]))
    );
}

#[test]
fn a_test_stops_at_the_first_item_that_decides_it() {
    let above_zero_or_fault = Expr::Choose(
        Box::new(apply(Op::Equal, &[Expr::Local(0), int(9)])),
        Box::new(faulting()),
        Box::new(item_above(0)),
    );
    assert_eq!(
        sweep(
            Sweep::Every,
            Shelf::Seq,
            seq(&[0, 9]),
            above_zero_or_fault.clone()
        ),
        NO
    );
    assert_eq!(
        sweep(
            Sweep::Some,
            Shelf::Seq,
            seq(&[1, 9]),
            above_zero_or_fault.clone()
        ),
        YES
    );
    assert_eq!(
        sweep(Sweep::Every, Shelf::Seq, seq(&[1, 9]), above_zero_or_fault),
        Err(Fault::NullRead)
    );
}

#[test]
fn a_test_over_something_that_is_not_a_collection_is_a_fault() {
    assert_eq!(
        sweep(Sweep::Every, Shelf::Set, null(), yes()),
        Err(Fault::NullRead)
    );
    assert_eq!(
        sweep(Sweep::Every, Shelf::Set, map(&[(1, 1)]), yes()),
        Err(Fault::WrongKind)
    );
    assert_eq!(
        sweep(Sweep::Some, Shelf::Set, int(1), yes()),
        Err(Fault::WrongKind)
    );
}

#[test]
fn a_test_that_does_not_give_a_boolean_is_a_fault() {
    assert_eq!(
        sweep(Sweep::Some, Shelf::Set, set(&[1]), int(1)),
        Err(Fault::WrongKind)
    );
}

#[test]
fn a_condition_is_true_or_false_and_anything_else_is_a_fault() {
    assert_eq!(condition(&yes()), Ok(true));
    assert_eq!(condition(&no()), Ok(false));
    assert_eq!(condition(&int(1)), Err(Fault::WrongKind));
    assert_eq!(condition(&null()), Err(Fault::NullRead));
    assert_eq!(condition(&faulting()), Err(Fault::NullRead));
}

#[test]
fn a_fault_inside_an_argument_is_the_fault_of_the_whole_expression() {
    let overflowing = apply(Op::Math(Math::Add), &[int(i64::MAX), int(1)]);
    let compared = apply(Op::Order(Order::Less), &[overflowing, int(0)]);
    assert_eq!(value_of(&compared), Err(Fault::Overflow));
}

#[test]
fn each_fault_finishes_a_sentence_about_what_was_evaluated() {
    assert_eq!(Fault::NullRead.describe(), "reads a value that is null");
    assert_eq!(
        Fault::Overflow.describe(),
        "computes a number too large to hold"
    );
    assert_eq!(
        Fault::Remainder.describe(),
        "takes a remainder with a negative number or a divisor below 1"
    );
    assert_eq!(
        Fault::Division.describe(),
        "takes a division with a negative number or a divisor below 1"
    );
    assert_eq!(
        Fault::WrongKind.describe(),
        "uses a value of the wrong kind"
    );
    assert_eq!(
        Fault::Outside(IntType::U8).describe(),
        "computes a number outside `u8`"
    );
    assert_eq!(
        Fault::Outside(IntType::Usize).describe(),
        "computes a number outside `usize`"
    );
}

fn fit(ty: IntType, number: i64) -> Result<Value, Fault> {
    value_of(&apply(Op::Fit(ty), &[int(number)]))
}

#[test]
fn a_number_inside_its_integer_type_is_given_back_unchanged() {
    assert_eq!(fit(IntType::U8, 0), Ok(Value::Int(0)));
    assert_eq!(fit(IntType::U8, 255), Ok(Value::Int(255)));
    assert_eq!(fit(IntType::I8, -128), Ok(Value::Int(-128)));
    assert_eq!(fit(IntType::Usize, i64::MAX), Ok(Value::Int(i64::MAX)));
}

#[test]
fn a_number_outside_its_integer_type_is_a_fault_that_names_the_type() {
    assert_eq!(fit(IntType::U8, -1), Err(Fault::Outside(IntType::U8)));
    assert_eq!(fit(IntType::U8, 256), Err(Fault::Outside(IntType::U8)));
    assert_eq!(fit(IntType::I8, 128), Err(Fault::Outside(IntType::I8)));
    assert_eq!(fit(IntType::Usize, -1), Err(Fault::Outside(IntType::Usize)));
    assert_eq!(
        fit(IntType::I32, 2_147_483_648),
        Err(Fault::Outside(IntType::I32))
    );
}

#[test]
fn a_difference_is_computed_first_and_then_must_fit() {
    let below = |start: i64| {
        let difference = apply(Op::Math(Math::Subtract), &[int(start), int(1)]);
        value_of(&apply(Op::Fit(IntType::U8), &[difference]))
    };
    assert_eq!(below(1), Ok(Value::Int(0)), "1 - 1 is a `u8`");
    assert_eq!(below(0), Err(Fault::Outside(IntType::U8)), "0 - 1 is not");
}

#[test]
fn only_a_number_can_fit_an_integer_type() {
    let fitted = |inner: Expr| value_of(&apply(Op::Fit(IntType::U8), &[inner]));
    assert_eq!(fitted(null()), Err(Fault::NullRead));
    assert_eq!(fitted(yes()), Err(Fault::WrongKind));
    assert_eq!(fitted(faulting()), Err(Fault::NullRead), "the fault inside");
}

#[test]
fn a_number_past_the_safe_integers_is_an_overflow_when_it_must_be_safe() {
    let safe = |number: i64| value_of(&apply(Op::Safe, &[int(number)]));
    assert_eq!(
        safe(MOST_SAFE_INTEGER),
        Ok(Value::Int(MOST_SAFE_INTEGER)),
        "2^53 - 1 is the last number JavaScript holds exactly"
    );
    assert_eq!(
        safe(-MOST_SAFE_INTEGER),
        Ok(Value::Int(-MOST_SAFE_INTEGER)),
        "and -(2^53 - 1) the last below"
    );
    assert_eq!(
        safe(MOST_SAFE_INTEGER + 1),
        Err(Fault::Overflow),
        "2^53 has a neighbour that rounds to it"
    );
    assert_eq!(
        safe(-MOST_SAFE_INTEGER - 1),
        Err(Fault::Overflow),
        "-2^53 likewise"
    );
    assert_eq!(
        value_of(&apply(Op::Safe, &[yes()])),
        Err(Fault::WrongKind),
        "only a number can be safe"
    );
}

fn overflowing() -> Expr {
    apply(Op::Math(Math::Add), &[int(i64::MAX), int(1)])
}

#[test]
fn every_argument_is_evaluated_before_the_kind_of_any_is_checked() {
    let compared = apply(Op::Order(Order::Less), &[null(), overflowing()]);
    assert_eq!(value_of(&compared), Err(Fault::Overflow));
    let summed = apply(Op::Math(Math::Add), &[yes(), overflowing()]);
    assert_eq!(value_of(&summed), Err(Fault::Overflow));
    let made_safe = apply(Op::Safe, &[apply(Op::Unwrap, &[null()])]);
    assert_eq!(value_of(&made_safe), Err(Fault::NullRead));
}

#[test]
fn the_kind_of_the_first_argument_is_checked_before_the_second() {
    let null_first = apply(Op::Order(Order::Less), &[null(), yes()]);
    assert_eq!(value_of(&null_first), Err(Fault::NullRead));
    let wrong_first = apply(Op::Math(Math::Subtract), &[yes(), null()]);
    assert_eq!(value_of(&wrong_first), Err(Fault::WrongKind));
    let wrong_second = apply(Op::Math(Math::Remainder), &[int(-1), null()]);
    assert_eq!(value_of(&wrong_second), Err(Fault::NullRead));
}

#[test]
fn an_argument_past_those_an_operation_takes_is_still_evaluated() {
    let extra = |op: Op, arguments: &[Expr]| value_of(&apply(op, arguments));
    assert_eq!(extra(Op::Safe, &[int(1), faulting()]), Err(Fault::NullRead));
    assert_eq!(
        extra(Op::Math(Math::Add), &[int(1), int(2), faulting()]),
        Err(Fault::NullRead)
    );
    assert_eq!(
        extra(Op::Order(Order::Less), &[int(1), int(2), faulting()]),
        Err(Fault::NullRead)
    );
    assert_eq!(extra(Op::Not, &[yes(), faulting()]), Err(Fault::NullRead));
    assert_eq!(extra(Op::Equal, &[int(1), int(1), int(2)]), YES);
    assert_eq!(
        extra(Op::Math(Math::Add), &[int(1), int(2), int(3)]),
        Ok(Value::Int(3))
    );
}

#[test]
fn a_missing_field_is_malformed_before_any_later_argument_is_evaluated() {
    let compared = apply(Op::Order(Order::Less), &[Expr::Field(9), overflowing()]);
    assert_eq!(value_in(&ints(&[1]), &compared), Err(Fault::WrongKind));
    let equal = apply(Op::Equal, &[Expr::Local(9), overflowing()]);
    assert_eq!(value_in(&ints(&[1]), &equal), Err(Fault::WrongKind));
}

#[test]
fn values_of_different_kinds_are_unequal_and_never_a_fault() {
    let state = vec![Value::Null, Value::Bool(true), Value::Int(4)];
    let equal = |left: Expr, right: Expr| value_in(&state, &apply(Op::Equal, &[left, right]));
    assert_eq!(equal(Expr::Field(0), int(0)), NO);
    assert_eq!(equal(int(1), Expr::Field(1)), NO);
    assert_eq!(equal(Expr::Field(2), int(4)), YES);
    assert_eq!(equal(int(4), Expr::Field(2)), YES);
    assert_eq!(equal(Expr::Field(0), null()), YES);
    assert_eq!(equal(Expr::Field(1), yes()), YES);
    assert_eq!(equal(Expr::Field(1), overflowing()), Err(Fault::Overflow));
    let differ = apply(Op::Differ, &[Expr::Field(0), int(0)]);
    assert_eq!(value_in(&state, &differ), YES);
}

fn wide_after(expr: &Expr) -> (Result<Value, Fault>, bool) {
    let ran = both_values(&[], &[], expr);
    (ran.answer, ran.beyond_32_bits)
}

#[test]
fn only_arithmetic_beyond_32_bits_or_too_large_to_hold_is_marked() {
    let above_32_bits = i64::from(i32::MAX) + 1;
    let sum = |left: i64, right: i64| apply(Op::Math(Math::Add), &[int(left), int(right)]);
    assert_eq!(
        wide_after(&sum(i64::from(i32::MAX), 0)),
        (Ok(Value::Int(i64::from(i32::MAX))), false)
    );
    assert_eq!(
        wide_after(&sum(i64::from(i32::MAX), 1)),
        (Ok(Value::Int(above_32_bits)), true)
    );
    assert_eq!(wide_after(&overflowing()), (Err(Fault::Overflow), true));
    let unsafe_sum = apply(Op::Safe, &[sum(MOST_SAFE_INTEGER, 1)]);
    assert_eq!(wide_after(&unsafe_sum), (Err(Fault::Overflow), true));
    let unfit = apply(Op::Fit(IntType::U8), &[sum(above_32_bits, 0)]);
    assert_eq!(wide_after(&unfit), (Err(Fault::Outside(IntType::U8)), true));
    let remainder = apply(Op::Math(Math::Remainder), &[int(above_32_bits), int(0)]);
    assert_eq!(wide_after(&remainder), (Err(Fault::Remainder), false));
    let kind = apply(Op::Math(Math::Add), &[null(), int(above_32_bits)]);
    assert_eq!(wide_after(&kind), (Err(Fault::NullRead), false));
    let negated = apply(Op::Negate, &[int(above_32_bits)]);
    assert_eq!(
        wide_after(&negated),
        (Ok(Value::Int(-above_32_bits)), false)
    );
    let compared = apply(Op::Order(Order::Less), &[int(above_32_bits), int(0)]);
    assert_eq!(wide_after(&compared), (NO, false));
}

#[test]
fn a_condition_that_chooses_a_value_that_is_not_a_boolean_is_a_fault() {
    let chosen = Expr::Choose(Box::new(yes()), Box::new(int(1)), Box::new(no()));
    assert_eq!(condition(&chosen), Err(Fault::WrongKind));
    let negated = apply(Op::Not, &[null()]);
    assert_eq!(condition(&negated), Err(Fault::NullRead));
    let and = Expr::And(Box::new(int(1)), Box::new(faulting()));
    assert_eq!(condition(&and), Err(Fault::WrongKind));
}

const FIELDS: usize = 3;

fn edge_number() -> impl Strategy<Value = i64> {
    prop_oneof![
        -3_i64..12,
        Just(i64::MAX),
        Just(i64::MIN),
        Just(MOST_SAFE_INTEGER),
        Just(-MOST_SAFE_INTEGER - 1),
        Just(i64::from(i32::MAX)),
        Just(i64::from(i32::MAX) + 1),
        Just(255),
        Just(256),
    ]
}

const NAMES: usize = 3;

fn any_name() -> impl Strategy<Value = Sym> {
    (0..NAMES).prop_map(|position| {
        let mut symbols = Symbols::default();
        let mut named = symbols.intern("a");
        for text in ["b", "c"].iter().take(position) {
            named = symbols.intern(text);
        }
        named
    })
}

fn any_value() -> impl Strategy<Value = Value> {
    let plain = prop_oneof![
        3 => edge_number().prop_map(Value::Int),
        1 => any::<bool>().prop_map(Value::Bool),
        1 => Just(Value::Null),
        1 => any_name().prop_map(Value::Str),
    ];
    plain.prop_recursive(2, 6, 3, |inner| {
        prop_oneof![
            1 => proptest::collection::vec(inner.clone(), 0..3).prop_map(Value::seq),
            1 => proptest::collection::btree_set(inner.clone(), 0..3).prop_map(Value::set),
            1 => proptest::collection::btree_map(inner.clone(), inner.clone(), 0..3)
                .prop_map(Value::map),
            2 => proptest::collection::btree_map(any_name(), inner, 0..4)
                .prop_map(|fields| Value::record(fields.into_iter().collect())),
        ]
    })
}

fn any_op() -> impl Strategy<Value = Op> {
    let set_ops = prop_oneof![
        Just(SetOp::Has),
        Just(SetOp::Add),
        Just(SetOp::Delete),
        Just(SetOp::Union),
        Just(SetOp::Intersect),
        Just(SetOp::Subtract),
        Just(SetOp::IsSubset),
        Just(SetOp::Size),
    ];
    let map_ops = prop_oneof![
        Just(MapOp::Has),
        Just(MapOp::Get),
        Just(MapOp::Set),
        Just(MapOp::Delete),
        Just(MapOp::Size),
        Just(MapOp::Keys),
    ];
    let sequence_ops = prop_oneof![
        Just(SeqOp::Size),
        Just(SeqOp::Get),
        Just(SeqOp::First),
        Just(SeqOp::Last),
        Just(SeqOp::Push),
        Just(SeqOp::Shift),
        Just(SeqOp::Pop),
        Just(SeqOp::Includes),
    ];
    prop_oneof![
        2 => Just(Op::Not),
        1 => Just(Op::Negate),
        2 => Just(Op::SomeOf),
        1 => Just(Op::Unwrap),
        1 => Just(Op::Safe),
        1 => Just(Op::Fit(IntType::U8)),
        1 => Just(Op::Fit(IntType::I32)),
        3 => Just(Op::Equal),
        3 => Just(Op::Differ),
        1 => Just(Op::Order(Order::Less)),
        1 => Just(Op::Order(Order::LessOrEqual)),
        1 => Just(Op::Order(Order::Greater)),
        1 => Just(Op::Order(Order::GreaterOrEqual)),
        1 => Just(Op::Math(Math::Add)),
        1 => Just(Op::Math(Math::Subtract)),
        1 => Just(Op::Math(Math::Multiply)),
        1 => Just(Op::Math(Math::Remainder)),
        1 => Just(Op::Math(Math::Divide)),
        2 => set_ops.prop_map(Op::Set),
        2 => map_ops.prop_map(Op::Map),
        2 => sequence_ops.prop_map(Op::Seq),
    ]
}

fn any_sweep() -> impl Strategy<Value = (Sweep, Shelf)> {
    (
        prop_oneof![Just(Sweep::Every), Just(Sweep::Some), Just(Sweep::Filter)],
        prop_oneof![Just(Shelf::Set), Just(Shelf::Seq)],
    )
}

fn any_expr() -> impl Strategy<Value = Expr> {
    let leaf = prop_oneof![
        2 => any_value().prop_map(Expr::Const),
        2 => (0..FIELDS + 1).prop_map(Expr::Field),
        3 => (0..SLOTS + 1).prop_map(Expr::Local),
    ];
    leaf.prop_recursive(5, 40, 4, |inner| {
        let boxed = || inner.clone().prop_map(Box::new);
        let named = || proptest::collection::vec((any_name(), inner.clone()), 0..4);
        prop_oneof![
            6 => (any_op(), proptest::collection::vec(inner.clone(), 0..4))
                .prop_map(|(op, arguments)| Expr::Apply(op, arguments)),
            2 => (boxed(), boxed()).prop_map(|(left, right)| Expr::And(left, right)),
            2 => (boxed(), boxed()).prop_map(|(left, right)| Expr::Or(left, right)),
            1 => (boxed(), boxed(), boxed()).prop_map(|(test, yes, no)| Expr::Choose(test, yes, no)),
            2 => (0..SLOTS, boxed(), boxed())
                .prop_map(|(slot, value, body)| Expr::Bind { slot, value, body }),
            1 => (any_sweep(), 0..SLOTS, boxed(), boxed()).prop_map(
                |((sweep, shelf), slot, source, body)| Expr::Sweep {
                    sweep,
                    shelf,
                    slot,
                    source,
                    body,
                },
            ),
            1 => proptest::collection::vec(inner.clone(), 0..3).prop_map(Expr::SeqOf),
            1 => proptest::collection::vec(inner.clone(), 0..3).prop_map(Expr::SetOf),
            1 => proptest::collection::vec((inner.clone(), inner.clone()), 0..3)
                .prop_map(Expr::MapOf),
            2 => named().prop_map(Expr::Record),
            3 => (boxed(), any_name()).prop_map(|(record, name)| Expr::Get(record, name)),
            1 => (boxed(), named()).prop_map(|(base, changes)| Expr::With(base, changes)),
        ]
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn the_prepared_form_agrees_with_the_reference_walk(
        expr in any_expr(),
        state in proptest::collection::vec(any_value(), FIELDS),
        locals in proptest::collection::vec(any_value(), SLOTS),
    ) {
        both_values(&state, &locals, &expr);
        both_truths(&state, &locals, &expr);
    }
}

const CODES: usize = 4;
const SHARED_BINDINGS: usize = 3;

#[derive(Clone, Debug)]
enum Step {
    Value(usize),
    Truth(usize),
    Apart(usize),
    Other(usize),
    Put(usize, Value),
}

fn any_step() -> impl Strategy<Value = Step> {
    prop_oneof![
        3 => (0..CODES).prop_map(Step::Value),
        3 => (0..CODES).prop_map(Step::Truth),
        1 => (0..CODES).prop_map(Step::Apart),
        1 => (0..CODES).prop_map(Step::Other),
        1 => (0..SLOTS, any_value()).prop_map(|(slot, value)| Step::Put(slot, value)),
    ]
}

fn wrapped(body: Expr, bindings: &[(usize, Expr)], chosen: &[bool]) -> Expr {
    bindings
        .iter()
        .zip(chosen)
        .rev()
        .filter(|(_, wanted)| **wanted)
        .fold(body, |inner, ((slot, value), _)| Expr::Bind {
            slot: *slot,
            value: Box::new(value.clone()),
            body: Box::new(inner),
        })
}

fn any_unit() -> impl Strategy<Value = Vec<Expr>> {
    let bindings = proptest::collection::vec((0..SLOTS, any_expr()), 1..=SHARED_BINDINGS);
    let parts = proptest::collection::vec(
        (
            any_expr(),
            any_expr(),
            proptest::collection::vec(any::<bool>(), SHARED_BINDINGS),
            any::<bool>(),
        ),
        CODES,
    );
    (bindings, parts).prop_map(|(bindings, parts)| {
        parts
            .into_iter()
            .map(|(body, test, chosen, behind_a_test)| {
                let shared = wrapped(body, &bindings, &chosen);
                if behind_a_test {
                    Expr::Choose(Box::new(test.clone()), Box::new(shared), Box::new(test))
                } else {
                    shared
                }
            })
            .collect()
    })
}

fn slots_of(frame: &mut Frame<'_>) -> Vec<Result<Value, Fault>> {
    (0..SLOTS)
        .map(|slot| eval(&Expr::Local(slot), frame))
        .collect()
}

struct Units<'e> {
    exprs: &'e [Expr],
    others: &'e [Expr],
    together: Vec<Code>,
    apart: Vec<Code>,
    other: Vec<Code>,
}

impl Units<'_> {
    fn chosen(&self, step: &Step) -> Option<(&Expr, &Code)> {
        let (exprs, codes, at) = match step {
            Step::Value(at) | Step::Truth(at) => (self.exprs, &self.together, at),
            Step::Apart(at) => (self.exprs, &self.apart, at),
            Step::Other(at) => (self.others, &self.other, at),
            Step::Put(..) => return None,
        };
        Some((exprs.get(*at)?, codes.get(*at)?))
    }
}

fn agreed_in_turn(
    state: &[Value],
    locals: &[Value],
    (exprs, others): (&[Expr], &[Expr]),
    steps: &[Step],
) {
    let units = Units {
        exprs,
        others,
        together: Code::together(exprs.iter().cloned()),
        apart: exprs.iter().cloned().map(Code::new).collect(),
        other: Code::together(others.iter().cloned()),
    };
    let mut reference = Frame::new(state, SLOTS);
    let mut prepared = Frame::new(state, SLOTS);
    for (slot, value) in locals.iter().enumerate() {
        reference.put(slot, value.clone());
        prepared.put(slot, value.clone());
    }
    for step in steps {
        if let Step::Put(slot, value) = step {
            reference.put(*slot, value.clone());
            prepared.put(*slot, value.clone());
        } else if let Some((expr, code)) = units.chosen(step) {
            if matches!(step, Step::Truth(_)) {
                assert_eq!(
                    truth(expr, &mut reference),
                    code.truth(&mut prepared),
                    "the same truth or fault at {step:?}"
                );
            } else {
                assert_eq!(
                    eval(expr, &mut reference),
                    code.eval(&mut prepared),
                    "the same value or fault at {step:?}"
                );
            }
        }
        assert_eq!(
            reference.computed_beyond_32_bits(),
            prepared.computed_beyond_32_bits(),
            "the same mark for numbers beyond 32 bits after {step:?}"
        );
        assert_eq!(
            slots_of(&mut reference),
            slots_of(&mut prepared),
            "the same slots after {step:?}"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn codes_prepared_together_agree_with_the_reference_walk_in_any_order(
        exprs in any_unit(),
        others in any_unit(),
        state in proptest::collection::vec(any_value(), FIELDS),
        locals in proptest::collection::vec(any_value(), SLOTS),
        steps in proptest::collection::vec(any_step(), 1..12),
    ) {
        agreed_in_turn(&state, &locals, (&exprs, &others), &steps);
    }
}

const INPUT_SLOT: usize = 0;

fn small_field() -> impl Strategy<Value = Value> {
    prop_oneof![
        3 => (0..2_i64).prop_map(Value::Int),
        1 => Just(Value::Null),
    ]
}

fn any_input_value() -> impl Strategy<Value = Value> {
    prop_oneof![
        6 => proptest::collection::btree_map(any_name(), small_field(), 1..=NAMES)
            .prop_map(|fields| Value::record(fields.into_iter().collect())),
        1 => small_field(),
    ]
}

fn any_reading_expr() -> impl Strategy<Value = Expr> {
    let leaf = prop_oneof![
        2 => any_value().prop_map(Expr::Const),
        2 => (0..FIELDS).prop_map(Expr::Field),
        1 => (0..SLOTS).prop_map(Expr::Local),
        6 => (0..SLOTS, any_name())
            .prop_map(|(slot, name)| Expr::Get(Box::new(Expr::Local(slot)), name)),
    ];
    leaf.prop_recursive(4, 24, 3, |inner| {
        let boxed = || inner.clone().prop_map(Box::new);
        prop_oneof![
            4 => (any_op(), proptest::collection::vec(inner.clone(), 1..3))
                .prop_map(|(op, arguments)| Expr::Apply(op, arguments)),
            2 => (boxed(), boxed()).prop_map(|(left, right)| Expr::And(left, right)),
            1 => (boxed(), boxed(), boxed()).prop_map(|(test, yes, no)| Expr::Choose(test, yes, no)),
            3 => (1..SLOTS, 0..SLOTS, boxed()).prop_map(|(slot, source, body)| Expr::Bind {
                slot,
                value: Box::new(Expr::Local(source)),
                body,
            }),
            1 => (1..SLOTS, boxed(), boxed())
                .prop_map(|(slot, value, body)| Expr::Bind { slot, value, body }),
            2 => proptest::collection::vec((any_name(), inner.clone()), 1..3).prop_map(Expr::Record),
        ]
    })
}

fn reading_action(values: Vec<Value>, exprs: Vec<Expr>) -> Action {
    let mut codes = Code::together(exprs).into_iter();
    let guard = codes
        .next()
        .unwrap_or_else(|| Code::new(Expr::Const(Value::Bool(true))));
    Action {
        id: "act".to_owned(),
        label: "acts".to_owned(),
        line: 1,
        inputs: vec![Input {
            name: "input".to_owned(),
            slot: INPUT_SLOT,
            domain: Domain::OneOf(values.clone()),
            values,
        }],
        guard,
        updates: codes.map(Some).collect(),
        slots: SLOTS,
    }
}

type Answers = Vec<(Result<Value, Fault>, bool)>;

fn answers_in_turn(
    action: &Action,
    state: &[Value],
    left_over: &[Value],
    tried: &[bool],
) -> Vec<Option<Answers>> {
    let mut frame = Frame::new(state, SLOTS);
    for (slot, value) in left_over.iter().enumerate() {
        frame.put(slot, value.clone());
    }
    let values = action.inputs.first().map_or(&[][..], |input| &input.values);
    values
        .iter()
        .zip(tried)
        .map(|(value, tried)| {
            if !tried {
                return None;
            }
            frame.put(INPUT_SLOT, value.clone());
            Some(
                std::iter::once(&action.guard)
                    .chain(action.updates.iter().flatten())
                    .map(|code| {
                        (
                            eval(code.expr(), &mut frame),
                            frame.computed_beyond_32_bits(),
                        )
                    })
                    .collect(),
            )
        })
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn a_search_that_skips_values_not_told_apart_sees_the_same_answers(
        values in proptest::collection::vec(any_input_value(), 1..8),
        exprs in proptest::collection::vec(any_reading_expr(), 1..4),
        state in proptest::collection::vec(any_value(), FIELDS),
        left_over in proptest::collection::vec(any_value(), SLOTS),
    ) {
        let action = reading_action(values, exprs);
        let told = action.told_apart();
        prop_assert_eq!(told.len(), 1, "one answer for each input");
        let firsts = told.first().cloned().unwrap_or_default();
        let count = action.inputs.first().map_or(0, |input| input.values.len());
        prop_assert_eq!(firsts.len(), count, "one answer for each value");
        prop_assert!(firsts.first().copied().unwrap_or(true), "the first value is always new");
        let every = answers_in_turn(&action, &state, &left_over, &vec![true; count]);
        let skipping = answers_in_turn(&action, &state, &left_over, &firsts);
        for (position, (all, skipped)) in every.iter().zip(&skipping).enumerate() {
            match skipped {
                Some(answers) => prop_assert_eq!(Some(answers), all.as_ref(), "value {} answers as it does in the full walk", position),
                None => prop_assert!(
                    every.iter().take(position).any(|earlier| earlier == all),
                    "value {} answers like no value before it", position
                ),
            }
        }
    }
}
