use crate::build::{
    Sketch, bind, both, boxed, choose, constant, either, helpers_of, input, int, map_value, no,
    null, one, set_value, two, yes,
};
use crate::ops::{Stage, stage};
use earnest_ir::{Expr, MapOp, Math, Op, Order, SeqOp, SetOp, Shelf, Sweep};
use earnest_value::{Domain, Value};

fn below(left: &Expr, right: i64) -> Expr {
    two(Op::Order(Order::Less), left, int(right))
}

fn sweep(sweep: Sweep, shelf: Shelf, slot: usize, source: &Expr, body: Expr) -> Expr {
    Expr::Sweep {
        sweep,
        shelf,
        slot,
        source: Box::new(source.clone()),
        body: Box::new(body),
    }
}

fn pair(stage: &mut Stage) -> Expr {
    let domain = stage.sketch.record(&[
        ("x", Domain::Int { lo: 0, hi: 1 }),
        ("y", Domain::Optional(boxed(Domain::Bool))),
    ]);
    stage.sketch.field("pair", domain, Vec::new())
}

#[test]
fn plain_constants_are_the_literals_of_the_language() {
    assert_eq!(stage().sketch.shown(yes()), "TRUE");
    assert_eq!(stage().sketch.shown(no()), "FALSE");
    assert_eq!(stage().sketch.shown(int(7)), "7");
    assert_eq!(stage().sketch.shown(null()), "<<>>");
    let mut texts = stage();
    let text = texts.sketch.text("open");
    assert_eq!(texts.sketch.shown(constant(text)), "\"open\"");
}

#[test]
fn a_negative_number_is_put_in_brackets() {
    assert_eq!(stage().sketch.shown(int(-7)), "(-7)");
    let stage = stage();
    assert_eq!(
        stage
            .sketch
            .shown(two(Op::Math(Math::Subtract), &stage.n, int(-1))),
        "(n - (-1))"
    );
}

#[test]
fn a_text_value_is_a_string_with_its_quotes_and_backslashes_escaped() {
    let mut stage = stage();
    let text = stage.sketch.text("say \"hi\" \\ then\tstop\n");
    assert_eq!(
        stage.sketch.shown(constant(text)),
        "\"say \\\"hi\\\" \\\\ then\\tstop\\n\""
    );
}

#[test]
fn a_constant_collection_is_written_item_by_item() {
    assert_eq!(
        stage()
            .sketch
            .shown(constant(Value::seq(vec![Value::Int(1), Value::Int(2)]))),
        "<<1, 2>>"
    );
    assert_eq!(
        stage()
            .sketch
            .shown(constant(set_value(vec![Value::Int(2), Value::Int(1)]))),
        "{1, 2}"
    );
    assert_eq!(stage().sketch.shown(constant(set_value(Vec::new()))), "{}");
    assert_eq!(
        stage().sketch.shown(constant(Value::seq(Vec::new()))),
        "<<>>"
    );
}

#[test]
fn a_constant_map_is_built_key_by_key_from_the_empty_map() {
    let mut stage = stage();
    let entries = vec![
        (stage.sketch.text("a"), Value::Int(1)),
        (stage.sketch.text("b"), Value::Int(2)),
    ];
    stage.sketch.rule("map", constant(map_value(entries)));
    stage.sketch.rule("none", constant(map_value(Vec::new())));
    let module = stage.sketch.written().module;
    assert!(
        module.contains("    InRange => MapSet(MapSet(EmptyMap, \"a\", 1), \"b\", 2)\n"),
        "{module}"
    );
    assert!(module.contains("    InRange => EmptyMap\n"), "{module}");
    assert_eq!(
        helpers_of(&module),
        [
            "EmptyMap == [key \\in {} |-> key]",
            "MapSet(map, key, value) == [other \\in (DOMAIN map) \\cup {key} |-> \
             IF other = key THEN value ELSE map[other]]",
        ]
    );
}

#[test]
fn a_constant_record_names_each_field() {
    let mut stage = stage();
    let x = stage.sketch.sym("x");
    let y = stage.sketch.sym("y");
    let record = Value::record(vec![(x, Value::Int(1)), (y, Value::Null)]);
    assert_eq!(
        stage.sketch.shown(constant(record)),
        "[x |-> 1, y |-> <<>>]"
    );
    assert_eq!(
        self::stage()
            .sketch
            .shown(constant(Value::record(Vec::new()))),
        "<<>>"
    );
}

