use earnest::model::{Model, model, require, view};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Dial {
    #[range(0, 5)]
    #[initial(0)]
    x: u8,
    #[range(0, 5)]
    #[initial(0)]
    y: u8,
}

#[model]
impl Dial {
    #[action("the dial turns east")]
    fn east(&self) -> Option<Self> {
        require!(self.x < 5);
        Some(Self {
            x: self.x + 1,
            ..self.clone()
        })
    }

    #[action("the dial turns north")]
    fn north(&self) -> Option<Self> {
        require!(self.y < 5);
        Some(Self {
            y: self.y + 1,
            ..self.clone()
        })
    }

    #[rule("The dial never reaches the far corner")]
    fn near(&self) -> bool {
        !(self.x == 5 && self.y == 5)
    }

    #[end("The dial is at the far corner")]
    fn far(&self) -> bool {
        self.x == 5 && self.y == 5
    }
}

#[view(title = "Dial")]
impl Dial {
    #[state]
    fn state(&self) -> String {
        format!("x{} y{}", self.x, self.y)
    }
}
