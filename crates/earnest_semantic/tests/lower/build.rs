use earnest_ast::{
    ActionDraft, Binary, Draft, FieldDraft, HelperDraft, RuleDraft, Shape, ShapeKind, Term,
    TermKind, Unary, ViewDraft,
};
use earnest_diagnostics::{Place, Problem};
use earnest_ir::{Fault, Frame, Model, eval, truth};
use earnest_semantic::lower;
use earnest_value::Value;

pub(crate) const FILE: &str = "src/cart.model.ts";

pub(crate) const HERE: Place = Place { line: 7, column: 3 };

pub(crate) fn term(kind: TermKind) -> Term {
    Term::new(Place { line: 1, column: 1 }, kind)
}

pub(crate) fn placed(mut moved: Term, place: Place) -> Term {
    moved.place = place;
    moved
}

pub(crate) fn here(moved: Term) -> Term {
    placed(moved, HERE)
}

pub(crate) fn null() -> Term {
    term(TermKind::Null)
}

pub(crate) fn yes() -> Term {
    term(TermKind::Bool(true))
}

pub(crate) fn no() -> Term {
    term(TermKind::Bool(false))
}

pub(crate) fn int(number: i64) -> Term {
    term(TermKind::Int(number))
}

pub(crate) fn text(value: &str) -> Term {
    term(TermKind::Text(value.to_owned()))
}

pub(crate) fn name(known: &str) -> Term {
    term(TermKind::Name(known.to_owned()))
}

pub(crate) fn prop(target: Term, property: &str) -> Term {
    term(TermKind::Member(Box::new(target), property.to_owned()))
}

pub(crate) fn field(known: &str) -> Term {
    prop(name("s"), known)
}

pub(crate) fn call(target: Term, method: &str, arguments: Vec<Term>) -> Term {
    term(TermKind::Call {
        target: Box::new(target),
        method: method.to_owned(),
        arguments,
    })
}

pub(crate) fn helper_call(known: &str, arguments: Vec<Term>) -> Term {
    term(TermKind::Helper {
        name: known.to_owned(),
        arguments,
    })
}

pub(crate) fn not(inner: Term) -> Term {
    term(TermKind::Unary(Unary::Not, Box::new(inner)))
}

pub(crate) fn negate(inner: Term) -> Term {
    term(TermKind::Unary(Unary::Negate, Box::new(inner)))
}

pub(crate) fn binary(op: Binary, left: Term, right: Term) -> Term {
    term(TermKind::Binary(op, Box::new(left), Box::new(right)))
}

pub(crate) fn equal(left: Term, right: Term) -> Term {
    binary(Binary::Equal, left, right)
}

pub(crate) fn differ(left: Term, right: Term) -> Term {
    binary(Binary::Differ, left, right)
}

pub(crate) fn choose(test: Term, first: Term, second: Term) -> Term {
    term(TermKind::Choose(
        Box::new(test),
        Box::new(first),
        Box::new(second),
    ))
}

pub(crate) fn bind(known: &str, value: Term, body: Term) -> Term {
    term(TermKind::Bind {
        name: known.to_owned(),
        value: Box::new(value),
        body: Box::new(body),
    })
}

pub(crate) fn wrap(inner: Term) -> Term {
    term(TermKind::Wrap(Box::new(inner)))
}

pub(crate) fn lambda(parameter: &str, body: Term) -> Term {
    term(TermKind::Lambda {
        parameters: vec![parameter.to_owned()],
        body: Box::new(body),
    })
}

pub(crate) fn record(fields: Vec<(&str, Term)>) -> Term {
    spread(None, fields)
}

pub(crate) fn spread(base: Option<Term>, fields: Vec<(&str, Term)>) -> Term {
    term(TermKind::Record {
        base: base.map(Box::new),
        fields: fields
            .into_iter()
            .map(|(known, value)| (known.to_owned(), value))
            .collect(),
    })
}

pub(crate) fn set_of(items: Vec<Term>) -> Term {
    term(TermKind::SetOf(items))
}

pub(crate) fn seq_of(items: Vec<Term>) -> Term {
    term(TermKind::SeqOf(items))
}

pub(crate) fn map_of(entries: Vec<(Term, Term)>) -> Term {
    term(TermKind::MapOf(entries))
}

pub(crate) fn ints(numbers: &[i64]) -> Vec<Term> {
    numbers.iter().copied().map(int).collect()
}

pub(crate) fn shape(kind: ShapeKind) -> Shape {
    Shape {
        place: Place { line: 2, column: 2 },
        kind,
        initial: None,
    }
}

pub(crate) fn starting(mut declared: Shape, initial: Term) -> Shape {
    declared.initial = Some(initial);
    declared
}

pub(crate) fn boolean() -> Shape {
    shape(ShapeKind::Bool)
}

pub(crate) fn range(lo: i64, hi: i64) -> Shape {
    shape(ShapeKind::Int { lo, hi })
}

pub(crate) fn one_of(choices: &[&str]) -> Shape {
    shape(ShapeKind::OneOf(
        choices.iter().map(|choice| text(choice)).collect(),
    ))
}

pub(crate) fn optional(inner: Shape) -> Shape {
    shape(ShapeKind::Optional(Box::new(inner)))
}

