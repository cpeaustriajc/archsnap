use earnest::model::{Model, model, require};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Weigh {
    #[range(0, 3)]
    #[initial(None)]
    chosen: Option<u8>,
}

#[model]
impl Weigh {
    #[action("a number is chosen")]
    fn choose(&self, #[range(0, 3)] number: u8) -> Option<Self> {
        require!(self.chosen.is_none());
        Some(Self {
            chosen: Some(number),
        })
    }

    #[rule("The weight stays small")]
    fn light(&self) -> bool {
        self.chosen.map_or(0, |weight: u16| weight * 100) <= 300
    }

    #[end("A number is chosen")]
    fn done(&self) -> bool {
        self.chosen.is_some()
    }
}
