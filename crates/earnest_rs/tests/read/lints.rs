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
