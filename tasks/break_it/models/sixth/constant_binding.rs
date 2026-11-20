use earnest::model::{Model, model, require};

const TOP: u8 = 3;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Take {
    #[range(0, 3)]
    #[initial(None)]
    chosen: Option<u8>,
    #[range(0, 3)]
    #[initial(0)]
    taken: u8,
}

#[model]
impl Take {
    #[action("a number is chosen")]
    fn choose(&self, #[range(0, 3)] number: u8) -> Option<Self> {
        require!(self.chosen.is_none());
        Some(Self {
            chosen: Some(number),
            ..self.clone()
        })
    }

    #[action("the chosen number is taken")]
    fn take(&self) -> Option<Self> {
        let TOP = self.chosen?;
        require!(self.taken == 0);
        Some(Self {
            taken: TOP,
            ..self.clone()
        })
    }

    #[rule("The taken number is below the top")]
    fn below(&self) -> bool {
        self.taken < 3
    }
}
