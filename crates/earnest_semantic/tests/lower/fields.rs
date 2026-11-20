use crate::build::{
    FILE, HERE, binary, boolean, draft, here, int, ints, lock_shape, lowered, map_shape, name,
    null, one_of, optional, queue_shape, range, record_shape, seq_of, seq_shape, set_of, set_shape,
    shape, starting, text, the_problem, yes,
};
use earnest_ast::{Binary, Shape, ShapeKind};
use earnest_ir::Model;
use earnest_value::Domain;

fn model_of(declared: Shape) -> Model {
    lowered(&draft(vec![("only", declared)]))
}

fn starts(model: &Model) -> Vec<String> {
    model.fields[0]
        .starts
        .iter()
        .map(|value| model.symbols.show(value))
        .collect()
}

fn starts_of(declared: Shape) -> Vec<String> {
    starts(&model_of(declared))
}

fn press() -> Shape {
    one_of(&["keyboard", "virtual", "mouse"])
}

#[test]
fn a_model_keeps_its_name_file_line_and_field_order() {
    let model = lowered(&draft(vec![
        ("surface", one_of(&["results", "shortlist"])),
        ("paid", boolean()),
        ("visitors", range(0, 3)),
    ]));
    assert_eq!(model.name, "Cart");
    assert_eq!(model.file, FILE);
    assert_eq!(model.line, 5);
    let names: Vec<&str> = model
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect();
    assert_eq!(names, ["surface", "paid", "visitors"]);
    for field in &model.fields {
        assert_eq!(model.symbols.name(field.sym), field.name);
    }
}

#[test]
fn a_boolean_without_a_starting_value_starts_at_both() {
    assert_eq!(starts_of(boolean()), ["false", "true"]);
    assert_eq!(model_of(boolean()).fields[0].domain, Domain::Bool);
}

#[test]
fn a_number_without_a_starting_value_starts_at_every_number_in_its_range() {
    assert_eq!(starts_of(range(0, 3)), ["0", "1", "2", "3"]);
    assert_eq!(
        model_of(range(0, 3)).fields[0].domain,
        Domain::Int { lo: 0, hi: 3 }
    );
}

#[test]
fn an_enum_without_a_starting_value_starts_at_every_choice_in_the_listed_order() {
    assert_eq!(
        starts_of(press()),
        ["\"keyboard\"", "\"virtual\"", "\"mouse\""]
    );
}

#[test]
fn an_optional_without_a_starting_value_starts_at_null_and_every_inner_value() {
    assert_eq!(
        starts_of(optional(press())),
        ["null", "\"keyboard\"", "\"virtual\"", "\"mouse\""]
    );
}

#[test]
fn a_record_without_a_starting_value_starts_at_every_combination_of_its_fields_printed_by_name() {
    let pair = record_shape(vec![("on", boolean()), ("level", range(0, 2))]);
    let mut found = starts_of(pair);
    found.sort();
    assert_eq!(
        found,
        [
            "{level: 0, on: false}",
            "{level: 0, on: true}",
            "{level: 1, on: false}",
            "{level: 1, on: true}",
            "{level: 2, on: false}",
            "{level: 2, on: true}",
        ]
    );
}

#[test]
fn a_field_with_a_starting_value_starts_only_there() {
    assert_eq!(starts_of(starting(boolean(), yes())), ["true"]);
    assert_eq!(starts_of(starting(range(0, 3), int(2))), ["2"]);
    assert_eq!(starts_of(starting(press(), text("mouse"))), ["\"mouse\""]);
    assert_eq!(starts_of(starting(optional(press()), null())), ["null"]);
    assert_eq!(
        starts_of(starting(optional(press()), text("virtual"))),
        ["\"virtual\""]
    );
}

#[test]
fn collections_without_a_starting_value_start_empty() {
    assert_eq!(starts_of(set_shape(range(0, 2))), ["{}"]);
    assert_eq!(starts_of(map_shape(range(0, 2), boolean())), ["{}"]);
    assert_eq!(starts_of(seq_shape(range(0, 2), 3)), ["[]"]);
    assert_eq!(starts_of(queue_shape(range(0, 2), 3)), ["[]"]);
}

#[test]
fn a_collection_with_a_starting_value_starts_there() {
    let numbers = set_shape(range(0, 5));
    assert_eq!(
        starts_of(starting(numbers, set_of(ints(&[2, 1])))),
        ["{1, 2}"]
    );
    let list = seq_shape(range(0, 5), 3);
    assert_eq!(starts_of(starting(list, seq_of(ints(&[2, 1])))), ["[2, 1]"]);
}

#[test]
fn a_lock_starts_free_a_timer_and_a_request_idle_and_a_retry_at_zero() {
    assert_eq!(starts_of(lock_shape(None)), ["false"]);
    assert_eq!(starts_of(lock_shape(Some(press()))), ["null"]);
    assert_eq!(starts_of(shape(ShapeKind::Timer)), ["\"idle\""]);
    assert_eq!(starts_of(shape(ShapeKind::Request)), ["\"idle\""]);
    assert_eq!(starts_of(shape(ShapeKind::Retry { max: 3 })), ["0"]);
}

