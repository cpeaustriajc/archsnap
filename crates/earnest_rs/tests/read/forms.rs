use crate::{actions, drafts, only_draft, search};
use earnest_ast::{Binary, ShapeKind, Term, TermKind};
use earnest_check::{Status, Verdict};
use earnest_value::IntType;

const PARTY: &str = include_str!("models/party.rs");
const TICKET: &str = include_str!("models/ticket.rs");

fn name(term: &Term) -> Option<&str> {
    match &term.kind {
        TermKind::Name(found) => Some(found),
        _ => None,
    }
}

fn verdict(outcome: &earnest_check::Outcome, id: &str) -> Verdict {
    outcome
        .rules
        .iter()
        .find(|rule| rule.id == id)
        .map(|rule| rule.verdict)
        .expect("the rule is checked")
}

#[test]
fn a_field_is_declared_by_its_type_and_its_attributes() {
    let draft = only_draft("party.rs", PARTY);
    let fields: Vec<&str> = draft
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect();
    assert_eq!(fields, ["inside", "drinks"], "fields in declaration order");
    let ShapeKind::Set(item) = &draft.fields[0].shape.kind else {
        panic!("`Set<Guest>` is a set: {:?}", draft.fields[0].shape.kind);
    };
    let ShapeKind::OneOf(guests) = &item.kind else {
        panic!("`Guest` is an enum: {:?}", item.kind);
    };
    let variants: Vec<&TermKind> = guests.iter().map(|guest| &guest.kind).collect();
    assert_eq!(
        variants,
        [
            &TermKind::Text("Ann".to_owned()),
            &TermKind::Text("Bob".to_owned()),
            &TermKind::Text("Cy".to_owned()),
        ],
        "an enum is known by its variant names"
    );
    assert_eq!(draft.fields[0].shape.initial, None, "no `#[initial]`");
    assert_eq!(
        draft.fields[1].shape.kind,
        ShapeKind::Int {
            lo: 0,
            hi: 3,
            ty: Some(IntType::U8),
        },
        "a bound may be a constant of the file"
    );
    let initial = draft.fields[1]
        .shape
        .initial
        .as_ref()
        .map(|term| &term.kind);
    assert_eq!(initial, Some(&TermKind::Int(0)), "`#[initial(0)]`");
}

#[test]
fn an_action_takes_its_inputs_from_parameters_and_its_guard_from_every_require() {
    let draft = only_draft("party.rs", PARTY);
    let arrive = &draft.actions[0];
    assert_eq!(arrive.id, "arrive", "the id is the method name");
    assert_eq!(arrive.label, "a guest arrives", "the sentence");
    assert_eq!(
        arrive.change.state_name.as_deref(),
        Some("self"),
        "the state"
    );
    assert_eq!(
        arrive.change.input_name, None,
        "inputs are read by their own names"
    );
    assert_eq!(arrive.change.taken, ["guest"], "every parameter is taken");
    assert_eq!(
        arrive.guard.as_ref().map(|guard| guard.taken.as_slice()),
        Some(["guest".to_owned()].as_slice()),
        "the guard takes them too"
    );
    assert_eq!(arrive.inputs.len(), 1, "one parameter after `&self`");
    assert_eq!(arrive.inputs[0].0, "guest", "the input's name");
    assert!(
        matches!(arrive.inputs[0].1.kind, ShapeKind::OneOf(_)),
        "the input's declaration is its type"
    );
    let Some(TermKind::Binary(Binary::And, first, second)) =
        arrive.guard.as_ref().map(|guard| &guard.term.kind)
    else {
        panic!("two `require!`s are joined with and: {:?}", arrive.guard);
    };
    assert!(
        matches!(first.kind, TermKind::Unary(_, _)),
        "the first `require!` comes first: {first:?}"
    );
    assert!(
        matches!(second.kind, TermKind::Binary(Binary::Or, _, _)),
        "then the second: {second:?}"
    );
    let TermKind::Wrap(result) = &arrive.change.term.kind else {
        panic!("`Some(..)` wraps the result: {:?}", arrive.change);
    };
    let TermKind::Record { base, fields } = &result.kind else {
        panic!("`Self {{ .. }}` is a record: {result:?}");
    };
    assert_eq!(
        base.as_deref().and_then(name),
        Some("self"),
        "`..self.clone()` spreads the state"
    );
    assert_eq!(fields.len(), 1, "one changed field");
    assert_eq!(fields[0].0, "inside", "the changed field");
    let pour = &draft.actions[1];
    let inputs: Vec<&str> = pour
        .inputs
        .iter()
        .map(|(input, _)| input.as_str())
        .collect();
    assert_eq!(inputs, ["glasses", "measure"], "inputs in parameter order");
    assert_eq!(
        pour.inputs[0].1.kind,
        ShapeKind::Int {
            lo: 1,
            hi: 2,
            ty: Some(IntType::U8),
        },
        "`#[range]` on a parameter"
    );
}

