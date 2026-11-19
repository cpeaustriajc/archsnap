use crate::build::{SMALL, Sketch, boxed, helpers_of, int, null, one, two};
use earnest_ir::{Expr, MapOp, Math, Op, Order, SeqOp, SetOp};
use earnest_value::{Domain, Value};

pub(crate) struct Stage {
    pub(crate) sketch: Sketch,
    pub(crate) n: Expr,
    pub(crate) flag: Expr,
    pub(crate) maybe: Expr,
    pub(crate) seen: Expr,
    pub(crate) book: Expr,
    pub(crate) loose: Expr,
    pub(crate) line: Expr,
    pub(crate) gaps: Expr,
}

pub(crate) fn stage() -> Stage {
    let mut sketch = Sketch::new("Stage");
    let n = sketch.number("n", 0, 3, 0);
    let flag = sketch.field("flag", Domain::Bool, vec![Value::Bool(false)]);
    let maybe = sketch.field("maybe", Domain::Optional(boxed(SMALL)), vec![Value::Null]);
    let seen = sketch.field("seen", Domain::Set(boxed(SMALL)), Vec::new());
    let keys = sketch.one_of(&["a", "b"]);
    let book = sketch.field(
        "book",
        Domain::Map(boxed(keys.clone()), boxed(SMALL)),
        Vec::new(),
    );
    let loose = sketch.field(
        "loose",
        Domain::Map(boxed(keys), boxed(Domain::Optional(boxed(SMALL)))),
        Vec::new(),
    );
    let line = sketch.field(
        "line",
        Domain::Seq {
            item: boxed(SMALL),
            max: 2,
        },
        Vec::new(),
    );
    let gaps = sketch.field(
        "gaps",
        Domain::Seq {
            item: boxed(Domain::Optional(boxed(SMALL))),
            max: 2,
        },
        Vec::new(),
    );
    Stage {
        sketch,
        n,
        flag,
        maybe,
        seen,
        book,
        loose,
        line,
        gaps,
    }
}

fn a_key(stage: &mut Stage) -> Expr {
    Expr::Const(stage.sketch.text("a"))
}

fn some_numbers() -> Expr {
    Expr::SetOf(vec![int(1), int(2)])
}

#[test]
fn not_is_the_negation_sign() {
    let stage = stage();
    assert_eq!(stage.sketch.shown(one(Op::Not, &stage.flag)), "(~flag)");
}

#[test]
fn negate_is_the_minus_sign() {
    let stage = stage();
    assert_eq!(stage.sketch.shown(one(Op::Negate, &stage.n)), "(-n)");
}

#[test]
fn a_present_optional_is_a_tuple_of_the_value() {
    let stage = stage();
    assert_eq!(stage.sketch.shown(one(Op::SomeOf, &stage.n)), "<<n>>");
}

#[test]
fn unwrapping_reads_the_first_item_of_the_tuple() {
    let stage = stage();
    assert_eq!(
        stage.sketch.shown(one(Op::Unwrap, &stage.maybe)),
        "maybe[1]"
    );
}

#[test]
fn equal_and_differ_are_the_two_equality_signs() {
    let first = stage();
    assert_eq!(
        first.sketch.shown(two(Op::Equal, &first.n, int(1))),
        "(n = 1)"
    );
    let second = stage();
    assert_eq!(
        second.sketch.shown(two(Op::Differ, &second.n, int(1))),
        "(n # 1)"
    );
}

#[test]
fn an_optional_is_compared_with_the_empty_tuple_or_a_wrapped_value() {
    let first = stage();
    assert_eq!(
        first.sketch.shown(two(Op::Differ, &first.maybe, null())),
        "(maybe # <<>>)"
    );
    let second = stage();
    let present = one(Op::SomeOf, &int(1));
    assert_eq!(
        second.sketch.shown(two(Op::Equal, &second.maybe, present)),
        "(maybe = <<1>>)"
    );
}

#[test]
fn each_order_has_its_sign() {
    let signs = [
        (Order::Less, "(n < 1)"),
        (Order::LessOrEqual, "(n <= 1)"),
        (Order::Greater, "(n > 1)"),
        (Order::GreaterOrEqual, "(n >= 1)"),
    ];
    for (order, expected) in signs {
        let stage = stage();
        assert_eq!(
            stage.sketch.shown(two(Op::Order(order), &stage.n, int(1))),
            expected
        );
    }
}

