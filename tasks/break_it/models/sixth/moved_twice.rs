use earnest::model::{Domain, Model, model, require};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
struct Tag {
    #[range(0, 2)]
    size: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Moved {
    #[initial(None)]
    tag: Option<Tag>,
    #[range(0, 4)]
    #[initial(0)]
    total: u8,
}

#[model]
impl Moved {
    #[action("a tag is put on")]
    fn tag_it(&self) -> Option<Self> {
        require!(self.tag.is_none());
        Some(Self {
            tag: Some(Tag { size: 2 }),
            ..self.clone()
        })
    }

    #[action("the tag is counted twice")]
    fn count(&self) -> Option<Self> {
        let held = self.tag.clone();
        let first = held?;
        let second = held?;
        require!(self.total == 0);
        Some(Self {
            total: first.size + second.size,
            ..self.clone()
        })
    }

    #[end("The tag is counted")]
    fn counted(&self) -> bool {
        self.total > 0
    }

    #[rule("The total stays small")]
    fn small(&self) -> bool {
        self.total <= 4
    }
}
