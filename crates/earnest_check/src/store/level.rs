use super::index::{Id, Links, SHARD_COUNT, hash_row, row_at, same_row};
use hashbrown::HashTable;
use std::sync::{Mutex, PoisonError};

impl<T: Id> Links<T> {
    pub(super) fn place_level(&mut self, rows: &mut Vec<u8>, at: Level<'_>) -> Option<usize>
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
pub(super) struct Level<'a> {
    pub(super) parts: &'a [Fresh<'a>],
    pub(super) spread: &'a dyn Spread,
    pub(super) stride: usize,
    pub(super) count: usize,
    pub(super) most: usize,
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
