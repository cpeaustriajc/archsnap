use crate::build::{
    HERE, binary, boolean, call, choose, differ, equal, fails, field, here, holds, int, lambda,
    name, no, null, one_of, optional, prop, range, record, record_shape, refusal, starting, text,
    verdict, wrap, yes,
};
use earnest_ast::{Binary, Shape, Term};
use earnest_ir::Fault;

fn press() -> Shape {
    one_of(&["keyboard", "virtual", "mouse"])
}

fn fields() -> Vec<(&'static str, Shape)> {
    let spot = || record_shape(vec![("x", range(0, 5))]);
    vec![
        ("by", starting(optional(press()), null())),
        ("set_by", starting(optional(press()), text("keyboard"))),
        ("maybe", starting(optional(range(0, 5)), int(2))),
        ("none", starting(optional(range(0, 5)), null())),
        ("n", starting(range(0, 10), int(2))),
        ("flag", starting(boolean(), yes())),
        (
            "spot",
            starting(optional(spot()), record(vec![("x", int(1))])),
        ),
        ("nowhere", starting(optional(spot()), null())),
    ]
}

fn is_true(condition: Term) -> bool {
    holds(fields(), condition)
}

fn is_false(condition: Term) -> bool {
    fails(fields(), condition)
}

fn above(limit: i64) -> Term {
    lambda("x", binary(Binary::Greater, name("x"), int(limit)))
}

#[test]
fn an_optional_value_compares_with_null() {
    assert!(is_true(equal(field("by"), null())));
    assert!(is_false(differ(field("by"), null())));
    assert!(is_false(equal(field("set_by"), null())));
    assert!(is_true(differ(field("set_by"), null())));
    assert!(is_true(equal(null(), field("by"))));
}

#[test]
fn a_present_optional_value_equals_the_plain_value_it_holds() {
    assert!(is_true(equal(field("set_by"), text("keyboard"))));
    assert!(is_true(equal(text("keyboard"), field("set_by"))));
    assert!(is_false(equal(field("set_by"), text("mouse"))));
    assert!(is_true(differ(field("set_by"), text("mouse"))));
    assert!(is_true(equal(field("maybe"), field("n"))));
    assert!(is_true(equal(field("n"), field("maybe"))));
}

#[test]
fn an_absent_optional_value_equals_no_plain_value_and_is_no_fault() {
    assert!(is_false(equal(field("by"), text("keyboard"))));
    assert!(is_true(differ(field("by"), text("keyboard"))));
    assert!(is_false(equal(field("none"), field("n"))));
    assert!(is_false(equal(field("n"), field("none"))));
}

#[test]
fn two_optional_values_compare_as_a_whole() {
    assert!(is_true(equal(field("by"), field("by"))));
    assert!(is_false(equal(field("maybe"), field("none"))));
    assert!(is_true(differ(field("by"), field("set_by"))));
}

#[test]
fn is_some_and_is_none_ask_whether_a_value_is_present() {
    assert!(is_true(call(field("maybe"), "is_some", Vec::new())));
    assert!(is_false(call(field("maybe"), "is_none", Vec::new())));
    assert!(is_false(call(field("none"), "is_some", Vec::new())));
    assert!(is_true(call(field("none"), "is_none", Vec::new())));
}

#[test]
fn is_some_and_tests_a_present_value_and_is_false_for_an_absent_one() {
    assert!(is_true(call(field("maybe"), "is_some_and", vec![above(1)])));
    assert!(is_false(call(
        field("maybe"),
        "is_some_and",
        vec![above(2)]
    )));
    assert!(is_false(call(field("none"), "is_some_and", vec![above(1)])));
}

#[test]
fn an_optional_value_compares_with_some_of_a_plain_value() {
    assert!(is_true(equal(field("maybe"), wrap(int(2)))));
    assert!(is_false(equal(field("maybe"), wrap(int(3)))));
    assert!(is_false(equal(field("none"), wrap(int(2)))));
    assert!(is_true(equal(wrap(field("n")), field("maybe"))));
}

#[test]
fn a_choice_between_null_and_a_value_is_an_optional_value() {
    let picked = |test: Term| choose(test, null(), int(3));
    assert!(is_true(equal(picked(no()), int(3))));
    assert!(is_false(equal(picked(yes()), int(3))));
    assert!(is_true(equal(picked(field("flag")), null())));
    assert!(is_true(equal(choose(no(), int(3), null()), null())));
}

#[test]
fn a_present_optional_number_is_used_as_a_number() {
    let sum = binary(Binary::Add, field("maybe"), int(1));
    assert!(is_true(equal(sum, int(3))));
    assert!(is_true(binary(Binary::Less, field("maybe"), int(3))));
}

#[test]
fn arithmetic_or_order_on_an_absent_number_is_a_null_read() {
    let sum = equal(binary(Binary::Add, field("none"), int(1)), int(3));
    assert_eq!(verdict(fields(), sum), Err(Fault::NullRead));
    let below = binary(Binary::Less, field("none"), int(3));
    assert_eq!(verdict(fields(), below), Err(Fault::NullRead));
}

#[test]
fn a_field_of_a_present_optional_record_is_read_through() {
    assert!(is_true(equal(prop(field("spot"), "x"), int(1))));
}

#[test]
fn a_field_of_null_is_a_null_read() {
    let read = equal(prop(field("nowhere"), "x"), int(1));
    assert_eq!(verdict(fields(), read), Err(Fault::NullRead));
}

#[test]
fn a_guarded_read_of_an_optional_record_is_no_fault() {
    let present = differ(field("nowhere"), null());
    let read = equal(prop(field("nowhere"), "x"), int(1));
    assert!(is_false(binary(Binary::And, present, read)));
}

#[test]
fn an_optional_value_never_equals_a_value_of_another_kind() {
    let problem = refusal(fields(), here(equal(field("maybe"), text("two"))));
    assert_eq!(problem.place(), HERE);
    assert_eq!(
        problem.feature,
        "an optional value compared with a text value"
    );
}

#[test]
fn an_optional_value_can_not_hold_another_optional_value() {
    let problem = refusal(fields(), equal(field("maybe"), here(wrap(wrap(int(2))))));
    assert_eq!(problem.place(), HERE);
    assert_eq!(
        problem.feature,
        "an optional value inside an optional value"
    );
    let wrapped_null = refusal(fields(), equal(field("maybe"), here(wrap(null()))));
    assert_eq!(wrapped_null.place(), HERE);
}
