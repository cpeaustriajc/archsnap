use earnest::model::{Domain, Model, model, require};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
pub enum Door {
    Closed,
    Open,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
pub enum Heat {
    Off,
    On,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Oven {
    #[initial(Door::Closed)]
    door: Door,
    #[initial(Heat::Off)]
    heat: Heat,
}

#[model]
impl Oven {
    #[action("the cook opens the door")]
    fn open(&self) -> Option<Self> {
        require!(self.door == Door::Closed);
        Some(Self { door: Door::Open, ..self.clone() })
    }

    #[action("the cook closes the door")]
    fn close(&self) -> Option<Self> {
        require!(self.door == Door::Open);
        Some(Self { door: Door::Closed, ..self.clone() })
    }

    #[action("the cook starts the heat")]
    fn start(&self) -> Option<Self> {
        require!(self.heat == Heat::Off && self.door == Door::Closed);
        Some(Self { heat: Heat::On, ..self.clone() })
    }

    #[action("the cook stops the heat")]
    fn stop(&self) -> Option<Self> {
        require!(self.heat == Heat::On);
        Some(Self { heat: Heat::Off, ..self.clone() })
    }

    #[rule("The oven never heats with the door open")]
    fn safe(&self) -> bool {
        !(self.door == Door::Open && self.heat == Heat::On)
    }

    #[rule("The door is closed whenever the heat is off")]
    fn closed(&self) -> bool {
        self.door == Door::Closed || self.heat == Heat::On
    }
}
