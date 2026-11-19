use earnest::model::{Model, model, require};

fn double(number: u8) -> u8 {
    number + number
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Within {
    #[range(0, 3)]
    #[initial(0)]
    n: u8,
}

#[model]
impl Within {
    fn below(&self, limit: u8) -> bool {
        self.n < limit
    }

    #[action("the count goes up")]
    fn up(&self) -> Option<Self> {
        require!(self.below(3));
        Some(Self { n: self.n + 1, ..self.clone() })
    }

    #[rule("The count is below two")]
    fn low(&self) -> bool {
        self.below(double(1))
    }

    #[end("The count is three")]
    fn top(&self) -> bool {
        !self.below(3)
    }
}
