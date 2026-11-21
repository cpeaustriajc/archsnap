use super::layout::hash_words;
use super::rows::Rows;
use hashbrown::HashTable;
use hashbrown::hash_table::Entry as Slot;

const SHARD_BITS: u32 = 6;
pub(crate) const SHARD_COUNT: usize = 1 << SHARD_BITS;
const SHARD_SHIFT: u32 = 51;
const SHARD_MASK: u64 = (1 << SHARD_BITS) - 1;
const NARROW_KEY_BYTES: usize = 4;
const KEY_BYTES: usize = 8;

pub(crate) fn shard_of(hash: u64) -> usize {
    usize::try_from(hash.checked_shr(SHARD_SHIFT).unwrap_or(0) & SHARD_MASK).unwrap_or(0)
}

pub(super) trait Filed: Copy + Send + Sync {
    const HOLDS_ROW: bool;
    fn new(row: &[u64], id: usize) -> Option<Self>;
    fn holds(self, rows: &Rows, row: &[u64]) -> bool;
    fn hash(self, rows: &Rows) -> u64;
    fn id(self) -> Option<usize>;
}

#[derive(Clone, Copy, Debug)]
pub(super) struct NarrowKey(u32);

#[derive(Clone, Copy, Debug)]
pub(super) struct Key(u64);

#[derive(Clone, Copy, Debug)]
pub(super) struct NarrowId(u32);

#[derive(Clone, Copy, Debug)]
pub(super) struct WideId(usize);

impl Filed for NarrowKey {
    const HOLDS_ROW: bool = true;

    fn new(row: &[u64], _: usize) -> Option<Self> {
        u32::try_from(row.first().copied().unwrap_or(0))
            .ok()
            .map(Self)
    }

    fn holds(self, _: &Rows, row: &[u64]) -> bool {
        u64::from(self.0) == row.first().copied().unwrap_or(0)
    }

    fn hash(self, _: &Rows) -> u64 {
        hash_words(&[u64::from(self.0)])
    }

    fn id(self) -> Option<usize> {
        None
    }
}

impl Filed for Key {
    const HOLDS_ROW: bool = true;

    fn new(row: &[u64], _: usize) -> Option<Self> {
        Some(Self(row.first().copied().unwrap_or(0)))
    }

    fn holds(self, _: &Rows, row: &[u64]) -> bool {
        self.0 == row.first().copied().unwrap_or(0)
    }

    fn hash(self, _: &Rows) -> u64 {
        hash_words(&[self.0])
    }

    fn id(self) -> Option<usize> {
        None
    }
}

impl Filed for NarrowId {
    const HOLDS_ROW: bool = false;

    fn new(_: &[u64], id: usize) -> Option<Self> {
        u32::try_from(id)
            .ok()
            .filter(|narrow| *narrow < u32::MAX)
            .map(Self)
    }

    fn holds(self, rows: &Rows, row: &[u64]) -> bool {
        usize::try_from(self.0).is_ok_and(|id| rows.holds(id, row))
    }

    fn hash(self, rows: &Rows) -> u64 {
        usize::try_from(self.0).map_or(0, |id| rows.hash_of(id))
    }

    fn id(self) -> Option<usize> {
        usize::try_from(self.0).ok()
    }
}

impl Filed for WideId {
    const HOLDS_ROW: bool = false;

    fn new(_: &[u64], id: usize) -> Option<Self> {
        Some(Self(id))
    }

    fn holds(self, rows: &Rows, row: &[u64]) -> bool {
        rows.holds(self.0, row)
    }

    fn hash(self, rows: &Rows) -> u64 {
        rows.hash_of(self.0)
    }

