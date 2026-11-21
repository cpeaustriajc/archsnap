use earnest_ir::Field;
use earnest_value::{Domain, Value};
use hashbrown::HashTable;
use rustc_hash::{FxHashMap, FxHasher};
use std::hash::Hasher;

const WORD_BITS: u32 = u64::BITS;
const SHARD_BITS: u32 = 6;
const SHARD_COUNT: usize = 1 << SHARD_BITS;
const SHARD_SHIFT: u32 = 51;
const SHARD_MASK: u64 = (1 << SHARD_BITS) - 1;
const MOST_LISTED_VALUES: u128 = 1 << 16;
const GROWN_BITS: u32 = 32;
const MOST_GROWN_VALUES: usize = 1 << GROWN_BITS;

#[derive(Debug, Default)]
struct Dict {
    values: Vec<Value>,
    inside: Vec<bool>,
    codes: FxHashMap<Value, u64>,
}

impl Dict {
    fn add(&mut self, value: Value, domain: Option<&Domain>) -> Option<u64> {
        if self.values.len() >= MOST_GROWN_VALUES {
            return None;
        }
        let inside = domain.is_none_or(|domain| domain.contains(&value));
        let code = u64::try_from(self.values.len()).ok()?;
        self.codes.insert(value.clone(), code);
        self.values.push(value);
        self.inside.push(inside);
        Some(code)
    }
}

#[derive(Debug)]
enum Kind {
    Int { lo: i64, hi: i64 },
    Bool,
    Listed(Dict),
    Grown { dict: Dict, domain: Domain },
}

#[derive(Debug)]
struct Codec {
    word: usize,
    shift: u32,
    mask: u64,
    kind: Kind,
}

fn bits_for(count: u128) -> u32 {
    if count <= 1 {
        0
    } else {
        (count - 1).bit_width()
    }
}

fn mask_of(bits: u32) -> u64 {
    if bits >= WORD_BITS {
        u64::MAX
    } else {
        (1_u64 << bits) - 1
    }
}

impl Codec {
    fn kind_of(domain: &Domain) -> (Kind, u32) {
        match domain {
            Domain::Int { lo, hi } => (Kind::Int { lo: *lo, hi: *hi }, bits_for(domain.count())),
            Domain::Bool => (Kind::Bool, 1),
            _ if domain.count() <= MOST_LISTED_VALUES => {
                let mut dict = Dict::default();
                for value in domain.values() {
                    if domain.contains(&value) && !dict.codes.contains_key(&value) {
                        dict.add(value, None);
                    }
                }
                let bits = bits_for(u128::try_from(dict.values.len()).unwrap_or(u128::MAX));
                (Kind::Listed(dict), bits)
            }
            _ => (
                Kind::Grown {
                    dict: Dict::default(),
                    domain: domain.clone(),
                },
                GROWN_BITS,
            ),
        }
    }

    fn code(&self, value: &Value) -> Option<u64> {
        match (&self.kind, value) {
            (Kind::Int { lo, hi }, Value::Int(number)) if lo <= number && number <= hi => {
                Some(number.abs_diff(*lo))
            }
            (Kind::Bool, Value::Bool(truth)) => Some(u64::from(*truth)),
            (Kind::Listed(dict) | Kind::Grown { dict, .. }, found) => {
                dict.codes.get(found).copied()
            }
            (Kind::Int { .. } | Kind::Bool, _) => None,
        }
    }

    fn code_or_add(&mut self, value: &Value) -> Option<u64> {
        if let Some(code) = self.code(value) {
            return Some(code);
        }
        match &mut self.kind {
            Kind::Grown { dict, domain } => dict.add(value.clone(), Some(domain)),
            Kind::Int { .. } | Kind::Bool | Kind::Listed(_) => None,
        }
    }

    fn value(&self, code: u64) -> Value {
        match &self.kind {
            Kind::Int { lo, .. } => lo
                .checked_add_unsigned(code)
                .map_or(Value::Null, Value::Int),
            Kind::Bool => Value::Bool(code != 0),
            Kind::Listed(dict) | Kind::Grown { dict, .. } => usize::try_from(code)
                .ok()
                .and_then(|at| dict.values.get(at))
                .cloned()
                .unwrap_or(Value::Null),
        }
    }

