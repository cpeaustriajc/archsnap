use crate::cut::cut_points;
use crate::expand::{
    Asked, Budget, Clock, Expanded, Index, Key, Known, State, Whence, Width, expand_state,
};
use crate::helpers::Helpers;
use crate::outcome::Outcome;
use earnest_ir::{Fault, Field, Model};
use earnest_value::Value;
use std::sync::Arc;
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
    /// Idle threads the search may borrow, on top of `jobs`, for each level it spreads and
    /// gives back when the level is done. Searches that run side by side share one handle, so
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

const SMALLEST_SHARED_LEVEL: usize = 512;
const WORTH_SPREADING: Duration = Duration::from_millis(1);
const STATES_BETWEEN_CLOCK_READS: usize = 64;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Parent {
    pub(crate) state: usize,
    pub(crate) action: usize,
    pub(crate) input: usize,
}

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
    pub(crate) states: Vec<State>,
    index: Index,
    pub(crate) parents: Vec<Option<Parent>>,
    pub(crate) notes: Vec<Note>,
    pub(crate) arrows: Vec<Arrow>,
    pub(crate) first_broken: Vec<Option<usize>>,
    pub(crate) unjudged: Vec<bool>,
    pub(crate) stuck: Option<usize>,
    pub(crate) outside: Option<(usize, usize)>,
    pub(crate) trouble: Option<(usize, Whence, Fault)>,
    pub(crate) reason: Option<String>,
    pub(crate) wide: bool,
    judged: usize,
}

struct Expansion {
    done: Vec<Expanded>,
    stopped: bool,
    workers: usize,
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
        states: Vec::new(),
        index: Index::default(),
        parents: Vec::new(),
        notes: Vec::new(),
        arrows: Vec::new(),
        first_broken: vec![None; model.rules.len()],
        unjudged: vec![false; model.rules.len()],
        stuck: None,
        outside: None,
        trouble: None,
        reason: None,
        wide: false,
        judged: 0,
    };
    search.run();
    search.outcome()
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

