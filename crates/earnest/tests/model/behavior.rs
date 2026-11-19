use crate::Guest::{self, Ann, Bob, Cy};
use earnest::model::{Behavior, Claim, Domain, Model, Set, Step, model, require};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Measure {
    Single,
    Double,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Party {
    inside: Set<Guest>,
    #[range(0, 3)]
    #[initial(0)]
    drinks: u8,
}

#[model]
impl Party {
    #[action("a guest arrives")]
    fn arrive(&self, guest: Guest) -> Option<Self> {
        require!(!self.inside.has(guest));
        require!(guest != Cy || self.has_company());
        Some(Self {
            inside: self.inside.add(guest),
            ..self.clone()
        })
    }

    #[action("the host pours a round")]
    fn pour(&self, #[range(1, 2)] glasses: u8, measure: Measure) -> Option<Self> {
        require!(self.drinks + glasses <= 3);
        let poured = if measure == Measure::Double {
            glasses + 1
        } else {
            glasses
        };
        if poured > 2 {
            return Some(Self {
                drinks: 3,
                ..self.clone()
            });
        }
        Some(Self {
            drinks: self.drinks + poured,
            ..self.clone()
        })
    }

    fn has_company(&self) -> bool {
        !self.inside.is_empty()
    }

    #[rule("Cy never drinks alone")]
    fn accompanied(&self) -> bool {
        !self.inside.has(Cy) || self.inside.size() > 1
    }

    #[rule("At most two drinks are poured")]
    fn sober(&self) -> bool {
        self.drinks <= 2
    }

    #[end("The bar is dry")]
    fn dry(&self) -> bool {
        self.drinks == 3
    }
}

fn party(inside: &[Guest], drinks: u8) -> Party {
    Party {
        inside: Set::of(inside.iter().copied()),
        drinks,
    }
}

fn arrival(guest: Guest, state: Party) -> Step<Party> {
    Step {
        action: "arrive",
        label: "a guest arrives",
        input: vec![("guest", format!("{guest:?}"))],
        state,
    }
}

fn round(glasses: u8, measure: Measure, state: Party) -> Step<Party> {
    Step {
        action: "pour",
        label: "the host pours a round",
        input: vec![
            ("glasses", glasses.to_string()),
            ("measure", format!("{measure:?}")),
        ],
        state,
    }
}

#[test]
fn an_action_method_takes_one_step_or_none() {
    let empty = party(&[], 0);
    assert_eq!(Party::starts(), [party(&[], 0)], "the one start state");
    assert_eq!(empty.arrive(Ann), Some(party(&[Ann], 0)), "Ann arrives");
    assert_eq!(empty.arrive(Cy), None, "Cy needs company");
    assert_eq!(party(&[Ann], 0).arrive(Ann), None, "Ann is inside already");
    assert_eq!(empty, party(&[], 0), "the state stepped from is unchanged");
}

#[test]
fn a_let_binding_and_an_early_return_run_as_ordinary_rust() {
    let empty = party(&[], 0);
    assert_eq!(
        empty.pour(1, Measure::Single),
        Some(party(&[], 1)),
        "one single"
    );
    assert_eq!(
        empty.pour(2, Measure::Double),
        Some(party(&[], 3)),
        "the early return"
    );
    assert_eq!(
        party(&[], 2).pour(2, Measure::Single),
        None,
        "the guard counts glasses"
    );
}

#[test]
fn steps_tries_every_action_with_every_input_in_order() {
    let empty = party(&[], 0);
    assert_eq!(
        empty.steps(),
        [
            arrival(Ann, party(&[Ann], 0)),
            arrival(Bob, party(&[Bob], 0)),
            round(1, Measure::Single, party(&[], 1)),
            round(1, Measure::Double, party(&[], 2)),
            round(2, Measure::Single, party(&[], 2)),
            round(2, Measure::Double, party(&[], 3)),
        ],
        "Cy can't arrive alone; the first input changes slowest"
    );
    let full = party(&[Ann, Bob, Cy], 3);
    assert_eq!(full.steps(), [], "everyone is inside and the bar is dry");
}

#[test]
fn rules_and_ends_are_judged_in_source_order() {
    let state = party(&[Cy], 3);
    assert_eq!(
        state.rules(),
        [
            Claim {
                id: "accompanied",
                says: "Cy never drinks alone",
                holds: false,
            },
            Claim {
                id: "sober",
                says: "At most two drinks are poured",
                holds: false,
            },
        ],
        "both rules break"
    );
    assert_eq!(
        state.ends(),
        [Claim {
            id: "dry",
            says: "The bar is dry",
            holds: true,
        }],
        "the one end marks the state"
    );
    let calm = party(&[Ann, Cy], 1);
    assert!(calm.accompanied() && calm.sober(), "a rule is a method");
    assert!(
        calm.rules().iter().all(|rule| rule.holds),
        "both rules hold"
    );
    assert!(!calm.dry(), "an end is a method");
}
