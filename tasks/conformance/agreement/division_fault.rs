use earnest::model::{Model, model};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Split {
    #[range(0, 3)]
    #[initial(3)]
    n: u8,
    #[range(0, 3)]
    #[initial(0)]
    q: u8,
}

#[model]
impl Split {
    #[action("the number is cut")]
    fn cut(&self, #[range(0, 2)] by: u8) -> Self {
        Self {
            q: self.n / by,
            ..self.clone()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Below {
    #[range(-1, 1)]
    #[initial(-1)]
    n: i8,
    #[range(-1, 1)]
    #[initial(0)]
    q: i8,
}

#[model]
impl Below {
    #[action("the number is cut")]
    fn slice(&self) -> Self {
        Self {
            q: self.n / 1,
            ..self.clone()
        }
    }
}
