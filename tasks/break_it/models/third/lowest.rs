use earnest::model::{Model, model, require};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Lowest {
    #[range(-128, -126)]
    #[initial(-127)]
    x: i8,
}

#[model]
impl Lowest {
    #[action("the number goes down")]
    fn down(&self) -> Option<Self> {
        require!(self.x > -128);
        Some(Self { x: self.x - 1, ..self.clone() })
    }

    #[rule("The number is at least the smallest `i8`")]
    fn above(&self) -> bool {
        self.x >= -128
    }

    #[end("The number is at the bottom")]
    fn bottom(&self) -> bool {
        self.x == -128
    }
}
