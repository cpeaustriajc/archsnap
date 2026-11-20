use r#earnest::model::{Model, model, require};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Climb {
    #[range(0, 3)]
    #[initial(0)]
    n: u8,
}

#[model]
impl Climb {
    #[action("one up")]
    fn inc(&self) -> Option<Self> {
        require!(self.n < 3);
        Some(Self { n: self.n + 1 })
    }

    #[rule("The count stays at most one")]
    fn low(&self) -> bool {
        self.n <= 1
    }

    #[end("The count is three")]
    fn top(&self) -> bool {
        self.n == 3
    }
}
