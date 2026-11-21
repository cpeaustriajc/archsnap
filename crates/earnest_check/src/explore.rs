use crate::cut::cut_points;
use crate::expand::{Asked, Budget, Clock, Expand, Mark, Scratch, Successors, Whence, Width};
use crate::helpers::{Helpers, Loan};
use crate::outcome::Outcome;
use crate::store::{
    Fresh, Placed, SHARD_COUNT, Spread, Store, hash_row, row_at, same_row, shard_of,
};
use earnest_ir::{Fault, Field, Model};
use earnest_value::Value;
use hashbrown::HashTable;
use rustc_hash::FxHashSet;
use std::any::Any;
use std::ops::Range;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};
use std::thread::Scope;
use std::time::{Duration, Instant};

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
    /// spreads, when it starts the level and whenever it is about to take a part, and gives
    /// back as soon as the level has no part left for them. Searches that run side by side share one handle, so
    /// a thread one of them leaves idle goes to another. The outcome is the same whatever is
    /// borrowed.
    pub helpers: Option<Helpers>,
    /// Whether [`Outcome::graph`] is filled. It costs memory for every arrow, so leave it off
    /// unless a chart is wanted.
    pub keep_graph: bool,
    /// Check only the rule with this id. The others are not evaluated and are left out of
    /// [`Outcome::rules`]. An id no rule has leaves no rule checked.
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

const TIME_LIMIT_REASON: &str = "time limit of";
const SMALLEST_SHARED_LEVEL: usize = 512;
const MOST_PARTS: usize = 64;
const WORTH_SPREADING: Duration = Duration::from_millis(1);
const STATES_BETWEEN_CLOCK_READS: usize = 64;

#[derive(Clone, Debug, Default)]
pub(crate) struct Note {
    pub(crate) end: bool,
    pub(crate) stuck: bool,
    pub(crate) outside: Option<usize>,
    pub(crate) broken: Vec<usize>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Arrow {
    pub(crate) from: usize,
    pub(crate) to: usize,
    pub(crate) action: usize,
}

pub(crate) struct Search<'a> {
    pub(crate) model: &'a Model,
    pub(crate) limits: &'a Limits,
    slots: usize,
    pub(crate) store: Store,
    pub(crate) notes: Vec<Note>,
    pub(crate) arrows: Vec<Arrow>,
    pub(crate) first_broken: Vec<Option<usize>>,
    pub(crate) unjudged: Vec<bool>,
    pub(crate) stuck: Option<usize>,
    pub(crate) outside: Option<(usize, usize)>,
    pub(crate) trouble: Option<(usize, Whence, Fault)>,
    pub(crate) reason: Option<String>,
    pub(crate) wide: bool,
    pub(crate) cut_short: bool,
    judged: usize,
}

#[derive(Clone, Copy, Debug)]
enum Target {
    Known(usize),
    Fresh(usize),
    Raw { fresh_before: usize },
}

#[derive(Clone, Copy, Debug)]
struct Entry {
    from: usize,
    action: usize,
    target: Target,
}

#[derive(Default)]
struct Part {
    first: usize,
    expanded: usize,
    marks: Vec<(usize, Mark)>,
    entries: Vec<Entry>,
    rows: Vec<u8>,
    hashes: Vec<u64>,
    froms: Vec<usize>,
    shards: Vec<Vec<usize>>,
    raws: Vec<Box<[Value]>>,
    stopped: bool,
}

struct Expansion {
    parts: Vec<Part>,
    stopped: bool,
    workers: usize,
}

/// Whether `reason`, the [`Outcome::reason`] of a search, says the time limit stopped it.
///
/// A search that a state limit stopped is the same on every run, one that a time limit stopped
/// is not: it ends wherever the clock caught it.
#[must_use]
pub fn stopped_by_time_limit(reason: &str) -> bool {
    reason.starts_with(TIME_LIMIT_REASON)
}