#[test]
fn sum_difference_and_product_have_their_signs() {
    let signs = [
        (Math::Add, "(n + 2)"),
        (Math::Subtract, "(n - 2)"),
        (Math::Multiply, "(n * 2)"),
    ];
    for (math, expected) in signs {
        let stage = stage();
        assert_eq!(
            stage.sketch.shown(two(Op::Math(math), &stage.n, int(2))),
            expected
        );
    }
}

#[test]
fn a_remainder_goes_through_the_helper_that_faults_where_the_engine_does() {
    let mut stage = stage();
    stage
        .sketch
        .rule("odd", two(Op::Math(Math::Remainder), &stage.n, int(2)));
    let module = stage.sketch.written().module;
    assert!(
        module.contains("    InRange => ((~((n >= 0) /\\ (2 >= 1))) \\/ Remainder(n, 2))\n"),
        "{module}"
    );
    assert_eq!(
        helpers_of(&module),
        [
            "Fault(message) == Assert(FALSE, message)",
            "Remainder(left, right) == IF (left >= 0) /\\ (right >= 1) THEN left % right \
             ELSE Fault(\"a remainder with a negative number or a divisor below 1\")",
        ]
    );
}

#[test]
fn set_primitives_are_the_set_operators() {
    let cases: [(SetOp, Expr, &str); 7] = [
        (SetOp::Has, int(1), "(1 \\in seen)"),
        (SetOp::Add, int(1), "(seen \\cup {1})"),
        (SetOp::Delete, int(1), "(seen \\ {1})"),
        (SetOp::Union, some_numbers(), "(seen \\cup {1, 2})"),
        (SetOp::Intersect, some_numbers(), "(seen \\cap {1, 2})"),
        (SetOp::Subtract, some_numbers(), "(seen \\ {1, 2})"),
        (SetOp::IsSubset, some_numbers(), "(seen \\subseteq {1, 2})"),
    ];
    for (op, other, expected) in cases {
        let stage = stage();
        assert_eq!(
            stage.sketch.shown(two(Op::Set(op), &stage.seen, other)),
            expected
        );
    }
}

#[test]
fn the_size_of_a_set_is_its_cardinality() {
    let stage = stage();
    assert_eq!(
        stage.sketch.shown(one(Op::Set(SetOp::Size), &stage.seen)),
        "Cardinality(seen)"
    );
}

#[test]
fn a_map_is_asked_about_its_domain() {
    let mut first = stage();
    let key = a_key(&mut first);
    assert_eq!(
        first
            .sketch
            .shown(two(Op::Map(MapOp::Has), &first.book, key)),
        "(\"a\" \\in DOMAIN book)"
    );
    let second = stage();
    assert_eq!(
        second.sketch.shown(one(Op::Map(MapOp::Size), &second.book)),
        "Cardinality(DOMAIN book)"
    );
    let third = stage();
    assert_eq!(
        third.sketch.shown(one(Op::Map(MapOp::Keys), &third.book)),
        "(DOMAIN book)"
    );
}

#[test]
fn a_map_read_wraps_a_plain_value_and_leaves_an_optional_one() {
    let mut first = stage();
    let key = a_key(&mut first);
    first
        .sketch
        .rule("read", two(Op::Map(MapOp::Get), &first.book, key));
    let module = first.sketch.written().module;
    assert!(
        module.contains("    InRange => MapGet(book, \"a\")\n"),
        "{module}"
    );
    assert_eq!(
        helpers_of(&module),
        ["MapGet(map, key) == IF key \\in DOMAIN map THEN <<map[key]>> ELSE <<>>"]
    );
    let mut second = stage();
    let again = a_key(&mut second);
    assert_eq!(
        second
            .sketch
            .shown(two(Op::Map(MapOp::Get), &second.loose, again)),
        "MapGetOptional(loose, \"a\")"
    );
}

#[test]
fn a_map_is_changed_by_building_a_new_function() {
    let mut first = stage();
    let key = a_key(&mut first);
    let set = Expr::Apply(Op::Map(MapOp::Set), vec![first.book.clone(), key, int(1)]);
    first.sketch.rule("set", set);
    let module = first.sketch.written().module;
    assert!(
        module.contains("    InRange => MapSet(book, \"a\", 1)\n"),
        "{module}"
    );
    assert_eq!(
        helpers_of(&module),
        [
            "MapSet(map, key, value) == [other \\in (DOMAIN map) \\cup {key} |-> \
             IF other = key THEN value ELSE map[other]]"
        ]
    );
    let mut second = stage();
    let again = a_key(&mut second);
    second
        .sketch
        .rule("delete", two(Op::Map(MapOp::Delete), &second.book, again));
    let other = second.sketch.written().module;
    assert!(
        other.contains("    InRange => MapDelete(book, \"a\")\n"),
        "{other}"
    );
    assert_eq!(
        helpers_of(&other),
        ["MapDelete(map, key) == [other \\in (DOMAIN map) \\ {key} |-> map[other]]"]
    );
}

