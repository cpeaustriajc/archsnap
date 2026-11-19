use crate::build::{
    binary, bind, boolean, call, choose, differ, draft, equal, fails, field, helper, helper_call,
    holds, int, ints, lambda, lock_shape, map_of, map_shape, name, negate, no, not, null, one_of,
    prop, queue_shape, range, record, record_shape, rule, seq_of, seq_shape, set_of, set_shape,
    shape, spread, starting, term, text, verdict_of, yes,
};
use earnest_ast::{Binary, Reach, Shape, ShapeKind, Term, TermKind};

fn fields() -> Vec<(&'static str, Shape)> {
    let scores = map_of(vec![(int(1), int(10)), (int(2), int(20))]);
    let point = record_shape(vec![("x", range(0, 5)), ("y", range(0, 5))]);
    vec![
        ("flag", starting(boolean(), yes())),
        ("off", starting(boolean(), no())),
        ("n", starting(range(0, 10), int(7))),
        (
            "cta",
            starting(one_of(&["Add", "Adding", "InCart"]), text("Adding")),
        ),
        (
            "point",
            starting(point, record(vec![("x", int(1)), ("y", int(2))])),
        ),
        (
            "items",
            starting(set_shape(range(0, 9)), set_of(ints(&[1, 2]))),
        ),
        (
            "scores",
            starting(map_shape(range(0, 9), range(0, 99)), scores),
        ),
        (
            "list",
            starting(seq_shape(range(0, 9), 5), seq_of(ints(&[7, 8, 9]))),
        ),
        (
            "inbox",
            starting(queue_shape(range(0, 9), 3), seq_of(ints(&[4, 5]))),
        ),
        ("lock", lock_shape(None)),
        ("owned", lock_shape(Some(one_of(&["a", "b"])))),
        ("timer", shape(ShapeKind::Timer)),
        ("request", shape(ShapeKind::Request)),
        ("retry", shape(ShapeKind::Retry { max: 2 })),
    ]
}

fn is_true(condition: Term) -> bool {
    holds(fields(), condition)
}

fn is_false(condition: Term) -> bool {
    fails(fields(), condition)
}

fn on(known: &str, method: &str, arguments: Vec<Term>) -> Term {
    call(field(known), method, arguments)
}

fn then(target: Term, method: &str) -> Term {
    call(target, method, Vec::new())
}

fn above(limit: i64) -> Term {
    lambda("x", binary(Binary::Greater, name("x"), int(limit)))
}

fn across(reach: Reach, declared: Shape, body: Term) -> Term {
    term(TermKind::Across {
        reach,
        shape: Box::new(declared),
        parameter: "x".to_owned(),
        body: Box::new(body),
    })
}

#[test]
fn booleans_combine_with_and_or_not_and_equality() {
    assert!(is_true(binary(
        Binary::And,
        field("flag"),
        not(field("off"))
    )));
    assert!(is_false(binary(Binary::And, field("flag"), field("off"))));
    assert!(is_true(binary(Binary::Or, field("off"), field("flag"))));
    assert!(is_false(binary(Binary::Or, field("off"), field("off"))));
    assert!(is_false(not(field("flag"))));
    assert!(is_true(equal(field("flag"), yes())));
    assert!(is_true(differ(field("flag"), field("off"))));
}

#[test]
fn numbers_add_subtract_multiply_and_take_remainders() {
    let n = || field("n");
    assert!(is_true(equal(binary(Binary::Add, n(), int(1)), int(8))));
    assert!(is_true(equal(
        binary(Binary::Subtract, n(), int(10)),
        int(-3)
    )));
    assert!(is_true(equal(
        binary(Binary::Multiply, n(), int(2)),
        int(14)
    )));
    assert!(is_true(equal(
        binary(Binary::Remainder, n(), int(4)),
        int(3)
    )));
    assert!(is_true(equal(negate(n()), int(-7))));
    assert!(is_false(equal(binary(Binary::Add, n(), int(1)), int(7))));
}

