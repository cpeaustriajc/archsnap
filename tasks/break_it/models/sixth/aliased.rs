use earnest::model::{Behavior as Walks, Claim, Model as Starts, Step};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Climb {
    n: u8,
}

impl Starts for Climb {
    fn starts() -> Vec<Self> {
        vec![Self { n: 0 }]
    }
}

impl Walks for Climb {
    fn steps(&self) -> Vec<Step<Self>> {
        if self.n < 3 {
            vec![Step {
                action: "inc",
                label: "the count goes up",
                input: Vec::new(),
                state: Self { n: self.n + 1 },
            }]
        } else {
            Vec::new()
        }
    }

    fn rules(&self) -> Vec<Claim> {
        vec![Claim {
            id: "low",
            says: "The count stays at most one",
            holds: self.n <= 1,
        }]
    }

    fn ends(&self) -> Vec<Claim> {
        vec![Claim {
            id: "top",
            says: "The count is at the top",
            holds: self.n == 3,
        }]
    }
}
