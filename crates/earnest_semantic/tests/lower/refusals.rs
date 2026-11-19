use crate::build::{
    FILE, HERE, action, binary, boolean, call, choose, draft, equal, field, helper, helper_call,
    here, int, ints, lambda, lock_shape, map_shape, name, not, one_of, optional, prop, queue_shape,
    range, record, record_shape, refusal, refused, rule, seq_shape, set_shape, shape, spread,
    starting, term, text, the_problem, verdict_of, with_rule, yes,
};
use earnest_ast::{Binary, Draft, Reach, Shape, ShapeKind, TemplatePiece, Term, TermKind};

fn fields() -> Vec<(&'static str, Shape)> {
    vec![
        ("n", starting(range(0, 10), int(7))),
        ("flag", starting(boolean(), yes())),
        ("maybe", starting(optional(range(0, 5)), int(2))),
        (
            "point",
            starting(
                record_shape(vec![("x", range(0, 5))]),
                record(vec![("x", int(1))]),
            ),
        ),
        ("items", set_shape(range(0, 9))),
        ("scores", map_shape(range(0, 9), range(0, 9))),
        ("list", seq_shape(range(0, 9), 3)),
        ("inbox", queue_shape(range(0, 9), 3)),
        ("lock", lock_shape(None)),
        ("owned", lock_shape(Some(one_of(&["a", "b"])))),
        ("timer", shape(ShapeKind::Timer)),
        ("request", shape(ShapeKind::Request)),
        ("retry", shape(ShapeKind::Retry { max: 2 })),
    ]
}

fn refused_here(condition: Term) -> String {
    let problem = refusal(fields(), condition);
    assert_eq!(problem.file, FILE, "the problem names the draft's file");
    assert_eq!(problem.place(), HERE, "the problem is where the term is");
    problem.feature
}

fn refused_draft_here(source: &Draft) -> String {
    let problem = the_problem(source);
    assert_eq!(problem.file, FILE, "the problem names the draft's file");
    assert_eq!(problem.place(), HERE, "the problem is where the term is");
    problem.feature
}

fn declared(mut refused: Shape) -> String {
    refused.place = HERE;
    refused_draft_here(&draft(vec![("only", refused)]))
}

fn unknown_method(known: &str, method: &str) -> String {
    refused_here(here(call(field(known), method, Vec::new())))
}

#[test]
fn a_name_the_file_does_not_define_is_refused() {
    let problem = refusal(fields(), equal(here(name("missing")), int(1)));
    assert_eq!(problem.place(), HERE, "the problem is where the name is");
    assert_eq!(
        problem.feature, "`missing`",
        "the feature is the name alone, in backticks"
    );
    assert_eq!(
        problem.message,
        "the model file does not define this name, so the checker can't know its value",
        "the sentence says what is wrong with the name"
    );
}

#[test]
fn a_state_field_the_model_does_not_declare_is_refused() {
    let feature = refused_here(equal(here(field("total")), int(1)));
    assert_eq!(feature, "the state field `total`");
}

#[test]
fn a_record_field_the_record_does_not_declare_is_refused() {
    let feature = refused_here(equal(here(prop(field("point"), "z")), int(1)));
    assert_eq!(feature, "the field `z`");
}

#[test]
fn a_call_to_a_function_the_file_does_not_define_is_refused() {
    let feature = refused_here(here(helper_call("fetchCart", Vec::new())));
    assert_eq!(feature, "a call to `fetchCart`");
}

#[test]
fn a_method_that_is_not_a_primitive_of_its_value_is_refused() {
    assert_eq!(unknown_method("items", "sort"), "`sort` on a set");
    assert_eq!(unknown_method("scores", "push"), "`push` on a map");
    assert_eq!(unknown_method("list", "add"), "`add` on a sequence");
    assert_eq!(unknown_method("inbox", "pop"), "`pop` on a queue");
    assert_eq!(unknown_method("lock", "holder"), "`holder` on a lock");
    assert_eq!(unknown_method("owned", "steal"), "`steal` on a lock");
    assert_eq!(unknown_method("timer", "pause"), "`pause` on a timer");
    assert_eq!(unknown_method("request", "retry"), "`retry` on a request");
    assert_eq!(
        unknown_method("retry", "attempt"),
        "`attempt` on a retry counter"
    );
    assert_eq!(unknown_method("n", "size"), "`size` on a number");
    assert_eq!(unknown_method("flag", "toggle"), "`toggle` on a boolean");
    assert_eq!(unknown_method("point", "size"), "`size` on a record");
    assert_eq!(unknown_method("maybe", "size"), "`size` on a number");
}