    fn inside(&self, code: u64) -> bool {
        match &self.kind {
            Kind::Grown { dict, .. } => usize::try_from(code)
                .ok()
                .and_then(|at| dict.inside.get(at))
                .copied()
                .unwrap_or(false),
            Kind::Int { .. } | Kind::Bool | Kind::Listed(_) => true,
        }
    }

    fn read(&self, row: &[u64]) -> u64 {
        row.get(self.word).map_or(0, |word| {
            word.checked_shr(self.shift).unwrap_or(0) & self.mask
        })
    }

    fn write(&self, row: &mut [u64], code: u64) {
        if let Some(word) = row.get_mut(self.word) {
            let placed = self.mask.checked_shl(self.shift).unwrap_or(0);
            *word = (*word & !placed) | (code.checked_shl(self.shift).unwrap_or(0) & placed);
        }
    }
}

/// Whether a row was in the store, was put in it, or could not be put in because the store
/// already holds the most states it may.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Placed {
    Known(usize),
    New(usize),
    Full,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Parent {
    pub(crate) state: usize,
    pub(crate) action: usize,
    pub(crate) input: usize,
}

#[derive(Debug)]
pub(crate) struct Store {
    codecs: Vec<Codec>,
    words: usize,
    rows: Vec<u64>,
    count: usize,
    parents: Vec<Option<Parent>>,
    shards: Vec<HashTable<usize>>,
    grown: Vec<usize>,
    wide: FxHashMap<usize, Box<[Value]>>,
    wide_ids: FxHashMap<Box<[Value]>, usize>,
}

pub(crate) fn same_row(one: &[u64], other: &[u64]) -> bool {
    one.len() == other.len() && one.iter().zip(other).all(|(left, right)| left == right)
}

pub(crate) fn hash_row(row: &[u64]) -> u64 {
    let mut hasher = FxHasher::default();
    for word in row {
        hasher.write_u64(*word);
    }
    hasher.finish()
}

fn shard_of(hash: u64) -> usize {
    usize::try_from(hash.checked_shr(SHARD_SHIFT).unwrap_or(0) & SHARD_MASK).unwrap_or(0)
}

pub(crate) fn row_at(rows: &[u64], words: usize, id: usize) -> &[u64] {
    let start = id.saturating_mul(words);
    rows.get(start..start.saturating_add(words)).unwrap_or(&[])
}