    fn id(self) -> Option<usize> {
        Some(self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lookup {
    Id,
    Presence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Found {
    pub(crate) id: Option<usize>,
}

pub(super) enum Claim {
    Known(Found),
    New,
    Full,
}

#[derive(Debug)]
pub(super) struct Shards<F> {
    pub(super) tables: Vec<HashTable<F>>,
}

impl<F: Filed> Shards<F> {
    fn new() -> Self {
        Self {
            tables: (0..SHARD_COUNT).map(|_| HashTable::new()).collect(),
        }
    }

    fn find(&self, rows: &Rows, row: &[u64], hash: u64) -> Option<Found> {
        self.tables
            .get(shard_of(hash))?
            .find(hash, |filed| filed.holds(rows, row))
            .map(|filed| Found { id: filed.id() })
    }

    fn claim(&mut self, rows: &Rows, row: &[u64], hash: u64, id: Option<usize>) -> Claim {
        let Some(table) = self.tables.get_mut(shard_of(hash)) else {
            return Claim::Full;
        };
        match table.entry(
            hash,
            |filed| filed.holds(rows, row),
            |filed| filed.hash(rows),
        ) {
            Slot::Occupied(found) => Claim::Known(Found {
                id: found.get().id(),
            }),
            Slot::Vacant(vacant) => match id.and_then(|id| F::new(row, id)) {
                Some(filed) => {
                    vacant.insert(filed);
                    Claim::New
                }
                None => Claim::Full,
            },
        }
    }

    fn forget(&mut self, rows: &Rows, kept: usize, filed: &[usize], words: usize) {
        if !F::HOLDS_ROW {
            for table in &mut self.tables {
                table.retain(|filed| filed.id().is_none_or(|id| id < kept));
            }
            return;
        }
        let mut row = Vec::with_capacity(words);
        for id in filed {
            rows.unpack(*id, &mut row, words);
            let hash = hash_words(&row);
            if let Some(table) = self.tables.get_mut(shard_of(hash))
                && let Ok(found) = table.find_entry(hash, |filed| filed.holds(rows, &row))
            {
                found.remove();
            }
        }
    }
}

#[derive(Debug)]
pub(super) enum Index {
    NarrowKeys(Shards<NarrowKey>),
    Keys(Shards<Key>),
    NarrowIds(Shards<NarrowId>),
    WideIds(Shards<WideId>),
}

impl Index {
    pub(super) fn new(stride: usize, most: usize, lookup: Lookup) -> Self {
        let presence = lookup == Lookup::Presence;
        if presence && stride <= NARROW_KEY_BYTES {
            Self::NarrowKeys(Shards::new())
        } else if presence && stride <= KEY_BYTES {
            Self::Keys(Shards::new())
        } else if u32::try_from(most).is_ok_and(|narrow| narrow < u32::MAX) {
            Self::NarrowIds(Shards::new())
        } else {
            Self::WideIds(Shards::new())
        }
    }

    pub(super) fn find(&self, rows: &Rows, row: &[u64], hash: u64) -> Option<Found> {
        match self {
            Self::NarrowKeys(shards) => shards.find(rows, row, hash),
            Self::Keys(shards) => shards.find(rows, row, hash),
            Self::NarrowIds(shards) => shards.find(rows, row, hash),
            Self::WideIds(shards) => shards.find(rows, row, hash),
        }
    }

    pub(super) fn claim(
        &mut self,
        rows: &Rows,
        row: &[u64],
        hash: u64,
        id: Option<usize>,
    ) -> Claim {
        match self {
            Self::NarrowKeys(shards) => shards.claim(rows, row, hash, id),
            Self::Keys(shards) => shards.claim(rows, row, hash, id),
            Self::NarrowIds(shards) => shards.claim(rows, row, hash, id),
            Self::WideIds(shards) => shards.claim(rows, row, hash, id),
        }
    }

    pub(super) fn forget(&mut self, rows: &Rows, kept: usize, filed: &[usize], words: usize) {
        match self {
            Self::NarrowKeys(shards) => shards.forget(rows, kept, filed, words),
            Self::Keys(shards) => shards.forget(rows, kept, filed, words),
            Self::NarrowIds(shards) => shards.forget(rows, kept, filed, words),
            Self::WideIds(shards) => shards.forget(rows, kept, filed, words),
        }
    }
}
