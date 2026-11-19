use super::{Set, Start};
use std::borrow::Borrow;
use std::collections::BTreeMap;
use std::fmt;

/// A map from keys to values, with at most one value for a key. As a model field it starts
/// empty.
///
/// `Debug` prints the entries in the keys' `Ord` order: `{Pen: 1, Ink: 2}`.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Map<K, V> {
    entries: BTreeMap<K, V>,
}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for Map<K, V> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_map().entries(&self.entries).finish()
    }
}

impl<K, V> Default for Map<K, V> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }
}

impl<K, V> Start for Map<K, V> {
    fn starts() -> Vec<Self> {
        vec![Self::default()]
    }
}

impl<K: Clone + Ord, V: Clone> Map<K, V> {
    /// The map of the listed pairs: `Map::of([(Item::Pen, 1)])`. When a key is listed twice
    /// its last value is kept, and `Map::of([])` is the empty map.
    pub fn of(entries: impl IntoIterator<Item = (K, V)>) -> Self {
        Self {
            entries: entries.into_iter().collect(),
        }
    }

    /// Whether the map has a value for `key`.
    pub fn has(&self, key: impl Borrow<K>) -> bool {
        self.entries.contains_key(key.borrow())
    }

    /// The value for `key`, or `None` when the map has none.
    pub fn get(&self, key: impl Borrow<K>) -> Option<V> {
        self.entries.get(key.borrow()).cloned()
    }

    /// This map with `value` for `key`, in place of any value `key` had.
    #[must_use]
    pub fn set(&self, key: K, value: V) -> Self {
        let mut entries = self.entries.clone();
        entries.insert(key, value);
        Self { entries }
    }

    /// This map without `key`. Deleting a key the map lacks gives an equal map.
    #[must_use]
    pub fn delete(&self, key: impl Borrow<K>) -> Self {
        let mut entries = self.entries.clone();
        entries.remove(key.borrow());
        Self { entries }
    }

    /// How many keys the map holds.
    pub fn size(&self) -> usize {
        self.entries.len()
    }

    /// Whether the map holds nothing.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The keys that have a value.
    pub fn keys(&self) -> Set<K> {
        Set::from_sorted(self.entries.keys().cloned().collect())
    }
}
