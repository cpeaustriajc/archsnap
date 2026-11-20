use earnest::model::{Model, model, require};

const LOW: i8 = -128;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Min {
    #[range(-128, -126)]
    #[initial(-126)]
    y: i8,
}

#[model]
impl Min {
    #[action("down")]
    fn down(&self) -> Option<Self> {
        require!(self.y > -128);
        Some(Self { y: self.y - 1 })
    }

    #[rule("The number is at least the smallest i8")]
    fn least(&self) -> bool {
        self.y >= -128
    }

    #[rule("The number is never the smallest i8")]
    fn never(&self) -> bool {
        self.y != -128
    }

    #[rule("The number is never the constant LOW")]
    fn never_low(&self) -> bool {
        self.y != LOW
    }

    #[end("The number is the smallest i8")]
    fn bottom(&self) -> bool {
        self.y == -128
    }
}
