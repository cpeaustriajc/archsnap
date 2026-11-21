use earnest_ir::Field;
use earnest_value::{Domain, Value};
use hashbrown::HashTable;
use rustc_hash::{FxHashMap, FxHasher};
use std::hash::Hasher;
use std::sync::{Mutex, PoisonError};

const WORD_BITS: u32 = u64::BITS;
const WORD_BYTES: usize = 8;
const SHARD_BITS: u32 = 6;
pub(crate) const SHARD_COUNT: usize = 1 << SHARD_BITS;
const SHARD_SHIFT: u32 = 51;
const SHARD_MASK: u64 = (1 << SHARD_BITS) - 1;
const MOST_LISTED_VALUES: u128 = 1 << 12;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Placed {
    Known(usize),
    New(usize),
    Full,
}

trait Id: Copy + Eq {
    const NONE: Self;
    fn from_index(id: usize) -> Option<Self>;
    fn index(self) -> usize;
}

impl Id for u32 {
    const NONE: Self = Self::MAX;

    fn from_index(id: usize) -> Option<Self> {
        Self::try_from(id)
            .ok()
            .filter(|narrow| *narrow != Self::NONE)
    }

    fn index(self) -> usize {
        usize::try_from(self).unwrap_or(usize::MAX)
    }
}

impl Id for usize {
    const NONE: Self = Self::MAX;

    fn from_index(id: usize) -> Option<Self> {
        Some(id).filter(|wide| *wide != Self::NONE)
    }

    fn index(self) -> usize {
        self
    }
}

#[derive(Debug)]
struct Links<T> {
    parents: Vec<T>,
    shards: Vec<HashTable<T>>,
}

impl<T: Id> Links<T> {
    fn new() -> Self {
        Self {
            parents: Vec::new(),
            shards: (0..SHARD_COUNT).map(|_| HashTable::new()).collect(),
        }
    }

    fn parent(&self, id: usize) -> Option<usize> {
        self.parents
            .get(id)
            .copied()
            .filter(|parent| *parent != T::NONE)
            .map(Id::index)
    }

    fn push_parent(&mut self, parent: Option<usize>) {
        self.parents
            .push(parent.and_then(T::from_index).unwrap_or(T::NONE));
    }

    fn find(&self, rows: &[u8], stride: usize, row: &[u8], hash: u64) -> Option<usize> {
        self.shards
            .get(shard_of(hash))?
            .find(hash, |id| same_row(row_at(rows, stride, id.index()), row))
            .map(|id| id.index())
    }

    fn place(&mut self, rows: &mut Vec<u8>, at: Place<'_>) -> Placed {
        let Some(shard) = self.shards.get_mut(shard_of(at.hash)) else {
            return Placed::Full;
        };
        let stride = at.stride;
        let entry = shard.entry(
            at.hash,
            |id| same_row(row_at(rows, stride, id.index()), at.row),
            |id| hash_row(row_at(rows, stride, id.index())),
        );
        match entry {
            hashbrown::hash_table::Entry::Occupied(found) => Placed::Known(found.get().index()),
            hashbrown::hash_table::Entry::Vacant(vacant) => {
                let Some(id) = T::from_index(at.count).filter(|_| at.count < at.most) else {
                    return Placed::Full;
                };
                vacant.insert(id);
                rows.extend_from_slice(at.row);
                self.push_parent(at.parent);
                Placed::New(at.count)
            }
        }
    }

