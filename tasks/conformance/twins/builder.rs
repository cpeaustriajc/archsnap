use earnest::model::{Domain, Model, model, require, view};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Kind {
    Small,
    Large,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
struct Tag {
    kind: Kind,
    #[range(0, 3)]
    size: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Builder {
    #[initial(Tag { kind: Kind::Small, size: 0 })]
    tag: Tag,
    #[range(0, 3)]
    #[initial(0)]
    count: u8,
    #[initial(false)]
    flagged: bool,
    #[range(0, 700)]
    #[initial(0)]
    wide: u16,
}

fn grown(tag: &Tag) -> Tag {
    Tag {
        size: tag.size + 1,
        ..tag.clone()
    }
}

fn large(tag: &Tag) -> Tag {
    Tag {
        kind: Kind::Large,
        ..tag.clone()
    }
}

#[model]
impl Builder {
    fn with_count(&self, count: u8) -> Self {
        Self {
            count,
            flagged: count > 1,
            ..self.clone()
        }
    }

    fn widened(&self, by: u8) -> Self {
        Self {
            wide: u16::from(self.count) * 200 + u16::from(by),
            ..self.clone()
        }
    }

    fn name(&self) -> &'static str {
        if self.flagged { "flagged" } else { "plain" }
    }

    fn weight(&self) -> u8 {
        u8::from(self.flagged) + u8::from(self.tag.kind == Kind::Large)
    }

    #[action("the tag grows and the count goes up")]
    fn grow(&self) -> Option<Self> {
        require!(self.tag.size < 3 && self.count < 3);
        Some(Self {
            tag: grown(&self.tag),
            ..self.with_count(self.count + 1)
        })
    }

    #[action("the tag becomes large and the width is recounted")]
    fn enlarge(&self, #[range(0, 5)] by: u8) -> Option<Self> {
        require!(self.tag.kind == Kind::Small);
        Some(Self {
            tag: large(&self.tag),
            ..self.widened(by)
        })
    }

    #[action("the count is reset")]
    fn reset(&self) -> Self {
        self.with_count(0)
    }

    #[rule("A plain builder is not flagged")]
    fn plain_is_unflagged(&self) -> bool {
        self.name() != "plain" || !self.flagged
    }

    #[rule("The weight is below two")]
    fn light(&self) -> bool {
        self.weight() < 2
    }

    #[rule("The width stays under six hundred")]
    fn narrow(&self) -> bool {
        self.wide < 600
    }

    #[rule("A flagged builder has a count above one")]
    fn flagged_counts(&self) -> bool {
        !self.flagged || self.count > 1
    }
}

#[view(title = "Builder")]
impl Builder {
    #[state]
    fn state(&self) -> String {
        format!("{}, weight {}", self.name(), self.weight())
    }

    #[group]
    fn group(&self) -> String {
        self.name().to_owned()
    }
}
