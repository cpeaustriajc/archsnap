use super::index::{Filed, SHARD_COUNT, Shards};
use super::rows::{Parents, ParentsTail, Rows, pack};
use hashbrown::HashTable;
use hashbrown::hash_table::Entry as Slot;
use std::sync::{Mutex, PoisonError};

const BITS_PER_WORD: usize = 64;

pub(crate) trait Spread: Sync {
    fn run(&self, tasks: usize, job: &(dyn Fn(usize) + Sync));
}

pub(crate) struct Fresh<'a> {
    pub(crate) rows: &'a [u64],
    pub(crate) hashes: &'a [u64],
    pub(crate) froms: &'a [usize],
    pub(crate) by_shard: &'a [usize],
    pub(crate) shard_starts: &'a [usize],
}

#[derive(Clone, Copy)]
pub(super) struct Level<'a> {
    pub(super) parts: &'a [Fresh<'a>],
    pub(super) spread: &'a dyn Spread,
    pub(super) words: usize,
    pub(super) count: usize,
    pub(super) most: usize,
}

impl Level<'_> {
    fn row_of(&self, part: usize, ordinal: usize) -> &[u64] {
        let start = ordinal.saturating_mul(self.words);
        self.parts
            .get(part)
            .and_then(|fresh| fresh.rows.get(start..start.saturating_add(self.words)))
            .unwrap_or(&[])
    }

    fn hash_of(&self, part: usize, ordinal: usize) -> u64 {
        self.parts
            .get(part)
            .and_then(|fresh| fresh.hashes.get(ordinal))
            .copied()
            .unwrap_or(0)
    }

    fn in_shard(&self, part: usize, shard: usize) -> &[usize] {
        self.parts
            .get(part)
            .and_then(|fresh| {
                let start = *fresh.shard_starts.get(shard)?;
                let end = *fresh.shard_starts.get(shard + 1)?;
                fresh.by_shard.get(start..end)
            })
            .unwrap_or(&[])
    }
}

type Won = Vec<Vec<usize>>;

impl<F: Filed> Shards<F> {
    pub(super) fn place_level(
        &mut self,
        rows: &mut Rows,
        parents: &mut Parents,
        at: Level<'_>,
    ) -> Option<usize> {
        let wins = if F::HOLDS_ROW {
            self.claim_level(rows, &at)
        } else {
            first_in_level(&at)
        };
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
            if F::HOLDS_ROW {
                self.unclaim_level(rows, &at, &wins);
            }
            return None;
        }
        write_level(rows, parents, &at, &maps, total);
        if !F::HOLDS_ROW {
            self.file_level(rows, &at, &wins, &maps, &bases);
        }
        Some(total)
    }

    fn claim_level(&mut self, rows: &Rows, at: &Level<'_>) -> Vec<Won> {
        let tables: Vec<Mutex<&mut HashTable<F>>> =
            self.tables.iter_mut().map(Mutex::new).collect();
        gather(at.spread, SHARD_COUNT, |shard| {
            let mut won = Vec::with_capacity(at.parts.len());
            let Some(table) = tables.get(shard) else {
                return won;
            };
            let mut table = table.lock().unwrap_or_else(PoisonError::into_inner);
            for part in 0..at.parts.len() {
                let mut mine = Vec::new();
                for ordinal in at.in_shard(part, shard) {
                    let row = at.row_of(part, *ordinal);
                    let slot = table.entry(
                        at.hash_of(part, *ordinal),
                        |filed| filed.holds(rows, row),
                        |filed| filed.hash(rows),
                    );
                    if let (Slot::Vacant(vacant), Some(filed)) = (slot, F::new(row, 0)) {
                        vacant.insert(filed);
                        mine.push(*ordinal);
                    }
                }
                won.push(mine);
            }
            won
        })
    }

    fn unclaim_level(&mut self, rows: &Rows, at: &Level<'_>, wins: &[Won]) {
        let tables: Vec<Mutex<&mut HashTable<F>>> =
            self.tables.iter_mut().map(Mutex::new).collect();
        at.spread.run(tables.len(), &|shard| {
            let (Some(table), Some(won)) = (tables.get(shard), wins.get(shard)) else {
                return;
            };
            let mut table = table.lock().unwrap_or_else(PoisonError::into_inner);
            for (part, ordinals) in won.iter().enumerate() {
                for ordinal in ordinals {
                    let row = at.row_of(part, *ordinal);
                    let hash = at.hash_of(part, *ordinal);
                    if let Ok(found) = table.find_entry(hash, |filed| filed.holds(rows, row)) {
                        found.remove();
                    }
                }
            }
        });
    }

    fn file_level(
        &mut self,
        rows: &Rows,
        at: &Level<'_>,
        wins: &[Won],
        maps: &[Winners],
        bases: &[usize],
    ) {
        let tables: Vec<Mutex<&mut HashTable<F>>> =
            self.tables.iter_mut().map(Mutex::new).collect();
        at.spread.run(tables.len(), &|shard| {
            let (Some(table), Some(won)) = (tables.get(shard), wins.get(shard)) else {
                return;
            };
            let mut table = table.lock().unwrap_or_else(PoisonError::into_inner);
            let hasher = |filed: &F| filed.hash(rows);
            table.reserve(won.iter().map(Vec::len).sum(), hasher);
            for (part, ordinals) in won.iter().enumerate() {
                let (Some(map), Some(base)) = (maps.get(part), bases.get(part)) else {
                    continue;
                };
                for ordinal in ordinals {
                    let id = at.count + base + map.rank(*ordinal);
                    let row = at.row_of(part, *ordinal);
                    if let Some(filed) = F::new(row, id) {
                        table.insert_unique(at.hash_of(part, *ordinal), filed, hasher);
                    }
                }
            }
        });
    }
}