#[test]
fn a_constant_set_that_holds_null_wraps_its_present_values() {
    let mut stage = stage();
    let every = set_value(vec![
        stage.sketch.text("a"),
        Value::Null,
        stage.sketch.text("b"),
    ]);
    assert_eq!(
        stage.sketch.shown(constant(every)),
        "{<<>>, <<\"a\">>, <<\"b\">>}"
    );
}

#[test]
fn a_tuple_in_a_tuple_keeps_its_brackets_apart() {
    let nested = Value::seq(vec![
        Value::seq(vec![Value::Int(1)]),
        Value::seq(vec![Value::Int(2)]),
    ]);
    assert_eq!(stage().sketch.shown(constant(nested)), "<< <<1>>, <<2>> >>");
    assert_eq!(
        stage()
            .sketch
            .shown(one(Op::SomeOf, &Expr::SeqOf(Vec::new()))),
        "<< <<>> >>"
    );
    assert_eq!(
        stage()
            .sketch
            .shown(Expr::SeqOf(vec![one(Op::SomeOf, &int(1))])),
        "<< <<1>> >>"
    );
}

#[test]
fn a_state_field_is_its_variable() {
    let stage = stage();
    assert_eq!(stage.sketch.shown(stage.n), "n");
}

#[test]
fn a_field_or_a_local_the_model_lacks_stops_the_checker_instead_of_guessing() {
    assert_eq!(
        stage().sketch.shown(Expr::Field(99)),
        "Fault(\"the TLA+ export does not know a state field the model does not have\")"
    );
    let mut stage = stage();
    stage.sketch.rule("stray", Expr::Local(5));
    let module = stage.sketch.written().module;
    assert!(
        module.contains(
            "    InRange => Fault(\"the TLA+ export does not know a local value that nothing \
             binds\")\n"
        ),
        "{module}"
    );
    assert_eq!(
        helpers_of(&module),
        ["Fault(message) == Assert(FALSE, message)"]
    );
}

#[test]
fn and_and_or_keep_their_sides_in_order() {
    let first = stage();
    assert_eq!(
        first
            .sketch
            .shown(both(first.flag.clone(), below(&first.n, 1))),
        "(flag /\\ (n < 1))"
    );
    let second = stage();
    assert_eq!(
        second
            .sketch
            .shown(either(below(&second.n, 1), second.flag.clone())),
        "((n < 1) \\/ flag)"
    );
    let third = stage();
    let guarded = either(
        two(Op::Equal, &third.maybe, null()),
        below(&one(Op::Unwrap, &third.maybe), 1),
    );
    assert_eq!(
        third.sketch.shown(both(guarded, third.flag)),
        "(((maybe = <<>>) \\/ (maybe[1] < 1)) /\\ flag)"
    );
}

#[test]
fn a_choice_is_an_if_with_both_branches() {
    let stage = stage();
    assert_eq!(
        stage
            .sketch
            .shown(choose(stage.flag.clone(), int(1), stage.n)),
        "(IF flag THEN 1 ELSE n)"
    );
}

#[test]
fn a_binding_is_a_let_and_each_one_in_a_definition_has_its_own_name() {
    let first = stage();
    let squared = two(Op::Math(Math::Multiply), &Expr::Local(0), Expr::Local(0));
    let bound = bind(0, two(Op::Math(Math::Add), &first.n, int(1)), squared);
    assert_eq!(
        first.sketch.shown(bound),
        "(LET local == (n + 1) IN (local * local))"
    );
    let second = stage();
    let sum = two(Op::Math(Math::Add), &Expr::Local(0), Expr::Local(1));
    let nested = bind(0, second.n.clone(), bind(1, int(2), sum));
    assert_eq!(
        second.sketch.shown(nested),
        "(LET local == n IN (LET local_2 == 2 IN (local + local_2)))"
    );
}

#[test]
fn a_record_names_each_field_and_a_read_is_a_dot() {
    let mut stage = stage();
    let x = stage.sketch.sym("x");
    let y = stage.sketch.sym("y");
    let record = Expr::Record(vec![(x, stage.n.clone()), (y, null())]);
    assert_eq!(stage.sketch.shown(record), "[x |-> n, y |-> <<>>]");
    let mut second = self::stage();
    let pair = pair(&mut second);
    assert_eq!(second.sketch.shown(Expr::Get(Box::new(pair), x)), "pair.x");
    assert_eq!(self::stage().sketch.shown(Expr::Record(Vec::new())), "<<>>");
}