#[test]
fn numbers_compare_in_all_four_orders() {
    let n = || field("n");
    assert!(is_true(binary(Binary::Less, n(), int(8))));
    assert!(is_false(binary(Binary::Less, n(), int(7))));
    assert!(is_true(binary(Binary::LessOrEqual, n(), int(7))));
    assert!(is_false(binary(Binary::LessOrEqual, n(), int(6))));
    assert!(is_true(binary(Binary::Greater, n(), int(6))));
    assert!(is_false(binary(Binary::Greater, n(), int(7))));
    assert!(is_true(binary(Binary::GreaterOrEqual, n(), int(7))));
    assert!(is_false(binary(Binary::GreaterOrEqual, n(), int(8))));
}

#[test]
fn enum_values_compare_by_name() {
    assert!(is_true(equal(field("cta"), text("Adding"))));
    assert!(is_false(equal(field("cta"), text("Add"))));
    assert!(is_true(differ(field("cta"), text("Add"))));
    assert!(is_false(differ(field("cta"), text("Adding"))));
}

#[test]
fn a_record_gives_its_fields_and_a_spread_replaces_only_the_named_ones() {
    let point = || field("point");
    assert!(is_true(equal(prop(point(), "x"), int(1))));
    assert!(is_true(equal(prop(point(), "y"), int(2))));
    let moved = || spread(Some(point()), vec![("y", int(5))]);
    assert!(is_true(equal(prop(moved(), "y"), int(5))));
    assert!(is_true(equal(prop(moved(), "x"), int(1))));
    let same = record(vec![("y", int(2)), ("x", int(1))]);
    assert!(is_true(equal(point(), same)));
    assert!(is_false(equal(point(), moved())));
}

#[test]
fn a_set_answers_has_size_and_is_empty() {
    assert!(is_true(on("items", "has", ints(&[2]))));
    assert!(is_false(on("items", "has", ints(&[3]))));
    assert!(is_true(equal(prop(field("items"), "size"), int(2))));
    assert!(is_true(equal(on("items", "size", Vec::new()), int(2))));
    assert!(is_false(prop(field("items"), "is_empty")));
    assert!(is_true(prop(set_of(Vec::new()), "is_empty")));
}

#[test]
fn a_set_adds_and_deletes_items() {
    assert!(is_true(equal(
        on("items", "add", ints(&[3])),
        set_of(ints(&[1, 2, 3]))
    )));
    assert!(is_true(equal(
        on("items", "add", ints(&[2])),
        set_of(ints(&[1, 2]))
    )));
    assert!(is_true(equal(
        on("items", "delete", ints(&[1])),
        set_of(ints(&[2]))
    )));
}

#[test]
fn two_sets_combine_and_compare() {
    let other = || vec![set_of(ints(&[2, 3]))];
    assert!(is_true(equal(
        on("items", "union", other()),
        set_of(ints(&[1, 2, 3]))
    )));
    assert!(is_true(equal(
        on("items", "intersect", other()),
        set_of(ints(&[2]))
    )));
    assert!(is_true(equal(
        on("items", "subtract", other()),
        set_of(ints(&[1]))
    )));
    assert!(is_true(on(
        "items",
        "is_subset",
        vec![set_of(ints(&[1, 2, 3]))]
    )));
    assert!(is_false(on("items", "is_subset", vec![set_of(ints(&[1]))])));
}

#[test]
fn a_set_tests_its_items_with_every_some_and_filter() {
    assert!(is_true(on("items", "every", vec![above(0)])));
    assert!(is_false(on("items", "every", vec![above(1)])));
    assert!(is_true(on("items", "some", vec![above(1)])));
    assert!(is_false(on("items", "some", vec![above(2)])));
    assert!(is_true(equal(
        on("items", "filter", vec![above(1)]),
        set_of(ints(&[2]))
    )));
}

