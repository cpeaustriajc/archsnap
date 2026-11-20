use earnest::model::{Domain, Model, model};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Alpha {
    On,
    Off,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Beta {
    On,
    Off,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Twin {
    #[initial(Beta::On)]
    beta: Beta,
    #[initial(Alpha::Off)]
    alpha: Alpha,
}

#[model]
impl Twin {
    #[action("the switch flips")]
    fn flip(&self) -> Self {
        Self {
            beta: if self.beta == Beta::On {
                Beta::Off
            } else {
                Beta::On
            },
            ..self.clone()
        }
    }

    #[rule("The switch is a switch")]
    fn known(&self) -> bool {
        self.beta == Alpha::On || self.beta == Beta::Off
    }
}