#[test]
fn a_field_of_an_optional_record_is_read_from_inside_the_tuple() {
    let mut stage = stage();
    let qty = stage.sketch.sym("qty");
    let item = stage
        .sketch
        .record(&[("qty", Domain::Int { lo: 1, hi: 2 })]);
    let chosen = stage
        .sketch
        .field("chosen", Domain::Optional(boxed(item)), Vec::new());
    let read = Expr::Get(Box::new(one(Op::Unwrap, &chosen)), qty);
    assert_eq!(stage.sketch.shown(read), "chosen[1].qty");
}

#[test]
fn a_record_with_fields_replaced_is_an_except() {
    let mut first = stage();
    let pair = pair(&mut first);
    let x = first.sketch.sym("x");
    let y = first.sketch.sym("y");
    let one_change = Expr::With(Box::new(pair.clone()), vec![(x, int(1))]);
    let two_changes = Expr::With(
        Box::new(pair.clone()),
        vec![(x, int(1)), (y, one(Op::SomeOf, &yes()))],
    );
    let no_change = Expr::With(Box::new(pair), Vec::new());
    first.sketch.rule("one", one_change);
    first.sketch.rule("two", two_changes);
    first.sketch.rule("none", no_change);
    let module = first.sketch.written().module;
    assert!(
        module.contains("    InRange => [pair EXCEPT !.x = 1]\n"),
        "{module}"
    );
    assert!(
        module.contains("    InRange => [pair EXCEPT !.x = 1, !.y = <<TRUE>>]\n"),
        "{module}"
    );
    assert!(module.contains("    InRange => pair\n"), "{module}");
}

#[test]
fn a_record_given_a_new_field_is_built_again_from_the_old_one() {
    let mut stage = stage();
    let pair = pair(&mut stage);
    let x = stage.sketch.sym("x");
    let z = stage.sketch.sym("z");
    let wider = Expr::With(Box::new(pair), vec![(z, yes()), (x, int(1))]);
    assert_eq!(
        stage.sketch.shown(wider),
        "(LET local == pair IN [x |-> 1, y |-> local.y, z |-> TRUE])"
    );
}

#[test]
fn collections_are_written_item_by_item() {
    let first = stage();
    assert_eq!(
        first.sketch.shown(Expr::SetOf(vec![first.n, int(1)])),
        "{n, 1}"
    );
    let second = stage();
    assert_eq!(
        second.sketch.shown(Expr::SeqOf(vec![second.n, int(1)])),
        "<<n, 1>>"
    );
    assert_eq!(stage().sketch.shown(Expr::SetOf(Vec::new())), "{}");
    assert_eq!(stage().sketch.shown(Expr::SeqOf(Vec::new())), "<<>>");
}

#[test]
fn a_map_is_built_pair_by_pair_so_a_later_pair_replaces_an_earlier_one() {
    let mut stage = stage();
    let a = constant(stage.sketch.text("a"));
    let pairs = Expr::MapOf(vec![(a.clone(), stage.n.clone()), (a, int(2))]);
    assert_eq!(
        stage.sketch.shown(pairs),
        "MapSet(MapSet(EmptyMap, \"a\", n), \"a\", 2)"
    );
    assert_eq!(
        self::stage().sketch.shown(Expr::MapOf(Vec::new())),
        "EmptyMap"
    );
}

#[test]
fn a_test_on_every_or_some_item_of_a_set_is_a_quantifier() {
    let first = stage();
    let every = sweep(
        Sweep::Every,
        Shelf::Set,
        0,
        &first.seen,
        below(&Expr::Local(0), 2),
    );
    assert_eq!(
        first.sketch.shown(every),
        "(\\A item \\in seen : (item < 2))"
    );
    let second = stage();
    let some = sweep(
        Sweep::Some,
        Shelf::Set,
        0,
        &second.seen,
        below(&Expr::Local(0), 2),
    );
    assert_eq!(
        second.sketch.shown(some),
        "(\\E item \\in seen : (item < 2))"
    );
}

