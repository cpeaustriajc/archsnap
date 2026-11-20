use earnest::model::{Model, model, require};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Tried {
    #[range(1, 3)]
    #[initial(None)]
    last: Option<u8>,
    #[range(0, 9)]
    #[initial(0)]
    total: u8,
}

fn shifted(state: &Tried, by: u8) -> Tried {
    Tried {
        total: state.total + by,
        ..state.clone()
    }
}

#[model]
impl Tried {
    #[action("a number is put in")]
    fn put(&self, #[range(1, 3)] n: u8) -> Self {
        Self {
            last: Some(n),
            ..self.clone()
        }
    }

    #[action("the number is added")]
    fn add(&self) -> Option<Self> {
        let Some(x) = self.last else {
            return None;
        };
        require!(self.total + x <= 9);
        Some(Self {
            total: self.total + x,
            last: None,
            ..self.clone()
        })
    }

    #[action("the total is shifted by two")]
    fn bump(&self) -> Option<Self> {
        require!(self.total < 7);
        Some(shifted(self, 2))
    }

    #[rule("The total stays below eight")]
    fn small(&self) -> bool {
        self.total < 8
    }
}
