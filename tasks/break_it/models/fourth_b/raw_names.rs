use earnest::model::{Domain, Model, model, require};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
pub enum Kind {
    Plain,
    Fancy,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Raw {
    #[range(0, 3)]
    #[initial(0)]
    r#type: u8,
    #[initial(Kind::Plain)]
    r#match: Kind,
}

#[model]
impl Raw {
    #[action("one up")]
    fn r#loop(&self, r#in: bool) -> Option<Self> {
        require!(self.r#type < 3 && r#in);
        Some(Self {
            r#type: self.r#type + 1,
            r#match: Kind::Fancy,
        })
    }

    #[rule("The count stays at most one")]
    fn r#fn(&self) -> bool {
        self.r#type <= 1
    }

    #[end("The count is three")]
    fn r#else(&self) -> bool {
        self.r#type == 3
    }
}
