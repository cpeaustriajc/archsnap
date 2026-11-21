use crate::build::{
    binary, bind, boolean, call, declared, draft, equal, field, helper, helper_call, int, lambda,
    lock_shape, lowered, map_shape, method, method_call, name, negate, null, one_of, optional,
    queue_shape, range, record, refused, rule, rust, seq_of, seq_shape, set_of, set_shape,
    starting, the_problem, typed_range, verdict_of, whole, with_rule, wrap, yes,
};
use earnest_ast::{Binary, Draft, Shape, Term};
use earnest_ir::Fault;
use earnest_value::IntType;

fn fields_with(number: fn(IntType, i64, i64) -> Shape) -> Vec<(&'static str, Shape)> {
    vec![
        ("n", starting(number(IntType::U8, 0, 3), int(0))),
        ("top", starting(number(IntType::U8, 0, 255), int(255))),
        ("wide", starting(number(IntType::U16, 0, 9), int(5))),
        ("low", starting(number(IntType::I8, -128, 127), int(-128))),
        (
            "maybe",
            starting(optional(number(IntType::U8, 0, 3)), int(2)),
        ),
        ("items", seq_shape(boolean(), 2)),
        ("on", starting(boolean(), yes())),
    ]
}

fn fields() -> Vec<(&'static str, Shape)> {
    fields_with(typed_range)
}

fn in_rust(condition: Term) -> Result<bool, Fault> {
    verdict_of(&rust(with_rule(fields(), condition)))
}

fn in_typescript(condition: Term) -> Result<bool, Fault> {
    verdict_of(&with_rule(
        fields_with(|_, lo, hi| range(lo, hi)),
        condition,
    ))
}

fn refused_in_rust(condition: Term) -> String {
    the_problem(&rust(with_rule(fields(), condition))).feature
}

fn outside(ty: IntType) -> Result<bool, Fault> {
    Err(Fault::Outside(ty))
}

fn less(left: Term, right: Term) -> Term {
    binary(Binary::Less, left, right)
}

fn minus(left: Term, right: Term) -> Term {
    binary(Binary::Subtract, left, right)
}

fn plus(left: Term, right: Term) -> Term {
    binary(Binary::Add, left, right)
}

fn times(left: Term, right: Term) -> Term {
    binary(Binary::Multiply, left, right)
}

#[test]
fn a_difference_below_zero_of_a_type_without_a_sign_is_a_fault_in_rust_only() {
    let one_below = || less(minus(field("n"), int(1)), int(3));
    assert_eq!(
        in_rust(one_below()),
        outside(IntType::U8),
        "0 - 1 as a `u8`"
    );
    assert_eq!(in_typescript(one_below()), Ok(true), "-1 is below 3");
    let none_below = less(minus(field("n"), int(0)), int(3));
    assert_eq!(in_rust(none_below), Ok(true), "0 - 0 is a `u8`");
}

#[test]
fn a_sum_or_a_product_above_the_largest_number_of_its_type_is_a_fault() {
    let above = |sum: Term| binary(Binary::Greater, sum, int(0));
    assert_eq!(
        in_rust(above(plus(field("top"), int(1)))),
        outside(IntType::U8),
        "255 + 1 as a `u8`"
    );
    assert_eq!(
        in_rust(above(times(field("top"), int(2)))),
        outside(IntType::U8),
        "255 * 2 as a `u8`"
    );
    assert_eq!(
        in_rust(above(plus(field("wide"), int(250)))),
        Ok(true),
        "5 + 250 is a `u16`"
    );
    assert_eq!(
        in_typescript(above(plus(field("top"), int(1)))),
        Ok(true),
        "256 is a number like any other"
    );
}

#[test]
fn a_type_with_a_sign_has_both_ends() {
    assert_eq!(
        in_rust(less(minus(field("low"), int(1)), int(0))),
        outside(IntType::I8),
        "-128 - 1 as an `i8`"
    );
    assert_eq!(
        in_rust(less(negate(field("low")), int(0))),
        outside(IntType::I8),
        "the opposite of -128 is 128, which is no `i8`"
    );
    assert_eq!(
        in_rust(less(plus(field("low"), int(1)), int(0))),
        Ok(true),
        "-127 is an `i8`"
    );
}