#[test]
fn a_map_answers_has_get_size_is_empty_and_keys() {
    assert!(is_true(on("scores", "has", ints(&[1]))));
    assert!(is_false(on("scores", "has", ints(&[3]))));
    assert!(is_true(equal(on("scores", "get", ints(&[1])), int(10))));
    assert!(is_true(equal(on("scores", "get", ints(&[3])), null())));
    assert!(is_true(equal(prop(field("scores"), "size"), int(2))));
    assert!(is_false(prop(field("scores"), "is_empty")));
    assert!(is_true(equal(
        prop(field("scores"), "keys"),
        set_of(ints(&[1, 2]))
    )));
}

#[test]
fn a_map_sets_and_deletes_entries() {
    let added = on("scores", "set", ints(&[3, 30]));
    assert!(is_true(equal(prop(added, "size"), int(3))));
    let replaced = on("scores", "set", ints(&[1, 11]));
    assert!(is_true(equal(call(replaced, "get", ints(&[1])), int(11))));
    let deleted = on("scores", "delete", ints(&[1]));
    assert!(is_false(call(deleted, "has", ints(&[1]))));
    let whole = map_of(vec![(int(2), int(20)), (int(1), int(10))]);
    assert!(is_true(equal(field("scores"), whole)));
}

#[test]
fn a_sequence_answers_size_get_first_last_and_includes() {
    assert!(is_true(equal(prop(field("list"), "size"), int(3))));
    assert!(is_false(prop(field("list"), "is_empty")));
    assert!(is_true(equal(on("list", "get", ints(&[0])), int(7))));
    assert!(is_true(equal(on("list", "get", ints(&[5])), null())));
    assert!(is_true(equal(prop(field("list"), "first"), int(7))));
    assert!(is_true(equal(prop(field("list"), "last"), int(9))));
    assert!(is_true(on("list", "includes", ints(&[8]))));
    assert!(is_false(on("list", "includes", ints(&[1]))));
}

#[test]
fn a_sequence_pushes_shifts_and_pops() {
    assert!(is_true(equal(
        on("list", "push", ints(&[1])),
        seq_of(ints(&[7, 8, 9, 1]))
    )));
    assert!(is_true(equal(
        on("list", "shift", Vec::new()),
        seq_of(ints(&[8, 9]))
    )));
    assert!(is_true(equal(
        on("list", "pop", Vec::new()),
        seq_of(ints(&[7, 8]))
    )));
}

#[test]
fn a_sequence_tests_its_items_with_every_some_and_filter() {
    assert!(is_true(on("list", "every", vec![above(6)])));
    assert!(is_false(on("list", "every", vec![above(7)])));
    assert!(is_true(on("list", "some", vec![above(8)])));
    assert!(is_false(on("list", "some", vec![above(9)])));
    assert!(is_true(equal(
        on("list", "filter", vec![above(7)]),
        seq_of(ints(&[8, 9]))
    )));
}

#[test]
fn forall_and_exists_try_every_value_of_a_declaration() {
    let at_most = |limit: i64| binary(Binary::LessOrEqual, name("x"), int(limit));
    assert!(is_true(across(Reach::Every, range(0, 3), at_most(3))));
    assert!(is_false(across(Reach::Every, range(0, 3), at_most(2))));
    let is_n = || equal(name("x"), field("n"));
    assert!(is_true(across(Reach::Some, range(0, 10), is_n())));
    assert!(is_false(across(Reach::Some, range(0, 6), is_n())));
}

#[test]
fn a_queue_answers_size_is_empty_is_full_head_and_includes() {
    assert!(is_true(equal(prop(field("inbox"), "size"), int(2))));
    assert!(is_false(prop(field("inbox"), "is_empty")));
    assert!(is_false(prop(field("inbox"), "is_full")));
    assert!(is_true(equal(prop(field("inbox"), "head"), int(4))));
    assert!(is_true(on("inbox", "includes", ints(&[5]))));
    assert!(is_false(on("inbox", "includes", ints(&[6]))));
}

