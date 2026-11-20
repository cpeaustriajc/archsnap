use earnest::model::{Domain, Model, model};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Stage {
    Cold,
    Warm,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Heater {
    #[initial(Stage::Cold)]
    stage: Stage,
}

#[model]
impl Heater {
    #[action("the heater warms")]
    fn warm(&self) -> Self {
        Self { stage: Stage::Warm }
    }

    #[rule("The stage is one of the two")]
    fn known(&self) -> bool {
        let stage = self.stage;
        stage == Stage::Cold || stage == Stage::Warm
    }
}
