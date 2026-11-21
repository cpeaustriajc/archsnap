use super::encode::{encode_next, with_updates};
use super::{STATES_BETWEEN_CLOCK_READS, Search};
use crate::expand::{Asked, Clock, Mark, Scratch, Successors};
use crate::store::{SHARD_COUNT, Store, shard_of};
use earnest_value::Value;
use hashbrown::HashTable;
use rustc_hash::FxHashSet;
use std::ops::Range;
use std::time::Instant;

#[derive(Clone, Copy, Debug)]
pub(super) enum Target {
    Known(usize),
    Fresh(usize),
    Raw { fresh_before: usize },
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Entry {
    pub(super) from: usize,
    pub(super) action: usize,
    pub(super) target: Target,
}

#[derive(Default)]
pub(super) struct Part {
    pub(super) first: usize,
    pub(super) expanded: usize,
    pub(super) marks: Vec<(usize, Mark)>,
    pub(super) entries: Vec<Entry>,
    pub(super) rows: Vec<u64>,
    pub(super) hashes: Vec<u64>,
    pub(super) froms: Vec<usize>,
    pub(super) by_shard: Vec<usize>,
    pub(super) shard_starts: Vec<usize>,
    pub(super) raws: Vec<Box<[Value]>>,
    pub(super) stopped: bool,
    pub(super) crowded: bool,
}

#[derive(Default)]
pub(super) struct Kit {
    rows: RowKit,
    fresh_per_state: (usize, usize),
    scratch: Scratch,
    state: Vec<Value>,
}

#[derive(Default)]
struct RowKit {
    base: Vec<u64>,
    row: Vec<u64>,
    met: HashTable<usize>,
    met_words: HashTable<u64>,
    met_raw: FxHashSet<Box<[Value]>>,
}

struct PartSink<'a> {
    store: &'a Store,
    graph: bool,
    base_is_row: bool,
    from: usize,
    counted: usize,
    kit: &'a mut RowKit,
    part: Part,
}

fn same_row(one: &[u64], other: &[u64]) -> bool {
    one.len() == other.len() && one.iter().zip(other).all(|(left, right)| left == right)
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
        append_row(&mut self.part.rows, &self.kit.row);
        self.part.hashes.push(hash);
        self.part.froms.push(self.from);
        if self.graph {
            self.push(action, Target::Fresh(self.part.hashes.len() - 1));
        }
    }

    fn offer_new_with_graph(&mut self, action: usize, hash: u64) -> bool {
        let words = self.store.layout().words();
        let (rows, hashes, row) = (&self.part.rows, &self.part.hashes, &self.kit.row);
        let met = self.kit.met.entry(
            hash,
            |at| same_row(row_at(rows, words, *at), row),
            |at| hashes.get(*at).copied().unwrap_or(0),
        );
        match met {
            hashbrown::hash_table::Entry::Occupied(earlier) => {
                let at = *earlier.get();
                self.push(action, Target::Fresh(at));
                false
            }
            hashbrown::hash_table::Entry::Vacant(vacant) => {
                vacant.insert(hashes.len());
                self.add_fresh(action, hash);
                true
            }
        }
    }

    fn offer_unseen(&mut self, action: usize, hash: u64) -> bool {
        if let [word] = self.kit.row.as_slice() {
            return self.offer_unseen_word(action, *word, hash);
        }
        let words = self.store.layout().words();
        let (rows, hashes, row) = (&self.part.rows, &self.part.hashes, &self.kit.row);
        let met = self.kit.met.entry(
            hash,
            |at| same_row(row_at(rows, words, *at), row),
            |at| hashes.get(*at).copied().unwrap_or(0),
        );
        let hashbrown::hash_table::Entry::Vacant(vacant) = met else {
            return false;
        };
        if self.store.find(row, hash).is_some() {
            return false;
        }
        vacant.insert(hashes.len());
        self.add_fresh(action, hash);
        true
    }
}

impl PartSink<'_> {
    fn offer_unseen_word(&mut self, action: usize, word: u64, hash: u64) -> bool {
        let layout = self.store.layout();
        let met = self.kit.met_words.entry(
            hash,
            |earlier| *earlier == word,
            |earlier| layout.hash(&[*earlier]),
        );
        let hashbrown::hash_table::Entry::Vacant(vacant) = met else {
            return false;
        };
        vacant.insert(word);
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
        let counted = self.offer_unless_met(action, state, updates);
        self.counted += usize::from(counted);
        counted
    }
}