#[test]
fn numbers_written_out_take_the_type_of_the_number_they_meet() {
    let literal_first = less(plus(minus(int(1), int(2)), field("n")), int(3));
    assert_eq!(
        in_rust(literal_first),
        outside(IntType::U8),
        "`1 - 2` is computed as a `u8` because it is added to one"
    );
    let chosen = less(
        minus(crate::build::choose(field("on"), int(1), int(2)), int(2)),
        field("n"),
    );
    assert_eq!(
        in_rust(chosen),
        outside(IntType::U8),
        "`1 - 2` is a `u8` because it is compared with one"
    );
    let fits = less(plus(minus(int(2), int(1)), field("n")), int(3));
    assert_eq!(in_rust(fits), Ok(true), "1 + 0 is below 3");
}

#[test]
fn numbers_nothing_gives_a_type_are_thirty_two_bits_in_rust() {
    let below_zero = || less(minus(int(1), int(2)), int(0));
    assert_eq!(in_rust(below_zero()), Ok(true), "-1 is an `i32`");
    let largest = i64::from(i32::MAX);
    let above = || binary(Binary::Greater, plus(int(largest), int(1)), int(0));
    assert_eq!(in_rust(above()), outside(IntType::I32), "one past an `i32`");
    assert_eq!(in_typescript(above()), Ok(true), "64 bits hold it");
    let same = equal(plus(int(largest), int(1)), int(0));
    assert_eq!(
        in_rust(same),
        outside(IntType::I32),
        "`==` gives the type too"
    );
}

#[test]
fn a_size_is_a_usize_in_rust() {
    let size = || call(field("items"), "size", Vec::new());
    let one_fewer = || less(minus(size(), int(1)), int(2));
    assert_eq!(
        in_rust(one_fewer()),
        outside(IntType::Usize),
        "no item less one is below zero"
    );
    assert_eq!(in_typescript(one_fewer()), Ok(true), "-1 is below 2");
    assert_eq!(
        refused_in_rust(equal(size(), field("n"))),
        "a `usize` number with a `u8` number"
    );
    assert_eq!(
        refused_in_rust(equal(call(field("items"), "get", vec![field("n")]), null())),
        "a `u8` number with a `usize` number",
        "a position is a `usize`"
    );
}

#[test]
fn two_integer_types_do_not_mix_in_arithmetic_order_equality_or_a_choice() {
    let mixed = "a `u8` number with a `u16` number";
    assert_eq!(
        refused_in_rust(less(plus(field("n"), field("wide")), int(9))),
        mixed
    );
    assert_eq!(refused_in_rust(less(field("n"), field("wide"))), mixed);
    assert_eq!(refused_in_rust(equal(field("n"), field("wide"))), mixed);
    assert_eq!(
        refused_in_rust(less(
            crate::build::choose(field("on"), field("n"), field("wide")),
            int(9)
        )),
        mixed
    );
    assert_eq!(
        refused_in_rust(equal(field("maybe"), wrap(field("wide")))),
        mixed,
        "also inside an optional value"
    );
}

#[test]
fn a_binding_must_state_the_type_of_a_number_in_rust() {
    let bound = |value: Term| bind("k", value, less(field("n"), name("k")));
    assert_eq!(
        refused_in_rust(bound(int(3))),
        "`let k` with a number of no stated type"
    );
    assert_eq!(
        refused_in_rust(bind(
            "all",
            set_of(vec![int(1), int(2)]),
            call(name("all"), "has", vec![field("n")])
        )),
        "`let all` with a number of no stated type",
        "also a number inside a collection"
    );
    assert_eq!(in_typescript(bound(int(3))), Ok(true), "0 is below 3");
    assert_eq!(
        in_rust(bound(declared(whole(IntType::U8), int(3)))),
        Ok(true),
        "`let k: u8 = 3`"
    );
    assert_eq!(
        in_rust(bound(plus(field("n"), int(3)))),
        Ok(true),
        "a number computed from a field has the field's type"
    );
    assert_eq!(
        in_rust(bind("flag", yes(), name("flag"))),
        Ok(true),
        "a binding without a number needs no type"
    );
}

#[test]
fn a_declared_type_gives_its_value_the_type_and_checks_it() {
    let small = |value: Term| less(declared(whole(IntType::U8), value), int(9));
    assert_eq!(in_rust(small(int(3))), Ok(true), "3 is a `u8`");
    assert_eq!(in_rust(small(int(300))), outside(IntType::U8), "300 is not");
    assert_eq!(
        in_rust(small(minus(int(1), int(2)))),
        outside(IntType::U8),
        "`1 - 2` is computed as the declared type"
    );
    assert_eq!(
        refused_in_rust(small(yes())),
        "a boolean where a number is needed"
    );
    assert_eq!(
        refused_in_rust(small(field("wide"))),
        "a `u16` number with a `u8` number"
    );
    let present = equal(
        declared(optional(whole(IntType::U8)), wrap(minus(int(1), int(2)))),
        field("maybe"),
    );
    assert_eq!(
        in_rust(present),
        outside(IntType::U8),
        "also the number inside `Some`"
    );
}