#[test]
fn each_declaration_has_as_many_values_as_it_declares() {
    let count = |declared: Shape| model_of(declared).fields[0].domain.count();
    assert_eq!(count(boolean()), 2);
    assert_eq!(count(range(0, 3)), 4);
    assert_eq!(count(press()), 3);
    assert_eq!(count(optional(press())), 4);
    assert_eq!(count(set_shape(press())), 8);
    assert_eq!(count(map_shape(boolean(), press())), 16);
    assert_eq!(count(seq_shape(boolean(), 2)), 7);
    assert_eq!(count(queue_shape(one_of(&["a", "b"]), 2)), 7);
    assert_eq!(count(lock_shape(None)), 2);
    assert_eq!(count(lock_shape(Some(press()))), 4);
    assert_eq!(count(shape(ShapeKind::Timer)), 3);
    assert_eq!(count(shape(ShapeKind::Request)), 4);
    assert_eq!(count(shape(ShapeKind::Retry { max: 3 })), 4);
}

#[test]
fn an_enum_that_lists_a_choice_twice_is_refused_at_the_second() {
    let mut twice = one_of(&["a", "b", "a"]);
    if let ShapeKind::OneOf(choices) = &mut twice.kind {
        choices[2].place = HERE;
    }
    let problem = the_problem(&draft(vec![("only", twice)]));
    assert_eq!(problem.place(), HERE, "the second `a`");
    assert_eq!(problem.feature, "the choice \"a\" listed twice");
    assert_eq!(problem.message, "list each value once");
    let numbers = shape(ShapeKind::OneOf(ints(&[1, 2, 1])));
    assert_eq!(
        the_problem(&draft(vec![("only", numbers)])).feature,
        "the choice 1 listed twice"
    );
    assert_eq!(
        starts_of(one_of(&["a", "b"])),
        ["\"a\"", "\"b\""],
        "each value once is fine"
    );
}

#[test]
fn a_starting_value_can_be_computed_from_constants() {
    let mut source = draft(vec![(
        "level",
        starting(range(0, 9), binary(Binary::Add, name("BASE"), int(2))),
    )]);
    source.constants.push(("BASE".to_owned(), int(3)));
    assert_eq!(starts(&lowered(&source)), ["5"]);
}

#[test]
fn a_starting_value_outside_the_declared_range_is_refused_where_it_is_written() {
    let source = draft(vec![("level", starting(range(0, 3), here(int(7))))]);
    let problem = the_problem(&source);
    assert_eq!(problem.file, FILE);
    assert_eq!(problem.place(), HERE);
    assert_eq!(problem.feature, "the starting value 7");
}

#[test]
fn a_starting_value_that_is_not_a_listed_choice_is_refused() {
    let source = draft(vec![("by", starting(press(), here(text("voice"))))]);
    let problem = the_problem(&source);
    assert_eq!(problem.place(), HERE);
    assert_eq!(problem.feature, "the starting value \"voice\"");
}

#[test]
fn a_starting_collection_with_an_item_outside_or_too_many_items_is_refused() {
    let wide = starting(set_shape(range(0, 2)), here(set_of(ints(&[1, 5]))));
    assert_eq!(
        the_problem(&draft(vec![("items", wide)])).feature,
        "the starting value {1, 5}"
    );
    let long = starting(seq_shape(range(0, 2), 2), here(seq_of(ints(&[0, 1, 2]))));
    let problem = the_problem(&draft(vec![("items", long)]));
    assert_eq!(problem.place(), HERE);
    assert_eq!(problem.feature, "the starting value [0, 1, 2]");
}

#[test]
fn a_starting_value_of_the_wrong_kind_is_refused() {
    let source = draft(vec![("level", starting(range(0, 3), here(yes())))]);
    let problem = the_problem(&source);
    assert_eq!(problem.place(), HERE);
    assert_eq!(problem.feature, "a boolean where a number is needed");
}

#[test]
fn a_starting_value_that_can_not_be_computed_is_refused() {
    let negative = binary(Binary::Remainder, int(-7), int(2));
    let source = draft(vec![("level", starting(range(0, 3), here(negative)))]);
    let problem = the_problem(&source);
    assert_eq!(problem.place(), HERE);
    assert_eq!(problem.feature, "a starting value");
    assert_eq!(
        problem.message,
        "takes a remainder with a negative number or a divisor below 1"
    );
}

#[test]
fn exactly_a_million_starting_values_are_all_tried() {
    let model = model_of(range(0, 999_999));
    assert_eq!(model.fields[0].starts.len(), 1_000_000);
}

#[test]
fn more_than_a_million_starting_values_are_refused_at_the_declaration() {
    let mut wide = range(0, 1_000_000);
    wide.place = HERE;
    let problem = the_problem(&draft(vec![("level", wide)]));
    assert_eq!(problem.file, FILE);
    assert_eq!(problem.place(), HERE);
    assert_eq!(
        problem.feature,
        "a declaration with more than a million values"
    );
}

#[test]
fn a_wide_declaration_with_a_starting_value_is_accepted() {
    let wide = starting(range(0, 1_000_000_000), int(0));
    assert_eq!(starts_of(wide), ["0"]);
}

#[test]
fn a_collection_too_large_to_list_still_starts_empty() {
    assert_eq!(starts_of(set_shape(range(0, 200))), ["{}"]);
}
