mod codec;
mod index;
mod level;

use codec::{Codec, Kind, mask_of};
use earnest_ir::Field;
use earnest_value::Value;
use index::{Id, Links, Place};
pub(crate) use index::{SHARD_COUNT, hash_row, row_at, same_row, shard_of};
use level::Level;
pub(crate) use level::{Fresh, Spread};
use rustc_hash::FxHashMap;

const WORD_BITS: u32 = u64::BITS;
const WORD_BYTES: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Placed {
    Known(usize),
    New(usize),
    Full,
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

fn word_of(chunk: &[u8]) -> u64 {
    if let Ok(whole) = <[u8; WORD_BYTES]>::try_from(chunk) {
        return u64::from_le_bytes(whole);
    }
    chunk
        .iter()
        .rev()
        .fold(0, |word, byte| (word << u8::BITS) | u64::from(*byte))
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
