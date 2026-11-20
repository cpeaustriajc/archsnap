use earnest::model::{Model, Seq, model, require};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Short {
    items: Seq<bool, 2>,
}

#[model]
impl Short {
    #[action("an item is added")]
    fn add(&self, item: bool) -> Option<Self> {
        require!(self.items.size() < 2);
        Some(Self { items: self.items.push(item), ..self.clone() })
    }

    #[rule("One less than the size is below two")]
    fn small(&self) -> bool {
        self.items.size() - 1 < 2
    }

    #[end("The list is full")]
    fn full(&self) -> bool {
        self.items.size() == 2
    }
}
