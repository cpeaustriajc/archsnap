use earnest::model::{Domain, Model, Queue, model, require};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
pub enum Letter {
    Bill,
    Card,
    Note,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
pub struct Tally {
    #[range(0, 9)]
    a: u8,
    #[range(0, 9)]
    b: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Ledger {
    inbox: Queue<Letter, 5>,
    #[initial(Tally { a: 0, b: 0 })]
    tally: Tally,
}

#[model]
impl Ledger {
    #[action("a letter arrives")]
    fn deliver(&self, letter: Letter) -> Option<Self> {
        require!(!self.inbox.is_full());
        Some(Self { inbox: self.inbox.send(letter), ..self.clone() })
    }

    #[action("the oldest letter is read")]
    fn read(&self) -> Option<Self> {
        require!(!self.inbox.is_empty());
        Some(Self { inbox: self.inbox.receive(), ..self.clone() })
    }

    #[action("the first count goes up")]
    fn bump_a(&self) -> Option<Self> {
        require!(self.tally.a < 9);
        Some(Self { tally: Tally { a: self.tally.a + 1, ..self.tally }, ..self.clone() })
    }

    #[action("the second count goes up")]
    fn bump_b(&self) -> Option<Self> {
        require!(self.tally.b < 9);
        Some(Self { tally: Tally { b: self.tally.b + 1, ..self.tally }, ..self.clone() })
    }

    #[rule("The inbox is within its capacity and the counts are on their dials")]
    fn consistent(&self) -> bool {
        self.inbox.size() <= 5
            && self.inbox.every(|letter| self.inbox.includes(letter))
            && self.tally.a <= 9
            && self.tally.b <= 9
    }
}
