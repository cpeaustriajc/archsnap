use earnest::model::forall as exists;
use earnest::model::{Domain, Model, model, require};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
pub enum Gate {
    Locked,
    Open,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Alias {
    #[initial(Gate::Locked)]
    gate: Gate,
}

#[model]
impl Alias {
    #[action("the gate opens")]
    fn open(&self) -> Option<Self> {
        require!(self.gate == Gate::Locked);
        Some(Self { gate: Gate::Open, ..self.clone() })
    }

    #[rule("Some value of a gate is the gate")]
    fn known(&self) -> bool {
        exists::<Gate>(|value| value == self.gate)
    }

    #[end("The gate is open")]
    fn opened(&self) -> bool {
        self.gate == Gate::Open
    }
}
