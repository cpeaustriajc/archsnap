use earnest::model::{Model, model, require};

fn pick(_first: bool, second: bool) -> bool {
    second
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Twins {
    #[initial(false)]
    on: bool,
}

#[model]
impl Twins {
    fn pick(&self, second: bool) -> bool {
        !second
    }

    #[action("the lamp turns on")]
    fn light(&self) -> Option<Self> {
        require!(!self.on);
        Some(Self { on: true, ..self.clone() })
    }

    #[rule("The function outside the model gives back its second argument")]
    fn outside(&self) -> bool {
        pick(false, true)
    }

    #[rule("The method of the model gives back its argument")]
    fn kept(&self) -> bool {
        self.pick(true)
    }

    #[end("The lamp is lit")]
    fn lit(&self) -> bool {
        self.on
    }
}
