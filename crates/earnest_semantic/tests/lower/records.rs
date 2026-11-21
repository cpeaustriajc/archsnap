use crate::build::{
    action, after, bind, choose, draft, equal, fails, field, helper, helper_call, holds, int,
    lowered, map_of, map_shape, name, null, one_of, optional, range, record, record_shape, refusal,
    seq_of, seq_shape, spread, starting, take, text,
};
use earnest_ast::{ActionDraft, Draft, Shape, Term};
use earnest_ir::{Expr, Model, Op};
use earnest_value::Value;

fn guest() -> Shape {
    one_of(&["ann", "bob"])
}

fn item() -> Shape {
    record_shape(vec![("qty", range(0, 2)), ("who", optional(guest()))])
}

fn fields() -> Vec<(&'static str, Shape)> {
    vec![("chosen", starting(optional(item()), null()))]
}

fn picking(chosen: Term) -> Draft {
    let mut source = draft(fields());
    let mut pick: ActionDraft = action("pick", None, record(vec![("chosen", chosen)]));
    take(&mut pick, "g", guest());
    source.actions.push(pick);
    source
}

fn chosen_by(model: &Model, guest: &str) -> String {
    let given = Value::Str(model.symbols.find(guest).expect("the guest is a choice"));
    let state = after(model, 0, &[given]);
    model.symbols.show(&state[0])
}

fn wrapped(expr: &Expr) -> usize {
    let inside = |all: &[Expr]| all.iter().map(wrapped).sum::<usize>();
    let named = |all: &[(earnest_value::Sym, Expr)]| -> usize {
        all.iter().map(|(_, each)| wrapped(each)).sum()
    };
    match expr {
        Expr::Apply(Op::SomeOf, operands) => 1 + inside(operands),
        Expr::Apply(_, operands) | Expr::SetOf(operands) | Expr::SeqOf(operands) => {
            inside(operands)
        }
        Expr::And(left, right) | Expr::Or(left, right) => wrapped(left) + wrapped(right),
        Expr::Choose(test, yes, no) => wrapped(test) + wrapped(yes) + wrapped(no),
        Expr::Bind { value, body, .. } => wrapped(value) + wrapped(body),
        Expr::Record(fields) => named(fields),
        Expr::Get(inner, _) => wrapped(inner),
        Expr::With(base, changes) => wrapped(base) + named(changes),
        _ => 0,
    }
}

fn update(model: &Model) -> &Expr {
    model.actions[0].updates[0]
        .as_ref()
        .expect("the action sets `chosen`")
        .expr()
}

fn named_item(who: Term) -> Term {
    record(vec![("qty", int(0)), ("who", who)])
}

#[test]
fn a_choice_between_a_record_with_a_value_and_one_with_null_is_the_declared_record() {
    let either = choose(
        equal(name("g"), text("ann")),
        named_item(name("g")),
        record(vec![("qty", int(1)), ("who", null())]),
    );
    let model = lowered(&picking(either));
    assert_eq!(chosen_by(&model, "ann"), "{qty: 0, who: \"ann\"}");
    assert_eq!(chosen_by(&model, "bob"), "{qty: 1, who: null}");
    assert_eq!(
        wrapped(update(&model)),
        2,
        "the record is present in the optional field, and `g` is present in `who`; the \
         `null` of the other branch is absent as it is: {:?}",
        update(&model)
    );
}

#[test]
fn a_record_bound_to_a_name_and_spread_is_the_declared_record() {
    let bound = bind(
        "fresh",
        named_item(name("g")),
        record(vec![(
            "chosen",
            spread(Some(name("fresh")), vec![("qty", int(1))]),
        )]),
    );
    let mut source = draft(fields());
    let mut pick = action("pick", None, bound);
    take(&mut pick, "g", guest());
    source.actions.push(pick);
    let model = lowered(&source);
    assert_eq!(chosen_by(&model, "ann"), "{qty: 1, who: \"ann\"}");
    assert_eq!(chosen_by(&model, "bob"), "{qty: 1, who: \"bob\"}");
    assert_eq!(
        wrapped(update(&model)),
        2,
        "the record is present in the optional field, and the `who` it was bound with is \
         made present where the record meets its declaration: {:?}",
        update(&model)
    );
}

#[test]
fn a_helper_that_gives_a_record_or_null_is_the_declared_optional_record() {
    let mut source = picking(helper_call("item", vec![name("g")]));
    source.helpers.push(helper(
        "item",
        &["by"],
        choose(
            equal(name("by"), text("ann")),
            null(),
            named_item(name("by")),
        ),
    ));
    let model = lowered(&source);
    assert_eq!(chosen_by(&model, "ann"), "null");
    assert_eq!(chosen_by(&model, "bob"), "{qty: 0, who: \"bob\"}");
}

