use super::Start;

/// A count of attempts, meant to stop at `MAX`. As a model field it starts at zero.
///
/// Trying again past `MAX` is not an error here: the count just grows, and the checker
/// reports the state as out of range. Guard the action with `can_retry()` to rule it out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Retry<const MAX: usize> {
    attempts: usize,
}

impl<const MAX: usize> Start for Retry<MAX> {
    fn starts() -> Vec<Self> {
        vec![Self::default()]
    }
}

impl<const MAX: usize> Retry<MAX> {
    /// How many attempts were counted since the start or the last reset.
    pub fn attempts(&self) -> usize {
        self.attempts
    }

    /// Whether fewer than `MAX` attempts were counted, so one more is allowed.
    pub fn can_retry(&self) -> bool {
        self.attempts < MAX
    }

    /// Whether `MAX` attempts or more were counted.
    pub fn is_exhausted(&self) -> bool {
        self.attempts >= MAX
    }

    /// The counter with one more attempt.
    #[must_use]
    pub fn again(&self) -> Self {
        Self {
            attempts: self.attempts + 1,
        }
    }

    /// The counter back at zero.
    #[must_use]
    pub fn reset(&self) -> Self {
        Self::default()
    }
}
