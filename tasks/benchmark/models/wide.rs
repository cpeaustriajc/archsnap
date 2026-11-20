use earnest::model::{Model, model};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Wide {
    #[range(0, 99)]
    #[initial(0)]
    x: u8,
    #[range(0, 99)]
    #[initial(0)]
    y: u8,
}

#[model]
impl Wide {
    #[action("the wheels spin by three amounts")]
    fn spin(
        &self,
        #[range(0, 9)] a: u8,
        #[range(0, 9)] b: u8,
        #[range(0, 9)] c: u8,
    ) -> Option<Self> {
        Some(Self { x: (self.x + a) % 100, y: (self.y + b + c) % 100 })
    }

    #[rule("Both wheels stay on their dials")]
    fn dials(&self) -> bool {
        self.x <= 99 && self.y <= 99
    }
}
