use crate::value::{Sym, Value};
use std::collections::{BTreeMap, BTreeSet};

/// The finite set of values a field or an input declares.
///
/// A value outside its field's domain is what the checker reports as out of range.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Domain {
    /// True and false.
    Bool,
    /// The whole numbers from `lo` to `hi`, both included. Empty when `lo` is above `hi`.
    Int {
        /// The smallest number.
        lo: i64,
        /// The largest number.
        hi: i64,
    },
    /// Exactly the listed values. List each value once, or [`Domain::count`] counts it twice.
    OneOf(Vec<Value>),
    /// `null` and every value of the inner domain. The inner domain must not hold `null` itself.
    Optional(Box<Self>),
    /// Records with exactly these fields, listed in the order they were declared in. That order
    /// is the order [`Domain::values`] walks them in, the first field changing slowest.
    Record(Vec<(Sym, Self)>),
    /// Every set of items from the inner domain, the empty set included.
    Set(Box<Self>),
    /// Every map from some keys of the first domain to values of the second, the empty map
    /// included.
    Map(Box<Self>, Box<Self>),
    /// Every sequence of at most `max` items. A queue's domain is this, with its capacity as
    /// `max`.
    Seq {
        /// The domain of each item.
        item: Box<Self>,
        /// The most items a sequence holds.
        max: usize,
    },
}

/// The most values the checker lists for one declaration. Compare [`Domain::count`] with it
/// before calling [`Domain::values`].
pub const MOST_VALUES_LISTED: u128 = 1_000_000;

impl Domain {
    /// How many values the domain holds, without listing them. A count too large for `u128` is
    /// given as `u128::MAX`.
    #[must_use]
    pub fn count(&self) -> u128 {
        match self {
            Self::Bool => 2,
            Self::Int { lo, hi } if lo > hi => 0,
            Self::Int { lo, hi } => u128::from(hi.abs_diff(*lo)).saturating_add(1),
            Self::OneOf(values) => u128::try_from(values.len()).unwrap_or(u128::MAX),
            Self::Optional(inner) => inner.count().saturating_add(1),
            Self::Record(fields) => fields.iter().fold(1_u128, |total, (_, inner)| {
                total.saturating_mul(inner.count())
            }),
            Self::Set(item) => power(2, item.count()),
            Self::Map(key, value) => power(value.count().saturating_add(1), key.count()),
            Self::Seq { item, max } => seq_count(item.count(), *max),
        }
    }

    /// Every value of the domain, each once, in the same order on every call.
    ///
    /// The list is as long as [`Domain::count`] says, so check the count first: a wide number
    /// range or a set of many items does not fit in memory.
    #[must_use]
    pub fn values(&self) -> Vec<Value> {
        match self {
            Self::Bool => vec![Value::Bool(false), Value::Bool(true)],
            Self::Int { lo, hi } => (*lo..=*hi).map(Value::Int).collect(),
            Self::OneOf(values) => values.clone(),
            Self::Optional(inner) => {
                let mut all = vec![Value::Null];
                all.extend(inner.values());
                all
            }
            Self::Record(fields) => record_values(fields),
            Self::Set(item) => set_values(&item.values()),
            Self::Map(key, value) => map_values(&key.values(), &value.values()),
            Self::Seq { item, max } => seq_values(&item.values(), *max),
        }
    }

    /// Whether `value` is one of the domain's values.
    #[must_use]
    pub fn contains(&self, value: &Value) -> bool {
        match (self, value) {
            (Self::Bool, Value::Bool(_)) | (Self::Optional(_), Value::Null) => true,
            (Self::Int { lo, hi }, Value::Int(number)) => lo <= number && number <= hi,
            (Self::OneOf(values), found) => values.contains(found),
            (Self::Optional(inner), found) => inner.contains(found),
            (Self::Record(fields), Value::Record(found)) => {
                fields.len() == found.len() && fields.iter().all(|field| field_fits(field, found))
            }
            (Self::Set(item), Value::Set(found)) => found.iter().all(|each| item.contains(each)),
            (Self::Map(key, inner), Value::Map(found)) => {
                found.iter().all(|(found_key, found_value)| {
                    key.contains(found_key) && inner.contains(found_value)
                })
            }
            (Self::Seq { item, max }, Value::Seq(found)) => {
                found.len() <= *max && found.iter().all(|each| item.contains(each))
            }
            (
                Self::Bool
                | Self::Int { .. }
                | Self::Record(_)
                | Self::Set(_)
                | Self::Map(_, _)
                | Self::Seq { .. },
                Value::Null
                | Value::Bool(_)
                | Value::Int(_)
                | Value::Str(_)
                | Value::Record(_)
                | Value::Set(_)
                | Value::Map(_)
                | Value::Seq(_),
            ) => false,
        }
    }
}

fn field_fits((name, inner): &(Sym, Domain), found: &[(Sym, Value)]) -> bool {
    found
        .iter()
        .find(|(found_name, _)| found_name == name)
        .is_some_and(|(_, found_value)| inner.contains(found_value))
}

