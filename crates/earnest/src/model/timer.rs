use super::Start;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Stage {
    #[default]
    Idle,
    Running,
    Fired,
}

/// A timer that is idle, running or fired. As a model field it starts idle.
///
/// No operation checks the stage it starts from: `fire()` on an idle timer gives a fired one.
/// Guard the action with `is_running()` when only a running timer may fire.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timer {
    stage: Stage,
}

impl Start for Timer {
    fn starts() -> Vec<Self> {
        vec![Self::default()]
    }
}

impl Timer {
    pub(super) fn stage_name(self) -> &'static str {
        match self.stage {
            Stage::Idle => "idle",
            Stage::Running => "running",
            Stage::Fired => "fired",
        }
    }

    /// Whether the timer is not running and has not fired.
    pub fn is_idle(&self) -> bool {
        self.stage == Stage::Idle
    }

    /// Whether the timer was started and has neither fired nor been cancelled.
    pub fn is_running(&self) -> bool {
        self.stage == Stage::Running
    }

    /// Whether the timer fired and was not reset since.
    pub fn has_fired(&self) -> bool {
        self.stage == Stage::Fired
    }

    /// The running timer.
    #[must_use]
    pub fn start(&self) -> Self {
        Self {
            stage: Stage::Running,
        }
    }

    /// The fired timer.
    #[must_use]
    pub fn fire(&self) -> Self {
        Self {
            stage: Stage::Fired,
        }
    }

    /// The idle timer, for a timer stopped before it fired.
    #[must_use]
    pub fn cancel(&self) -> Self {
        Self::default()
    }

    /// The idle timer, for a timer made ready again after it fired.
    #[must_use]
    pub fn reset(&self) -> Self {
        Self::default()
    }
}
