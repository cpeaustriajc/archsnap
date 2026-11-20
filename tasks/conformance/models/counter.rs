use earnest::model::{Model, model, require};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Counter {
    #[range(0, 3)]
    #[initial(0)]
    count: u8,
}

#[model]
impl Counter {
    #[action("the counter goes up by one")]
    fn increment(&self) -> Option<Self> {
        require!(self.count < 3);
        Some(Self { count: self.count + 1, ..self.clone() })
    }

    #[rule("The count is never above three")]
    fn bounded(&self) -> bool {
        self.count <= 3
    }

    #[rule("Twice the count is even")]
    fn even(&self) -> bool {
        (self.count * 2) % 2 == 0
    }

    #[end("The counter is full")]
    fn full(&self) -> bool {
        self.count == 3
    }
}
