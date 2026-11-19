use super::Start;
use std::borrow::Borrow;
use std::fmt;

/// Items in order, with repeats allowed, meant to hold at most `MAX`. As a model field it
/// starts empty.
///
/// Going past `MAX` is not an error here: the value just grows, and the checker reports the
/// state as out of range. `Debug` prints the items front to back: `[North, South]`.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Seq<T, const MAX: usize> {
    items: Vec<T>,
}

impl<T: fmt::Debug, const MAX: usize> fmt::Debug for Seq<T, MAX> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_list().entries(&self.items).finish()
    }
}

impl<T, const MAX: usize> Default for Seq<T, MAX> {
    fn default() -> Self {
        Self { items: Vec::new() }
    }
}

impl<T, const MAX: usize> Start for Seq<T, MAX> {
    fn starts() -> Vec<Self> {
        vec![Self::default()]
    }
}

impl<T: Clone + PartialEq, const MAX: usize> Seq<T, MAX> {
    /// The sequence of the listed items, in that order: `Seq::of([Stop::North, Stop::South])`.
    /// `Seq::of([])` is the empty sequence.
    pub fn of(items: impl IntoIterator<Item = T>) -> Self {
        Self {
            items: items.into_iter().collect(),
        }
    }

    /// How many items the sequence holds, repeats counted.
    pub fn size(&self) -> usize {
        self.items.len()
    }

    /// Whether the sequence holds nothing.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The item at `index`, counted from 0, or `None` when the sequence is shorter.
    pub fn get(&self, index: usize) -> Option<T> {
        self.items.get(index).cloned()
    }

    /// The item at the front, or `None` when the sequence is empty.
    pub fn first(&self) -> Option<T> {
        self.items.first().cloned()
    }

    /// The item at the back, or `None` when the sequence is empty.
    pub fn last(&self) -> Option<T> {
        self.items.last().cloned()
    }

    /// This sequence with `item` added at the back.
    #[must_use]
    pub fn push(&self, item: T) -> Self {
        let mut items = self.items.clone();
        items.push(item);
        Self { items }
    }

    /// This sequence without its front item. An empty sequence stays empty.
    #[must_use]
    pub fn shift(&self) -> Self {
        Self {
            items: self.items.iter().skip(1).cloned().collect(),
        }
    }

    /// This sequence without its back item. An empty sequence stays empty.
    #[must_use]
    pub fn pop(&self) -> Self {
        let mut items = self.items.clone();
        items.pop();
        Self { items }
    }

    /// Whether `item` is anywhere in the sequence.
    pub fn includes(&self, item: impl Borrow<T>) -> bool {
        self.items.contains(item.borrow())
    }

    /// Whether `test` is true for every item. True for the empty sequence.
    pub fn every(&self, test: impl Fn(T) -> bool) -> bool {
        self.items.iter().cloned().all(test)
    }

    /// Whether `test` is true for at least one item. False for the empty sequence.
    pub fn some(&self, test: impl Fn(T) -> bool) -> bool {
        self.items.iter().cloned().any(test)
    }

    /// The items for which `test` is true, in their order.
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
}