    fn place_level(&mut self, rows: &mut Vec<u8>, at: Level<'_>) -> Option<usize>
    where
        T: Send + Sync,
    {
        let wins = winners(&at);
        let maps: Vec<Winners> = gather(at.spread, at.parts.len(), |part| {
            Winners::of(
                &wins,
                part,
                at.parts.get(part).map_or(0, |fresh| fresh.hashes.len()),
            )
        });
        let mut bases = Vec::with_capacity(maps.len());
        let mut total = 0;
        for map in &maps {
            bases.push(total);
            total += map.count;
        }
        if at.count + total > at.most {
            return None;
        }
        let first_new = rows.len();
        rows.resize(first_new + total * at.stride, 0);
        let first_parent = self.parents.len();
        self.parents.resize(first_parent + total, T::NONE);
        {
            let row_parts = split_by(
                rows.get_mut(first_new..).unwrap_or_default(),
                &maps,
                at.stride,
            );
            let parent_parts = split_by(
                self.parents.get_mut(first_parent..).unwrap_or_default(),
                &maps,
                1,
            );
            at.spread.run(maps.len(), &|part| {
                let (Some(map), Some(fresh), Some(row_part), Some(parent_part)) = (
                    maps.get(part),
                    at.parts.get(part),
                    row_parts.get(part),
                    parent_parts.get(part),
                ) else {
                    return;
                };
                let mut row_part = row_part.lock().unwrap_or_else(PoisonError::into_inner);
                let mut parent_part = parent_part.lock().unwrap_or_else(PoisonError::into_inner);
                for (place, won) in map.ordinals().enumerate() {
                    let start = place * at.stride;
                    if let Some(row) = row_part.get_mut(start..start + at.stride) {
                        row.copy_from_slice(row_at(fresh.rows, at.stride, won));
                    }
                    if let Some(parent) = parent_part.get_mut(place) {
                        *parent = fresh
                            .froms
                            .get(won)
                            .and_then(|from| T::from_index(*from))
                            .unwrap_or(T::NONE);
                    }
                }
            });
        }
        let arena: &[u8] = rows;
        let stride = at.stride;
        let tables: Vec<Mutex<&mut HashTable<T>>> =
            self.shards.iter_mut().map(Mutex::new).collect();
        at.spread.run(tables.len(), &|shard| {
            let (Some(table), Some(won)) = (tables.get(shard), wins.get(shard)) else {
                return;
            };
            let mut table = table.lock().unwrap_or_else(PoisonError::into_inner);
            let hasher = |id: &T| hash_row(row_at(arena, stride, id.index()));
            table.reserve(won.iter().map(Vec::len).sum(), hasher);
            for (part, ordinals) in won.iter().enumerate() {
                let (Some(map), Some(base), Some(fresh)) =
                    (maps.get(part), bases.get(part), at.parts.get(part))
                else {
                    continue;
                };
                for ordinal in ordinals {
                    let id = at.count + base + map.rank(*ordinal);
                    let hash = fresh.hashes.get(*ordinal).copied().unwrap_or(0);
                    table.insert_unique(hash, T::from_index(id).unwrap_or(T::NONE), hasher);
                }
            }
        });
        Some(total)
    }

    fn truncate(&mut self, kept: usize) {
        self.parents.truncate(kept);
        for shard in &mut self.shards {
            shard.retain(|id| id.index() < kept);
        }
    }
}

pub(crate) trait Spread: Sync {
    fn run(&self, tasks: usize, job: &(dyn Fn(usize) + Sync));
}

pub(crate) struct Fresh<'a> {
    pub(crate) rows: &'a [u8],
    pub(crate) hashes: &'a [u64],
    pub(crate) froms: &'a [usize],
    pub(crate) shards: &'a [Vec<usize>],
}

#[derive(Clone, Copy)]
struct Level<'a> {
    parts: &'a [Fresh<'a>],
    spread: &'a dyn Spread,
    stride: usize,
    count: usize,
    most: usize,
}

#[derive(Default)]
struct Winners {
    bits: Vec<u64>,
    before: Vec<usize>,
    count: usize,
}

const BITS_PER_WORD: usize = 64;

impl Winners {
    fn of(wins: &[Vec<Vec<usize>>], part: usize, found: usize) -> Self {
        let mut bits = vec![0_u64; found.div_ceil(BITS_PER_WORD)];
        for shard in wins {
            for ordinal in shard.get(part).map_or(&[][..], Vec::as_slice) {
                if let Some(word) = bits.get_mut(ordinal / BITS_PER_WORD) {
                    *word |= 1 << (ordinal % BITS_PER_WORD);
                }
            }
        }
        let mut before = Vec::with_capacity(bits.len());
        let mut count = 0;
        for word in &bits {
            before.push(count);
            count += usize::try_from(word.count_ones()).unwrap_or(0);
        }
        Self {
            bits,
            before,
            count,
        }
    }

    fn rank(&self, ordinal: usize) -> usize {
        let word = ordinal / BITS_PER_WORD;
        let below = (1_u64 << (ordinal % BITS_PER_WORD)) - 1;
        let within = self.bits.get(word).map_or(0, |bits| {
            usize::try_from((bits & below).count_ones()).unwrap_or(0)
        });
        self.before.get(word).copied().unwrap_or(0) + within
    }

