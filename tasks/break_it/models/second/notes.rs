use earnest::model::{Domain, Map, Model, forall, model, require};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
pub enum Key {
    Left,
    Right,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Notes {
    notes: Map<Key, Option<bool>>,
}

#[model]
impl Notes {
    #[action("a key is noted without a value")]
    fn note(&self, key: Key) -> Option<Self> {
        require!(!self.notes.has(key));
        Some(Self { notes: self.notes.set(key, None), ..self.clone() })
    }

    #[rule("No key has a note")]
    fn blank(&self) -> bool {
        forall::<Key>(|key| self.notes.get(key).is_none())
    }

    #[end("Both keys are noted")]
    fn done(&self) -> bool {
        self.notes.size() == 2
    }
}