#[test]
fn a_queue_sends_at_the_back_receives_from_the_front_and_clears() {
    let sent = || on("inbox", "send", ints(&[6]));
    assert!(is_true(equal(sent(), seq_of(ints(&[4, 5, 6])))));
    assert!(is_true(prop(sent(), "is_full")));
    assert!(is_true(equal(
        on("inbox", "receive", Vec::new()),
        seq_of(ints(&[5]))
    )));
    let cleared = on("inbox", "clear", Vec::new());
    assert!(is_true(prop(cleared, "is_empty")));
    let drained = then(then(field("inbox"), "receive"), "receive");
    assert!(is_true(equal(prop(drained, "head"), null())));
}

#[test]
fn a_queue_tests_its_messages_with_every_and_some() {
    assert!(is_true(on("inbox", "every", vec![above(3)])));
    assert!(is_false(on("inbox", "every", vec![above(4)])));
    assert!(is_true(on("inbox", "some", vec![above(4)])));
    assert!(is_false(on("inbox", "some", vec![above(5)])));
}

#[test]
fn a_lock_without_an_owner_is_free_until_acquired() {
    assert!(is_true(prop(field("lock"), "is_free")));
    assert!(is_false(prop(field("lock"), "is_held")));
    let acquired = || then(field("lock"), "acquire");
    assert!(is_true(prop(acquired(), "is_held")));
    assert!(is_false(prop(acquired(), "is_free")));
    assert!(is_true(prop(then(acquired(), "release"), "is_free")));
}

#[test]
fn a_lock_with_an_owner_knows_who_holds_it() {
    assert!(is_true(prop(field("owned"), "is_free")));
    assert!(is_false(prop(field("owned"), "is_held")));
    assert!(is_true(equal(prop(field("owned"), "holder"), null())));
    let by_a = || on("owned", "acquire", vec![text("a")]);
    assert!(is_true(prop(by_a(), "is_held")));
    assert!(is_true(call(by_a(), "is_held_by", vec![text("a")])));
    assert!(is_false(call(by_a(), "is_held_by", vec![text("b")])));
    assert!(is_true(equal(prop(by_a(), "holder"), text("a"))));
    assert!(is_true(prop(then(by_a(), "release"), "is_free")));
    assert!(is_false(on("owned", "is_held_by", vec![text("a")])));
}

#[test]
fn a_timer_moves_between_idle_running_and_fired() {
    let timer = || field("timer");
    assert!(is_true(prop(timer(), "is_idle")));
    assert!(is_false(prop(timer(), "is_running")));
    assert!(is_false(prop(timer(), "has_fired")));
    assert!(is_true(prop(then(timer(), "start"), "is_running")));
    assert!(is_true(prop(then(timer(), "fire"), "has_fired")));
    assert!(is_true(prop(
        then(then(timer(), "start"), "cancel"),
        "is_idle"
    )));
    assert!(is_true(prop(
        then(then(timer(), "fire"), "reset"),
        "is_idle"
    )));
    assert!(is_false(prop(then(timer(), "start"), "is_idle")));
}

#[test]
fn a_request_moves_between_idle_pending_succeeded_and_failed() {
    let request = || field("request");
    assert!(is_true(prop(request(), "is_idle")));
    assert!(is_false(prop(request(), "is_settled")));
    let sent = || then(request(), "send");
    assert!(is_true(prop(sent(), "is_pending")));
    assert!(is_false(prop(sent(), "is_settled")));
    assert!(is_true(prop(then(sent(), "succeed"), "has_succeeded")));
    assert!(is_true(prop(then(sent(), "succeed"), "is_settled")));
    assert!(is_true(prop(then(sent(), "fail"), "has_failed")));
    assert!(is_true(prop(then(sent(), "fail"), "is_settled")));
    assert!(is_false(prop(then(sent(), "fail"), "has_succeeded")));
    assert!(is_true(prop(
        then(then(sent(), "fail"), "reset"),
        "is_idle"
    )));
}

