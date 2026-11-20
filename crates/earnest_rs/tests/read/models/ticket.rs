use earnest::model::{Domain, Model, exists, forall, model, require};

const LAST_ROW: u8 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Fan {
    Kim,
    Lee,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
struct Seat {
    #[range(1, LAST_ROW)]
    row: u8,
    aisle: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Ticket {
    #[initial(Seat { row: 1, aisle: false })]
    seat: Seat,
    #[initial(None)]
    owner: Option<Fan>,
}

fn next(row: u8) -> u8 {
    if row == LAST_ROW { 1 } else { row + 1 }
}

#[model]
impl Ticket {
    #[action("an unsold seat moves back a row")]
    fn back(&self) -> Option<Self> {
        require!(self.owner.is_none());
        Some(Self {
            seat: Seat {
                row: next(self.seat.row),
                ..self.seat.clone()
            },
            ..self.clone()
        })
    }

    #[action("an unsold seat changes side")]
    fn flip(&self) -> Option<Self> {
        require!(self.owner.is_none());
        let aisle = !self.seat.aisle;
        Some(Self {
            seat: Seat {
                aisle,
                ..self.seat.clone()
            },
            ..self.clone()
        })
    }

    #[action("a fan buys the ticket")]
    fn buy(&self, fan: Fan) -> Option<Self> {
        require!(self.owner.is_none());
        Some(Self {
            owner: Some(fan),
            ..self.clone()
        })
    }

    #[action("the owner returns the ticket")]
    fn refund(&self) -> Option<Self> {
        require!(self.owner.is_some());
        Some(Self {
            owner: None,
            ..self.clone()
        })
    }

    #[rule("The seat is in the hall")]
    fn seated(&self) -> bool {
        self.seat.row >= 1 && self.seat.row <= LAST_ROW
    }

    #[rule("An owned ticket has a fan as its owner")]
    fn owned(&self) -> bool {
        self.owner.is_none() || exists::<Fan>(|fan| self.owner == Some(fan))
    }

    #[rule("No fan owns every ticket")]
    fn shared(&self) -> bool {
        !forall(|fan: Fan| self.owner == Some(fan))
    }

    #[rule("Kim never sits on the aisle of the last row")]
    fn kim(&self) -> bool {
        !(self.owner.is_some_and(|fan| fan == Fan::Kim) && self.seat.aisle && self.seat.row == 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kim_reaches_the_aisle_of_the_last_row_in_three_steps() {
        let start = Ticket::starts().remove(0);
        let moved = start.back().unwrap().flip().unwrap();
        assert!(moved.kim(), "nobody owns the ticket yet");
        let bought = moved.buy(Fan::Kim).unwrap();
        assert_eq!(
            bought.seat,
            Seat {
                row: 2,
                aisle: true
            },
            "the seat"
        );
        assert!(!bought.kim(), "the rule the checker reports as broken");
        assert!(
            bought.seated() && bought.owned() && bought.shared(),
            "the others hold"
        );
        assert_eq!(bought.buy(Fan::Lee), None, "the ticket is taken");
        assert_eq!(bought.refund().unwrap(), moved, "returned");
    }
}