#[test]
fn constants_functions_and_plain_methods_become_constants_and_helpers() {
    let draft = only_draft("party.rs", PARTY);
    assert_eq!(draft.constants.len(), 1, "one constant");
    assert_eq!(draft.constants[0].0, "MOST_DRINKS", "its name");
    let TermKind::Declared { shape, value } = &draft.constants[0].1.kind else {
        panic!("a constant has its type: {:?}", draft.constants[0].1);
    };
    assert_eq!(
        shape.kind,
        ShapeKind::Int {
            lo: 0,
            hi: 255,
            ty: Some(IntType::U8),
        },
        "the whole of the type it is declared with"
    );
    assert_eq!(value.kind, TermKind::Int(3), "its value");
    let helpers: Vec<(&str, Vec<&str>)> = draft
        .helpers
        .iter()
        .map(|helper| {
            let parameters = helper.parameters.iter().map(String::as_str).collect();
            (helper.name.as_str(), parameters)
        })
        .collect();
    assert_eq!(
        helpers,
        [("strength", vec!["measure"]), ("has_company", vec!["self"]),],
        "a method without an attribute is a helper that takes the state"
    );
    let TermKind::Bind { name, value, body } = &draft.helpers[0].body.kind else {
        panic!(
            "a parameter is bound with its type: {:?}",
            draft.helpers[0].body
        );
    };
    assert_eq!(name, "measure", "the parameter");
    assert!(
        matches!(&value.kind, TermKind::Declared { shape, .. }
            if matches!(shape.kind, ShapeKind::OneOf(_))),
        "its type is the enum: {value:?}"
    );
    let TermKind::Declared { shape, value } = &body.kind else {
        panic!("the result has the type the function returns: {body:?}");
    };
    assert!(
        matches!(
            shape.kind,
            ShapeKind::Int {
                ty: Some(IntType::U8),
                ..
            }
        ),
        "a `u8`: {shape:?}"
    );
    assert!(
        matches!(value.kind, TermKind::Choose(_, _, _)),
        "an early `return` is a choice: {value:?}"
    );
}

#[test]
fn the_party_model_gives_the_result_worked_out_by_hand() {
    let draft = only_draft("party.rs", PARTY);
    let outcome = search(&draft);
    assert_eq!(
        outcome.states, 28,
        "7 sets of guests (Cy is never alone) times drinks 0 to 3"
    );
    assert_eq!(outcome.status, Status::Fail, "`sober` is broken");
    assert_eq!(verdict(&outcome, "accompanied"), Verdict::Holds, "Cy");
    assert_eq!(verdict(&outcome, "sober"), Verdict::Broken, "three drinks");
    assert_eq!(outcome.findings, [], "three drinks is a marked end");
    let path = outcome.rules[1].path.as_ref().expect("a broken rule");
    assert_eq!(actions(path), ["pour"], "one strong round of two");
    let input: Vec<(&str, &str)> = path.steps[0]
        .input
        .iter()
        .map(|given| (given.name.as_str(), given.value.text.as_str()))
        .collect();
    assert_eq!(
        input,
        [("glasses", "2"), ("measure", "\"Double\"")],
        "the inputs of the step"
    );
}

#[test]
fn records_options_and_choices_over_a_type_give_the_result_worked_out_by_hand() {
    let draft = only_draft("ticket.rs", TICKET);
    let ShapeKind::Record(seat) = &draft.fields[0].shape.kind else {
        panic!("a struct that derives `Domain` is a record");
    };
    assert_eq!(seat[0].0, "row", "the record's first field");
    assert_eq!(
        seat[0].1.kind,
        ShapeKind::Int {
            lo: 1,
            hi: 2,
            ty: Some(IntType::U8),
        },
        "its range"
    );
    assert_eq!(seat[1].1.kind, ShapeKind::Bool, "the second field");
    assert!(
        matches!(&draft.fields[1].shape.kind, ShapeKind::Optional(inner)
            if matches!(inner.kind, ShapeKind::OneOf(_))),
        "`Option<Fan>` is an optional enum"
    );
    assert!(
        matches!(
            draft.fields[1]
                .shape
                .initial
                .as_ref()
                .map(|term| &term.kind),
            Some(TermKind::Null)
        ),
        "`None` is null"
    );
    let outcome = search(&draft);
    assert_eq!(outcome.states, 12, "2 rows x 2 sides x 3 owners");
    assert_eq!(verdict(&outcome, "seated"), Verdict::Holds, "seated");
    assert_eq!(verdict(&outcome, "owned"), Verdict::Holds, "owned");
    assert_eq!(verdict(&outcome, "shared"), Verdict::Holds, "shared");
    assert_eq!(verdict(&outcome, "kim"), Verdict::Broken, "kim");
    assert_eq!(
        outcome.findings,
        [],
        "an unsold seat moves, a sold one is returned"
    );
    let path = outcome.rules[3].path.as_ref().expect("a broken rule");
    let mut taken = actions(path);
    taken.sort_unstable();
    assert_eq!(
        taken,
        ["back", "buy", "flip"],
        "the seat moves and changes side, in either order, before Kim buys"
    );
}