#[test]
fn an_unknown_method_is_named_before_its_arguments_are_counted() {
    let feature = refused_here(here(call(field("timer"), "pause", ints(&[1]))));
    assert_eq!(feature, "`pause` on a timer");
}

#[test]
fn a_primitive_with_the_wrong_number_of_arguments_is_refused() {
    let none = refused_here(here(call(field("items"), "has", Vec::new())));
    assert_eq!(none, "`has` with 0 arguments");
    let one = refused_here(equal(
        here(call(field("items"), "size", ints(&[1]))),
        int(1),
    ));
    assert_eq!(one, "`size` with 1 argument");
    let two = refused_here(here(call(field("timer"), "start", ints(&[1, 2]))));
    assert_eq!(two, "`start` with 2 arguments");
}

#[test]
fn a_value_of_the_wrong_kind_for_an_operator_is_refused() {
    let and = refused_here(binary(Binary::And, here(field("n")), yes()));
    assert_eq!(and, "a number where a boolean is needed");
    let negated = refused_here(not(here(int(5))));
    assert_eq!(negated, "a number where a boolean is needed");
    let sum = binary(Binary::Add, int(1), here(yes()));
    assert_eq!(
        refused_here(equal(sum, int(2))),
        "a boolean where a number is needed"
    );
    let ordered = refused_here(binary(Binary::Less, here(text("a")), int(2)));
    assert_eq!(ordered, "a text value where a number is needed");
}

#[test]
fn a_rule_that_is_not_true_or_false_is_refused() {
    assert_eq!(
        refused_here(here(field("n"))),
        "a number where a boolean is needed"
    );
}

#[test]
fn an_argument_of_the_wrong_kind_is_refused() {
    let has = call(field("items"), "has", vec![here(text("one"))]);
    assert_eq!(refused_here(has), "a text value where a number is needed");
}

#[test]
fn comparing_values_of_different_kinds_is_refused() {
    let feature = refused_here(here(equal(field("n"), text("seven"))));
    assert_eq!(feature, "a number compared with a text value");
}

#[test]
fn a_choice_between_values_of_different_kinds_is_refused() {
    let mixed = here(choose(field("flag"), int(1), text("one")));
    assert_eq!(
        refused_here(equal(mixed, int(1))),
        "a choice between a number and a text value"
    );
}

#[test]
fn a_helper_that_calls_itself_is_refused() {
    let mut source = with_rule(fields(), helper_call("loops", ints(&[1])));
    let again = here(helper_call("loops", vec![name("x")]));
    source.helpers.push(helper("loops", &["x"], again));
    assert_eq!(refused_draft_here(&source), "recursion in `loops`");
}

#[test]
fn helpers_that_call_each_other_in_a_circle_are_refused() {
    let mut source = with_rule(fields(), helper_call("ping", ints(&[1])));
    let forth = helper_call("pong", vec![name("x")]);
    let back = here(helper_call("ping", vec![name("x")]));
    source.helpers.push(helper("ping", &["x"], forth));
    source.helpers.push(helper("pong", &["x"], back));
    assert_eq!(refused_draft_here(&source), "recursion in `ping`");
}

#[test]
fn a_helper_called_from_two_places_is_not_recursion() {
    let mut source = with_rule(
        fields(),
        binary(
            Binary::And,
            helper_call("positive", vec![field("n")]),
            helper_call("positive", ints(&[1])),
        ),
    );
    let body = binary(Binary::Greater, name("x"), int(0));
    source.helpers.push(helper("positive", &["x"], body));
    assert_eq!(verdict_of(&source), Ok(true));
}

#[test]
fn a_constant_defined_by_itself_is_refused() {
    let mut source = with_rule(fields(), equal(name("LIMIT"), int(1)));
    source
        .constants
        .push(("LIMIT".to_owned(), here(name("LIMIT"))));
    assert_eq!(refused_draft_here(&source), "recursion in `LIMIT`");
}

