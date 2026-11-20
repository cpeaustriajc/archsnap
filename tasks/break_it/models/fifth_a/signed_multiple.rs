use earnest::model::{Model, model, require};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Even {
    #[range(0, 3)]
    #[initial(0)]
    n: i8,
}

#[model]
impl Even {
    #[action("two up")]
    fn inc(&self) -> Option<Self> {
        require!(self.n < 2);
        Some(Self { n: self.n + 2 })
    }

    #[rule("The count is even")]
    fn even(&self) -> bool {
        self.n.is_multiple_of(2)
    }

    #[end("At two")]
    fn top(&self) -> bool {
        self.n == 2
    }
}