fn with_helpers(condition: Term) -> Draft {
    let mut source = rust(with_rule(fields(), condition));
    let number = || declared(whole(IntType::U8), name("number"));
    source.helpers.push(helper(
        "double",
        &["number"],
        bind(
            "number",
            number(),
            declared(whole(IntType::U8), plus(name("number"), name("number"))),
        ),
    ));
    source
        .helpers
        .push(helper("pick", &["first", "second"], name("second")));
    source.helpers.push(method(
        "pick",
        &["self", "second"],
        crate::build::not(name("second")),
    ));
    source
        .helpers
        .push(helper("echo", &["given"], name("given")));
    source.helpers.push(method(
        "echo",
        &["self", "given"],
        helper_call("echo", vec![crate::build::not(name("given"))]),
    ));
    source
}

#[test]
fn a_helper_computes_in_the_type_of_its_parameter() {
    let doubled = |given: i64| less(helper_call("double", vec![int(given)]), int(9));
    assert_eq!(
        verdict_of(&with_helpers(doubled(4))),
        Ok(true),
        "8 is below 9"
    );
    assert_eq!(
        verdict_of(&with_helpers(doubled(100))),
        Ok(false),
        "200 is a `u8`"
    );
    assert_eq!(
        verdict_of(&with_helpers(doubled(200))),
        outside(IntType::U8),
        "200 + 200 is not"
    );
    assert_eq!(
        verdict_of(&with_helpers(doubled(300))),
        outside(IntType::U8),
        "300 is no `u8` to begin with"
    );
}

#[test]
fn a_function_and_a_method_of_one_name_are_each_called_by_their_kind() {
    let state = || name("s");
    assert_eq!(
        verdict_of(&with_helpers(helper_call("pick", vec![yes(), yes()]))),
        Ok(true),
        "the function gives its second argument"
    );
    assert_eq!(
        verdict_of(&with_helpers(method_call("pick", vec![state(), yes()]))),
        Ok(false),
        "the method gives the opposite of its argument"
    );
    assert_eq!(
        verdict_of(&with_helpers(method_call("echo", vec![state(), yes()]))),
        Ok(false),
        "a method that calls the function of its own name is no recursion: the function \
         gives back the `false` the method hands it"
    );
    let missing = the_problem(&with_helpers(method_call("double", vec![state(), int(1)])));
    assert_eq!(
        missing.feature, "a call to `double`",
        "`double` is a function, and no method has the name"
    );
}

fn declaring(only: Shape) -> Draft {
    draft(vec![("only", only)])
}

fn refused_shape(source: &Draft) -> String {
    the_problem(source).feature
}

#[test]
fn an_optional_value_in_a_map_a_sequence_or_a_queue_is_refused_in_rust_only() {
    let maybe = || optional(boolean());
    let keys = || one_of(&["a", "b"]);
    assert_eq!(
        refused_shape(&rust(declaring(map_shape(keys(), maybe())))),
        "an optional value as a map value"
    );
    assert_eq!(
        refused_shape(&rust(declaring(seq_shape(maybe(), 1)))),
        "an optional value as a sequence item"
    );
    assert_eq!(
        refused_shape(&rust(declaring(queue_shape(maybe(), 1)))),
        "an optional value as a queue item"
    );
    for kept in [
        map_shape(keys(), maybe()),
        seq_shape(maybe(), 1),
        queue_shape(maybe(), 1),
    ] {
        assert_eq!(
            lowered(&declaring(kept)).fields.len(),
            1,
            "TypeScript has one `null`, so the same declaration is read"
        );
    }
    for kept in [set_shape(maybe()), map_shape(maybe(), boolean())] {
        assert_eq!(
            lowered(&rust(declaring(kept))).fields.len(),
            1,
            "a set of optional values and a map keyed by them give no optional of an optional"
        );
    }
}

