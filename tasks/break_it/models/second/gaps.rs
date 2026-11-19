use earnest::model::{Model, Queue, Seq, model, require};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Gaps {
    items: Seq<Option<bool>, 1>,
    inbox: Queue<Option<bool>, 1>,
}

#[model]
impl Gaps {
    #[action("a gap is pushed")]
    fn push(&self) -> Option<Self> {
        require!(self.items.is_empty());
        Some(Self { items: self.items.push(None), ..self.clone() })
    }

    #[action("a gap is sent")]
    fn send(&self) -> Option<Self> {
        require!(self.inbox.is_empty());
        Some(Self { inbox: self.inbox.send(None), ..self.clone() })
    }

    #[rule("The list has no first item")]
    fn headless(&self) -> bool {
        self.items.first().is_none()
    }

    #[rule("The list has no last item")]
    fn tailless(&self) -> bool {
        self.items.last() == None
    }

    #[rule("The list has no item at zero")]
    fn zeroless(&self) -> bool {
        self.items.get(0).is_none()
    }

    #[rule("The inbox has no head")]
    fn quiet(&self) -> bool {
        self.inbox.head().is_none()
    }

    #[end("Both are full")]
    fn done(&self) -> bool {
        self.items.size() == 1 && self.inbox.is_full()
    }
}
