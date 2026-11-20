use earnest::model::{Model, model, require};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Pick {
    #[one_of(1, 5, 9)]
    size: u8,
    #[one_of("Kim", "Lee")]
    #[initial(None)]
    buyer: Option<&'static str>,
}

#[model]
impl Pick {
    #[action("the buyer picks another size")]
    fn resize(&self, #[one_of(1, 5, 9)] to: u8) -> Option<Self> {
        require!(self.size != to);
        Some(Self {
            size: to,
            ..self.clone()
        })
    }

    #[action("a buyer takes the pick")]
    fn sell(&self, #[one_of("Kim", "Lee")] who: &'static str) -> Option<Self> {
        require!(self.buyer.is_none());
        Some(Self {
            buyer: Some(who),
            ..self.clone()
        })
    }

    #[action("the buyer returns the pick")]
    fn refund(&self) -> Option<Self> {
        require!(self.buyer.is_some());
        Some(Self {
            buyer: None,
            ..self.clone()
        })
    }

    #[rule("A sold pick is never the largest size")]
    fn fits(&self) -> bool {
        self.buyer.is_none() || self.size <= 5
    }

    #[rule("A sold pick has a buyer on the list")]
    fn listed(&self) -> bool {
        self.buyer.is_none() || self.buyer == Some("Kim") || self.buyer == Some("Lee")
    }
}
