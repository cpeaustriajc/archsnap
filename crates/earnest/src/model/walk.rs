use super::{Map, Queue, Request, Retry, Seq, Set, Timer, Unowned};
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

/// The order in which the items of a [`Set`] and the keys of a [`Map`] are walked, which is
/// the order the checker and the TypeScript library use for the same items.
///
/// The order is the same in every language: `None` first, `false` before `true`, numbers
/// upward, texts by code point (the order of their UTF-8 bytes), an enum's values by the
/// name of the variant, a record's fields one after the other in the order of their names, and
/// a collection item by item with the shorter one first when one is the start of the other.
/// It is not the order of `Ord`, which puts an enum's variants in source order and a record's
/// fields in the order they were declared.
///
/// `#[derive(Domain)]` implements it for an enum and for a struct, and the library has it for
/// `bool`, the integers, `&str`, `Option` and every collection and part. A hand-written
/// implementation must give a total order that is `Equal` exactly for equal values.
pub trait Walk: Eq {
    /// Which of `self` and `other` is walked first.
    fn walk_cmp(&self, other: &Self) -> Ordering;
}

#[derive(Clone, Debug)]
pub(super) struct Walked<T>(pub(super) T);

impl<T: Walk> PartialEq for Walked<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T: Walk> Eq for Walked<T> {}

impl<T: Walk> PartialOrd for Walked<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Walk> Ord for Walked<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.walk_cmp(&other.0)
    }
}

impl<T: Hash> Hash for Walked<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

macro_rules! walk_by_ord {
    ($($ty:ty),* $(,)?) => {
        $(
            impl Walk for $ty {
                fn walk_cmp(&self, other: &Self) -> Ordering {
                    Ord::cmp(self, other)
                }
            }
        )*
    };
}

walk_by_ord!(
    bool, u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, Unowned,
);

impl Walk for &str {
    fn walk_cmp(&self, other: &Self) -> Ordering {
        str::cmp(self, other)
    }
}

impl<T: Walk> Walk for Option<T> {
    fn walk_cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(left), Some(right)) => left.walk_cmp(right),
        }
    }
}

pub(super) fn walk_items<'a, T: Walk + 'a>(
    left: impl ExactSizeIterator<Item = &'a T>,
    right: impl ExactSizeIterator<Item = &'a T>,
) -> Ordering {
    let lengths = left.len().cmp(&right.len());
    left.zip(right)
        .map(|(first, second)| first.walk_cmp(second))
        .find(|order| order.is_ne())
        .unwrap_or(lengths)
}

impl<T: Clone + Walk> Walk for Set<T> {
    fn walk_cmp(&self, other: &Self) -> Ordering {
        walk_items(self.walked(), other.walked())
    }
}

impl<K: Clone + Walk, V: Clone + Walk> Walk for Map<K, V> {
    fn walk_cmp(&self, other: &Self) -> Ordering {
        let lengths = self.size().cmp(&other.size());
        self.entries_walked()
            .zip(other.entries_walked())
            .map(|((left_key, left), (right_key, right))| {
                left_key.walk_cmp(right_key).then(left.walk_cmp(right))
            })
            .find(|order| order.is_ne())
            .unwrap_or(lengths)
    }
}

impl<T: Walk, const MAX: usize> Walk for Seq<T, MAX> {
    fn walk_cmp(&self, other: &Self) -> Ordering {
        walk_items(self.walked(), other.walked())
    }
}

impl<T: Walk, const CAPACITY: usize> Walk for Queue<T, CAPACITY> {
    fn walk_cmp(&self, other: &Self) -> Ordering {
        walk_items(self.walked(), other.walked())
    }
}

impl Walk for Timer {
    fn walk_cmp(&self, other: &Self) -> Ordering {
        self.stage_name().cmp(other.stage_name())
    }
}

impl Walk for Request {
    fn walk_cmp(&self, other: &Self) -> Ordering {
        self.stage_name().cmp(other.stage_name())
    }
}

impl<const MAX: usize> Walk for Retry<MAX> {
    fn walk_cmp(&self, other: &Self) -> Ordering {
        self.attempts().cmp(&other.attempts())
    }
}