#[test]
fn a_retry_counts_attempts_up_to_its_max() {
    let retry = || field("retry");
    assert!(is_true(equal(prop(retry(), "attempts"), int(0))));
    assert!(is_true(prop(retry(), "can_retry")));
    assert!(is_false(prop(retry(), "is_exhausted")));
    let once = || then(retry(), "again");
    assert!(is_true(equal(prop(once(), "attempts"), int(1))));
    assert!(is_true(prop(once(), "can_retry")));
    let twice = || then(once(), "again");
    assert!(is_true(prop(twice(), "is_exhausted")));
    assert!(is_false(prop(twice(), "can_retry")));
    assert!(is_true(equal(
        prop(then(twice(), "reset"), "attempts"),
        int(0)
    )));
}

#[test]
fn a_choice_gives_the_branch_its_test_picks() {
    assert!(is_true(equal(
        choose(field("flag"), int(1), int(2)),
        int(1)
    )));
    assert!(is_true(equal(choose(field("off"), int(1), int(2)), int(2))));
}

#[test]
fn a_binding_names_a_value_for_the_rest_of_the_body() {
    let twice = binary(Binary::Multiply, field("n"), int(2));
    let body = equal(name("twice"), int(14));
    assert!(is_true(bind("twice", twice, body)));
}

#[test]
fn a_top_level_constant_is_read_by_name() {
    let mut source = draft(fields());
    source.constants.push(("LIMIT".to_owned(), int(8)));
    source.rules.push(rule(
        "below",
        binary(Binary::Less, field("n"), name("LIMIT")),
    ));
    assert_eq!(verdict_of(&source), Ok(true));
}

#[test]
fn a_helper_function_is_called_with_values_or_with_the_whole_state() {
    let mut source = draft(fields());
    source.helpers.push(helper(
        "double",
        &["x"],
        binary(Binary::Multiply, name("x"), int(2)),
    ));
    source.helpers.push(helper(
        "isBusy",
        &["state"],
        equal(prop(name("state"), "cta"), text("Adding")),
    ));
    let doubled = equal(helper_call("double", vec![field("n")]), int(14));
    let busy = helper_call("isBusy", vec![name("s")]);
    source
        .rules
        .push(rule("both", binary(Binary::And, doubled, busy)));
    assert_eq!(verdict_of(&source), Ok(true));
}

#[test]
fn a_helper_called_twice_keeps_each_call_s_arguments_apart() {
    let mut source = draft(fields());
    source.helpers.push(helper(
        "double",
        &["x"],
        binary(Binary::Multiply, name("x"), int(2)),
    ));
    let nested = helper_call("double", vec![helper_call("double", vec![int(3)])]);
    let sum = binary(
        Binary::Add,
        helper_call("double", vec![int(1)]),
        helper_call("double", vec![int(5)]),
    );
    let both = binary(Binary::And, equal(nested, int(12)), equal(sum, int(12)));
    source.rules.push(rule("apart", both));
    assert_eq!(verdict_of(&source), Ok(true));
}

fn named_like_properties() -> Vec<(&'static str, Shape)> {
    let tray = record_shape(vec![("isEmpty", boolean())]);
    vec![
        ("isFull", starting(boolean(), yes())),
        ("inbox", queue_shape(range(0, 9), 1)),
        ("tray", starting(tray, record(vec![("isEmpty", no())]))),
    ]
}

#[test]
fn a_primitive_property_is_read_by_either_spelling_of_its_name() {
    let reads = |property: &str| prop(field("inbox"), property);
    assert!(holds(named_like_properties(), reads("isEmpty")));
    assert!(holds(named_like_properties(), reads("is_empty")));
    assert!(fails(named_like_properties(), reads("isFull")));
    assert!(fails(named_like_properties(), reads("is_full")));
}

#[test]
fn a_field_named_like_a_primitive_property_is_still_the_field() {
    assert!(
        holds(named_like_properties(), field("isFull")),
        "the state field, which starts true"
    );
    assert!(
        fails(named_like_properties(), prop(field("tray"), "isEmpty")),
        "the record's own field, which is false"
    );
    assert!(
        holds(
            named_like_properties(),
            differ(field("isFull"), prop(field("inbox"), "isFull"))
        ),
        "the field is true while the empty queue is not full"
    );
}
