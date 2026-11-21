use crate::helpers::Helpers;
use std::time::Duration;

/// How far a search may go and what it keeps.
///
/// The default is the CLI's defaults on one thread: five million states, no time limit, no
/// graph and every rule.
#[derive(Clone, Debug)]
pub struct Limits {
    /// The most distinct states to count. The search stops as unfinished when it finds one
    /// more. The rules of every counted state are judged, also of the states it found last
    /// and tried no action from.
    pub max_states: usize,
    /// How long the search may run before it stops as unfinished. `None` is no limit.
    pub time_limit: Option<Duration>,
    /// Worker threads for a level of 512 states or more. 0 counts as 1. The outcome is the
    /// same for every number.
    pub jobs: usize,
    /// Idle threads the search may borrow, on top of `jobs`, for the parts of each level it
    /// spreads. It borrows when it starts the level and whenever it is about to take a part,
    /// and gives a thread back as soon as the level has no part left for it. Searches that
    /// run side by side share one handle, so a thread one of them leaves idle goes to
    /// another. The outcome is the same whatever is borrowed.
    pub helpers: Option<Helpers>,
    /// Whether [`crate::Outcome::graph`] is filled. It costs memory for every arrow, so leave
    /// it off unless a chart is wanted.
    pub keep_graph: bool,
    /// Check only the rule with this id. The others are not evaluated and are left out of
    /// [`crate::Outcome::rules`]. An id no rule has leaves no rule checked.
    pub only_rule: Option<String>,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_states: 5_000_000,
            time_limit: None,
            jobs: 1,
            helpers: None,
            keep_graph: false,
            only_rule: None,
        }
    }
}

/// The stack, in bytes, a thread that evaluates a model needs: an expression a file nests a
/// thousand levels deep is evaluated one frame per level, so a thread started for the
/// search or for a model should be given this much.
pub const WORKER_STACK: usize = 256 * 1024 * 1024;

pub(crate) const TIME_LIMIT_REASON: &str = "time limit of";

/// Whether `reason`, the [`crate::Outcome::reason`] of a search, says the time limit stopped it.
///
/// A search that a state limit stopped is the same on every run, one that a time limit stopped
/// is not: it ends wherever the clock caught it.
#[must_use]
pub fn stopped_by_time_limit(reason: &str) -> bool {
    reason.starts_with(TIME_LIMIT_REASON)
}
