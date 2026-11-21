use super::encode::{encode_next, with_updates};
use super::{STATES_BETWEEN_CLOCK_READS, Search};
use crate::expand::{Asked, Clock, Mark, Scratch, Successors};
use crate::store::{SHARD_COUNT, Store, hash_row, row_at, same_row, shard_of};
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
    pub(super) rows: Vec<u8>,
    pub(super) hashes: Vec<u64>,
    pub(super) froms: Vec<usize>,
    pub(super) shards: Vec<Vec<usize>>,
    pub(super) raws: Vec<Box<[Value]>>,
    pub(super) stopped: bool,
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

    fn offer_new_with_graph(&mut self, action: usize, hash: u64) -> bool {
        let stride = self.store.stride();
        let (rows, hashes, row) = (&self.part.rows, &self.part.hashes, &self.packed);
        let met = self.met.entry(
            hash,
            |at| same_row(row_at(rows, stride, *at), row),
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
            return false;
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
                return false;
            }
            let first = self.met_raw.insert(next.clone());
            if !self.graph && !first {
                return false;
            }
            self.part.raws.push(next);
            let fresh_before = self.part.hashes.len();
            self.push(action, Target::Raw { fresh_before });
            return first;
        }
        self.store.pack(&self.row, &mut self.packed);
        let hash = hash_row(&self.packed);
        if !self.graph {
            return self.offer_unseen(action, hash);
        }
        if let Some(to) = self.store.find(&self.packed, hash) {
            self.push(action, Target::Known(to));
            return false;
        }
        self.offer_new_with_graph(action, hash)
    }
}

impl Search<'_> {
    pub(super) fn expand_ids(
        &self,
        ids: Range<usize>,
        deadline: Option<Instant>,
        asked: Asked,
    ) -> Part {
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
}