/// Visits every state `model` can reach from its start states, breadth first, and says what
/// it found.
///
/// A rule is checked in every state, so the path given for a broken rule is a shortest one.
/// A state out of range is counted and reported but not searched further. The search stops
/// early only at a limit, and then the outcome is unfinished unless something was found
/// broken. Every state that is counted has its rules judged, also at a limit: the states the
/// search found and had no time or room to go on from are judged before it gives its outcome,
/// and no action is tried from them.
#[must_use]
pub fn explore(model: &Model, limits: &Limits) -> Outcome {
    let slots = model
        .actions
        .iter()
        .map(|action| action.slots)
        .chain(model.rules.iter().map(|rule| rule.slots))
        .chain(model.ends.iter().map(|end| end.slots))
        .max()
        .unwrap_or(0);
    let mut search = Search {
        model,
        limits,
        slots,
        store: Store::new(&model.fields, limits.max_states),
        notes: Vec::new(),
        arrows: Vec::new(),
        first_broken: vec![None; model.rules.len()],
        unjudged: vec![false; model.rules.len()],
        stuck: None,
        outside: None,
        trouble: None,
        reason: None,
        wide: false,
        cut_short: false,
        judged: 0,
    };
    search.run();
    search.outcome()
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

struct Starts<'a> {
    fields: &'a [Field],
    digits: Vec<usize>,
    done: bool,
}

impl<'a> Starts<'a> {
    fn new(fields: &'a [Field]) -> Self {
        Self {
            fields,
            digits: vec![0; fields.len()],
            done: false,
        }
    }

    fn advance(&mut self) {
        for (digit, field) in self.digits.iter_mut().zip(self.fields).rev() {
            let next = digit.saturating_add(1);
            if next < field.starts.len() {
                *digit = next;
                return;
            }
            *digit = 0;
        }
        self.done = true;
    }
}

impl Iterator for Starts<'_> {
    type Item = Vec<Value>;

    fn next(&mut self) -> Option<Vec<Value>> {
        if self.done {
            return None;
        }
        let state: Option<Vec<Value>> = self
            .fields
            .iter()
            .zip(&self.digits)
            .map(|(field, digit)| field.starts.get(*digit).cloned())
            .collect();
        if state.is_none() {
            self.done = true;
        }
        self.advance();
        state
    }
}

fn with_updates(state: &[Value], updates: &[(usize, Value)]) -> Box<[Value]> {
    let mut next = state.to_vec();
    for (position, value) in updates {
        if let Some(place) = next.get_mut(*position) {
            *place = value.clone();
        }
    }
    next.into_boxed_slice()
}

fn encode_next(
    store: &Store,
    base: Option<&[u64]>,
    row: &mut Vec<u64>,
    state: &[Value],
    updates: &[(usize, Value)],
) -> bool {
    match base {
        Some(base) => {
            row.clear();
            row.extend_from_slice(base);
            updates
                .iter()
                .all(|(position, value)| store.encode_update(row, *position, value))
        }
        None => store.encode(&with_updates(state, updates), row),
    }
}

struct PartSink<'a> {
    store: &'a Store,
    graph: bool,
    base: Vec<u64>,
    base_is_row: bool,
    from: usize,
    row: Vec<u64>,
    packed: Vec<u8>,
    met: HashTable<usize>,
    met_raw: FxHashSet<Box<[Value]>>,
    part: Part,
}

impl PartSink<'_> {
    fn push(&mut self, action: usize, target: Target) {
        self.part.entries.push(Entry {
            from: self.from,
            action,
            target,
        });
    }

    fn add_fresh(&mut self, action: usize, hash: u64) {
        self.part.rows.extend_from_slice(&self.packed);
        if self.part.shards.is_empty() {
            self.part.shards.resize_with(SHARD_COUNT, Vec::new);
        }
        if let Some(shard) = self.part.shards.get_mut(shard_of(hash)) {
            shard.push(self.part.hashes.len());
        }
        self.part.hashes.push(hash);
        self.part.froms.push(self.from);
        if self.graph {
            self.push(action, Target::Fresh(self.part.hashes.len() - 1));
        }
    }

    fn offer_unseen(&mut self, action: usize, hash: u64) -> bool {
        let stride = self.store.stride();
        let (rows, hashes, row) = (&self.part.rows, &self.part.hashes, &self.packed);
        let met = self.met.entry(
            hash,
            |at| same_row(row_at(rows, stride, *at), row),
            |at| hashes.get(*at).copied().unwrap_or(0),
        );
        let hashbrown::hash_table::Entry::Vacant(vacant) = met else {
            return false;
        };
        if self.store.find(row, hash).is_some() {
            return true;
        }
        vacant.insert(hashes.len());
        self.add_fresh(action, hash);
        true
    }
}