pub(crate) fn record_shape(fields: Vec<(&str, Shape)>) -> Shape {
    shape(ShapeKind::Record(
        fields
            .into_iter()
            .map(|(known, inner)| (known.to_owned(), inner))
            .collect(),
    ))
}

pub(crate) fn set_shape(item: Shape) -> Shape {
    shape(ShapeKind::Set(Box::new(item)))
}

pub(crate) fn map_shape(key: Shape, value: Shape) -> Shape {
    shape(ShapeKind::Map(Box::new(key), Box::new(value)))
}

pub(crate) fn seq_shape(item: Shape, max: usize) -> Shape {
    shape(ShapeKind::Seq {
        item: Box::new(item),
        max,
    })
}

pub(crate) fn queue_shape(item: Shape, capacity: usize) -> Shape {
    shape(ShapeKind::Queue {
        item: Box::new(item),
        capacity,
    })
}

pub(crate) fn lock_shape(owner: Option<Shape>) -> Shape {
    shape(ShapeKind::Lock(owner.map(Box::new)))
}

pub(crate) fn rule(id: &str, holds: Term) -> RuleDraft {
    RuleDraft {
        id: id.to_owned(),
        says: format!("{id} holds"),
        place: Place {
            line: 30,
            column: 1,
        },
        state_name: Some("s".to_owned()),
        holds,
        text: String::new(),
    }
}

pub(crate) fn action(id: &str, guard: Option<Term>, change: Term) -> ActionDraft {
    ActionDraft {
        id: id.to_owned(),
        label: format!("{id} happens"),
        place: Place {
            line: 20,
            column: 1,
        },
        inputs: Vec::new(),
        state_name: Some("s".to_owned()),
        input_name: None,
        guard,
        change,
    }
}

pub(crate) fn draft(fields: Vec<(&str, Shape)>) -> Draft {
    Draft {
        name: "Cart".to_owned(),
        file: FILE.to_owned(),
        place: Place {
            line: 5,
            column: 14,
        },
        fields: fields
            .into_iter()
            .map(|(known, declared)| FieldDraft {
                name: known.to_owned(),
                shape: declared,
            })
            .collect(),
        actions: Vec::new(),
        rules: Vec::new(),
        ends: Vec::new(),
        view: ViewDraft::default(),
        constants: Vec::new(),
        helpers: Vec::new(),
    }
}

pub(crate) fn helper(known: &str, parameters: &[&str], body: Term) -> HelperDraft {
    HelperDraft {
        name: known.to_owned(),
        parameters: parameters
            .iter()
            .map(|parameter| (*parameter).to_owned())
            .collect(),
        body,
    }
}

pub(crate) fn lowered(source: &Draft) -> Model {
    match lower(source) {
        Ok(model) => model,
        Err(problems) => panic!("the draft should lower, but: {problems:?}"),
    }
}

pub(crate) fn refused(source: &Draft) -> Vec<Problem> {
    match lower(source) {
        Ok(_) => panic!("the draft should be refused"),
        Err(problems) => problems,
    }
}

pub(crate) fn the_problem(source: &Draft) -> Problem {
    let mut problems = refused(source);
    assert_eq!(problems.len(), 1, "one problem expected in {problems:?}");
    problems.remove(0)
}

pub(crate) fn first_state(model: &Model) -> Vec<Value> {
    model
        .fields
        .iter()
        .map(|each| each.starts[0].clone())
        .collect()
}

pub(crate) fn with_rule(fields: Vec<(&str, Shape)>, condition: Term) -> Draft {
    let mut source = draft(fields);
    source.rules.push(rule("checked", condition));
    source
}

pub(crate) fn verdict_of(source: &Draft) -> Result<bool, Fault> {
    let model = lowered(source);
    let state = first_state(&model);
    let checked = &model.rules[0];
    truth(&checked.holds, &mut Frame::new(&state, checked.slots))
}

pub(crate) fn verdict(fields: Vec<(&str, Shape)>, condition: Term) -> Result<bool, Fault> {
    verdict_of(&with_rule(fields, condition))
}

pub(crate) fn holds(fields: Vec<(&str, Shape)>, condition: Term) -> bool {
    verdict(fields, condition) == Ok(true)
}

pub(crate) fn fails(fields: Vec<(&str, Shape)>, condition: Term) -> bool {
    verdict(fields, condition) == Ok(false)
}

pub(crate) fn refusal(fields: Vec<(&str, Shape)>, condition: Term) -> Problem {
    the_problem(&with_rule(fields, condition))
}

pub(crate) fn after(model: &Model, position: usize, inputs: &[Value]) -> Vec<Value> {
    let state = first_state(model);
    let taken = &model.actions[position];
    let mut frame = Frame::new(&state, taken.slots);
    for (each, value) in taken.inputs.iter().zip(inputs) {
        frame.put(each.slot, value.clone());
    }
    state
        .iter()
        .enumerate()
        .map(|(index, old)| match taken.updates.get(index) {
            Some(Some(update)) => {
                eval(update, &mut frame).expect("the update has a value in the start state")
            }
            Some(None) | None => old.clone(),
        })
        .collect()
}