    fn ordinals(&self) -> impl Iterator<Item = usize> + '_ {
        self.bits.iter().enumerate().flat_map(|(word, bits)| {
            let mut rest = *bits;
            std::iter::from_fn(move || {
                if rest == 0 {
                    return None;
                }
                let bit = usize::try_from(rest.trailing_zeros()).unwrap_or(0);
                rest &= rest - 1;
                Some(word * BITS_PER_WORD + bit)
            })
        })
    }
}

fn winners(at: &Level<'_>) -> Vec<Vec<Vec<usize>>> {
    let stride = at.stride;
    let parts = at.parts;
    let row_of = |part: usize, ordinal: usize| {
        parts
            .get(part)
            .map_or(&[][..], |fresh| row_at(fresh.rows, stride, ordinal))
    };
    let hash_of = |part: usize, ordinal: usize| {
        parts
            .get(part)
            .and_then(|fresh| fresh.hashes.get(ordinal))
            .copied()
            .unwrap_or(0)
    };
    gather(at.spread, SHARD_COUNT, |shard| {
        let mut seen: HashTable<(usize, usize)> = HashTable::new();
        let mut won = Vec::with_capacity(parts.len());
        for (part, fresh) in parts.iter().enumerate() {
            let mut mine = Vec::new();
            for ordinal in fresh.shards.get(shard).map_or(&[][..], Vec::as_slice) {
                let (hash, row) = (hash_of(part, *ordinal), row_of(part, *ordinal));
                if let hashbrown::hash_table::Entry::Vacant(vacant) = seen.entry(
                    hash,
                    |(other, at)| same_row(row_of(*other, *at), row),
                    |(other, at)| hash_of(*other, *at),
                ) {
                    vacant.insert((part, *ordinal));
                    mine.push(*ordinal);
                }
            }
            won.push(mine);
        }
        won
    })
}

fn gather<R: Send + Default>(
    spread: &dyn Spread,
    tasks: usize,
    job: impl Fn(usize) -> R + Sync,
) -> Vec<R> {
    let results: Vec<Mutex<R>> = (0..tasks).map(|_| Mutex::new(R::default())).collect();
    spread.run(tasks, &|task| {
        if let Some(slot) = results.get(task) {
            *slot.lock().unwrap_or_else(PoisonError::into_inner) = job(task);
        }
    });
    results
        .into_iter()
        .map(|slot| slot.into_inner().unwrap_or_else(PoisonError::into_inner))
        .collect()
}

fn split_by<'a, U>(
    mut rest: &'a mut [U],
    maps: &[Winners],
    width: usize,
) -> Vec<Mutex<&'a mut [U]>> {
    let mut parts = Vec::with_capacity(maps.len());
    for map in maps {
        let (part, after) = rest.split_at_mut((map.count * width).min(rest.len()));
        parts.push(Mutex::new(part));
        rest = after;
    }
    parts
}

#[derive(Clone, Copy)]
struct Place<'a> {
    row: &'a [u8],
    hash: u64,
    stride: usize,
    count: usize,
    most: usize,
    parent: Option<usize>,
}

#[derive(Debug)]
enum Ids {
    Narrow(Links<u32>),
    Wide(Links<usize>),
}

#[derive(Debug)]
pub(crate) struct Store {
    codecs: Vec<Codec>,
    words: usize,
    stride: usize,
    rows: Vec<u8>,
    count: usize,
    ids: Ids,
    grown: Vec<usize>,
    wide: FxHashMap<usize, Box<[Value]>>,
    wide_ids: FxHashMap<Box<[Value]>, usize>,
}

pub(crate) fn same_row(one: &[u8], other: &[u8]) -> bool {
    one.len() == other.len() && one.iter().zip(other).all(|(left, right)| left == right)
}

pub(crate) fn hash_row(row: &[u8]) -> u64 {
    let mut hasher = FxHasher::default();
    hasher.write(row);
    hasher.finish()
}

fn word_of(chunk: &[u8]) -> u64 {
    if let Ok(whole) = <[u8; WORD_BYTES]>::try_from(chunk) {
        return u64::from_le_bytes(whole);
    }
    chunk
        .iter()
        .rev()
        .fold(0, |word, byte| (word << u8::BITS) | u64::from(*byte))
}

pub(crate) fn shard_of(hash: u64) -> usize {
    usize::try_from(hash.checked_shr(SHARD_SHIFT).unwrap_or(0) & SHARD_MASK).unwrap_or(0)
}