impl Store {
    pub(crate) fn new(fields: &[Field]) -> Self {
        let mut codecs = Vec::with_capacity(fields.len());
        let mut word = 0;
        let mut used = 0;
        for field in fields {
            let (kind, bits) = Codec::kind_of(&field.domain);
            if used > 0 && used + bits > WORD_BITS {
                word += 1;
                used = 0;
            }
            codecs.push(Codec {
                word,
                shift: used,
                mask: mask_of(bits),
                kind,
            });
            used += bits;
        }
        let words = if used > 0 { word + 1 } else { word };
        let grown = codecs
            .iter()
            .enumerate()
            .filter(|(_, codec)| matches!(codec.kind, Kind::Grown { .. }))
            .map(|(position, _)| position)
            .collect();
        Self {
            grown,
            codecs,
            words,
            rows: Vec::new(),
            count: 0,
            parents: Vec::new(),
            shards: (0..SHARD_COUNT).map(|_| HashTable::new()).collect(),
            wide: FxHashMap::default(),
            wide_ids: FxHashMap::default(),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.count
    }

    pub(crate) fn words(&self) -> usize {
        self.words
    }

    pub(crate) fn row(&self, id: usize) -> Option<&[u64]> {
        if id >= self.count || self.wide_state(id).is_some() {
            return None;
        }
        Some(row_at(&self.rows, self.words, id))
    }

    fn wide_state(&self, id: usize) -> Option<&[Value]> {
        if self.wide.is_empty() {
            return None;
        }
        self.wide.get(&id).map(AsRef::as_ref)
    }

    pub(crate) fn parent(&self, id: usize) -> Option<Parent> {
        self.parents.get(id).copied().flatten()
    }

    pub(crate) fn state(&self, id: usize) -> Vec<Value> {
        let mut values = Vec::with_capacity(self.codecs.len());
        self.read_state(id, &mut values);
        values
    }

    pub(crate) fn read_state(&self, id: usize, values: &mut Vec<Value>) {
        if let Some(full) = self.wide_state(id) {
            values.extend(full.iter().cloned());
            return;
        }
        let row = row_at(&self.rows, self.words, id);
        values.extend(self.codecs.iter().map(|codec| codec.value(codec.read(row))));
    }

    pub(crate) fn outside(&self, id: usize, fields: &[Field]) -> Option<usize> {
        if let Some(full) = self.wide_state(id) {
            return fields
                .iter()
                .zip(full.iter())
                .position(|(field, value)| !field.domain.contains(value));
        }
        let row = row_at(&self.rows, self.words, id);
        self.grown.iter().copied().find(|position| {
            self.codecs
                .get(*position)
                .is_some_and(|codec| !codec.inside(codec.read(row)))
        })
    }

    pub(crate) fn encode_update(&self, row: &mut [u64], position: usize, value: &Value) -> bool {
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
        row.resize(self.words, 0);
        values.len() == self.codecs.len()
            && values
                .iter()
                .enumerate()
                .all(|(position, value)| self.encode_update(row, position, value))
    }

    fn encode_or_add(&mut self, values: &[Value], row: &mut Vec<u64>) -> bool {
        row.clear();
        row.resize(self.words, 0);
        values.len() == self.codecs.len()
            && self.codecs.iter_mut().zip(values).all(|(codec, value)| {
                let Some(code) = codec.code_or_add(value) else {
                    return false;
                };
                codec.write(row, code);
                true
            })
    }

    pub(crate) fn find_values(&self, values: &[Value]) -> Option<usize> {
        self.wide_ids.get(values).copied()
    }

    pub(crate) fn find(&self, row: &[u64], hash: u64) -> Option<usize> {
        let (rows, words) = (&self.rows, self.words);
        self.shards
            .get(shard_of(hash))?
            .find(hash, |id| same_row(row_at(rows, words, *id), row))
            .copied()
    }

    pub(crate) fn place(
        &mut self,
        row: &[u64],
        hash: u64,
        parent: Option<Parent>,
        most: usize,
    ) -> Placed {
        let Some(shard) = self.shards.get_mut(shard_of(hash)) else {
            return Placed::Full;
        };
        let (rows, words) = (&self.rows, self.words);
        let entry = shard.entry(
            hash,
            |id| same_row(row_at(rows, words, *id), row),
            |id| hash_row(row_at(rows, words, *id)),
        );
        match entry {
            hashbrown::hash_table::Entry::Occupied(found) => Placed::Known(*found.get()),
            hashbrown::hash_table::Entry::Vacant(_) if self.count >= most => Placed::Full,
            hashbrown::hash_table::Entry::Vacant(vacant) => {
                let id = self.count;
                vacant.insert(id);
                self.rows.extend_from_slice(row);
                self.parents.push(parent);
                self.count += 1;
                Placed::New(id)
            }
        }
    }

    pub(crate) fn place_values(
        &mut self,
        values: Box<[Value]>,
        parent: Option<Parent>,
        most: usize,
    ) -> Placed {
        let mut row = Vec::with_capacity(self.words);
        if self.encode_or_add(&values, &mut row) {
            let hash = hash_row(&row);
            return self.place(&row, hash, parent, most);
        }
        if let Some(id) = self.wide_ids.get(&values) {
            return Placed::Known(*id);
        }
        if self.count >= most {
            return Placed::Full;
        }
        let id = self.count;
        self.rows
            .resize(self.rows.len().saturating_add(self.words), 0);
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
        self.count = kept;
        self.rows.truncate(kept.saturating_mul(self.words));
        self.parents.truncate(kept);
        self.wide.retain(|id, _| *id < kept);
        self.wide_ids.retain(|_, id| *id < kept);
        for shard in &mut self.shards {
            shard.retain(|id| *id < kept);
        }
    }
}
