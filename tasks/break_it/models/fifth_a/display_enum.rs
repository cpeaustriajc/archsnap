use earnest::model::{Domain, Model, model, require, view};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Mode {
    Off,
    On,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Lamp {
    #[initial(Mode::Off)]
    mode: Mode,
}

#[model]
impl Lamp {
    #[action("the lamp turns on")]
    fn turn(&self) -> Option<Self> {
        require!(self.mode == Mode::Off);
        Some(Self { mode: Mode::On })
    }

    #[end("The lamp is lit")]
    fn lit(&self) -> bool {
        self.mode == Mode::On
    }
}

#[view(title = "Lamp")]
impl Lamp {
    #[state]
    fn state(&self) -> String {
        format!("the lamp is {}", self.mode)
    }

    #[group]
    fn group(&self) -> String {
        self.mode.to_string()
    }
}
