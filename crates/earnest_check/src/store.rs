mod codec;
mod index;
mod layout;
mod level;
mod rows;

use codec::{Codec, Kind};
use earnest_ir::Field;
use earnest_value::{Domain, Value};
use index::{Claim, Index};
pub(crate) use index::{Found, Lookup, SHARD_COUNT, shard_of};
pub use layout::{NumberSlot, RowLayout};
use level::Level;
pub(crate) use level::{Fresh, Spread};
use rows::{Parents, Rows};
use rustc_hash::FxHashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Placed {
    Known(Found),
    New(usize),
    Full,
}

#[derive(Debug)]
pub(crate) struct Store {
    codecs: Vec<Codec>,
    layout: RowLayout,
    rows: Rows,
    parents: Parents,
    count: usize,
    index: Index,
    grown: Vec<usize>,
    wide: FxHashMap<usize, Box<[Value]>>,
    wide_ids: FxHashMap<Box<[Value]>, usize>,
}

impl Store {
    pub(crate) fn new(fields: &[Field], most: usize, lookup: Lookup) -> Self {
        let domains: Vec<&Domain> = fields.iter().map(|field| &field.domain).collect();
        let kinds: Vec<(Kind, u32)> = domains
            .iter()
            .map(|domain| Codec::kind_of(domain))
            .collect();
        let widths: Vec<u32> = kinds.iter().map(|(_, width)| *width).collect();
        let layout = RowLayout::of(&domains, &widths);
        let codecs: Vec<Codec> = kinds
            .into_iter()
            .enumerate()
            .filter_map(|(position, (kind, _))| {
                Some(Codec {
                    place: layout.place(position)?,
                    kind,
                })
            })
            .collect();
        let grown = codecs
            .iter()
            .enumerate()
            .filter(|(_, codec)| matches!(codec.kind, Kind::Grown { .. }))
            .map(|(position, _)| position)
            .collect();
        Self {
            grown,
            codecs,
            rows: Rows::new(layout.stride()),
            index: Index::new(layout.stride(), most, lookup),
            layout,
            parents: Parents::for_up_to(most),
            count: 0,
            wide: FxHashMap::default(),
            wide_ids: FxHashMap::default(),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.count
    }

    pub(crate) fn layout(&self) -> &RowLayout {
        &self.layout
    }

    fn wide_state(&self, id: usize) -> Option<&[Value]> {
        if self.wide.is_empty() {
            return None;
        }
        self.wide.get(&id).map(AsRef::as_ref)
    }

    pub(crate) fn parent(&self, id: usize) -> Option<usize> {
        self.parents.get(id)
    }

    pub(crate) fn state(&self, id: usize) -> Vec<Value> {
        let mut values = Vec::with_capacity(self.codecs.len());
        self.read_state(id, &mut Vec::new(), &mut values);
        values
    }

    pub(crate) fn read_state(
        &self,
        id: usize,
        row: &mut Vec<u64>,
        values: &mut Vec<Value>,
    ) -> bool {
        values.clear();
        row.clear();
        if id >= self.count {
            return false;
        }
        if let Some(full) = self.wide_state(id) {
            values.extend(full.iter().cloned());
            return false;
        }
        self.rows.unpack(id, row, self.layout.words());
        values.extend(self.codecs.iter().map(|codec| codec.value(codec.read(row))));
        true
    }

    pub(crate) fn outside(&self, id: usize, row: &[u64], fields: &[Field]) -> Option<usize> {
        if let Some(full) = self.wide_state(id) {
            return fields
                .iter()
                .zip(full.iter())
                .position(|(field, value)| !field.domain.contains(value));
        }
        self.grown.iter().copied().find(|position| {
            self.codecs
                .get(*position)
                .is_some_and(|codec| !codec.inside(codec.read(row)))
        })
    }

    pub(crate) fn encode_update(&self, row: &mut [u64], position: usize, value: &Value) -> bool {
        if let (Some(slot), Value::Int(number)) = (self.layout.number(position), value) {
            return slot.write(row, *number);
        }
        let Some(codec) = self.codecs.get(position) else {
            return false;
        };
        let Some(code) = codec.code(value) else {
            return false;
        };
        codec.write(row, code);
        true
    }

    pub(crate) fn encode(&self, values: &[Value], row: &mut Vec<u64>) -> bool {
        row.clear();
        row.resize(self.layout.words(), 0);
        values.len() == self.codecs.len()
            && values
                .iter()
                .enumerate()
                .all(|(position, value)| self.encode_update(row, position, value))
    }

    fn encode_or_add(&mut self, values: &[Value], row: &mut Vec<u64>) -> bool {
        row.clear();
        row.resize(self.layout.words(), 0);
        values.len() == self.codecs.len()
            && self.codecs.iter_mut().zip(values).all(|(codec, value)| {
                let Some(code) = codec.code_or_add(value) else {
                    return false;
                };
                codec.write(row, code);
                true
            })
    }

    pub(crate) fn files_rows(&self) -> bool {
        self.index.files_rows()
    }

    pub(crate) fn find_values(&self, values: &[Value]) -> Option<usize> {
        self.wide_ids.get(values).copied()
    }

    pub(crate) fn find(&self, row: &[u64], hash: u64) -> Option<Found> {
        self.index.find(&self.rows, row, hash)
    }

    pub(crate) fn place(
        &mut self,
        row: &[u64],
        hash: u64,
        parent: Option<usize>,
        most: usize,
    ) -> Placed {
        let id = Some(self.count).filter(|count| *count < most);
        match self.index.claim(&self.rows, row, hash, id) {
            Claim::Known(found) => Placed::Known(found),
            Claim::Full => Placed::Full,
            Claim::New => {
                self.rows.push(row);
                self.parents.push(parent);
                self.count += 1;
                Placed::New(self.count - 1)
            }
        }
    }

    pub(crate) fn place_level(
        &mut self,
        parts: &[Fresh<'_>],
        spread: &dyn Spread,
        most: usize,
    ) -> Option<usize> {
        let at = Level {
            parts,
            spread,
            words: self.layout.words(),
            count: self.count,
            most,
        };
        let (rows, parents) = (&mut self.rows, &mut self.parents);
        let placed = match &mut self.index {
            Index::NarrowKeys(shards) => shards.place_level(rows, parents, at),
            Index::Keys(shards) => shards.place_level(rows, parents, at),
            Index::NarrowIds(shards) => shards.place_level(rows, parents, at),
            Index::WideIds(shards) => shards.place_level(rows, parents, at),
        }?;
        self.count += placed;
        Some(placed)
    }

    pub(crate) fn place_values(
        &mut self,
        values: Box<[Value]>,
        parent: Option<usize>,
        most: usize,
    ) -> Placed {
        let mut row = Vec::with_capacity(self.layout.words());
        if self.encode_or_add(&values, &mut row) {
            let hash = self.layout.hash(&row);
            return self.place(&row, hash, parent, most);
        }
        if let Some(id) = self.wide_ids.get(&values) {
            return Placed::Known(Found { id: Some(*id) });
        }
        if self.count >= most {
            return Placed::Full;
        }
        let id = self.count;
        self.rows.grow(1);
        self.parents.push(parent);
        self.wide.insert(id, values.clone());
        self.wide_ids.insert(values, id);
        self.count += 1;
        Placed::New(id)
    }

    pub(crate) fn truncate(&mut self, kept: usize) {
        if kept >= self.count {
            return;
        }
        let filed: Vec<usize> = (kept..self.count)
            .filter(|id| !self.wide.contains_key(id))
            .collect();
        self.index
            .forget(&self.rows, kept, &filed, self.layout.words());
        self.count = kept;
        self.rows.truncate(kept);
        self.parents.truncate(kept);
        self.wide.retain(|id, _| *id < kept);
        self.wide_ids.retain(|_, id| *id < kept);
    }
}
