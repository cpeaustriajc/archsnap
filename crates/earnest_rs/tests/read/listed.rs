use crate::{only_draft, search, shown};
use earnest_ast::{ShapeKind, TermKind};
use earnest_check::{FindingKind, Status, Verdict};
use earnest_rs::read;
use earnest_semantic::lower;

const PICK: &str = include_str!("../../../../tasks/conformance/twins/pick.rs");

const HEAD: &str = "\
use earnest::model::{Model, model, require};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Shelf {
";

fn shelf(fields: &str, rest: &str) -> String {
    format!("{HEAD}{fields}}}\n\n#[model]\nimpl Shelf {{\n{rest}}}\n")
}

fn refused(source: &str) -> Vec<(String, String)> {
    let reading = read("shelf.rs", source);
    assert_eq!(
        reading.drafts,
        [],
        "no model of an unreadable file is given"
    );
    reading
        .problems
        .into_iter()
        .map(|problem| {
            (
                problem.feature.trim_matches('`').to_owned(),
                problem.message,
            )
        })
        .collect()
}

const ONE_RULE: &str =
    "    #[rule(\"A shelf is a shelf\")]\n    fn shelf(&self) -> bool {\n        true\n    }\n";

#[test]
fn a_listed_number_is_a_choice_of_numbers_that_keeps_its_integer_type() {
    let draft = only_draft("pick.rs", PICK);
    let ShapeKind::OneOf(sizes) = &draft.fields[0].shape.kind else {
        panic!(
            "`#[one_of(1, 5, 9)]` is a choice: {:?}",
            draft.fields[0].shape.kind
        );
    };
    let numbers: Vec<i64> = sizes
        .iter()
        .map(|size| match &size.kind {
            TermKind::Declared { value, .. } => match value.kind {
                TermKind::Int(number) => number,
                _ => panic!("a number: {value:?}"),
            },
            other => panic!("a typed number: {other:?}"),
        })
        .collect();
    assert_eq!(
        numbers,
        [1, 5, 9],
        "the numbers in the order they are listed"
    );
    let ShapeKind::Optional(inner) = &draft.fields[1].shape.kind else {
        panic!("an optional text: {:?}", draft.fields[1].shape.kind);
    };
    let ShapeKind::OneOf(buyers) = &inner.kind else {
        panic!(
            "`#[one_of(\"Kim\", \"Lee\")]` is a choice: {:?}",
            inner.kind
        );
    };
    let texts: Vec<&TermKind> = buyers.iter().map(|buyer| &buyer.kind).collect();
    assert_eq!(
        texts,
        [
            &TermKind::Text("Kim".to_owned()),
            &TermKind::Text("Lee".to_owned())
        ],
        "the texts in the order they are listed"
    );
    let resize = &draft.actions[0];
    assert!(
        matches!(&resize.inputs[0].1.kind, ShapeKind::OneOf(listed) if listed.len() == 3),
        "an input is listed the same way: {:?}",
        resize.inputs
    );
}

#[test]
fn a_listed_choice_gives_nine_states_and_the_path_of_the_broken_rule() {
    let outcome = search(&only_draft("pick.rs", PICK));
    assert_eq!(outcome.status, Status::Fail, "the status");
    assert_eq!(outcome.states, 9, "3 sizes times 3 buyers");
    let fits = outcome
        .rules
        .iter()
        .find(|rule| rule.id == "fits")
        .expect("the rule is checked");
    assert_eq!(fits.verdict, Verdict::Broken, "the largest size breaks it");
    let path = fits.path.as_ref().expect("a path to the state");
    assert_eq!(
        shown(path),
        [
            "size = 9, buyer = null",
            "sell[who = \"Kim\"] -> size = 9, buyer = \"Kim\""
        ],
        "the start with the largest size, then one sale"
    );
    assert!(
        outcome
            .rules
            .iter()
            .any(|rule| rule.id == "listed" && rule.verdict == Verdict::Holds),
        "the buyer is always on the list"
    );
}

#[test]
fn a_listed_number_is_computed_in_its_integer_type() {
    let source = shelf(
        "    #[one_of(1, 255)]\n    count: u8,\n",
        "    #[rule(\"The next count is above zero\")]\n    fn above(&self) -> bool {\n        self.count + 1 > 0\n    }\n",
    );
    let draft = only_draft("shelf.rs", &source);
    let outcome = search(&draft);
    assert_eq!(outcome.states, 2, "the two listed counts");
    let finding = outcome
        .findings
        .iter()
        .find(|finding| finding.kind == FindingKind::Fault)
        .expect("255 + 1 does not fit a `u8`");
    assert!(
        finding.message.contains("outside `u8`"),
        "{}",
        finding.message
    );
}

#[test]
fn a_listed_text_may_be_compared_assigned_and_a_start_value() {
    let source = shelf(
        "    #[one_of(\"open\", \"shut\")]\n    #[initial(\"open\")]\n    door: &'static str,\n",
        "    #[action(\"the door shuts\")]\n    fn close(&self) -> Option<Self> {\n        require!(self.door == \"open\");\n        Some(Self {\n            door: \"shut\",\n            ..self.clone()\n        })\n    }\n\n    #[rule(\"The door is open or shut\")]\n    fn known(&self) -> bool {\n        self.door == \"open\" || self.door == \"shut\"\n    }\n\n    #[end(\"The door is shut\")]\n    fn done(&self) -> bool {\n        self.door == \"shut\"\n    }\n",
    );
    let outcome = search(&only_draft("shelf.rs", &source));
    assert_eq!(outcome.states, 2, "open, then shut");
    assert_eq!(outcome.status, Status::Pass, "the rule holds");
}

#[test]
fn a_list_that_is_wrong_is_refused_at_the_list() {
    let cases = [
        (
            "    #[one_of(1, 300)]\n    count: u8,\n",
            "300",
            "a `u8` holds 0 to 255",
        ),
        (
            "    #[one_of()]\n    count: u8,\n",
            "#[one_of()]",
            "list the values",
        ),
        (
            "    #[one_of(1, count)]\n    count: u8,\n",
            "count",
            "whole number",
        ),
        (
            "    #[one_of(\"a\", 1)]\n    door: &'static str,\n",
            "1",
            "string literal",
        ),
        (
            "    #[one_of(\"a\")]\n    flag: bool,\n",
            "#[one_of(\"a\")]",
            "lists the values of a whole number",
        ),
        (
            "    #[one_of(1)]\n    #[range(0, 3)]\n    count: u8,\n",
            "#[one_of(1)]",
            "never both",
        ),
        ("    door: &'static str,\n", "&'static str", "a text needs"),
    ];
    for (fields, feature, message) in cases {
        let found = refused(&shelf(fields, ONE_RULE));
        assert_eq!(found.len(), 1, "one problem for {fields}: {found:?}");
        assert_eq!(found[0].0, feature, "the feature for {fields}");
        assert!(found[0].1.contains(message), "{message} in {found:?}");
    }
}

#[test]
fn a_value_listed_twice_and_a_mixed_list_are_refused_when_the_model_is_lowered() {
    for (fields, feature) in [
        (
            "    #[one_of(1, 5, 1)]\n    count: u8,\n",
            "the choice 1 listed twice",
        ),
        (
            "    #[one_of(\"a\", \"a\")]\n    door: &'static str,\n",
            "the choice \"a\" listed twice",
        ),
    ] {
        let draft = only_draft("shelf.rs", &shelf(fields, ONE_RULE));
        let problems = lower(&draft).expect_err("a list holds each value once");
        assert_eq!(problems[0].feature, feature, "{fields}");
    }
}