#[test]
fn a_helper_called_with_the_wrong_number_of_arguments_is_refused() {
    let mut source = with_rule(fields(), here(helper_call("positive", Vec::new())));
    let body = binary(Binary::Greater, name("x"), int(0));
    source.helpers.push(helper("positive", &["x"], body));
    assert_eq!(refused_draft_here(&source), "a call to `positive`");
}

#[test]
fn a_function_used_as_a_value_is_refused() {
    assert_eq!(
        refused_here(here(lambda("x", yes()))),
        "a function used as a value"
    );
}

#[test]
fn a_test_without_a_one_parameter_function_is_refused() {
    let every = here(call(field("items"), "every", ints(&[1])));
    assert_eq!(
        refused_here(every),
        "`every` without a one-parameter function"
    );
}

#[test]
fn a_template_string_outside_a_view_is_refused() {
    let template = term(TermKind::Template(vec![TemplatePiece::Text(
        "n".to_owned(),
    )]));
    assert_eq!(
        refused_here(equal(here(template), text("n"))),
        "a template string"
    );
}

#[test]
fn the_whole_state_used_as_a_value_is_refused() {
    assert_eq!(
        refused_here(equal(here(name("s")), int(1))),
        "the whole state `s` used as a value"
    );
}

#[test]
fn the_whole_input_used_as_a_value_is_refused() {
    let mut source = draft(fields());
    let guard = equal(here(name("input")), int(1));
    let mut counted = action("count", Some(guard), record(vec![("n", int(1))]));
    counted.inputs.push(("amount".to_owned(), range(0, 2)));
    counted.input_name = Some("input".to_owned());
    source.actions.push(counted);
    assert_eq!(
        refused_draft_here(&source),
        "the whole input `input` used as a value"
    );
}

#[test]
fn a_spread_of_something_that_is_not_a_record_is_refused() {
    let spread_number = here(spread(Some(field("n")), vec![("x", int(1))]));
    assert_eq!(
        refused_here(equal(spread_number, field("point"))),
        "a spread of a number"
    );
}

#[test]
fn a_number_range_with_its_bounds_the_wrong_way_round_is_refused() {
    assert_eq!(declared(range(3, 0)), "a number range from 3 to 0");
}

#[test]
fn a_list_of_choices_must_hold_plain_values_of_one_kind() {
    assert_eq!(declared(one_of(&[])), "an empty list of choices");
    let mixed = shape(ShapeKind::OneOf(vec![text("a"), here(int(1))]));
    assert_eq!(
        refused_draft_here(&draft(vec![("only", mixed)])),
        "choices of different kinds"
    );
    let computed = shape(ShapeKind::OneOf(vec![here(name("A"))]));
    assert_eq!(
        refused_draft_here(&draft(vec![("only", computed)])),
        "a choice that is not a plain value"
    );
}

#[test]
fn an_optional_declaration_can_not_hold_another_optional_one() {
    assert_eq!(
        declared(optional(optional(boolean()))),
        "an optional value inside an optional value"
    );
}

#[test]
fn a_lock_can_not_be_owned_by_an_optional_value() {
    assert_eq!(
        declared(lock_shape(Some(optional(boolean())))),
        "a lock owned by an optional value"
    );
}

#[test]
fn a_retry_counter_can_not_have_a_max_below_zero() {
    assert_eq!(
        declared(shape(ShapeKind::Retry { max: -1 })),
        "a retry counter with a max of -1"
    );
}

#[test]
fn a_choice_over_more_than_a_million_values_is_refused_at_the_declaration() {
    let mut wide = range(0, 1_000_000);
    wide.place = HERE;
    let over_all = term(TermKind::Across {
        reach: Reach::Every,
        shape: Box::new(wide),
        parameter: "x".to_owned(),
        body: Box::new(yes()),
    });
    assert_eq!(
        refused_here(over_all),
        "a declaration with more than a million values"
    );
}

#[test]
fn a_broken_rule_does_not_hide_a_broken_declaration() {
    let mut source = with_rule(fields(), equal(here(name("missing")), int(1)));
    source.rules.push(rule("other", here(name("absent"))));
    source.fields[0].shape = range(3, 0);
    let features: Vec<String> = refused(&source)
        .into_iter()
        .map(|problem| problem.feature)
        .collect();
    assert_eq!(
        features,
        ["a number range from 3 to 0", "`missing`", "`absent`",]
    );
}
