use earnest::model::{Domain, Model, model, require};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Letter {
    Bill,
    Card,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
struct Spot {
    #[range(0, 1)]
    row: u8,
    lit: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Post {
    #[initial(None)]
    got: Option<Letter>,
    #[initial(false)]
    done: bool,
    #[initial(None)]
    at: Option<Spot>,
}

#[model]
impl Post {
    #[action("a letter or nothing comes")]
    fn deliver(&self, letter: Option<Letter>) -> Option<Self> {
        require!(!self.done);
        Some(Self {
            got: letter,
            done: true,
            ..self.clone()
        })
    }

    #[action("the post is put in a spot")]
    fn place(&self, spot: Spot) -> Option<Self> {
        require!(self.done && self.at.is_none());
        Some(Self {
            at: Some(spot),
            ..self.clone()
        })
    }

    #[rule("Nothing that came is put in the lit spot of the second row")]
    fn kept(&self) -> bool {
        self.got.is_some() || !self.at.is_some_and(|spot| spot.row == 1 && spot.lit)
    }

    #[end("The post is in a spot")]
    fn placed(&self) -> bool {
        self.at.is_some()
    }
}