impl Search<'_> {
    fn run(&mut self) {
        let began = Instant::now();
        let deadline = self
            .limits
            .time_limit
            .and_then(|limit| began.checked_add(limit));
        let mut per_state = Duration::ZERO;
        let mut frontier = Vec::new();
        for start in Starts::new(&self.model.fields) {
            if let Some(id) = self.insert(Key::new(Arc::from(start)), None) {
                frontier.push(id);
            }
            if self.reason.is_some() {
                break;
            }
        }
        while !frontier.is_empty() && self.reason.is_none() {
            let level_began = Instant::now();
            let level = self.expand_level(&frontier, deadline, Asked::Everything, per_state);
            per_state = level_began
                .elapsed()
                .saturating_mul(u32::try_from(level.workers).unwrap_or(u32::MAX))
                .checked_div(u32::try_from(frontier.len()).unwrap_or(u32::MAX))
                .unwrap_or_default();
            if level.stopped {
                let seconds = self.limits.time_limit.map_or(0, |limit| limit.as_secs());
                self.reason = Some(format!("time limit of {seconds} s"));
                break;
            }
            frontier = self.merge(&frontier, level.done);
        }
        self.judge_the_rest(deadline);
    }

    fn judge_the_rest(&mut self, deadline: Option<Instant>) {
        let unjudged: Vec<usize> = (self.judged..self.states.len()).collect();
        let level = self.expand_level(&unjudged, deadline, Asked::RulesOnly, Duration::ZERO);
        for (id, expanded) in unjudged.into_iter().zip(level.done) {
            self.record(id, &expanded);
        }
        if level.stopped {
            self.states.truncate(self.judged);
            self.parents.truncate(self.judged);
            let kept = self.judged;
            self.arrows
                .retain(|arrow| arrow.from < kept && arrow.to < kept);
        }
    }

    fn budget(&self, deadline: Option<Instant>) -> Budget {
        Budget {
            deadline,
            successors: self.limits.max_states,
        }
    }

    fn insert(&mut self, key: Key, parent: Option<Parent>) -> Option<usize> {
        if self.index.contains_key(&key) {
            return None;
        }
        if self.states.len() >= self.limits.max_states {
            self.reason = Some(format!("state limit of {}", self.limits.max_states));
            return None;
        }
        let id = self.states.len();
        self.states.push(Arc::clone(&key.state));
        self.index.insert(key, id);
        self.parents.push(parent);
        Some(id)
    }

    fn expand_ids(&self, ids: &[usize], deadline: Option<Instant>, asked: Asked) -> Expansion {
        let only_rule = self.limits.only_rule.as_deref();
        let budget = self.budget(deadline);
        let mut level = Vec::with_capacity(ids.len());
        let mut known = Known {
            index: &self.index,
            met: (!self.limits.keep_graph).then(Default::default),
        };
        for (position, id) in ids.iter().enumerate() {
            let clock_due = position.is_multiple_of(STATES_BETWEEN_CLOCK_READS);
            let late = deadline.is_some_and(|limit| Instant::now() >= limit);
            if asked == Asked::Everything && clock_due && late {
                return Expansion {
                    done: level,
                    stopped: true,
                    workers: 1,
                };
            }
            let Some(state) = self.states.get(*id) else {
                break;
            };
            let expanded = expand_state(
                self.model, only_rule, self.slots, state, asked, budget, &mut known,
            );
            if expanded.clock == Clock::Stopped {
                return Expansion {
                    done: level,
                    stopped: true,
                    workers: 1,
                };
            }
            level.push(expanded);
        }
        Expansion {
            done: level,
            stopped: false,
            workers: 1,
        }
    }

    fn expand_level(
        &self,
        frontier: &[usize],
        deadline: Option<Instant>,
        asked: Asked,
        per_state: Duration,
    ) -> Expansion {
        let states = u32::try_from(frontier.len()).unwrap_or(u32::MAX);
        let worth_spreading = frontier.len() >= SMALLEST_SHARED_LEVEL
            || per_state.saturating_mul(states) >= WORTH_SPREADING;
        if !worth_spreading {
            return self.expand_ids(frontier, deadline, asked);
        }
        let own = self.limits.jobs.max(1);
        let loan = self
            .limits
            .helpers
            .as_ref()
            .map(|helpers| helpers.borrow(frontier.len().saturating_sub(own)));
        let jobs = own.saturating_add(loan.as_ref().map_or(0, crate::helpers::Loan::threads));
        if jobs == 1 {
            return self.expand_ids(frontier, deadline, asked);
        }
        let share = frontier.len().div_ceil(jobs);
        let cuts = cut_points(frontier.len(), share);
        std::thread::scope(|scope| {
            let workers: Vec<_> = cuts
                .windows(2)
                .filter_map(|pair| match pair {
                    [from, to] => frontier.get(*from..*to),
                    _ => None,
                })
                .map(|ids| {
                    std::thread::Builder::new()
                        .stack_size(WORKER_STACK)
                        .spawn_scoped(scope, move || self.expand_ids(ids, deadline, asked))
                        .expect("a worker thread starts")
                })
                .collect();
            let mut level = Expansion {
                done: Vec::with_capacity(frontier.len()),
                stopped: false,
                workers: jobs,
            };
            for worker in workers {
                let part = match worker.join() {
                    Ok(part) => part,
                    Err(panic) => std::panic::resume_unwind(panic),
                };
                if !level.stopped {
                    level.done.extend(part.done);
                    level.stopped = part.stopped;
                }
            }
            level
        })
    }

    fn merge(&mut self, frontier: &[usize], level: Vec<Expanded>) -> Vec<usize> {
        let mut next = Vec::new();
        for (id, expanded) in frontier.iter().copied().zip(level) {
            self.record(id, &expanded);
            for successor in expanded.successors {
                let parent = Parent {
                    state: id,
                    action: successor.action,
                    input: successor.input,
                };
                let known = successor
                    .known
                    .or_else(|| self.index.get(&successor.next).copied());
                let target = match known {
                    Some(known) => Some(known),
                    None => self.insert(successor.next, Some(parent)).inspect(|new| {
                        next.push(*new);
                    }),
                };
                let Some(to) = target else {
                    return next;
                };
                if self.limits.keep_graph {
                    self.arrows.push(Arrow {
                        from: id,
                        to,
                        action: successor.action,
                    });
                }
            }
        }
        next
    }

    fn record(&mut self, id: usize, expanded: &Expanded) {
        self.judged = self.judged.max(id.saturating_add(1));
        self.wide |= expanded.width == Width::Beyond32Bits;
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
