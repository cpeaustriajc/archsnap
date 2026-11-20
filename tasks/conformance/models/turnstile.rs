use earnest::model::{Domain, Model, model, require};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
pub enum Gate {
    Locked,
    Open,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Turnstile {
    #[initial(Gate::Locked)]
    gate: Gate,
    #[initial(false)]
    paid: bool,
}

#[model]
impl Turnstile {
    #[action("the visitor pays")]
    fn pay(&self) -> Option<Self> {
        require!(!self.paid && self.gate == Gate::Locked);
        Some(Self { paid: true, ..self.clone() })
    }

    #[action("the visitor pushes")]
    fn push(&self) -> Option<Self> {
        require!(self.gate == Gate::Locked);
        Some(Self { gate: Gate::Open, ..self.clone() })
    }

    #[action("the visitor walks through")]
    fn enter(&self) -> Option<Self> {
        require!(self.gate == Gate::Open);
        Some(Self { gate: Gate::Locked, paid: false, ..self.clone() })
    }

    #[rule("The gate stays locked until the visitor pays")]
    fn locked(&self) -> bool {
        self.gate == Gate::Locked || self.paid
    }
}
