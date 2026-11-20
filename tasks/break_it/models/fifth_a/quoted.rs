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
    #[one_of("Kim", "Lee")]
    #[initial("Kim")]
    who: &'static str,
}

#[model]
impl Lamp {
    #[action("Lee turns the lamp on")]
    fn turn(&self) -> Option<Self> {
        require!(self.mode == Mode::Off);
        Some(Self {
            mode: Mode::On,
            who: "Lee",
        })
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
        format!(
            "{:?} by {:?}",
            self.mode,
            if self.mode == Mode::On {
                self.who
            } else {
                "Kim"
            }
        )
    }
}
