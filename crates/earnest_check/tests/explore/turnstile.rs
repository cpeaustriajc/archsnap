use crate::build::{steps, texts};
use earnest_ast::{
    ActionBody, ActionDraft, Binary, Draft, EndDraft, FieldDraft, Language, RuleDraft, Shape,
    ShapeKind, Term, TermKind, Unary, ViewDraft,
};
use earnest_check::{FindingKind, Limits, Outcome, Status, Verdict, explore};
use earnest_diagnostics::Place;
use earnest_semantic::lower;
use serde_json::json;

fn term(kind: TermKind) -> Term {
    Term::new(Place::default(), kind)
}

fn text(value: &str) -> Term {
    term(TermKind::Text(value.to_owned()))
}

fn field(name: &str) -> Term {
    let state = term(TermKind::Name("s".to_owned()));
    term(TermKind::Member(Box::new(state), name.to_owned()))
}

fn binary(op: Binary, left: Term, right: Term) -> Term {
    term(TermKind::Binary(op, Box::new(left), Box::new(right)))
}

fn not(inner: Term) -> Term {
    term(TermKind::Unary(Unary::Not, Box::new(inner)))
}

fn starting(kind: ShapeKind, initial: Term) -> Shape {
    Shape {
        place: Place::default(),
        kind,
        initial: Some(initial),
    }
}

fn action(id: &str, label: &str, changed: &str, value: Term) -> ActionDraft {
    let locked = binary(Binary::Equal, field("gate"), text("locked"));
    ActionDraft {
        id: id.to_owned(),
        label: label.to_owned(),
        place: Place::default(),
        inputs: Vec::new(),
        guard: Some(body(binary(Binary::And, locked, not(field("paid"))))),
        change: body(term(TermKind::Record {
            owner: None,
            base: None,
            fields: vec![(changed.to_owned(), value)],
        })),
    }
}

fn body(term: Term) -> ActionBody {
    ActionBody {
        state_name: Some("s".to_owned()),
        input_name: None,
        taken: Vec::new(),
        term,
    }
}

fn turnstile() -> Draft {
    let gate = ShapeKind::OneOf(vec![text("locked"), text("open")]);
    let stays_locked = binary(
        Binary::Or,
        binary(Binary::Equal, field("gate"), text("locked")),
        field("paid"),
    );
    Draft {
        name: "Turnstile".to_owned(),
        file: "src/turnstile.model.ts".to_owned(),
        language: Language::TypeScript,
        place: Place {
            line: 5,
            column: 14,
        },
        fields: vec![
            FieldDraft {
                name: "gate".to_owned(),
                shape: starting(gate, text("locked")),
            },
            FieldDraft {
                name: "paid".to_owned(),
                shape: starting(ShapeKind::Bool, term(TermKind::Bool(false))),
            },
        ],
        actions: vec![
            action("push", "the visitor pushes", "gate", text("open")),
            action(
                "pay",
                "the visitor pays",
                "paid",
                term(TermKind::Bool(true)),
            ),
        ],
        rules: vec![RuleDraft {
            id: "staysLocked".to_owned(),
            says: "The gate stays locked until the visitor pays".to_owned(),
            place: Place::default(),
            state_name: Some("s".to_owned()),
            holds: stays_locked,
            text: "s.gate === \"locked\" || s.paid".to_owned(),
        }],
        ends: vec![EndDraft {
            id: "through".to_owned(),
            says: "The visitor went through".to_owned(),
            place: Place::default(),
            state_name: Some("s".to_owned()),
            when: binary(Binary::Equal, field("gate"), text("open")),
        }],
        view: ViewDraft::default(),
        constants: Vec::new(),
        helpers: Vec::new(),
    }
}

fn checked() -> Outcome {
    let model = lower(&turnstile()).expect("the turnstile lowers");
    explore(&model, &Limits::default())
}

#[test]
fn the_readme_turnstile_has_three_states_and_fails() {
    let outcome = checked();
    assert_eq!(outcome.status, Status::Fail);
    assert_eq!(outcome.states, 3);
    assert_eq!(outcome.reason, None);
}

#[test]
fn the_readme_turnstile_breaks_its_rule_by_pushing_without_paying() {
    let outcome = checked();
    let rule = &outcome.rules[0];
    assert_eq!(rule.id, "staysLocked");
    assert_eq!(rule.says, "The gate stays locked until the visitor pays");
    assert_eq!(rule.verdict, Verdict::Broken);
    let path = rule.path.as_ref().expect("a broken rule has a path");
    assert_eq!(texts(&path.start), ["gate = \"locked\"", "paid = false"]);
    assert_eq!(steps(path), ["push  gate: \"locked\" -> \"open\""]);
}

#[test]
fn the_readme_turnstile_is_stuck_after_paying() {
    let outcome = checked();
    assert_eq!(outcome.findings.len(), 1);
    let finding = &outcome.findings[0];
    assert_eq!(finding.kind, FindingKind::Stuck);
    assert_eq!(
        finding.message,
        "no action can happen here, and the model does not mark it as an end"
    );
    assert_eq!(
        texts(&finding.path.start),
        ["gate = \"locked\"", "paid = false"]
    );
    assert_eq!(steps(&finding.path), ["pay  paid: false -> true"]);
}

#[test]
fn an_outcome_serializes_as_the_json_output_documents() {
    let start = json!([
        { "name": "gate", "value": "locked" },
        { "name": "paid", "value": false }
    ]);
    let expected = json!({
        "status": "fail",
        "states": 3,
        "reason": null,
        "rules": [{
            "id": "staysLocked",
            "says": "The gate stays locked until the visitor pays",
            "verdict": "broken",
            "path": {
                "start": start,
                "steps": [{
                    "action": "push",
                    "label": "the visitor pushes",
                    "input": [],
                    "changes": [{ "field": "gate", "before": "locked", "after": "open" }],
                    "state": [
                        { "name": "gate", "value": "open" },
                        { "name": "paid", "value": false }
                    ]
                }]
            }
        }],
        "findings": [{
            "kind": "stuck",
            "message": "no action can happen here, and the model does not mark it as an end",
            "path": {
                "start": start,
                "steps": [{
                    "action": "pay",
                    "label": "the visitor pays",
                    "input": [],
                    "changes": [{ "field": "paid", "before": false, "after": true }],
                    "state": [
                        { "name": "gate", "value": "locked" },
                        { "name": "paid", "value": true }
                    ]
                }]
            }
        }]
    });
    assert_eq!(serde_json::to_value(checked()).unwrap(), expected);
}

#[test]
fn the_kinds_verdicts_and_statuses_serialize_as_the_words_of_the_json_output() {
    assert_eq!(json!(FindingKind::OutOfRange), json!("out-of-range"));
    assert_eq!(json!(FindingKind::Fault), json!("fault"));
    assert_eq!(json!(Verdict::Holds), json!("holds"));
    assert_eq!(json!(Verdict::Unknown), json!("unknown"));
    assert_eq!(json!(Status::Pass), json!("pass"));
    assert_eq!(json!(Status::Unfinished), json!("unfinished"));
}
