use earnest::model::{Model, model, require};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Wrap {
    #[range(0, 3)]
    #[initial(0)]
    n: u8,
}

#[model]
impl Wrap {
    #[action("the count goes up")]
    fn up(&self) -> Option<Self> {
        require!(self.n < 3);
        Some(Self { n: self.n + 1, ..self.clone() })
    }

    #[rule("One below the count is at most two")]
    fn below(&self) -> bool {
        self.n - 1 <= 2
    }

    #[end("The count is at the top")]
    fn top(&self) -> bool {
        self.n == 3
    }
}
