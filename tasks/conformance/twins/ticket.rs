use earnest::model::{Domain, Model, model, require};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
struct Seat {
    #[range(1, 2)]
    row: u8,
    aisle: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Ticket {
    #[initial(Seat { row: 1, aisle: false })]
    seat: Seat,
    #[one_of("Kim", "Lee")]
    #[initial(None)]
    owner: Option<&'static str>,
}

#[model]
impl Ticket {
    #[action("the seat moves back a row")]
    fn r#move(&self) -> Option<Self> {
        require!(self.seat.row < 2);
        Some(Self {
            seat: Seat {
                row: self.seat.row + 1,
                ..self.seat.clone()
            },
            ..self.clone()
        })
    }

    #[action("the seat swaps sides")]
    fn swap(&self) -> Self {
        Self {
            seat: Seat {
                aisle: !self.seat.aisle,
                ..self.seat.clone()
            },
            ..self.clone()
        }
    }

    #[action("a buyer takes the ticket")]
    fn sell(&self, #[one_of("Kim", "Lee")] buyer: &'static str) -> Option<Self> {
        require!(self.owner.is_none());
        Some(Self {
            owner: Some(buyer),
            ..self.clone()
        })
    }

    #[action("the buyer returns the ticket")]
    fn refund(&self) -> Option<Self> {
        require!(self.owner.is_some());
        Some(Self {
            owner: None,
            ..self.clone()
        })
    }

    #[rule("A sold ticket has a seat in a real row")]
    fn seated(&self) -> bool {
        self.owner.is_none() || self.seat.row <= 2
    }
}
