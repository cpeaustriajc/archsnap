use earnest::model::{Domain, Model, model};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Stage {
    Cold,
    Warm,
    Hot,
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Mode {
    Slow,
    Fast,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Phases {
    #[initial(Stage::Cold, Stage::Warm)]
    stage: Stage,
    mode: Mode,
    #[range(0, 9)]
    #[initial(0)]
    score: u8,
}

#[model]
impl Phases {
    fn width(&self) -> u8 {
        match self.stage {
            Stage::Cold => 1,
            Stage::Warm | Stage::Hot => 2,
            Stage::Done => 3,
        }
    }

    fn speed(&self) -> u8 {
        match (self.stage, self.mode) {
            (Stage::Cold, _) => 0,
            (Stage::Warm, Mode::Slow) => 1,
            (Stage::Warm, Mode::Fast) => 2,
            (Stage::Hot | Stage::Done, Mode::Slow) => 3,
            (Stage::Hot | Stage::Done, Mode::Fast) => 4,
        }
    }

    fn heat(&self) -> u8 {
        match self.stage {
            Stage::Cold => 0,
            Stage::Warm => 1,
            _ => 2,
        }
    }

    fn next(&self) -> Stage {
        match self.stage {
            Stage::Cold => Stage::Warm,
            Stage::Warm => Stage::Hot,
            Stage::Hot | Stage::Done => Stage::Done,
        }
    }

    #[action("the stage moves on and the score grows")]
    fn advance(&self) -> Self {
        match self.stage {
            Stage::Cold => Self {
                stage: Stage::Warm,
                ..self.clone()
            },
            Stage::Warm | Stage::Hot => Self {
                stage: self.next(),
                score: self.score + self.speed(),
                ..self.clone()
            },
            Stage::Done => Self {
                score: 0,
                ..self.clone()
            },
        }
    }

    #[action("the mode is flipped")]
    fn flip(&self) -> Self {
        let mode = match self.mode {
            Mode::Slow => Mode::Fast,
            Mode::Fast => Mode::Slow,
        };
        Self {
            mode,
            ..self.clone()
        }
    }

    #[rule("A fast mode is only on a stage that is warm or hot")]
    fn fast_is_busy(&self) -> bool {
        !matches!(self.mode, Mode::Fast) || matches!(self.stage, Stage::Warm | Stage::Hot)
    }

    #[rule("The width follows the heat")]
    fn width_follows_heat(&self) -> bool {
        self.width() >= self.heat()
    }

    #[rule("The speed is below four")]
    fn slow_enough(&self) -> bool {
        self.speed() < 4
    }

    #[rule("The score stays under six")]
    fn modest(&self) -> bool {
        self.score < 6
    }
}
