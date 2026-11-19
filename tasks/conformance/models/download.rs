use earnest::model::{Domain, Model, model, require};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
pub enum Phase {
    Idle,
    Loading,
    Done,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Download {
    #[initial(Phase::Idle)]
    phase: Phase,
    #[initial(false)]
    saved: bool,
}

#[model]
impl Download {
    #[action("the user starts the download")]
    fn start(&self) -> Option<Self> {
        require!(self.phase == Phase::Idle);
        Some(Self { phase: Phase::Loading, ..self.clone() })
    }

    #[action("the last byte arrives")]
    fn finish(&self) -> Option<Self> {
        require!(self.phase == Phase::Loading);
        Some(Self { phase: Phase::Done, saved: true, ..self.clone() })
    }

    #[action("the connection drops")]
    fn fail(&self) -> Option<Self> {
        require!(self.phase == Phase::Loading);
        Some(Self { phase: Phase::Failed, ..self.clone() })
    }

    #[rule("Only a finished download is saved")]
    fn whole(&self) -> bool {
        !self.saved || self.phase == Phase::Done
    }

    #[end("The file is on disk")]
    fn done(&self) -> bool {
        self.phase == Phase::Done
    }
}
