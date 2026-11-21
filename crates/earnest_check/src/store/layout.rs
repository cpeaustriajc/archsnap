use super::codec::Codec;
use earnest_ir::Field;
use earnest_value::Domain;

pub(super) const WORD_BITS: u32 = u64::BITS;
pub(super) const WORD_BYTES: usize = 8;
const SEAT_SPREAD: u64 = 0x9E37_79B9_7F4A_7C15;
const MIX: u64 = 0xD6E8_FEB8_6659_FD93;
const HALF_WORD: u32 = 32;

/// Where each field of a model sits in the packed row of a state, and the hash of a row.
///
/// A row is a few `u64` words. Every field takes a run of bits in one word; a field never
/// spans two words, and bits no field uses are 0. Two states are the same exactly when their
/// rows are equal, so a caller that changes a row in place and hashes it with [`Self::hash`]
/// or [`Self::hash_with`] gets the hash the search files the state under.
#[derive(Clone, Debug)]
pub struct RowLayout {
    places: Vec<Place>,
    numbers: Vec<Option<NumberSlot>>,
    words: usize,
    stride: usize,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Place {
    pub(super) word: usize,
    pub(super) shift: u32,
    pub(super) mask: u64,
}

/// The place of a whole-number field in a packed row: bits `shift .. shift + width` of word
/// `word` hold the value minus `offset`.
///
/// Only values from the field's domain have a place; [`Self::code`] says which.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumberSlot {
    /// Which word of the row holds the field, counted from 0. The field never reaches into the
    /// next word.
    pub word: usize,
    /// How many bits lie below the field in that word: shift the word right by this to read
    /// the field.
    pub shift: u32,
    /// How many bits the field takes. A field whose domain has one value takes none.
    pub width: u32,
    /// The lowest value of the domain, which is stored as 0.
    pub offset: i64,
    highest: i64,
}

impl NumberSlot {
    /// The bits that stand for `value` before they are shifted into place, or `None` when
    /// `value` is outside the field's domain and so cannot be written into the row.
    #[must_use]
    pub fn code(&self, value: i64) -> Option<u64> {
        (self.offset <= value && value <= self.highest).then(|| value.abs_diff(self.offset))
    }

    /// The value of the field in `row`. A row too short for this layout gives the lowest value
    /// of the domain.
    #[must_use]
    pub fn read(&self, row: &[u64]) -> i64 {
        let code = row.get(self.word).map_or(0, |word| {
            word.checked_shr(self.shift).unwrap_or(0) & mask_of(self.width)
        });
        self.offset.saturating_add_unsigned(code)
    }

    /// Writes `value` into `row` and says whether it could: a value outside the domain, or
    /// a row too short for this layout, leaves `row` as it was and gives `false`.
    pub fn write(&self, row: &mut [u64], value: i64) -> bool {
        let (Some(code), Some(word)) = (self.code(value), row.get_mut(self.word)) else {
            return false;
        };
        let placed = mask_of(self.width).checked_shl(self.shift).unwrap_or(0);
        *word = (*word & !placed) | (code.checked_shl(self.shift).unwrap_or(0) & placed);
        true
    }
}

pub(super) fn mask_of(bits: u32) -> u64 {
    if bits >= WORD_BITS {
        u64::MAX
    } else {
        (1_u64 << bits) - 1
    }
}

fn mix(word: u64, at: usize) -> u64 {
    let seat = u64::try_from(at).unwrap_or(0).wrapping_mul(SEAT_SPREAD);
    let mut mixed = word ^ seat;
    mixed ^= mixed >> HALF_WORD;
    mixed = mixed.wrapping_mul(MIX);
    mixed ^= mixed >> HALF_WORD;
    mixed = mixed.wrapping_mul(MIX);
    mixed ^ (mixed >> HALF_WORD)
}

pub(super) fn hash_words(row: &[u64]) -> u64 {
    hash_each(row.iter().copied())
}

pub(super) fn hash_each(words: impl Iterator<Item = u64>) -> u64 {
    words
        .enumerate()
        .fold(0, |hash, (at, word)| hash ^ mix(word, at))
}

impl RowLayout {
    /// The layout the search uses for a model with `fields`, in the order of the fields.
    #[must_use]
    pub fn new(fields: &[Field]) -> Self {
        let domains: Vec<&Domain> = fields.iter().map(|field| &field.domain).collect();
        let widths: Vec<u32> = domains
            .iter()
            .map(|domain| Codec::kind_of(domain).1)
            .collect();
        Self::of(&domains, &widths)
    }

    pub(super) fn of(domains: &[&Domain], widths: &[u32]) -> Self {
        let mut places = Vec::with_capacity(widths.len());
        let mut numbers = Vec::with_capacity(widths.len());
        let mut word = 0;
        let mut used = 0;
        for (domain, width) in domains.iter().zip(widths) {
            if used > 0 && used + width > WORD_BITS {
                word += 1;
                used = 0;
            }
            places.push(Place {
                word,
                shift: used,
                mask: mask_of(*width),
            });
            numbers.push(match domain {
                Domain::Int { lo, hi } => Some(NumberSlot {
                    word,
                    shift: used,
                    width: *width,
                    offset: *lo,
                    highest: *hi,
                }),
                _ => None,
            });
            used += width;
        }
        let words = if used > 0 { word + 1 } else { word };
        let stride =
            word * WORD_BYTES + usize::try_from(used.div_ceil(u8::BITS)).unwrap_or(WORD_BYTES);
        Self {
            places,
            numbers,
            words,
            stride,
        }
    }

