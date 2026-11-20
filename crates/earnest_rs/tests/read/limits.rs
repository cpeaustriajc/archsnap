use crate::{drafts, search};
use earnest_check::{FindingKind, Status, Verdict};
use earnest_rs::read;

fn with_room<T: Send>(work: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn_scoped(scope, work)
            .expect("a thread with room starts")
            .join()
            .expect("the work does not panic")
    })
}

fn smallest(extra_rules: &str) -> String {
    format!(
        "use earnest::model::{{Model, model, require}};

const LOW: i8 = -128;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Min {{
    #[range(-128, -126)]
    #[initial(-126)]
    y: i8,
}}

#[model]
impl Min {{
    #[action(\"down\")]
    fn down(&self) -> Option<Self> {{
        require!(self.y > -128);
        Some(Self {{ y: self.y - 1 }})
    }}

    #[rule(\"least\")]
    fn least(&self) -> bool {{
        self.y >= -128
    }}

    #[rule(\"never\")]
    fn never(&self) -> bool {{
        self.y != -128
    }}

    #[rule(\"never low\")]
    fn never_low(&self) -> bool {{
        self.y != LOW
    }}

    #[rule(\"never suffixed\")]
    fn never_suffixed(&self) -> bool {{
        self.y != -128i8
    }}

    #[end(\"The number is the smallest\")]
    fn bottom(&self) -> bool {{
        self.y == -128
    }}
{extra_rules}}}
"
    )
}

#[test]
fn the_smallest_number_of_a_signed_type_is_one_number_of_that_type() {
    let outcome = search(&drafts("src/min.rs", &smallest(""))[0]);
    assert_eq!(outcome.states, 3, "-126, -127 and -128");
    let verdicts: Vec<(&str, Verdict)> = outcome
        .rules
        .iter()
        .map(|rule| (rule.id.as_str(), rule.verdict))
        .collect();
    assert_eq!(
        verdicts,
        [
            ("least", Verdict::Holds),
            ("never", Verdict::Broken),
            ("never_low", Verdict::Broken),
            ("never_suffixed", Verdict::Broken),
        ],
        "no `i8` is below -128, and the last state is -128 however it is written"
    );
    assert!(
        outcome.findings.is_empty(),
        "nothing leaves `i8`: {:?}",
        outcome.findings
    );
}

#[test]
fn the_smallest_number_of_every_signed_type_starts_a_field_and_bounds_a_range() {
    for (ty, lowest) in [
        ("i8", "-128"),
        ("i16", "-32768"),
        ("i32", "-2147483648"),
        ("i64", "-9223372036854775808"),
    ] {
        let source = format!(
            "use earnest::model::{{Model, model}};

const BOTTOM: {ty} = {lowest};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Low {{
    #[range({lowest}, {lowest})]
    #[initial({lowest})]
    n: {ty},
}}

#[model]
impl Low {{
    #[rule(\"The number is the smallest\")]
    fn bottom(&self) -> bool {{
        self.n == BOTTOM && self.n == {lowest}
    }}

    #[end(\"Always done\")]
    fn done(&self) -> bool {{
        true
    }}
}}
"
        );
        let outcome = search(&drafts("src/low.rs", &source)[0]);
        assert_eq!(outcome.states, 1, "{ty}: one number");
        assert_eq!(outcome.status, Status::Pass, "{ty}: {:?}", outcome.findings);
    }
}

#[test]
fn a_negated_value_is_still_negated_and_the_minimum_negated_is_a_fault() {
    let extra = "
    #[rule(\"negated\")]
    fn negated(&self) -> bool {
        -self.y > 0
    }
";
    let outcome = search(&drafts("src/min.rs", &smallest(extra))[0]);
    let faults: Vec<_> = outcome
        .findings
        .iter()
        .filter(|finding| finding.kind == FindingKind::Fault)
        .collect();
    assert_eq!(faults.len(), 1, "one state faults: {:?}", outcome.findings);
    assert_eq!(
        faults[0].message, "the rule `negated` computes a number outside `i8` in this state",
        "-(-128) is 128, which is not an `i8`"
    );
    assert_eq!(faults[0].path.steps.len(), 2, "after two steps, at -128");
}

fn rule_with(condition: &str) -> String {
    format!(
        "use earnest::model::{{Model, model, require}};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Deep {{
    #[range(0, 3)]
    #[initial(0)]
    n: u8,
}}

#[model]
impl Deep {{
    #[action(\"one up\")]
    fn inc(&self) -> Option<Self> {{
        require!(self.n < 3);
        Some(Self {{ n: self.n + 1 }})
    }}

    #[rule(\"The count is at most three\")]
    fn low(&self) -> bool {{
        {condition}
    }}

    #[end(\"The count is three\")]
    fn top(&self) -> bool {{
        self.n == 3
    }}
}}
"
    )
}

#[test]
fn an_expression_nested_past_the_limit_is_one_refusal_with_a_sentence() {
    let sums = format!("self.n {}<= 3", "+ 0 ".repeat(3_000));
    let reading = with_room(|| read("src/deep.rs", &rule_with(&sums)));
    assert_eq!(reading.drafts, [], "no model of a refused file");
    assert!(
        reading
            .problems
            .iter()
            .all(|problem| problem.message.contains("1000 levels deep")),
        "every problem is the one sentence: {:?}",
        reading.problems
    );
    assert!(!reading.problems.is_empty(), "the file is refused");
}

#[test]
fn a_chain_of_conditions_or_of_else_if_is_refused_past_the_limit_and_read_below_it() {
    let many = format!("self.n <= 3 {}", "&& self.n <= 3 ".repeat(10_000));
    let reading = with_room(|| read("src/deep.rs", &rule_with(&many)));
    assert_eq!(reading.drafts, [], "10 000 terms are too many to read");
    let chain = format!(
        "if self.n == 9 {{ false }} {}else {{ true }}",
        "else if self.n == 9 { false } ".repeat(3_000)
    );
    let reading = with_room(|| read("src/deep.rs", &rule_with(&chain)));
    assert_eq!(reading.drafts, [], "3 000 `else if` are too many too");
    let few = format!("self.n <= 3 {}", "&& self.n <= 3 ".repeat(400));
    let model = with_room(|| search(&drafts("src/deep.rs", &rule_with(&few))[0]));
    assert_eq!(model.status, Status::Pass, "400 terms are read and hold");
}

#[test]
fn brackets_and_negations_nested_past_the_limit_are_refused_before_the_parser_runs() {
    let parentheses = format!("{}self.n <= 3{}", "(".repeat(20_000), ")".repeat(20_000));
    let negations = format!("{}(self.n <= 3)", "!".repeat(20_000));
    for condition in [parentheses, negations] {
        let reading = read("src/deep.rs", &rule_with(&condition));
        assert_eq!(
            reading.problems.len(),
            1,
            "one problem: {:?}",
            reading.problems
        );
        assert!(
            reading.problems[0].message.contains("1000 levels deep"),
            "the sentence names the limit: {}",
            reading.problems[0].message
        );
    }
}
