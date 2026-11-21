use super::encode::{encode_next, with_updates};
use super::{Arrow, STATES_BETWEEN_CLOCK_READS, Search};
use crate::expand::{Asked, Budget, Clock, Expand, Mark, Scratch, Successors};
use crate::helpers::Helpers;
use crate::store::{Placed, Store};
use earnest_value::Value;
use rustc_hash::FxHashSet;
use std::ops::Range;
use std::time::Instant;

struct HereSink<'a> {
    store: &'a mut Store,
    arrows: &'a mut Vec<Arrow>,
    graph: bool,
    base: Vec<u64>,
    base_is_row: bool,
    from: usize,
    most: usize,
    full: bool,
    row: Vec<u64>,
    after_full: FxHashSet<Box<[Value]>>,
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
        if self.full {
            let next = with_updates(state, updates);
            let known = if encoded {
                let hash = self.store.layout().hash(&self.row);
                self.store.find(&self.row, hash).is_some()
            } else {
                self.store.find_values(&next).is_some()
            };
            return !known && self.after_full.insert(next);
        }
        let parent = Some(self.from);
        let placed = if encoded {
            let hash = self.store.layout().hash(&self.row);
            self.store.place(&self.row, hash, parent, self.most)
        } else {
            self.store
                .place_values(with_updates(state, updates), parent, self.most)
        };
        let (to, counted) = match placed {
            Placed::Known(found) => (found.id, false),
            Placed::New(to) => (Some(to), true),
            Placed::Full => {
                self.full = true;
                self.after_full.insert(with_updates(state, updates));
                return true;
            }
        };
        if self.graph
            && let Some(to) = to
        {
            self.arrows.push(Arrow {
                from: self.from,
                to,
                action,
            });
        }
        counted
    }
}

pub(super) struct Here {
    pub(super) stopped: bool,
    pub(super) full: bool,
    pub(super) handed: bool,
    pub(super) marks: Vec<(usize, Mark)>,
    pub(super) through: usize,
}

impl Search<'_> {
    pub(super) fn expand_here(
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
            most: self.limits.max_states,
            full: false,
            row: Vec::new(),
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
}