    /// The length of every row of this layout, in `u64` words: the length to give a buffer
    /// that holds one row.
    #[must_use]
    pub fn words(&self) -> usize {
        self.words
    }

    /// The place of the field at `position` when it is a whole number, `None` for a field of
    /// another kind and for a position past the last field.
    #[must_use]
    pub fn number(&self, position: usize) -> Option<NumberSlot> {
        self.numbers.get(position).copied().flatten()
    }

    /// The hash the search files the state with this `row` under. Give it all
    /// [`Self::words`] words of the row: the hash depends on every word and on its position.
    #[must_use]
    pub fn hash(&self, row: &[u64]) -> u64 {
        hash_words(row)
    }

    /// The hash `row` would have with the field in `slot` set to `value`, given `hash`, the
    /// hash of `row` as it is. It costs the same however many words the row has. `None` when
    /// `value` is outside the field's domain.
    #[must_use]
    pub fn hash_with(&self, row: &[u64], hash: u64, slot: NumberSlot, value: i64) -> Option<u64> {
        let old = *row.get(slot.word)?;
        let mut changed = [old];
        let alone = NumberSlot { word: 0, ..slot };
        alone.write(&mut changed, value).then(|| {
            let [new] = changed;
            hash ^ mix(old, slot.word) ^ mix(new, slot.word)
        })
    }

    pub(super) fn place(&self, position: usize) -> Option<Place> {
        self.places.get(position).copied()
    }

    pub(super) fn stride(&self) -> usize {
        self.stride
    }
}

impl Place {
    pub(super) fn read(self, row: &[u64]) -> u64 {
        row.get(self.word).map_or(0, |word| {
            word.checked_shr(self.shift).unwrap_or(0) & self.mask
        })
    }

    pub(super) fn write(self, row: &mut [u64], code: u64) {
        if let Some(word) = row.get_mut(self.word) {
            let placed = self.mask.checked_shl(self.shift).unwrap_or(0);
            *word = (*word & !placed) | (code.checked_shl(self.shift).unwrap_or(0) & placed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RowLayout;
    use earnest_value::Domain;

    fn layout(domains: &[Domain]) -> RowLayout {
        let widths: Vec<u32> = domains
            .iter()
            .map(|domain| super::Codec::kind_of(domain).1)
            .collect();
        RowLayout::of(&domains.iter().collect::<Vec<_>>(), &widths)
    }

    #[test]
    fn a_number_field_is_written_as_its_distance_from_the_lowest_value() {
        let digits = layout(&[Domain::Int { lo: 0, hi: 9 }, Domain::Int { lo: -3, hi: 3 }]);
        let second = digits.number(1).expect("a number field");
        assert_eq!(
            (second.word, second.shift, second.width),
            (0, 4, 3),
            "after the first"
        );
        let mut row = vec![0; digits.words()];
        assert!(second.write(&mut row, -1), "a value of the domain");
        assert_eq!(row, [2 << 4], "two above the lowest value");
        assert_eq!(second.read(&row), -1, "and read back");
        assert!(!second.write(&mut row, 4), "a value outside the domain");
        assert_eq!(row, [2 << 4], "is not written");
    }

    #[test]
    fn a_field_that_does_not_fit_in_the_word_starts_the_next_one() {
        let wide = layout(&[
            Domain::Int { lo: 0, hi: 1 << 40 },
            Domain::Int { lo: 0, hi: 1 << 40 },
        ]);
        let second = wide.number(1).expect("a number field");
        assert_eq!(
            (second.word, second.shift),
            (1, 0),
            "the start of the second word"
        );
        assert_eq!(wide.words(), 2, "two words");
    }

    #[test]
    fn the_hash_with_one_field_changed_is_the_hash_of_the_changed_row() {
        let wide = layout(&[
            Domain::Int { lo: 0, hi: 1 << 40 },
            Domain::Int { lo: 0, hi: 1 << 40 },
            Domain::Int { lo: 5, hi: 9 },
        ]);
        let mut row = vec![0; wide.words()];
        for (position, value) in [(0, 17), (1, 1 << 33), (2, 7)] {
            let slot = wide.number(position).expect("a number field");
            assert!(slot.write(&mut row, value), "{value} fits");
        }
        let hash = wide.hash(&row);
        for (position, value) in [(0, 18), (1, 3), (2, 9), (2, 7)] {
            let slot = wide.number(position).expect("a number field");
            let mut changed = row.clone();
            assert!(slot.write(&mut changed, value), "{value} fits");
            assert_eq!(
                wide.hash_with(&row, hash, slot, value),
                Some(wide.hash(&changed)),
                "field {position} set to {value}"
            );
        }
        let last = wide.number(2).expect("a number field");
        assert_eq!(
            wide.hash_with(&row, hash, last, 10),
            None,
            "outside the domain"
        );
    }

    #[test]
    fn rows_that_differ_in_one_field_hash_apart() {
        let digits = layout(&[Domain::Int { lo: 0, hi: 9 }, Domain::Int { lo: 0, hi: 9 }]);
        let hashes: std::collections::HashSet<u64> =
            (0..100_u64).map(|row| digits.hash(&[row])).collect();
        assert_eq!(hashes.len(), 100, "every row its own hash");
    }
}
