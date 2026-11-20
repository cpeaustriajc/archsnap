use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A shared count of idle threads that searches may borrow, a part of a level of states at a
/// time.
///
/// Give every search of a run the same handle, with the threads no search is using in it. A
/// search borrows what is idle when it starts a level and again before it takes each part of
/// the level, and gives each thread back when it has no part left, so threads a finished
/// search leaves behind go to the searches that still run, and no thread waits while another
/// search has a part of a level to spread.
#[derive(Clone, Debug, Default)]
pub struct Helpers {
    idle: Arc<AtomicUsize>,
}

/// Threads borrowed from [`Helpers`]. Dropping it gives them back.
#[derive(Debug)]
pub struct Loan<'a> {
    helpers: &'a Helpers,
    threads: usize,
}

impl Helpers {
    /// A handle with `idle` threads to lend.
    #[must_use]
    pub fn new(idle: usize) -> Self {
        Self {
            idle: Arc::new(AtomicUsize::new(idle)),
        }
    }

    /// Adds `threads` to the idle threads, as when a thread finishes its last search.
    pub fn add(&self, threads: usize) {
        self.idle.fetch_add(threads, Ordering::SeqCst);
    }

    /// How many threads are idle right now.
    #[must_use]
    pub fn idle(&self) -> usize {
        self.idle.load(Ordering::SeqCst)
    }

    /// Takes up to `most` idle threads, as many as there are.
    #[must_use]
    pub fn borrow(&self, most: usize) -> Loan<'_> {
        let before = self
            .idle
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |idle| {
                Some(idle.saturating_sub(most))
            })
            .unwrap_or_default();
        Loan {
            helpers: self,
            threads: before.min(most),
        }
    }
}

impl Loan<'_> {
    /// How many threads this loan holds.
    #[must_use]
    pub fn threads(&self) -> usize {
        self.threads
    }
}

impl Drop for Loan<'_> {
    fn drop(&mut self) {
        self.helpers.add(self.threads);
    }
}

#[cfg(test)]
mod tests {
    use super::Helpers;

    #[test]
    fn a_loan_takes_what_is_idle_up_to_what_was_asked_and_gives_it_back_when_dropped() {
        let helpers = Helpers::new(3);
        let first = helpers.borrow(2);
        assert_eq!(first.threads(), 2, "what was asked for");
        assert_eq!(helpers.idle(), 1, "one is left");
        let second = helpers.borrow(5);
        assert_eq!(second.threads(), 1, "only what was left");
        assert_eq!(helpers.idle(), 0, "none is left");
        assert_eq!(helpers.borrow(1).threads(), 0, "nothing to borrow");
        drop(first);
        assert_eq!(helpers.idle(), 2, "the first loan is back");
        drop(second);
        assert_eq!(helpers.idle(), 3, "all are back");
    }

    #[test]
    fn threads_added_later_can_be_borrowed() {
        let helpers = Helpers::new(0);
        helpers.add(2);
        assert_eq!(helpers.borrow(usize::MAX).threads(), 2, "everything idle");
    }

    #[test]
    fn clones_share_one_count() {
        let helpers = Helpers::new(1);
        let other = helpers.clone();
        let loan = other.borrow(1);
        assert_eq!(
            helpers.idle(),
            0,
            "the loan is seen through the first handle"
        );
        drop(loan);
        assert_eq!(helpers.idle(), 1, "and gives back to it");
    }
}