#[test]
fn an_optional_record_from_a_name_meets_its_declaration_when_present_and_when_absent() {
    let maybe = bind(
        "kept",
        choose(equal(name("g"), text("ann")), null(), named_item(name("g"))),
        record(vec![("chosen", name("kept"))]),
    );
    let mut source = draft(fields());
    let mut pick = action("pick", None, maybe);
    take(&mut pick, "g", guest());
    source.actions.push(pick);
    let model = lowered(&source);
    assert_eq!(
        chosen_by(&model, "ann"),
        "null",
        "an absent record stays absent"
    );
    assert_eq!(chosen_by(&model, "bob"), "{qty: 0, who: \"bob\"}");
}

fn with_ann() -> Vec<(&'static str, Shape)> {
    vec![(
        "chosen",
        starting(optional(item()), named_item(text("ann"))),
    )]
}

#[test]
fn a_declared_record_compares_with_a_record_that_gives_its_optional_field_a_value() {
    assert!(holds(
        with_ann(),
        equal(field("chosen"), named_item(text("ann")))
    ));
    assert!(fails(
        with_ann(),
        equal(field("chosen"), named_item(text("bob")))
    ));
    assert!(fails(
        with_ann(),
        equal(field("chosen"), named_item(null()))
    ));
    assert!(holds(
        with_ann(),
        equal(named_item(text("ann")), named_item(text("ann")))
    ));
    assert!(fails(
        with_ann(),
        equal(named_item(text("ann")), named_item(null()))
    ));
}

#[test]
fn records_that_differ_in_more_than_an_optional_field_still_do_not_meet() {
    let other_field = record(vec![("qty", int(0)), ("size", int(1))]);
    assert_eq!(
        refusal(
            with_ann(),
            equal(
                choose(equal(int(1), int(1)), named_item(null()), other_field),
                null()
            )
        )
        .feature,
        "a choice between a record and a record",
        "the fields have other names"
    );
    let other_kind = record(vec![("qty", text("many")), ("who", null())]);
    assert_eq!(
        refusal(with_ann(), equal(field("chosen"), other_kind)).feature,
        "an optional value compared with a record",
        "`qty` is a number in one and a text in the other"
    );
    assert_eq!(
        refusal(with_ann(), equal(named_item(null()), null())).feature,
        "a record compared with null",
        "a record written out is never null"
    );
}

fn shelf() -> Vec<(&'static str, Shape)> {
    vec![("book", map_shape(guest(), item()))]
}

fn booking(entries: Vec<(Term, Term)>) -> Model {
    let mut source = draft(shelf());
    source.actions.push(action(
        "book",
        None,
        record(vec![("book", map_of(entries))]),
    ));
    lowered(&source)
}

#[test]
fn a_map_literal_takes_a_record_that_gives_an_optional_field_a_value() {
    let model = booking(vec![(
        text("ann"),
        record(vec![("qty", int(1)), ("who", text("ann"))]),
    )]);
    let state = after(&model, 0, &[]);
    assert_eq!(
        model.symbols.show(&state[0]),
        "{\"ann\" -> {qty: 1, who: \"ann\"}}",
        "the key `ann` holds the record, whose `who` is the present guest"
    );
}

#[test]
fn the_entries_of_a_map_literal_join_into_one_record_type() {
    let model = booking(vec![
        (
            text("ann"),
            record(vec![("qty", int(1)), ("who", text("ann"))]),
        ),
        (text("bob"), record(vec![("qty", int(2)), ("who", null())])),
    ]);
    let state = after(&model, 0, &[]);
    assert_eq!(
        model.symbols.show(&state[0]),
        "{\"ann\" -> {qty: 1, who: \"ann\"}, \"bob\" -> {qty: 2, who: null}}",
        "the first entry names a guest, the second none, and both are items"
    );
}

#[test]
fn a_map_literal_inside_a_sequence_literal_is_coerced_as_deep_as_it_goes() {
    let mut source = draft(vec![("rows", seq_shape(map_shape(guest(), item()), 2))]);
    source.actions.push(action(
        "add",
        None,
        record(vec![(
            "rows",
            seq_of(vec![map_of(vec![(
                text("bob"),
                record(vec![("qty", int(0)), ("who", text("bob"))]),
            )])]),
        )]),
    ));
    let model = lowered(&source);
    let state = after(&model, 0, &[]);
    assert_eq!(
        model.symbols.show(&state[0]),
        "[{\"bob\" -> {qty: 0, who: \"bob\"}}]",
        "a sequence of one map"
    );
}