#[test]
fn a_filter_of_a_set_is_a_subset() {
    let stage = stage();
    let kept = sweep(
        Sweep::Filter,
        Shelf::Set,
        0,
        &stage.seen,
        below(&Expr::Local(0), 2),
    );
    assert_eq!(stage.sketch.shown(kept), "{item \\in seen : (item < 2)}");
}

#[test]
fn a_test_on_a_sequence_runs_over_the_set_of_its_items() {
    let mut stage = stage();
    let every = sweep(
        Sweep::Every,
        Shelf::Seq,
        0,
        &stage.line,
        below(&Expr::Local(0), 2),
    );
    let some = sweep(
        Sweep::Some,
        Shelf::Seq,
        0,
        &stage.line,
        below(&Expr::Local(0), 2),
    );
    stage.sketch.rule("every", every);
    stage.sketch.rule("some", some);
    let module = stage.sketch.written().module;
    assert!(
        module.contains("    InRange => (\\A item \\in SeqItems(line) : (item < 2))\n"),
        "{module}"
    );
    assert!(
        module.contains("    InRange => (\\E item \\in SeqItems(line) : (item < 2))\n"),
        "{module}"
    );
    assert_eq!(
        helpers_of(&module),
        ["SeqItems(seq) == {seq[index] : index \\in DOMAIN seq}"]
    );
}

#[test]
fn a_filter_of_a_sequence_selects_with_a_named_test() {
    let mut stage = stage();
    let kept = sweep(
        Sweep::Filter,
        Shelf::Seq,
        0,
        &stage.line,
        below(&Expr::Local(0), 2),
    );
    stage.sketch.rule("kept", kept);
    let module = stage.sketch.written().module;
    assert!(
        module.contains("    InRange => (LET Keep(item) == (item < 2) IN SelectSeq(line, Keep))\n"),
        "{module}"
    );
    assert_eq!(helpers_of(&module), Vec::<String>::new());
}

#[test]
fn a_set_filtered_as_a_sequence_has_no_order_to_write() {
    let stage = stage();
    let kept = sweep(Sweep::Filter, Shelf::Seq, 0, &stage.seen, yes());
    assert_eq!(
        stage.sketch.shown(kept),
        "Fault(\"the TLA+ export does not know the order of a set filtered as a sequence\")"
    );
}

#[test]
fn a_test_inside_a_test_has_its_own_item_name() {
    let stage = stage();
    let inner = sweep(
        Sweep::Some,
        Shelf::Set,
        1,
        &stage.seen,
        two(Op::Order(Order::Less), &Expr::Local(0), Expr::Local(1)),
    );
    let outer = sweep(Sweep::Every, Shelf::Set, 0, &stage.seen, inner);
    assert_eq!(
        stage.sketch.shown(outer),
        "(\\A item \\in seen : (\\E item_2 \\in seen : (item < item_2)))"
    );
}

#[test]
fn a_test_over_every_value_of_an_optional_declaration_compares_like_with_like() {
    let mut stage = stage();
    let every = set_value(vec![
        Value::Null,
        stage.sketch.text("Ada"),
        stage.sketch.text("Bo"),
    ]);
    let ada = one(Op::SomeOf, &constant(stage.sketch.text("Ada")));
    let body = either(
        two(Op::Equal, &Expr::Local(0), null()),
        two(Op::Differ, &Expr::Local(0), ada),
    );
    let all = sweep(Sweep::Every, Shelf::Set, 0, &constant(every), body);
    assert_eq!(
        stage.sketch.shown(all),
        "(\\A item \\in {<<>>, <<\"Ada\">>, <<\"Bo\">>} : \
         ((item = <<>>) \\/ (item # <<\"Ada\">>)))"
    );
}

