use crate::{only_draft, search};
use earnest_check::{FindingKind, Status};
use earnest_rs::read;

const HEAD: &str = "\
use earnest::model::{Model, model, require};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Meter {
    #[range(0, 6)]
    #[initial(0)]
    count: u8,
}

#[model]
impl Meter {
";

fn meter(methods: &str) -> String {
    format!("{HEAD}{methods}}}\n")
}

#[test]
fn an_action_that_nothing_can_stop_may_return_the_state_without_an_option() {
    let source = meter(
        "    #[action(\"the meter ticks\")]
    fn tick(&self) -> Self {
        Self { count: (self.count + 1) % 3 }
    }

    #[rule(\"The meter stays low\")]
    fn low(&self) -> bool {
        self.count < 3
    }
",
    );
    let outcome = search(&only_draft("meter.rs", &source));
    assert_eq!(outcome.states, 3, "0, 1, 2 and round again");
    assert_eq!(
        outcome.status,
        Status::Pass,
        "the rule holds and nothing is stuck"
    );
    assert!(
        outcome
            .findings
            .iter()
            .all(|finding| finding.kind != FindingKind::Stuck),
        "the action can always happen, so nothing is stuck"
    );
}

#[test]
fn an_action_returns_option_self_or_self_and_nothing_else() {
    let source = meter(
        "    #[action(\"the meter ticks\")]
    fn tick(&self) -> bool {
        true
    }
",
    );
    let reading = read("meter.rs", &source);
    assert_eq!(reading.problems.len(), 1, "{:?}", reading.problems);
    assert_eq!(reading.problems[0].feature, "`fn tick`");
    assert!(
        reading.problems[0]
            .message
            .contains("returns `Option<Self>`"),
        "{:?}",
        reading.problems
    );
}

#[test]
fn is_multiple_of_is_a_remainder_that_is_zero_and_a_zero_divisor_only_divides_zero() {
    let source = meter(
        "    #[action(\"the meter counts\")]
    fn count(&self) -> Option<Self> {
        require!(self.count < 6);
        Some(Self { count: self.count + 1 })
    }

    #[rule(\"A multiple of three is a multiple of its half\")]
    fn halves(&self) -> bool {
        !self.count.is_multiple_of(3) || (self.count * 2).is_multiple_of(6)
    }

    #[rule(\"Only zero is a multiple of zero\")]
    fn zero(&self) -> bool {
        self.count.is_multiple_of(0) == (self.count == 0)
    }

    #[end(\"The meter is full\")]
    fn full(&self) -> bool {
        self.count == 6
    }
",
    );
    let outcome = search(&only_draft("meter.rs", &source));
    assert_eq!(outcome.status, Status::Pass, "{:?}", outcome.findings);
    assert_eq!(outcome.states, 7, "0 to 6");
}

fn refusals_of_an_action(action: &str) -> Vec<String> {
    let source = meter(&format!(
        "    #[action(\"the meter ticks\")]\n{action}\n\n    #[rule(\"The meter stays low\")]\n    \
         fn low(&self) -> bool {{\n        self.count < 3\n    }}\n"
    ));
    read("meter.rs", &source)
        .problems
        .into_iter()
        .map(|problem| problem.feature)
        .collect()
}

#[test]
fn an_action_that_returns_self_has_no_require_none_some_or_question_mark() {
    for body in [
        "require!(self.count < 3);\n        Self { count: self.count + 1 }",
        "if self.count < 3 {\n            require!(self.count < 2);\n            return Self { count: 1 };\n        }\n        Self { count: 2 }",
        "return None;",
        "if self.count > 1 {\n            return None;\n        }\n        Self { count: 2 }",
        "None",
        "Some(Self { count: self.count + 1 })",
        "if self.count > 1 {\n            return Some(Self { count: 1 });\n        }\n        Self { count: 2 }",
        "Self { count: 1 }?",
        "let next = self.count + 1;\n        Some(Self { count: next })",
        "if self.count > 1 { Some(Self { count: 1 }) } else { Self { count: 2 } }",
    ] {
        let found = refusals_of_an_action(&format!(
            "    fn tick(&self) -> Self {{\n        {body}\n    }}"
        ));
        assert!(
            !found.is_empty(),
            "`{body}` in an action that returns `Self` does not compile"
        );
    }
}

#[test]
fn an_action_that_returns_option_self_gives_back_some_of_the_state() {
    for body in [
        "Self { count: self.count + 1 }",
        "require!(self.count < 3);\n        Self { count: self.count + 1 }",
        "if self.count > 1 {\n            return Self { count: 1 };\n        }\n        Some(Self { count: 2 })",
        "Some(Some(Self { count: 1 }))",
        "if self.count > 1 { Some(Self { count: 1 }) } else { Self { count: 2 } }",
        "let next = self.count + 1;\n        Self { count: next }",
    ] {
        let found = refusals_of_an_action(&format!(
            "    fn tick(&self) -> Option<Self> {{\n        {body}\n    }}"
        ));
        assert!(
            !found.is_empty(),
            "`{body}` in an action that returns `Option<Self>` does not compile"
        );
    }
}

#[test]
fn both_return_types_are_read_with_the_value_they_take() {
    for action in [
        "    fn tick(&self) -> Self {\n        Self { count: 1 }\n    }",
        "    fn tick(&self) -> Self {\n        if self.count > 1 {\n            return Self { count: 1 };\n        }\n        Self { count: 2 }\n    }",
        "    fn tick(&self) -> Self {\n        let next = self.count + 1;\n        Self { count: next % 3 }\n    }",
        "    fn tick(&self) -> Option<Self> {\n        require!(self.count < 3);\n        Some(Self { count: self.count + 1 })\n    }",
        "    fn tick(&self) -> Option<Self> {\n        if self.count > 1 {\n            return Some(Self { count: 1 });\n        }\n        Some(Self { count: 2 })\n    }",
        "    fn tick(&self) -> Option<Self> {\n        Some(if self.count > 1 { Self { count: 1 } } else { Self { count: 2 } })\n    }",
    ] {
        assert_eq!(
            refusals_of_an_action(action),
            Vec::<String>::new(),
            "{action}"
        );
    }
}

fn lowered_with_rule(rule_body: &str, field_type: &str) -> bool {
    let source = format!(
        "use earnest::model::{{Model, model}};\n\n\
         #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]\n\
         pub struct Meter {{\n    #[range(0, 3)]\n    #[initial(0)]\n    count: {field_type},\n}}\n\n\
         #[model]\nimpl Meter {{\n    #[rule(\"A rule\")]\n    fn low(&self) -> bool {{\n        {rule_body}\n    }}\n}}\n"
    );
    earnest_semantic::lower(&only_draft("meter.rs", &source)).is_ok()
}

#[test]
fn is_multiple_of_is_a_method_of_the_unsigned_integers_only() {
    for unsigned in ["u8", "u16", "u32", "u64", "u128", "usize"] {
        assert!(
            lowered_with_rule("self.count.is_multiple_of(2)", unsigned),
            "`is_multiple_of` on `{unsigned}`"
        );
    }
    for signed in ["i8", "i16", "i32", "i64", "i128", "isize"] {
        for rule in [
            "self.count.is_multiple_of(2)",
            "(self.count + 1).is_multiple_of(2)",
            "!self.count.is_multiple_of(2)",
            "self.count.is_multiple_of(self.count)",
        ] {
            assert!(
                !lowered_with_rule(rule, signed),
                "`{rule}` on `{signed}` is refused by rustc (E0599)"
            );
        }
    }
    assert!(
        !lowered_with_rule("6.is_multiple_of(2)", "u8"),
        "a number written out has no type to call the method on (E0689)"
    );
    assert!(
        lowered_with_rule("(self.count + 1_u8).is_multiple_of(2)", "u8"),
        "a sum of unsigned numbers is unsigned"
    );
}
