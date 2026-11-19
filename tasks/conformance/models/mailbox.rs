use earnest::model::{Domain, Model, Queue, model, require};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
pub enum Letter {
    Bill,
    Card,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Mailbox {
    inbox: Queue<Letter, 2>,
}

#[model]
impl Mailbox {
    #[action("the postman delivers a letter")]
    fn deliver(&self, letter: Letter) -> Option<Self> {
        require!(!self.inbox.is_full());
        Some(Self { inbox: self.inbox.send(letter), ..self.clone() })
    }

    #[action("the owner reads the oldest letter")]
    fn read(&self) -> Option<Self> {
        require!(!self.inbox.is_empty());
        Some(Self { inbox: self.inbox.receive(), ..self.clone() })
    }

    #[action("the owner empties a full inbox")]
    fn empty(&self) -> Option<Self> {
        require!(self.inbox.is_full());
        Some(Self { inbox: self.inbox.clear(), ..self.clone() })
    }

    #[rule("A full inbox holds a card")]
    fn bills(&self) -> bool {
        self.inbox.size() < 2 || self.inbox.some(|letter| letter == Letter::Card)
    }

    #[rule("An inbox with letters has a head and includes each of them")]
    fn fronted(&self) -> bool {
        (self.inbox.is_empty() || self.inbox.head().is_some())
            && self.inbox.every(|letter| self.inbox.includes(letter))
    }
}