#[test]
fn the_kind_of_a_collection_is_followed_through_what_is_built_from_it() {
    let first = stage();
    let present = two(Op::Differ, &Expr::Local(0), null());
    let kept = sweep(Sweep::Filter, Shelf::Seq, 0, &first.gaps, present);
    assert_eq!(
        first.sketch.shown(one(Op::Seq(SeqOp::First), &kept)),
        "SeqFirstOptional((LET Keep(item) == (item # <<>>) IN SelectSeq(gaps, Keep)))"
    );
    let second = stage();
    let pushed = two(
        Op::Seq(SeqOp::Push),
        &Expr::SeqOf(Vec::new()),
        one(Op::SomeOf, &second.n),
    );
    assert_eq!(
        second.sketch.shown(one(Op::Seq(SeqOp::Last), &pushed)),
        "SeqLastOptional(Append(<<>>, <<n>>))"
    );
    let third = stage();
    let either_one = choose(
        third.flag.clone(),
        Expr::SeqOf(Vec::new()),
        third.gaps.clone(),
    );
    assert_eq!(
        third.sketch.shown(one(Op::Seq(SeqOp::First), &either_one)),
        "SeqFirstOptional((IF flag THEN <<>> ELSE gaps))"
    );
    let fourth = stage();
    let bound = bind(0, fourth.loose.clone(), {
        let key = Expr::SetOf(Vec::new());
        two(Op::Map(MapOp::Get), &Expr::Local(0), key)
    });
    assert_eq!(
        fourth.sketch.shown(bound),
        "(LET local == loose IN MapGetOptional(local, {}))"
    );
}

#[test]
fn a_value_read_from_a_map_or_a_sequence_is_unwrapped_like_any_optional() {
    let mut first = stage();
    let key = constant(first.sketch.text("a"));
    let read = two(Op::Map(MapOp::Get), &first.book, key);
    assert_eq!(
        first.sketch.shown(one(Op::Unwrap, &read)),
        "MapGet(book, \"a\")[1]"
    );
    let second = stage();
    let head = one(Op::Seq(SeqOp::First), &second.line);
    assert_eq!(
        second.sketch.shown(two(Op::Differ, &head, null())),
        "(SeqFirst(line) # <<>>)"
    );
}

#[test]
fn the_lowered_form_of_each_library_primitive_is_written_from_the_same_parts() {
    let first = stage();
    let present = two(Op::Differ, &first.maybe, null());
    let test = bind(
        0,
        one(Op::Unwrap, &first.maybe),
        two(Op::Order(Order::Greater), &Expr::Local(0), int(0)),
    );
    assert_eq!(
        first.sketch.shown(both(present, test)),
        "((maybe # <<>>) /\\ (LET local == maybe[1] IN (local > 0)))"
    );
    let second = stage();
    let empty = two(Op::Equal, &one(Op::Set(SetOp::Size), &second.seen), int(0));
    assert_eq!(second.sketch.shown(empty), "(Cardinality(seen) = 0)");
    let third = stage();
    let full = two(
        Op::Order(Order::GreaterOrEqual),
        &one(Op::Seq(SeqOp::Size), &third.line),
        int(2),
    );
    assert_eq!(third.sketch.shown(full), "(Len(line) >= 2)");
    let mut fourth = stage();
    let stages = fourth.sketch.one_of(&["idle", "running", "fired"]);
    let idle = fourth.sketch.text("idle");
    let timer = fourth.sketch.field("timer", stages, vec![idle.clone()]);
    assert_eq!(
        fourth.sketch.shown(two(Op::Equal, &timer, constant(idle))),
        "(timer = \"idle\")"
    );
    let fifth = stage();
    let kept = choose(fifth.flag.clone(), int(0), fifth.n.clone());
    assert_eq!(fifth.sketch.shown(kept), "(IF flag THEN 0 ELSE n)");
}

#[test]
fn an_owned_lock_is_an_optional_owner_and_an_input_is_read_by_its_name() {
    let mut sketch = Sketch::new("Printer");
    let owners = sketch.one_of(&["Ada", "Bo"]);
    let job = sketch.field(
        "job",
        Domain::Optional(boxed(owners.clone())),
        vec![Value::Null],
    );
    let who = Expr::Local(0);
    sketch.action_with(
        "take",
        vec![input("who", 0, owners)],
        two(Op::Equal, &job, null()),
        &[(&job, one(Op::SomeOf, &who))],
    );
    sketch.action_with(
        "give",
        vec![input("who", 0, Domain::Bool)],
        two(Op::Equal, &job, one(Op::SomeOf, &who)),
        &[(&job, null())],
    );
    let module = sketch.written().module;
    let expected = "\
Action_take ==
    \\E who \\in {\"Ada\", \"Bo\"} :
        IF InRange /\\ (job = <<>>)
        THEN /\\ job' = <<who>>
        ELSE FALSE

Action_give ==
    \\E who \\in {FALSE, TRUE} :
        IF InRange /\\ (job = <<who>>)
        THEN /\\ job' = <<>>
        ELSE FALSE
";
    assert!(module.contains(expected), "{module}");
}
