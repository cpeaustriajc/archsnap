use super::Placed;
use hashbrown::HashTable;
use rustc_hash::FxHasher;
use std::hash::Hasher;

const SHARD_BITS: u32 = 6;
pub(crate) const SHARD_COUNT: usize = 1 << SHARD_BITS;
const SHARD_SHIFT: u32 = 51;
const SHARD_MASK: u64 = (1 << SHARD_BITS) - 1;

pub(super) trait Id: Copy + Eq {
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
pub(super) struct Links<T> {
    pub(super) parents: Vec<T>,
    pub(super) shards: Vec<HashTable<T>>,
}

impl<T: Id> Links<T> {
    pub(super) fn new() -> Self {
        Self {
            parents: Vec::new(),
            shards: (0..SHARD_COUNT).map(|_| HashTable::new()).collect(),
        }
    }

    pub(super) fn parent(&self, id: usize) -> Option<usize> {
        self.parents
            .get(id)
            .copied()
            .filter(|parent| *parent != T::NONE)
            .map(Id::index)
    }

    pub(super) fn push_parent(&mut self, parent: Option<usize>) {
        self.parents
            .push(parent.and_then(T::from_index).unwrap_or(T::NONE));
    }

    pub(super) fn find(&self, rows: &[u8], stride: usize, row: &[u8], hash: u64) -> Option<usize> {
        self.shards
            .get(shard_of(hash))?
            .find(hash, |id| same_row(row_at(rows, stride, id.index()), row))
            .map(|id| id.index())
    }

    pub(super) fn place(&mut self, rows: &mut Vec<u8>, at: Place<'_>) -> Placed {
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
    pub(super) fn truncate(&mut self, kept: usize) {
        self.parents.truncate(kept);
        for shard in &mut self.shards {
            shard.retain(|id| id.index() < kept);
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Place<'a> {
    pub(super) row: &'a [u8],
    pub(super) hash: u64,
    pub(super) stride: usize,
    pub(super) count: usize,
    pub(super) most: usize,
    pub(super) parent: Option<usize>,
}

pub(crate) fn same_row(one: &[u8], other: &[u8]) -> bool {
    one.len() == other.len() && one.iter().zip(other).all(|(left, right)| left == right)
}

pub(crate) fn hash_row(row: &[u8]) -> u64 {
    let mut hasher = FxHasher::default();
    hasher.write(row);
    hasher.finish()
}

pub(crate) fn shard_of(hash: u64) -> usize {
    usize::try_from(hash.checked_shr(SHARD_SHIFT).unwrap_or(0) & SHARD_MASK).unwrap_or(0)
}

pub(crate) fn row_at(rows: &[u8], stride: usize, id: usize) -> &[u8] {
    let start = id.saturating_mul(stride);
    rows.get(start..start.saturating_add(stride)).unwrap_or(&[])
}
