use super::Search;
use super::part::{Kit, Part};
use crate::cut::cut_points;
use crate::expand::Asked;
use crate::helpers::{Helpers, Loan};
use crate::limits::WORKER_STACK;
use crate::store::Spread;
use std::any::Any;
use std::ops::Range;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};
use std::thread::Scope;
use std::time::{Duration, Instant};

const SMALLEST_SHARED_LEVEL: usize = 512;
const MOST_PARTS: usize = 64;
const WORTH_SPREADING: Duration = Duration::from_millis(1);

pub(super) struct Expansion {
    pub(super) parts: Vec<Part>,
    pub(super) stopped: bool,
    pub(super) crowded: bool,
    pub(super) workers: usize,
}

struct Level {
    frontier: Range<usize>,
    cuts: Vec<usize>,
    deadline: Option<Instant>,
    asked: Asked,
}

#[derive(Default)]
struct Shared {
    next: AtomicUsize,
    stopped: AtomicBool,
    running: AtomicUsize,
    peak: AtomicUsize,
    done: Mutex<Vec<(usize, Part)>>,
    panic: Mutex<Option<Box<dyn Any + Send>>>,
}

impl Search<'_> {
    pub(super) fn spread_level(
        &self,
        frontier: &Range<usize>,
        deadline: Option<Instant>,
        asked: Asked,
        per_state: Duration,
    ) -> Option<Expansion> {
        let states = u32::try_from(frontier.len()).unwrap_or(u32::MAX);
        let worth_spreading = frontier.len() >= SMALLEST_SHARED_LEVEL
            || per_state.saturating_mul(states) >= WORTH_SPREADING;
        let own = self.limits.jobs.max(1);
        let helpers = self.limits.helpers.as_ref();
        let alone = own == 1 && helpers.is_none_or(|helpers| helpers.idle() == 0);
        if !worth_spreading || alone {
            return None;
        }
        let level = Level {
            frontier: frontier.clone(),
            cuts: cut_points(frontier.len(), frontier.len().div_ceil(MOST_PARTS)),
            deadline,
            asked,
        };
        let parts = level.cuts.len().saturating_sub(1);
        if parts <= 1 {
            return None;
        }
        let shared = Shared::default();
        std::thread::scope(|scope| {
            for _ in 1..own.min(parts) {
                self.start_worker(scope, &level, &shared, None);
            }
            for _ in 0..parts.saturating_sub(own) {
                let Some(loan) = helpers
                    .map(|helpers| helpers.borrow(1))
                    .filter(|loan| loan.threads() > 0)
                else {
                    break;
                };
                self.start_worker(scope, &level, &shared, Some(loan));
            }
            self.work(scope, &level, &shared, None);
        });
        if let Some(panic) = shared
            .panic
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner)
        {
            std::panic::resume_unwind(panic);
        }
        let mut finished = shared
            .done
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner);
        finished.sort_by_key(|(part, _)| *part);
        let mut expansion = Expansion {
            parts: Vec::with_capacity(finished.len()),
            stopped: false,
            crowded: finished.iter().any(|(_, part)| part.crowded),
            workers: shared.peak.into_inner(),
        };
        for (expected, (index, part)) in finished.into_iter().enumerate() {
            if index != expected {
                expansion.stopped = true;
                break;
            }
            let stopped = part.stopped;
            expansion.parts.push(part);
            if stopped {
                expansion.stopped = true;
                break;
            }
        }
        Some(expansion)
    }

    fn start_worker<'scope, 'env>(
        &'env self,
        scope: &'scope Scope<'scope, 'env>,
        level: &'env Level,
        shared: &'env Shared,
        loan: Option<Loan<'env>>,
    ) {
        std::thread::Builder::new()
            .stack_size(WORKER_STACK)
            .spawn_scoped(scope, move || self.work(scope, level, shared, loan))
            .expect("a worker thread starts");
    }

    fn work<'scope, 'env>(
        &'env self,
        scope: &'scope Scope<'scope, 'env>,
        level: &'env Level,
        shared: &'env Shared,
        loan: Option<Loan<'env>>,
    ) {
        let running = shared
            .running
            .fetch_add(1, Ordering::SeqCst)
            .saturating_add(1);
        shared.peak.fetch_max(running, Ordering::SeqCst);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            self.take_parts(scope, level, shared);
        }));
        drop(loan);
        shared.running.fetch_sub(1, Ordering::SeqCst);
        if let Err(panic) = outcome {
            shared.stopped.store(true, Ordering::SeqCst);
            shared
                .panic
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get_or_insert(panic);
        }
    }

    fn take_parts<'scope, 'env>(
        &'env self,
        scope: &'scope Scope<'scope, 'env>,
        level: &'env Level,
        shared: &'env Shared,
    ) {
        let parts = level.cuts.len().saturating_sub(1);
        let mut kit = Kit::default();
        loop {
            self.take_up_idle_threads(scope, level, shared, parts);
            let part = shared.next.fetch_add(1, Ordering::SeqCst);
            if part >= parts || shared.stopped.load(Ordering::SeqCst) {
                return;
            }
            let ids = level
                .cuts
                .get(part)
                .zip(level.cuts.get(part.saturating_add(1)))
                .map(|(from, to)| {
                    level.frontier.start.saturating_add(*from)
                        ..level.frontier.start.saturating_add(*to)
                });
            let Some(ids) = ids else {
                return;
            };
            let expansion = self.expand_ids(ids, level.deadline, level.asked, &mut kit);
            if expansion.stopped {
                shared.stopped.store(true, Ordering::SeqCst);
            }
            shared
                .done
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push((part, expansion));
        }
    }

    fn take_up_idle_threads<'scope, 'env>(
        &'env self,
        scope: &'scope Scope<'scope, 'env>,
        level: &'env Level,
        shared: &'env Shared,
        parts: usize,
    ) {
        let Some(helpers) = self.limits.helpers.as_ref() else {
            return;
        };
        while helpers.idle() > 0
            && shared.running.load(Ordering::SeqCst)
                < parts.saturating_sub(shared.next.load(Ordering::SeqCst))
        {
            let loan = helpers.borrow(1);
            if loan.threads() == 0 {
                return;
            }
            self.start_worker(scope, level, shared, Some(loan));
        }
    }
}

pub(super) struct Crew<'a> {
    pub(super) own: usize,
    pub(super) helpers: Option<&'a Helpers>,
}

impl Spread for Crew<'_> {
    fn run(&self, tasks: usize, job: &(dyn Fn(usize) + Sync)) {
        let next = AtomicUsize::new(0);
        let work = || {
            loop {
                let task = next.fetch_add(1, Ordering::Relaxed);
                if task >= tasks {
                    return;
                }
                job(task);
            }
        };
        let own = self.own.min(tasks);
        let loan = self
            .helpers
            .map(|helpers| helpers.borrow(tasks.saturating_sub(own)));
        let borrowed = loan.as_ref().map_or(0, Loan::threads);
        std::thread::scope(|scope| {
            for _ in 1..own.saturating_add(borrowed) {
                scope.spawn(work);
            }
            work();
        });
    }
}
