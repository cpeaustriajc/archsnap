use earnest::model::{Model, model, require};

const TOP: u8 = 3;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Pick {
    #[range(0, 3)]
    #[initial(None)]
    chosen: Option<u8>,
}

#[model]
impl Pick {
    #[action("a number is chosen")]
    fn choose(&self, #[range(0, 3)] number: u8) -> Option<Self> {
        require!(self.chosen.is_none());
        Some(Self {
            chosen: Some(number),
        })
    }

    #[rule("A chosen number is the top")]
    fn only_top(&self) -> bool {
        if let Some(TOP) = self.chosen {
            true
        } else {
            self.chosen.is_none()
        }
    }

    #[end("A number is chosen")]
    fn done(&self) -> bool {
        self.chosen.is_some()
    }
}
