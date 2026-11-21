use super::layout::{WORD_BYTES, hash_each};

#[derive(Debug)]
pub(super) struct Rows {
    bytes: Vec<u8>,
    stride: usize,
}

fn word_of(chunk: &[u8]) -> u64 {
    if let Ok(whole) = <[u8; WORD_BYTES]>::try_from(chunk) {
        return u64::from_le_bytes(whole);
    }
    let mut whole = [0; WORD_BYTES];
    if let Some(start) = whole.get_mut(..chunk.len()) {
        start.copy_from_slice(chunk);
    }
    u64::from_le_bytes(whole)
}

pub(super) fn pack(row: &[u64], stride: usize, into: &mut [u8]) {
    for (chunk, word) in into.chunks_mut(WORD_BYTES).zip(row).take(stride) {
        let bytes = word.to_le_bytes();
        chunk.copy_from_slice(bytes.get(..chunk.len()).unwrap_or(&bytes));
    }
}

impl Rows {
    pub(super) fn new(stride: usize) -> Self {
        Self {
            bytes: Vec::new(),
            stride,
        }
    }

    pub(super) fn stride(&self) -> usize {
        self.stride
    }

    fn bytes_of(&self, id: usize) -> &[u8] {
        let start = id.saturating_mul(self.stride);
        self.bytes
            .get(start..start.saturating_add(self.stride))
            .unwrap_or(&[])
    }

    pub(super) fn unpack(&self, id: usize, row: &mut Vec<u64>, words: usize) {
        row.clear();
        let bytes = self.bytes_of(id);
        if words == 1 {
            row.push(word_of(bytes));
            return;
        }
        row.extend(bytes.chunks(WORD_BYTES).map(word_of));
        row.resize(words, 0);
    }

    pub(super) fn holds(&self, id: usize, row: &[u64]) -> bool {
        let mut words = self.bytes_of(id).chunks(WORD_BYTES).map(word_of);
        row.iter().all(|word| words.next().unwrap_or(0) == *word) && words.all(|word| word == 0)
    }

    pub(super) fn hash_of(&self, id: usize) -> u64 {
        hash_each(self.bytes_of(id).chunks(WORD_BYTES).map(word_of))
    }

    pub(super) fn push(&mut self, row: &[u64]) {
        let start = self.bytes.len();
        self.bytes.resize(start.saturating_add(self.stride), 0);
        if let Some(into) = self.bytes.get_mut(start..) {
            pack(row, self.stride, into);
        }
    }

    pub(super) fn grow(&mut self, rows: usize) -> &mut [u8] {
        let start = self.bytes.len();
        self.bytes
            .resize(start.saturating_add(rows.saturating_mul(self.stride)), 0);
        self.bytes.get_mut(start..).unwrap_or_default()
    }

    pub(super) fn truncate(&mut self, kept: usize) {
        self.bytes.truncate(kept.saturating_mul(self.stride));
    }
}

#[derive(Debug)]
pub(super) enum Parents {
    Narrow(Vec<u32>),
    Wide(Vec<usize>),
}

impl Parents {
    pub(super) fn for_up_to(most: usize) -> Self {
        if u32::try_from(most).is_ok_and(|narrow| narrow < u32::MAX) {
            Self::Narrow(Vec::new())
        } else {
            Self::Wide(Vec::new())
        }
    }

    pub(super) fn get(&self, id: usize) -> Option<usize> {
        match self {
            Self::Narrow(all) => all
                .get(id)
                .filter(|parent| **parent != u32::MAX)
                .and_then(|parent| usize::try_from(*parent).ok()),
            Self::Wide(all) => all.get(id).copied().filter(|parent| *parent != usize::MAX),
        }
    }

    pub(super) fn push(&mut self, parent: Option<usize>) {
        match self {
            Self::Narrow(all) => all.push(narrow(parent)),
            Self::Wide(all) => all.push(parent.unwrap_or(usize::MAX)),
        }
    }

    pub(super) fn grow(&mut self, count: usize) -> ParentsTail<'_> {
        match self {
            Self::Narrow(all) => {
                let start = all.len();
                all.resize(start.saturating_add(count), u32::MAX);
                ParentsTail::Narrow(all.get_mut(start..).unwrap_or_default())
            }
            Self::Wide(all) => {
                let start = all.len();
                all.resize(start.saturating_add(count), usize::MAX);
                ParentsTail::Wide(all.get_mut(start..).unwrap_or_default())
            }
        }
    }

    pub(super) fn truncate(&mut self, kept: usize) {
        match self {
            Self::Narrow(all) => all.truncate(kept),
            Self::Wide(all) => all.truncate(kept),
        }
    }
}

fn narrow(parent: Option<usize>) -> u32 {
    parent
        .and_then(|id| u32::try_from(id).ok())
        .unwrap_or(u32::MAX)
}

pub(super) enum ParentsTail<'a> {
    Narrow(&'a mut [u32]),
    Wide(&'a mut [usize]),
}

impl ParentsTail<'_> {
    pub(super) fn split_at(self, count: usize) -> (Self, Self) {
        match self {
            Self::Narrow(all) => {
                let (head, tail) = all.split_at_mut(count.min(all.len()));
                (Self::Narrow(head), Self::Narrow(tail))
            }
            Self::Wide(all) => {
                let (head, tail) = all.split_at_mut(count.min(all.len()));
                (Self::Wide(head), Self::Wide(tail))
            }
        }
    }

    pub(super) fn set(&mut self, place: usize, parent: usize) {
        match self {
            Self::Narrow(all) => {
                if let Some(slot) = all.get_mut(place) {
                    *slot = narrow(Some(parent));
                }
            }
            Self::Wide(all) => {
                if let Some(slot) = all.get_mut(place) {
                    *slot = parent;
                }
            }
        }
    }
}