fn seq_count(each: u128, max: usize) -> u128 {
    match each {
        0 => return 1,
        1 => return u128::try_from(max).map_or(u128::MAX, |most| most.saturating_add(1)),
        _ => {}
    }
    let mut total = 0_u128;
    let mut of_length = 1_u128;
    for _ in 0..=max {
        if total == u128::MAX {
            break;
        }
        total = total.saturating_add(of_length);
        of_length = of_length.saturating_mul(each);
    }
    total
}

fn power(base: u128, exponent: u128) -> u128 {
    let mut total = 1_u128;
    let mut square = base;
    let mut left = exponent;
    while left > 0 {
        if left % 2 == 1 {
            total = total.saturating_mul(square);
        }
        left /= 2;
        if left > 0 {
            square = square.saturating_mul(square);
        }
    }
    total
}

fn extended<T: Clone>(prefix: &[T], last: T) -> Vec<T> {
    let mut longer = prefix.to_vec();
    longer.push(last);
    longer
}

fn product<T: Clone>(prefixes: &[Vec<T>], options: &[T]) -> Vec<Vec<T>> {
    prefixes
        .iter()
        .flat_map(|prefix| {
            options
                .iter()
                .map(|option| extended(prefix, option.clone()))
        })
        .collect()
}

fn record_values(fields: &[(Sym, Domain)]) -> Vec<Value> {
    let mut partial: Vec<Vec<(Sym, Value)>> = vec![Vec::new()];
    for (name, inner) in fields {
        let options: Vec<(Sym, Value)> = inner
            .values()
            .into_iter()
            .map(|value| (*name, value))
            .collect();
        partial = product(&partial, &options);
    }
    partial.into_iter().map(Value::record).collect()
}

fn with_item(subset: &BTreeSet<Value>, item: &Value) -> BTreeSet<Value> {
    let mut larger = subset.clone();
    larger.insert(item.clone());
    larger
}

fn set_values(items: &[Value]) -> Vec<Value> {
    let mut subsets: Vec<BTreeSet<Value>> = vec![BTreeSet::new()];
    for item in items {
        let grown: Vec<BTreeSet<Value>> = subsets
            .iter()
            .map(|subset| with_item(subset, item))
            .collect();
        subsets.extend(grown);
    }
    subsets.into_iter().map(Value::set).collect()
}

fn with_entry(map: &BTreeMap<Value, Value>, key: &Value, value: &Value) -> BTreeMap<Value, Value> {
    let mut larger = map.clone();
    larger.insert(key.clone(), value.clone());
    larger
}

fn map_values(keys: &[Value], values: &[Value]) -> Vec<Value> {
    let mut maps: Vec<BTreeMap<Value, Value>> = vec![BTreeMap::new()];
    for key in keys {
        let grown: Vec<BTreeMap<Value, Value>> = maps
            .iter()
            .flat_map(|map| values.iter().map(move |value| with_entry(map, key, value)))
            .collect();
        maps.extend(grown);
    }
    maps.into_iter().map(Value::map).collect()
}

fn seq_values(items: &[Value], max: usize) -> Vec<Value> {
    let mut all: Vec<Vec<Value>> = vec![Vec::new()];
    let mut longest: Vec<Vec<Value>> = vec![Vec::new()];
    for _ in 0..max {
        longest = product(&longest, items);
        all.extend(longest.iter().cloned());
    }
    all.into_iter().map(Value::seq).collect()
}

#[cfg(test)]
mod tests {
    use super::{power, seq_count};

    const TWO_TO_THE_127: u128 = 170_141_183_460_469_231_731_687_303_715_884_105_728;

    #[test]
    fn a_power_multiplies_the_base_exponent_times() {
        assert_eq!(power(2, 0), 1);
        assert_eq!(power(2, 10), 1024);
        assert_eq!(power(3, 4), 81);
        assert_eq!(power(7, 1), 7);
    }

    #[test]
    fn a_power_of_one_or_zero_needs_no_loop_over_a_huge_exponent() {
        assert_eq!(power(1, u128::MAX), 1);
        assert_eq!(power(0, u128::MAX), 0);
        assert_eq!(power(0, 0), 1);
    }

    #[test]
    fn a_power_too_large_to_hold_is_the_largest_number() {
        assert_eq!(power(2, 127), TWO_TO_THE_127);
        assert_eq!(power(2, 128), u128::MAX);
        assert_eq!(power(10, u128::MAX), u128::MAX);
    }

    #[test]
    fn sequences_are_counted_per_length() {
        assert_eq!(seq_count(2, 0), 1);
        assert_eq!(seq_count(2, 2), 1 + 2 + 4);
        assert_eq!(seq_count(3, 3), 1 + 3 + 9 + 27);
        assert_eq!(seq_count(0, 5), 1);
        assert_eq!(seq_count(1, 5), 6);
    }

    #[test]
    fn counting_sequences_stops_once_the_count_can_no_longer_change() {
        assert_eq!(seq_count(0, usize::MAX), 1);
        assert_eq!(seq_count(1, 1_000_000_000_000), 1_000_000_000_001);
        assert_eq!(seq_count(2, usize::MAX), u128::MAX);
    }
}