impl Successors for PartSink<'_> {
    fn offer(
        &mut self,
        action: usize,
        _: usize,
        state: &[Value],
        updates: &[(usize, Value)],
    ) -> bool {
        let base = self.base_is_row.then_some(self.base.as_slice());
        if !encode_next(self.store, base, &mut self.row, state, updates) {
            let next = with_updates(state, updates);
            if let Some(to) = self.store.find_values(&next) {
                if self.graph {
                    self.push(action, Target::Known(to));
                }
                return true;
            }
            if !self.graph && !self.met_raw.insert(next.clone()) {
                return false;
            }
            self.part.raws.push(next);
            let fresh_before = self.part.hashes.len();
            self.push(action, Target::Raw { fresh_before });
            return true;
        }
        self.store.pack(&self.row, &mut self.packed);
        let hash = hash_row(&self.packed);
        if !self.graph {
            return self.offer_unseen(action, hash);
        }
        if let Some(to) = self.store.find(&self.packed, hash) {
            self.push(action, Target::Known(to));
            return true;
        }
        self.add_fresh(action, hash);
        true
    }
}

struct HereSink<'a> {
    store: &'a mut Store,
    arrows: &'a mut Vec<Arrow>,
    graph: bool,
    base: Vec<u64>,
    base_is_row: bool,
    from: usize,
    level_start: usize,
    most: usize,
    full: bool,
    row: Vec<u64>,
    packed: Vec<u8>,
    after_full: FxHashSet<Box<[Value]>>,
}

impl HereSink<'_> {
    fn counts(&self, known: usize) -> bool {
        self.graph || known < self.level_start
    }

    fn after_the_limit(&mut self, known: Option<usize>, next: Box<[Value]>) -> bool {
        match known {
            Some(to) => self.counts(to),
            None => self.graph || self.after_full.insert(next),
        }
    }
}

impl Successors for HereSink<'_> {
    fn offer(
        &mut self,
        action: usize,
        _: usize,
        state: &[Value],
        updates: &[(usize, Value)],
    ) -> bool {
        let base = self.base_is_row.then_some(self.base.as_slice());
        let encoded = encode_next(self.store, base, &mut self.row, state, updates);
        if encoded {
            self.store.pack(&self.row, &mut self.packed);
        }
        if self.full {
            let next = with_updates(state, updates);
            let known = if encoded {
                self.store.find(&self.packed, hash_row(&self.packed))
            } else {
                self.store.find_values(&next)
            };
            return self.after_the_limit(known, next);
        }
        let parent = Some(self.from);
        let placed = if encoded {
            let hash = hash_row(&self.packed);
            self.store.place(&self.packed, hash, parent, self.most)
        } else {
            self.store
                .place_values(with_updates(state, updates), parent, self.most)
        };
        let (to, counted) = match placed {
            Placed::Known(to) => (to, self.counts(to)),
            Placed::New(to) => (to, true),
            Placed::Full => {
                self.full = true;
                self.after_full.insert(with_updates(state, updates));
                return true;
            }
        };
        if self.graph {
            self.arrows.push(Arrow {
                from: self.from,
                to,
                action,
            });
        }
        counted
    }
}

struct Choice<'a> {
    target: &'a [Value],
    found: Option<(usize, usize)>,
}