#[test]
fn every_model_of_a_file_is_read_in_file_order() {
    let source = "\
use earnest::model::{Model, model};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Porch {
    lit: bool,
    dim: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Hall {
    #[range(0, 2)]
    #[initial(0)]
    level: u8,
    #[initial(false)]
    locked: bool,
}

#[model]
impl Hall {
    #[action(\"the switch turns one notch\")]
    fn turn(&self) -> Option<Self> {
        Some(Self { level: (self.level + 1) % 3, ..self.clone() })
    }
}

#[model]
impl Porch {
    #[action(\"the switch flips\")]
    fn flip(&self) -> Option<Self> {
        Some(Self { lit: !self.lit, ..self.clone() })
    }
}
";
    let found = drafts("switches.rs", source);
    let names: Vec<&str> = found.iter().map(|draft| draft.name.as_str()).collect();
    assert_eq!(names, ["Porch", "Hall"], "in the order of the structs");
    assert_eq!(
        found[0].actions[0].id, "flip",
        "each model has its own block"
    );
    assert_eq!(
        found[1].actions[0].id, "turn",
        "each model has its own block"
    );
    assert_eq!(found[1].place.line, 10, "the second model's line");
    assert_eq!(
        search(&found[0]).states,
        4,
        "lit and dim start at every value"
    );
    assert_eq!(search(&found[1]).states, 3, "level cycles 0, 1, 2");
}

#[test]
fn collections_are_written_with_of_and_a_range_reaches_numbers_inside_a_type() {
    let source = "\
use earnest::model::{Domain, Map, Model, Seq, Set, model, require};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
pub enum Item {
    Pen,
    Ink,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Stock {
    #[range(1, 2)]
    #[initial(Map::of([(Item::Pen, 1)]))]
    count: Map<Item, u8>,
    #[initial(Set::of([Item::Pen]))]
    seen: Set<Item>,
    #[initial(Seq::of([]))]
    log: Seq<Item, 1>,
}

#[model]
impl Stock {
    #[action(\"a pen is restocked\")]
    fn restock(&self) -> Option<Self> {
        require!(self.count.get(Item::Pen) == Some(1));
        Some(Self { count: self.count.set(Item::Pen, 2), ..self.clone() })
    }

    #[action(\"ink is logged\")]
    fn note(&self) -> Option<Self> {
        require!(self.log.is_empty());
        Some(Self {
            log: self.log.push(Item::Ink),
            seen: self.seen.clone().union(&Set::of([Item::Ink])),
            ..self.clone()
        })
    }

    #[rule(\"Everything logged was seen\")]
    fn known(&self) -> bool {
        self.log.every(|item| self.seen.has(item))
    }

    #[end(\"Both steps are done\")]
    fn done(&self) -> bool {
        self.count.get(Item::Pen) == Some(2) && self.log.size() == 1
    }
}
";
    let draft = only_draft("stock.rs", source);
    let ShapeKind::Map(_, counted) = &draft.fields[0].shape.kind else {
        panic!("`Map<Item, u8>` is a map");
    };
    assert_eq!(
        counted.kind,
        ShapeKind::Int {
            lo: 1,
            hi: 2,
            ty: Some(IntType::U8),
        },
        "the field's `#[range]` is the range of the number inside its type"
    );
    assert!(
        matches!(
            draft.fields[0].shape.initial.as_ref().map(|term| &term.kind),
            Some(TermKind::MapOf(entries)) if entries.len() == 1
        ),
        "`Map::of([(k, v)])`"
    );
    assert!(
        matches!(draft.fields[2].shape.kind, ShapeKind::Seq { max: 1, .. }),
        "`Seq<Item, 1>` holds at most one"
    );
    let outcome = search(&draft);
    assert_eq!(outcome.states, 4, "restocked or not, logged or not");
    assert_eq!(
        outcome.status,
        Status::Pass,
        "the rule holds and the last state is an end"
    );
}

#[test]
fn a_byte_order_mark_before_the_file_changes_nothing() {
    let plain = only_draft("party.rs", PARTY);
    let marked = only_draft("party.rs", &format!("\u{feff}{PARTY}"));
    assert_eq!(
        marked, plain,
        "the same model, with every rule's text and every place as without the mark"
    );
    assert_eq!(
        marked.rules[1].text, "self.drinks <= 2",
        "the text of a rule is cut from the right bytes"
    );
}
