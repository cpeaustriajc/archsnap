use super::Start;
use std::borrow::Borrow;
use std::collections::BTreeSet;
use std::fmt;

/// A set of distinct items. As a model field it starts empty.
///
/// Two sets with the same items are equal, whatever order the items were added in. `Debug`
/// prints the items in their `Ord` order: `{Ann, Bob}`.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Set<T> {
    items: BTreeSet<T>,
}

impl<T: fmt::Debug> fmt::Debug for Set<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_set().entries(&self.items).finish()
    }
}

impl<T> Default for Set<T> {
    fn default() -> Self {
        Self {
            items: BTreeSet::new(),
        }
    }
}

impl<T> Start for Set<T> {
    fn starts() -> Vec<Self> {
        vec![Self::default()]
    }
}

impl<T: Clone + Ord> Set<T> {
    /// The set of the listed items: `Set::of([Guest::Ann, Guest::Bob])`. An item listed twice
    /// is held once, and `Set::of([])` is the empty set.
    pub fn of(items: impl IntoIterator<Item = T>) -> Self {
        Self {
            items: items.into_iter().collect(),
        }
    }

    /// Whether `item` is in the set.
    pub fn has(&self, item: impl Borrow<T>) -> bool {
        self.items.contains(item.borrow())
    }

    /// This set with `item` in it. Adding an item the set already has gives an equal set.
    #[must_use]
    pub fn add(&self, item: T) -> Self {
        let mut items = self.items.clone();
        items.insert(item);
        Self { items }
    }

    /// This set without `item`. Deleting an item the set lacks gives an equal set.
    #[must_use]
    pub fn delete(&self, item: impl Borrow<T>) -> Self {
        let mut items = self.items.clone();
        items.remove(item.borrow());
        Self { items }
    }

    /// The items that are in this set, in `other`, or in both.
    #[must_use]
    pub fn union(&self, other: impl Borrow<Self>) -> Self {
        Self {
            items: self.items.union(&other.borrow().items).cloned().collect(),
        }
    }

    /// The items that are in both this set and `other`.
    #[must_use]
    pub fn intersect(&self, other: impl Borrow<Self>) -> Self {
        Self {
            items: self
                .items
                .intersection(&other.borrow().items)
                .cloned()
                .collect(),
        }
    }

    /// The items of this set that are not in `other`.
    #[must_use]
    pub fn subtract(&self, other: impl Borrow<Self>) -> Self {
        Self {
            items: self
                .items
                .difference(&other.borrow().items)
                .cloned()
                .collect(),
        }
    }

    /// Whether every item of this set is also in `other`. The empty set is a subset of every
    /// set, and a set is a subset of itself.
    pub fn is_subset(&self, other: impl Borrow<Self>) -> bool {
        self.items.is_subset(&other.borrow().items)
    }

    /// How many items the set holds.
    pub fn size(&self) -> usize {
        self.items.len()
    }

    /// Whether the set holds nothing.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Whether `test` is true for every item. True for the empty set.
    pub fn every(&self, test: impl Fn(T) -> bool) -> bool {
        self.items.iter().cloned().all(test)
    }

    /// Whether `test` is true for at least one item. False for the empty set.
    pub fn some(&self, test: impl Fn(T) -> bool) -> bool {
        self.items.iter().cloned().any(test)
    }

    /// The items for which `test` is true.
    #[must_use]
    pub fn filter(&self, test: impl Fn(T) -> bool) -> Self {
        Self {
            items: self
                .items
                .iter()
                .filter(|item| test(T::clone(item)))
                .cloned()
                .collect(),
        }
    }

    pub(super) fn from_sorted(items: BTreeSet<T>) -> Self {
        Self { items }
    }
}