impl Successors for Choice<'_> {
    fn offer(
        &mut self,
        action: usize,
        input: usize,
        state: &[Value],
        updates: &[(usize, Value)],
    ) -> bool {
        let mut changed = updates.iter().peekable();
        let same = state.len() == self.target.len()
            && state
                .iter()
                .zip(self.target)
                .enumerate()
                .all(|(position, (old, wanted))| {
                    let value = changed
                        .next_if(|(at, _)| *at == position)
                        .map_or(old, |(_, new)| new);
                    value == wanted
                });
        if same && self.found.is_none() {
            self.found = Some((action, input));
        }
        false
    }

    fn done(&self) -> bool {
        self.found.is_some()
    }
}

struct Here {
    stopped: bool,
    full: bool,
    handed: bool,
    marks: Vec<(usize, Mark)>,
    through: usize,
}

impl Search<'_> {
    fn run(&mut self) {
        let began = Instant::now();
        let deadline = self
            .limits
            .time_limit
            .and_then(|limit| began.checked_add(limit));
        let mut per_state = Duration::ZERO;
        for start in Starts::new(&self.model.fields) {
            let placed =
                self.store
                    .place_values(start.into_boxed_slice(), None, self.limits.max_states);
            if placed == Placed::Full {
                self.reason = Some(self.limit_reason());
                break;
            }
        }
        let mut frontier = 0..self.store.len();
        while !frontier.is_empty() && self.reason.is_none() {
            let level_began = Instant::now();
            let found_before = self.store.len();
            let level = self.level(frontier.clone(), deadline, Asked::Everything, per_state);
            per_state = level_began
                .elapsed()
                .saturating_mul(u32::try_from(level.workers).unwrap_or(u32::MAX))
                .checked_div(u32::try_from(frontier.len()).unwrap_or(u32::MAX))
                .unwrap_or_default();
            if level.stopped {
                self.reason = Some(self.time_limit_reason());
                break;
            }
            frontier = found_before..self.store.len();
        }
        self.judge_the_rest(deadline);
    }

    fn judge_the_rest(&mut self, deadline: Option<Instant>) {
        let unjudged = self.judged..self.store.len();
        let level = self.level(unjudged, deadline, Asked::RulesOnly, Duration::ZERO);
        if level.stopped {
            self.reason = Some(self.time_limit_reason());
            let kept = self.judged;
            self.store.truncate(kept);
            self.arrows
                .retain(|arrow| arrow.from < kept && arrow.to < kept);
        }
    }

    pub(crate) fn choice(&self, from: usize, to: usize) -> Option<(usize, usize)> {
        let state = self.store.state(from);
        let target = self.store.state(to);
        let mut choice = Choice {
            target: &target,
            found: None,
        };
        let mut row = Vec::new();
        self.store.read_state(from, &mut row, &mut Vec::new());
        let outside = self.store.outside(from, &row, &self.model.fields);
        self.expand(None, Asked::Everything).state(
            &state,
            outside,
            &mut choice,
            &mut Scratch::default(),
        );
        choice.found
    }

    fn time_limit_reason(&self) -> String {
        let seconds = self.limits.time_limit.map_or(0, |limit| limit.as_secs());
        format!("{TIME_LIMIT_REASON} {seconds} s")
    }

    fn limit_reason(&self) -> String {
        format!("state limit of {}", self.limits.max_states)
    }

    fn expand(&self, deadline: Option<Instant>, asked: Asked) -> Expand<'_> {
        Expand {
            model: self.model,
            only_rule: self.limits.only_rule.as_deref(),
            slots: self.slots,
            asked,
            budget: Budget {
                deadline,
                successors: self.limits.max_states,
            },
        }
    }

    fn level(
        &mut self,
        frontier: Range<usize>,
        deadline: Option<Instant>,
        asked: Asked,
        per_state: Duration,
    ) -> Ran {
        if let Some(expansion) = self.spread_level(&frontier, deadline, asked, per_state) {
            let ran = Ran {
                stopped: expansion.stopped,
                workers: expansion.workers,
            };
            if !(ran.stopped && asked == Asked::Everything) {
                self.merge_any(expansion.parts);
            }
            return ran;
        }
        let mut marks = Vec::new();
        let mut helpers = self.limits.helpers.as_ref();
        let mut rest = frontier.clone();
        loop {
            let here = self.expand_here(rest.clone(), deadline, asked, helpers);
            if here.stopped && asked == Asked::Everything {
                self.undo_level(&frontier);
                return Ran {
                    stopped: true,
                    workers: 1,
                };
            }
            marks.extend(here.marks);
            rest.start = here.through;
            let spread = if here.handed {
                self.spread_level(&rest, deadline, asked, per_state)
            } else {
                None
            };
            let Some(expansion) = spread else {
                if here.handed {
                    helpers = None;
                    continue;
                }
                self.record_all(&marks, here.through);
                if here.full {
                    self.reason = Some(self.limit_reason());
                }
                return Ran {
                    stopped: here.stopped,
                    workers: 1,
                };
            };
            if expansion.stopped && asked == Asked::Everything {
                self.undo_level(&frontier);
                return Ran {
                    stopped: true,
                    workers: expansion.workers,
                };
            }
            self.record_all(&marks, rest.start);
            self.merge_any(expansion.parts);
            return Ran {
                stopped: expansion.stopped,
                workers: expansion.workers,
            };
        }
    }

    fn undo_level(&mut self, frontier: &Range<usize>) {
        self.store.truncate(frontier.end);
        self.arrows.retain(|arrow| arrow.from < frontier.start);
    }

    fn record_all(&mut self, marks: &[(usize, Mark)], through: usize) {
        for (id, mark) in marks {
            self.record(*id, mark);
        }
        self.judged = self.judged.max(through);
    }

    fn expand_here(
        &mut self,
        frontier: Range<usize>,
        deadline: Option<Instant>,
        asked: Asked,
        helpers: Option<&Helpers>,
    ) -> Here {
        let expand = Expand {
            model: self.model,
            only_rule: self.limits.only_rule.as_deref(),
            slots: self.slots,
            asked,
            budget: Budget {
                deadline,
                successors: self.limits.max_states,
            },
        };
        let mut here = Here {
            stopped: false,
            full: false,
            handed: false,
            marks: Vec::new(),
            through: frontier.start,
        };
        let mut sink = HereSink {
            store: &mut self.store,
            arrows: &mut self.arrows,
            graph: self.limits.keep_graph,
            base: Vec::new(),
            base_is_row: false,
            from: 0,
            level_start: frontier.end,
            most: self.limits.max_states,
            full: false,
            row: Vec::new(),
            packed: Vec::new(),
            after_full: FxHashSet::default(),
        };
        let mut scratch = Scratch::default();
        let mut state = Vec::new();
        for (position, id) in frontier.enumerate() {
            let clock_due = position.is_multiple_of(STATES_BETWEEN_CLOCK_READS);
            let late = deadline.is_some_and(|limit| Instant::now() >= limit);
            if asked == Asked::Everything && clock_due && late {
                here.stopped = true;
                break;
            }
            let idle = || helpers.is_some_and(|helpers| helpers.idle() > 0);
            if clock_due && position > 0 && idle() {
                here.handed = true;
                break;
            }
            sink.base_is_row = sink.store.read_state(id, &mut sink.base, &mut state);
            let outside = sink.store.outside(id, &sink.base, &self.model.fields);
            sink.from = id;
            let mark = expand.state(&state, outside, &mut sink, &mut scratch);
            if mark.clock == Clock::Stopped {
                here.stopped = true;
                break;
            }
            if !mark.is_plain() {
                here.marks.push((id, mark));
            }
            here.through = id.saturating_add(1);
            if sink.full {
                here.full = true;
                break;
            }
        }
        here
    }

    fn expand_ids(&self, ids: Range<usize>, deadline: Option<Instant>, asked: Asked) -> Part {
        let expand = self.expand(deadline, asked);
        let mut sink = PartSink {
            store: &self.store,
            graph: self.limits.keep_graph,
            base: Vec::new(),
            base_is_row: false,
            from: 0,
            row: Vec::new(),
            packed: Vec::new(),
            met: HashTable::new(),
            met_raw: FxHashSet::default(),
            part: Part {
                first: ids.start,
                ..Part::default()
            },
        };
        let mut scratch = Scratch::default();
        let mut state = Vec::new();
        for (position, id) in ids.enumerate() {
            let clock_due = position.is_multiple_of(STATES_BETWEEN_CLOCK_READS);
            let late = deadline.is_some_and(|limit| Instant::now() >= limit);
            if asked == Asked::Everything && clock_due && late {
                sink.part.stopped = true;
                return sink.part;
            }
            sink.base_is_row = self.store.read_state(id, &mut sink.base, &mut state);
            sink.from = id;
            let outside = self.store.outside(id, &sink.base, &self.model.fields);
            let mark = expand.state(&state, outside, &mut sink, &mut scratch);
            if mark.clock == Clock::Stopped {
                sink.part.stopped = true;
                return sink.part;
            }
            if !mark.is_plain() {
                sink.part.marks.push((id, mark));
            }
            sink.part.expanded = sink.part.expanded.saturating_add(1);
        }
        sink.part
    }

    fn spread_level(
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
            for _ in 0..own.min(parts) {
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
            workers: shared.peak.into_inner(),
        };
        for (_, part) in finished {
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
            .spawn_scoped(scope, move || {
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
            })
            .expect("a worker thread starts");
    }

    fn take_parts<'scope, 'env>(
        &'env self,
        scope: &'scope Scope<'scope, 'env>,
        level: &'env Level,
        shared: &'env Shared,
    ) {
        let parts = level.cuts.len().saturating_sub(1);
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
            let expansion = self.expand_ids(ids, level.deadline, level.asked);
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

    fn merge_any(&mut self, parts: Vec<Part>) {
        let found: usize = parts.iter().map(|part| part.hashes.len()).sum();
        let room = self.store.len().saturating_add(found) <= self.limits.max_states;
        let simple = !self.limits.keep_graph && parts.iter().all(|part| part.raws.is_empty());
        if room && simple && parts.len() > 1 {
            self.merge_spread(&parts);
        } else {
            self.merge(parts);
        }
    }

    fn merge_spread(&mut self, parts: &[Part]) {
        for part in parts {
            for (id, mark) in &part.marks {
                self.record(*id, mark);
            }
            self.judged = self.judged.max(part.first.saturating_add(part.expanded));
        }
        let fresh: Vec<Fresh<'_>> = parts
            .iter()
            .map(|part| Fresh {
                rows: &part.rows,
                hashes: &part.hashes,
                froms: &part.froms,
                shards: &part.shards,
            })
            .collect();
        let crew = Crew {
            own: self.limits.jobs.max(1),
            helpers: self.limits.helpers.as_ref(),
        };
        self.store.place_level(&fresh, &crew);
    }

    fn merge(&mut self, parts: Vec<Part>) {
        let most = self.limits.max_states;
        let stride = self.store.stride();
        let graph = self.limits.keep_graph;
        for part in parts {
            let mut marks = part.marks.into_iter().peekable();
            let mut raws = part.raws.into_iter();
            let mut listed = part.entries.iter().copied().peekable();
            let mut fresh = 0..part.hashes.len();
            let froms = &part.froms;
            let mut next = || {
                if graph {
                    return listed.next();
                }
                let due = |entry: &Entry| matches!(entry.target, Target::Raw { fresh_before } if fresh_before <= fresh.start);
                listed.next_if(due).or_else(|| {
                    let at = fresh.next()?;
                    Some(Entry {
                        from: froms.get(at).copied().unwrap_or(0),
                        action: 0,
                        target: Target::Fresh(at),
                    })
                })
            };
            while let Some(entry) = next() {
                while let Some((id, mark)) = marks.next_if(|(id, _)| *id <= entry.from) {
                    self.record(id, &mark);
                }
                self.judged = self.judged.max(entry.from.saturating_add(1));
                let parent = Some(entry.from);
                let placed = match entry.target {
                    Target::Known(to) => Placed::Known(to),
                    Target::Fresh(at) => {
                        let hash = part.hashes.get(at).copied().unwrap_or(0);
                        self.store
                            .place(row_at(&part.rows, stride, at), hash, parent, most)
                    }
                    Target::Raw { .. } => {
                        let Some(values) = raws.next() else {
                            continue;
                        };
                        self.store.place_values(values, parent, most)
                    }
                };
                let (Placed::Known(to) | Placed::New(to)) = placed else {
                    self.reason = Some(self.limit_reason());
                    return;
                };
                if graph {
                    self.arrows.push(Arrow {
                        from: entry.from,
                        to,
                        action: entry.action,
                    });
                }
            }
            for (id, mark) in marks {
                self.record(id, &mark);
            }
            self.judged = self.judged.max(part.first.saturating_add(part.expanded));
        }
    }

    fn record(&mut self, id: usize, expanded: &Mark) {
        self.judged = self.judged.max(id.saturating_add(1));
        self.wide |= expanded.width == Width::Beyond32Bits;
        self.cut_short |= expanded.cut_short;
        if let Some(field) = expanded.outside {
            self.outside.get_or_insert((id, field));
        }
        for rule in &expanded.broken {
            if let Some(first) = self.first_broken.get_mut(*rule) {
                first.get_or_insert(id);
            }
        }
        for rule in &expanded.unjudged {
            if let Some(unjudged) = self.unjudged.get_mut(*rule) {
                *unjudged = true;
            }
        }
        if let Some((whence, fault)) = expanded.trouble {
            self.trouble.get_or_insert((id, whence, fault));
        }
        if expanded.stuck {
            self.stuck.get_or_insert(id);
        }
        if !self.limits.keep_graph {
            return;
        }
        if self.notes.len() <= id {
            self.notes.resize_with(id.saturating_add(1), Note::default);
        }
        if let Some(note) = self.notes.get_mut(id) {
            *note = Note {
                end: expanded.ended,
                stuck: expanded.stuck,
                outside: expanded.outside,
                broken: expanded.broken.clone(),
            };
        }
    }
}

struct Crew<'a> {
    own: usize,
    helpers: Option<&'a Helpers>,
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

struct Ran {
    stopped: bool,
    workers: usize,
}

#[cfg(test)]
mod tests {
    use super::Starts;
    use earnest_ir::Field;
    use earnest_value::{Domain, Symbols, Value};

    fn field(symbols: &mut Symbols, name: &str, starts: &[i64]) -> Field {
        Field {
            name: name.to_owned(),
            sym: symbols.intern(name),
            domain: Domain::Int { lo: 0, hi: 9 },
            starts: starts.iter().copied().map(Value::Int).collect(),
        }
    }

    fn listed(fields: &[Field]) -> Vec<Vec<Value>> {
        Starts::new(fields).collect()
    }

    fn numbers(all: &[i64]) -> Vec<Value> {
        all.iter().copied().map(Value::Int).collect()
    }

    #[test]
    fn start_states_are_every_combination_and_the_last_field_turns_fastest() {
        let mut symbols = Symbols::default();
        let fields = [
            field(&mut symbols, "a", &[0, 1]),
            field(&mut symbols, "b", &[5, 6, 7]),
        ];
        let expected: Vec<Vec<Value>> = [[0, 5], [0, 6], [0, 7], [1, 5], [1, 6], [1, 7]]
            .iter()
            .map(|pair| numbers(pair))
            .collect();
        assert_eq!(listed(&fields), expected);
    }

    #[test]
    fn a_model_without_fields_has_one_empty_start_state() {
        assert_eq!(listed(&[]), vec![Vec::new()]);
    }

    #[test]
    fn a_field_with_no_start_value_leaves_no_start_state() {
        let mut symbols = Symbols::default();
        let fields = [
            field(&mut symbols, "a", &[0, 1]),
            field(&mut symbols, "b", &[]),
        ];
        assert_eq!(listed(&fields), Vec::<Vec<Value>>::new());
    }
}