#[test]
fn the_size_of_a_sequence_is_its_length_and_a_push_appends() {
    let first = stage();
    assert_eq!(
        first.sketch.shown(one(Op::Seq(SeqOp::Size), &first.line)),
        "Len(line)"
    );
    let second = stage();
    assert_eq!(
        second
            .sketch
            .shown(two(Op::Seq(SeqOp::Push), &second.line, int(1))),
        "Append(line, 1)"
    );
}

#[test]
fn reading_a_sequence_goes_through_a_helper_that_is_null_where_there_is_no_item() {
    let cases = [
        (SeqOp::First, "SeqFirst(line)", "SeqFirstOptional(gaps)"),
        (SeqOp::Last, "SeqLast(line)", "SeqLastOptional(gaps)"),
    ];
    for (op, plain, optional) in cases {
        let first = stage();
        assert_eq!(first.sketch.shown(one(Op::Seq(op), &first.line)), plain);
        let second = stage();
        assert_eq!(
            second.sketch.shown(one(Op::Seq(op), &second.gaps)),
            optional
        );
    }
    let third = stage();
    assert_eq!(
        third
            .sketch
            .shown(two(Op::Seq(SeqOp::Get), &third.line, int(0))),
        "SeqGet(line, 0)"
    );
    let fourth = stage();
    assert_eq!(
        fourth
            .sketch
            .shown(two(Op::Seq(SeqOp::Get), &fourth.gaps, int(0))),
        "SeqGetOptional(gaps, 0)"
    );
}

#[test]
fn taking_from_a_sequence_goes_through_a_helper_that_keeps_an_empty_one() {
    let mut stage = stage();
    stage
        .sketch
        .rule("shift", one(Op::Seq(SeqOp::Shift), &stage.line));
    stage
        .sketch
        .rule("pop", one(Op::Seq(SeqOp::Pop), &stage.line));
    let module = stage.sketch.written().module;
    assert!(
        module.contains("    InRange => SeqShift(line)\n"),
        "{module}"
    );
    assert!(module.contains("    InRange => SeqPop(line)\n"), "{module}");
    assert_eq!(
        helpers_of(&module),
        [
            "SeqShift(seq) == IF Len(seq) = 0 THEN seq ELSE Tail(seq)",
            "SeqPop(seq) == IF Len(seq) = 0 THEN seq ELSE SubSeq(seq, 1, Len(seq) - 1)",
        ]
    );
}

#[test]
fn a_sequence_includes_an_item_through_a_helper_over_its_positions() {
    let mut stage = stage();
    stage
        .sketch
        .rule("has", two(Op::Seq(SeqOp::Includes), &stage.line, int(1)));
    let module = stage.sketch.written().module;
    assert!(
        module.contains("    InRange => SeqIncludes(line, 1)\n"),
        "{module}"
    );
    assert_eq!(
        helpers_of(&module),
        ["SeqIncludes(seq, item) == \\E index \\in DOMAIN seq : seq[index] = item"]
    );
}

#[test]
fn an_operation_without_its_arguments_stops_the_checker_instead_of_guessing() {
    let first = stage();
    assert_eq!(
        first.sketch.shown(Expr::Apply(Op::Not, Vec::new())),
        "Fault(\"the TLA+ export does not know an operation without an argument\")"
    );
    let second = stage();
    assert_eq!(
        second.sketch.shown(Expr::Apply(Op::Equal, vec![second.n])),
        "Fault(\"the TLA+ export does not know an operation that lacks its second argument\")"
    );
    let mut third = stage();
    let key = a_key(&mut third);
    assert_eq!(
        third
            .sketch
            .shown(two(Op::Map(MapOp::Set), &third.book, key)),
        "Fault(\"the TLA+ export does not know a map set without a value\")"
    );
}

#[test]
fn an_argument_past_the_ones_an_operation_takes_is_left_out_as_the_engine_ignores_it() {
    let stage = stage();
    let extra = Expr::Apply(Op::Not, vec![stage.flag.clone(), int(1)]);
    assert_eq!(stage.sketch.shown(extra), "(~flag)");
}
