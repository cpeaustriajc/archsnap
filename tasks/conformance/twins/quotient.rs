use earnest::model::{Model, model, require};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Quotient {
    #[range(0, 9)]
    n: u8,
    #[range(0, 9)]
    #[initial(0)]
    q: u8,
    #[range(1, 3)]
    #[initial(1)]
    by: u8,
    #[initial(false)]
    done: bool,
}

#[model]
impl Quotient {
    #[action("the number is divided")]
    fn divide(&self, #[range(1, 3)] by: u8) -> Option<Self> {
        require!(!self.done);
        Some(Self {
            q: self.n / by,
            by,
            done: true,
            ..self.clone()
        })
    }

    #[action("the quotient becomes the number")]
    fn take(&self) -> Option<Self> {
        require!(self.done);
        Some(Self {
            n: self.q,
            done: false,
            ..self.clone()
        })
    }

    #[rule("The quotient is the number divided by the divisor, without a fraction")]
    fn floor(&self) -> bool {
        !self.done || (self.q * self.by <= self.n && self.n < self.q * self.by + self.by)
    }

    #[rule("The quotient is always below the number")]
    fn smaller(&self) -> bool {
        !self.done || self.q < self.n
    }
}
