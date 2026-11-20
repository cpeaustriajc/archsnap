use earnest::model::{Domain, Model, Set, model, require};

const MOST_DRINKS: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Guest {
    Ann,
    Bob,
    Cy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Measure {
    Single,
    Double,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Party {
    inside: Set<Guest>,
    #[range(0, MOST_DRINKS)]
    #[initial(0)]
    drinks: u8,
}

fn strength(measure: Measure) -> u8 {
    if measure == Measure::Double {
        return 1;
    }
    0
}

#[model]
impl Party {
    #[action("a guest arrives")]
    fn arrive(&self, guest: Guest) -> Option<Self> {
        require!(!self.inside.has(guest));
        require!(guest != Guest::Cy || self.has_company());
        Some(Self {
            inside: self.inside.add(guest),
            ..self.clone()
        })
    }

    #[action("the host pours a round")]
    fn pour(&self, #[range(1, 2)] glasses: u8, measure: Measure) -> Option<Self> {
        require!(self.drinks + glasses + strength(measure) <= MOST_DRINKS);
        let poured = glasses + strength(measure);
        if poured > 2 {
            return Some(Self {
                drinks: MOST_DRINKS,
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
        !self.inside.has(Guest::Cy) || self.inside.size() > 1
    }

    #[rule("At most two drinks are poured")]
    fn sober(&self) -> bool {
        self.drinks <= 2
    }

    #[end("The bar is dry")]
    fn dry(&self) -> bool {
        self.drinks == MOST_DRINKS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_runs_dry_in_one_strong_round() {
        let mut state = Party::starts().remove(0);
        for guest in [Guest::Ann, Guest::Cy] {
            state = state.arrive(guest).unwrap();
        }
        println!("{state:?}");
        assert!(state.accompanied(), "Cy came after Ann");
        let dry = state.pour(2, Measure::Double).unwrap();
        assert_eq!(dry.drinks, 3, "two doubles empty the bar");
        assert!(dry.dry() && !dry.sober(), "the end that breaks the rule");
    }
}