fn first_in_level(at: &Level<'_>) -> Vec<Won> {
    gather(at.spread, SHARD_COUNT, |shard| {
        let mut seen: HashTable<(usize, usize)> = HashTable::new();
        let mut won = Vec::with_capacity(at.parts.len());
        for part in 0..at.parts.len() {
            let mut mine = Vec::new();
            for ordinal in at.in_shard(part, shard) {
                let row = at.row_of(part, *ordinal);
                if let Slot::Vacant(vacant) = seen.entry(
                    at.hash_of(part, *ordinal),
                    |(other, earlier)| at.row_of(*other, *earlier) == row,
                    |(other, earlier)| at.hash_of(*other, *earlier),
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

fn write_level(
    rows: &mut Rows,
    parents: &mut Parents,
    at: &Level<'_>,
    maps: &[Winners],
    total: usize,
) {
    let stride = rows.stride();
    let mut row_rest = rows.grow(total);
    let mut parent_rest = parents.grow(total);
    let mut slices = Vec::with_capacity(maps.len());
    for map in maps {
        let row_count = (map.count * stride).min(row_rest.len());
        let (row_part, row_after) = std::mem::take(&mut row_rest).split_at_mut(row_count);
        row_rest = row_after;
        let (parent_part, parent_after) = parent_rest.split_at(map.count);
        parent_rest = parent_after;
        slices.push(Mutex::new((row_part, parent_part)));
    }
    at.spread.run(maps.len(), &|part| {
        let (Some(map), Some(fresh), Some(slice)) =
            (maps.get(part), at.parts.get(part), slices.get(part))
        else {
            return;
        };
        let mut slice = slice.lock().unwrap_or_else(PoisonError::into_inner);
        let (row_part, parent_part): &mut (&mut [u8], ParentsTail<'_>) = &mut slice;
        for (place, won) in map.ordinals().enumerate() {
            let start = place * stride;
            if let Some(into) = row_part.get_mut(start..start + stride) {
                pack(at.row_of(part, won), stride, into);
            }
            if let Some(from) = fresh.froms.get(won) {
                parent_part.set(place, *from);
            }
        }
    });
}

#[derive(Default)]
struct Winners {
    bits: Vec<u64>,
    before: Vec<usize>,
    count: usize,
}

impl Winners {
    fn of(wins: &[Won], part: usize, found: usize) -> Self {
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
