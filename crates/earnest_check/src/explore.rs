mod choice;
mod encode;
mod here;
mod merge;
mod part;
mod starts;
mod workers;

use crate::expand::{Asked, Budget, Expand, Mark, Whence, Width};
use crate::limits::{Limits, TIME_LIMIT_REASON};
use crate::outcome::Outcome;
use crate::store::{Placed, Store};
use earnest_ir::{Fault, Model};
use starts::Starts;
use std::ops::Range;
use std::time::{Duration, Instant};

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
        let mut helpers = self.limits.helpers.as_ref();
        match self.spread_level(&frontier, deadline, asked, per_state) {
            Some(expansion) if expansion.crowded => helpers = None,
            Some(expansion) => {
                let ran = Ran {
                    stopped: expansion.stopped,
                    workers: expansion.workers,
                };
                if !(ran.stopped && asked == Asked::Everything) {
                    self.merge_any(expansion.parts);
                }
                return ran;
            }
            None => {}
        }
        let mut marks = Vec::new();
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
            let Some(expansion) = spread.filter(|expansion| !expansion.crowded) else {
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

struct Ran {
    stopped: bool,
    workers: usize,
}
