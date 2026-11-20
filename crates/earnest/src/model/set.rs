use super::Start;
use super::walk::{Walk, Walked};
use std::borrow::Borrow;
use std::collections::BTreeSet;
use std::fmt;

/// A set of distinct items. As a model field it starts empty.
///
/// Two sets with the same items are equal, whatever order the items were added in. The items
/// are walked, and `Debug` prints them, in the order of [`Walk`]: `{Ann, Bob}`. That is the
/// order of the names of an enum's variants, not the order they are declared in.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Set<T: Walk> {
    items: BTreeSet<Walked<T>>,
}

impl<T: Walk + fmt::Debug> fmt::Debug for Set<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_set()
            .entries(self.items.iter().map(|item| &item.0))
            .finish()
    }
}

impl<T: Walk> Default for Set<T> {
    fn default() -> Self {
        Self {
            items: BTreeSet::new(),
        }
    }
}

impl<T: Walk> Start for Set<T> {
    fn starts() -> Vec<Self> {
        vec![Self::default()]
    }
}

impl<T: Clone + Walk> Set<T> {
    /// The set of the listed items: `Set::of([Guest::Ann, Guest::Bob])`. An item listed twice
    /// is held once, and `Set::of([])` is the empty set.
    pub fn of(items: impl IntoIterator<Item = T>) -> Self {
        Self {
            items: items.into_iter().map(Walked).collect(),
        }
    }

    /// Whether `item` is in the set.
    pub fn has(&self, item: impl Borrow<T>) -> bool {
        self.items.contains(&Walked(item.borrow().clone()))
    }

    /// This set with `item` in it. Adding an item the set already has gives an equal set.
    #[must_use]
    pub fn add(&self, item: T) -> Self {
        let mut items = self.items.clone();
        items.insert(Walked(item));
        Self { items }
    }

    /// This set without `item`. Deleting an item the set lacks gives an equal set.
    #[must_use]
    pub fn delete(&self, item: impl Borrow<T>) -> Self {
        let mut items = self.items.clone();
        items.remove(&Walked(item.borrow().clone()));
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
        self.items.iter().map(|item| item.0.clone()).all(test)
    }

    /// Whether `test` is true for at least one item. False for the empty set.
    pub fn some(&self, test: impl Fn(T) -> bool) -> bool {
        self.items.iter().map(|item| item.0.clone()).any(test)
    }

    /// The items for which `test` is true.
    #[must_use]
    pub fn filter(&self, test: impl Fn(T) -> bool) -> Self {
        Self {
            items: self
                .items
                .iter()
                .filter(|item| test(T::clone(&item.0)))
                .cloned()
                .collect(),
        }
    }

    pub(super) fn from_sorted(items: BTreeSet<Walked<T>>) -> Self {
        Self { items }
    }

    pub(super) fn walked(&self) -> impl ExactSizeIterator<Item = &T> {
        self.items.iter().map(|item| &item.0)
    }
}