impl PartSink<'_> {
    fn offer_unless_met(
        &mut self,
        action: usize,
        state: &[Value],
        updates: &[(usize, Value)],
    ) -> bool {
        let base = self.base_is_row.then_some(self.kit.base.as_slice());
        if !encode_next(self.store, base, &mut self.kit.row, state, updates) {
            let next = with_updates(state, updates);
            if let Some(to) = self.store.find_values(&next) {
                if self.graph {
                    self.push(action, Target::Known(to));
                }
                return false;
            }
            let first = self.kit.met_raw.insert(next.clone());
            if !self.graph && !first {
                return false;
            }
            self.part.raws.push(next);
            let fresh_before = self.part.hashes.len();
            self.push(action, Target::Raw { fresh_before });
            return first;
        }
        let hash = self.store.layout().hash(&self.kit.row);
        if !self.graph {
            return self.offer_unseen(action, hash);
        }
        if let Some(found) = self.store.find(&self.kit.row, hash) {
            if let Some(to) = found.id {
                self.push(action, Target::Known(to));
            }
            return false;
        }
        self.offer_new_with_graph(action, hash)
    }
}

impl Part {
    fn file_by_shard(&mut self) {
        let mut starts = vec![0; SHARD_COUNT + 1];
        for hash in &self.hashes {
            if let Some(count) = starts.get_mut(shard_of(*hash) + 1) {
                *count += 1;
            }
        }
        for shard in 0..SHARD_COUNT {
            let before = starts.get(shard).copied().unwrap_or(0);
            if let Some(start) = starts.get_mut(shard + 1) {
                *start += before;
            }
        }
        let mut next = starts.clone();
        self.by_shard = vec![0; self.hashes.len()];
        for (ordinal, hash) in self.hashes.iter().enumerate() {
            if let Some(at) = next.get_mut(shard_of(*hash))
                && let Some(slot) = self.by_shard.get_mut(*at)
            {
                *slot = ordinal;
                *at += 1;
            }
        }
        self.shard_starts = starts;
    }
}

fn append_row(rows: &mut Vec<u64>, row: &[u64]) {
    if let [word] = row {
        rows.push(*word);
    } else {
        rows.extend_from_slice(row);
    }
}

pub(super) fn row_at(rows: &[u64], words: usize, at: usize) -> &[u64] {
    let start = at.saturating_mul(words);
    rows.get(start..start.saturating_add(words)).unwrap_or(&[])
}

impl Search<'_> {
    pub(super) fn expand_ids(
        &self,
        ids: Range<usize>,
        deadline: Option<Instant>,
        asked: Asked,
        kit: &mut Kit,
    ) -> Part {
        let expand = self.expand(deadline, asked);
        kit.rows.met.clear();
        kit.rows.met_words.clear();
        kit.rows.met_raw.clear();
        let (fresh_before, states_before) = kit.fresh_per_state;
        let expected = (ids.len() * fresh_before)
            .checked_div(states_before)
            .unwrap_or(0);
        let words = self.store.layout().words();
        let mut part = Part {
            first: ids.start,
            ..Part::default()
        };
        if !self.limits.keep_graph {
            part.rows.reserve(expected * words);
            part.hashes.reserve(expected);
            part.froms.reserve(expected);
        }
        let mut sink = PartSink {
            store: &self.store,
            graph: self.limits.keep_graph,
            base_is_row: false,
            from: 0,
            counted: 0,
            kit: &mut kit.rows,
            part,
        };
        for (position, id) in ids.enumerate() {
            let clock_due = position.is_multiple_of(STATES_BETWEEN_CLOCK_READS);
            let late = deadline.is_some_and(|limit| Instant::now() >= limit);
            if asked == Asked::Everything && clock_due && late {
                sink.part.stopped = true;
                return sink.part;
            }
            sink.base_is_row = self
                .store
                .read_state(id, &mut sink.kit.base, &mut kit.state);
            let outside = self.store.outside(id, &sink.kit.base, &self.model.fields);
            sink.from = id;
            sink.counted = 0;
            let mark = expand.state(&kit.state, outside, &mut sink, &mut kit.scratch);
            if mark.clock == Clock::Stopped {
                sink.part.stopped = true;
                return sink.part;
            }
            if sink.counted > self.limits.max_states {
                sink.part.crowded = true;
                return sink.part;
            }
            if !mark.is_plain() {
                sink.part.marks.push((id, mark));
            }
            sink.part.expanded = sink.part.expanded.saturating_add(1);
        }
        sink.part.file_by_shard();
        kit.fresh_per_state = (sink.part.hashes.len(), sink.part.expanded);
        sink.part
    }
}
