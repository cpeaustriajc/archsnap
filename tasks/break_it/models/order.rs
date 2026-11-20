use earnest::model::{Model, model, require};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Order {
    #[range(0, 3)]
    #[initial(None)]
    num: Option<u8>,
}

#[model]
impl Order {
    #[action("the number is set")]
    fn set(&self) -> Option<Self> {
        require!(self.num.is_none());
        Some(Self { num: Some(1), ..self.clone() })
    }

    #[rule("The number is below five")]
    fn ordered(&self) -> bool {
        self.num < Some(5)
    }

    #[end("The number is set")]
    fn done(&self) -> bool {
        self.num.is_some()
    }
}
