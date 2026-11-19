use super::Start;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Stage {
    #[default]
    Idle,
    Pending,
    Succeeded,
    Failed,
}

/// An async request that is idle, pending, succeeded or failed. As a model field it starts
/// idle.
///
/// No operation checks the stage it starts from: `succeed()` on an idle request gives a
/// succeeded one. Guard the action with `is_pending()` when only a sent request may settle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Request {
    stage: Stage,
}

impl Start for Request {
    fn starts() -> Vec<Self> {
        vec![Self::default()]
    }
}

impl Request {
    /// Whether the request was never sent, or was reset.
    pub fn is_idle(&self) -> bool {
        self.stage == Stage::Idle
    }

    /// Whether the request was sent and has no answer yet.
    pub fn is_pending(&self) -> bool {
        self.stage == Stage::Pending
    }

    /// Whether the request ended well.
    pub fn has_succeeded(&self) -> bool {
        self.stage == Stage::Succeeded
    }

    /// Whether the request ended badly.
    pub fn has_failed(&self) -> bool {
        self.stage == Stage::Failed
    }

    /// Whether the request has an answer, good or bad.
    pub fn is_settled(&self) -> bool {
        matches!(self.stage, Stage::Succeeded | Stage::Failed)
    }

    /// The pending request.
    #[must_use]
    pub fn send(&self) -> Self {
        Self {
            stage: Stage::Pending,
        }
    }

    /// The succeeded request.
    #[must_use]
    pub fn succeed(&self) -> Self {
        Self {
            stage: Stage::Succeeded,
        }
    }

    /// The failed request.
    #[must_use]
    pub fn fail(&self) -> Self {
        Self {
            stage: Stage::Failed,
        }
    }

    /// The idle request, ready to be sent again.
    #[must_use]
    pub fn reset(&self) -> Self {
        Self::default()
    }
}
