//! Every operation of the expression language gives the value its definition says, and every
//! fault is reported as a fault.

use earnest_ir::{
    Expr, Fault, Frame, MapOp, Math, Op, Order, SeqOp, SetOp, Shelf, Sweep, eval, truth,
};
use earnest_value::{IntType, MOST_SAFE_INTEGER, Symbols, Value};
use std::collections::{BTreeMap, BTreeSet};

const SLOTS: usize = 4;

fn value_in(state: &[Value], expr: &Expr) -> Result<Value, Fault> {
    eval(expr, &mut Frame::new(state, SLOTS))
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
    let mut frame = Frame::new(&[], SLOTS);
    assert_eq!(truth(&yes(), &mut frame), Ok(true));
    assert_eq!(truth(&no(), &mut frame), Ok(false));
    assert_eq!(truth(&int(1), &mut frame), Err(Fault::WrongKind));
    assert_eq!(truth(&null(), &mut frame), Err(Fault::NullRead));
    assert_eq!(truth(&faulting(), &mut frame), Err(Fault::NullRead));
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