#[test]
fn a_lock_with_an_owner_in_a_map_a_sequence_or_a_queue_is_refused_in_both_languages() {
    let owned = || lock_shape(Some(one_of(&["ada", "bo"])));
    let keys = || one_of(&["a", "b"]);
    let languages: [fn(Draft) -> Draft; 2] = [rust, std::convert::identity];
    for language in languages {
        assert_eq!(
            refused_shape(&language(declaring(map_shape(keys(), owned())))),
            "a lock with an owner as a map value"
        );
        assert_eq!(
            refused_shape(&language(declaring(seq_shape(owned(), 1)))),
            "a lock with an owner as a sequence item"
        );
        assert_eq!(
            refused_shape(&language(declaring(queue_shape(owned(), 1)))),
            "a lock with an owner as a queue item"
        );
        assert_eq!(
            lowered(&language(declaring(map_shape(keys(), lock_shape(None)))))
                .fields
                .len(),
            1,
            "a lock without an owner is never `null`"
        );
        assert_eq!(
            lowered(&language(declaring(set_shape(owned()))))
                .fields
                .len(),
            1,
            "nothing reads one lock out of a set"
        );
    }
}

#[test]
fn an_optional_read_from_a_written_collection_of_optionals_is_refused_in_rust_only() {
    let gaps = || seq_of(vec![null(), wrap(yes())]);
    let first_is_absent = || equal(call(gaps(), "first", Vec::new()), null());
    assert_eq!(
        refused_in_rust(first_is_absent()),
        "`first` on a collection of optional values",
        "in Rust it is `Some(None)`, which is not `None`"
    );
    assert_eq!(
        in_typescript(first_is_absent()),
        Ok(true),
        "in TypeScript the first item is `null`"
    );
    let every_is_absent = call(
        gaps(),
        "every",
        vec![lambda("item", equal(name("item"), null()))],
    );
    assert_eq!(
        in_rust(every_is_absent),
        Ok(false),
        "a test of each item reads one level, and the second item is present"
    );
    assert_eq!(
        refused_in_rust(equal(wrap(null()), null())),
        "an optional value inside an optional value",
        "`Some(None)` can't be written either"
    );
}

#[test]
fn a_starting_value_outside_the_integer_type_is_refused() {
    let source = rust(draft(vec![(
        "n",
        starting(typed_range(IntType::U8, 0, 3), minus(int(1), int(2))),
    )]));
    let problems = refused(&source);
    assert_eq!(problems.len(), 1, "one problem: {problems:?}");
    assert_eq!(problems[0].feature, "a starting value");
    assert_eq!(problems[0].message, "computes a number outside `u8`");
}

#[test]
fn an_action_computes_its_new_values_in_the_type_of_their_field() {
    let mut source = rust(draft(vec![(
        "n",
        starting(typed_range(IntType::U8, 0, 3), int(0)),
    )]));
    source.actions.push(crate::build::action(
        "down",
        None,
        record(vec![("n", minus(int(0), int(1)))]),
    ));
    source.rules.push(rule("fine", yes()));
    let model = lowered(&source);
    let update = model.actions[0].updates[0]
        .as_ref()
        .expect("the action sets `n`");
    let state = crate::build::first_state(&model);
    let mut frame = earnest_ir::Frame::new(&state, model.actions[0].slots);
    assert_eq!(
        update.eval(&mut frame),
        Err(Fault::Outside(IntType::U8)),
        "`0 - 1` is computed as the `u8` the field is"
    );
}

const LAST_EXACT: i64 = 9_007_199_254_740_991;

fn above_zero(sum: Term) -> Term {
    binary(Binary::Greater, sum, int(0))
}

#[test]
fn a_typescript_number_computed_past_the_safe_integers_is_a_fault() {
    assert_eq!(
        in_typescript(above_zero(plus(int(LAST_EXACT - 1), int(1)))),
        Ok(true),
        "2^53 - 1 is exact"
    );
    assert_eq!(
        in_typescript(above_zero(plus(int(LAST_EXACT), int(1)))),
        Err(Fault::Overflow),
        "2^53 is where JavaScript starts to round"
    );
    assert_eq!(
        in_typescript(above_zero(minus(negate(int(LAST_EXACT)), int(1)))),
        Err(Fault::Overflow),
        "-2^53 too"
    );
    assert_eq!(
        in_typescript(above_zero(times(int(94_906_266), int(94_906_266)))),
        Err(Fault::Overflow),
        "94906266 squared is 9007199326062756, past 2^53, though it fits in 64 bits"
    );
    assert_eq!(
        in_typescript(above_zero(minus(plus(int(LAST_EXACT), int(1)), int(1)))),
        Err(Fault::Overflow),
        "a sum past 2^53 faults even when a later step brings it back"
    );
}

#[test]
fn a_rust_number_keeps_its_integer_type_and_the_64_bits_below_it() {
    let wide = || declared(whole(IntType::I64), int(LAST_EXACT));
    assert_eq!(
        in_rust(above_zero(plus(wide(), int(1)))),
        Ok(true),
        "an `i64` holds 2^53 and the checker adds exactly"
    );
}