pub(crate) fn row_at(rows: &[u8], stride: usize, id: usize) -> &[u8] {
    let start = id.saturating_mul(stride);
    rows.get(start..start.saturating_add(stride)).unwrap_or(&[])
}

impl Store {
    pub(crate) fn new(fields: &[Field], most: usize) -> Self {
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
        let stride =
            word * WORD_BYTES + usize::try_from(used.div_ceil(u8::BITS)).unwrap_or(WORD_BYTES);
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
            stride,
            rows: Vec::new(),
            count: 0,
            ids: if u32::from_index(most).is_some() {
                Ids::Narrow(Links::new())
            } else {
                Ids::Wide(Links::new())
            },
            wide: FxHashMap::default(),
            wide_ids: FxHashMap::default(),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.count
    }

    pub(crate) fn stride(&self) -> usize {
        self.stride
    }

    fn unpack(&self, id: usize, words: &mut Vec<u64>) -> bool {
        words.clear();
        if id >= self.count || self.wide_state(id).is_some() {
            return false;
        }
        let packed = row_at(&self.rows, self.stride, id);
        words.extend(packed.chunks(WORD_BYTES).map(word_of));
        words.resize(self.words, 0);
        true
    }

    pub(crate) fn pack(&self, words: &[u64], packed: &mut Vec<u8>) {
        packed.clear();
        for word in words {
            packed.extend_from_slice(&word.to_le_bytes());
        }
        packed.truncate(self.stride);
    }

    fn wide_state(&self, id: usize) -> Option<&[Value]> {
        if self.wide.is_empty() {
            return None;
        }
        self.wide.get(&id).map(AsRef::as_ref)
    }

    pub(crate) fn parent(&self, id: usize) -> Option<usize> {
        match &self.ids {
            Ids::Narrow(links) => links.parent(id),
            Ids::Wide(links) => links.parent(id),
        }
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
        if self.unpack(id, row) {
            values.extend(self.codecs.iter().map(|codec| codec.value(codec.read(row))));
            return true;
        }
        if let Some(full) = self.wide_state(id) {
            values.extend(full.iter().cloned());
        }
        false
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

    pub(crate) fn find(&self, row: &[u8], hash: u64) -> Option<usize> {
        match &self.ids {
            Ids::Narrow(links) => links.find(&self.rows, self.stride, row, hash),
            Ids::Wide(links) => links.find(&self.rows, self.stride, row, hash),
        }
    }

    pub(crate) fn place(
        &mut self,
        row: &[u8],
        hash: u64,
        parent: Option<usize>,
        most: usize,
    ) -> Placed {
        let at = Place {
            row,
            hash,
            stride: self.stride,
            count: self.count,
            most,
            parent,
        };
        let placed = match &mut self.ids {
            Ids::Narrow(links) => links.place(&mut self.rows, at),
            Ids::Wide(links) => links.place(&mut self.rows, at),
        };
        if matches!(placed, Placed::New(_)) {
            self.count += 1;
        }
        placed
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
            stride: self.stride,
            count: self.count,
            most,
        };
        let placed = match &mut self.ids {
            Ids::Narrow(links) => links.place_level(&mut self.rows, at),
            Ids::Wide(links) => links.place_level(&mut self.rows, at),
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
        let mut row = Vec::with_capacity(self.words);
        if self.encode_or_add(&values, &mut row) {
            let mut packed = Vec::with_capacity(self.stride);
            self.pack(&row, &mut packed);
            let hash = hash_row(&packed);
            return self.place(&packed, hash, parent, most);
        }
        if let Some(id) = self.wide_ids.get(&values) {
            return Placed::Known(*id);
        }
        if self.count >= most {
            return Placed::Full;
        }
        let id = self.count;
        self.rows
            .resize(self.rows.len().saturating_add(self.stride), 0);
        match &mut self.ids {
            Ids::Narrow(links) => links.push_parent(parent),
            Ids::Wide(links) => links.push_parent(parent),
        }
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
        self.rows.truncate(kept.saturating_mul(self.stride));
        match &mut self.ids {
            Ids::Narrow(links) => links.truncate(kept),
            Ids::Wide(links) => links.truncate(kept),
        }
        self.wide.retain(|id, _| *id < kept);
        self.wide_ids.retain(|_, id| *id < kept);
    }
}
