use earnest::model::{Model, model, require};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Climb {
    #[range(0, 3)]
    #[initial(0)]
    n: u8,
}

#[model]
impl Climb {
    #[action("one up")]
    fn inc(&self) -> Self {
        require!(self.n < 1);
        Self { n: self.n + 1 }
    }

    #[rule("The count stays at most one")]
    fn low(&self) -> bool {
        self.n <= 1
    }

    #[end("At one")]
    fn one(&self) -> bool {
        self.n == 1
    }
}
