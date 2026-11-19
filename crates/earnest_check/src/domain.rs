use crate::value::{Sym, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Domain {
    Bool,
    Int { lo: i64, hi: i64 },
    OneOf(Vec<Value>),
    Optional(Box<Domain>),
    Record(Vec<(Sym, Domain)>),
    Set(Box<Domain>),
    Map(Box<Domain>, Box<Domain>),
    Seq { item: Box<Domain>, max: usize },
}

pub const MOST_VALUES_LISTED: u128 = 1_000_000;

impl Domain {
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

    #[must_use]
    pub fn contains(&self, value: &Value) -> bool {
        match (self, value) {
            (Self::Bool, Value::Bool(_)) | (Self::Optional(_), Value::Null) => true,
            (Self::Int { lo, hi }, Value::Int(number)) => lo <= number && number <= hi,
            (Self::OneOf(values), found) => values.contains(found),
            (Self::Optional(inner), found) => inner.contains(found),
            (Self::Record(fields), Value::Record(found)) => {
                fields.len() == found.len() && fields.iter().zip(found.iter()).all(field_fits)
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

fn field_fits(pair: (&(Sym, Domain), &(Sym, Value))) -> bool {
    let ((name, inner), (found_name, found_value)) = pair;
    name == found_name && inner.contains(found_value)
}

fn seq_count(each: u128, max: usize) -> u128 {
    let mut total = 0_u128;
    let mut of_length = 1_u128;
    for _ in 0..=max {
        total = total.saturating_add(of_length);
        of_length = of_length.saturating_mul(each);
    }
    total
}

fn power(base: u128, exponent: u128) -> u128 {
    let mut total = 1_u128;
    let mut left = exponent;
    while left > 0 && total < u128::MAX {
        total = total.saturating_mul(base);
        left = left.saturating_sub(1);
    }
    total
}

pub(crate) fn extended<T: Clone>(prefix: &[T], last: T) -> Vec<T> {
    let mut longer = prefix.to_vec();
    longer.push(last);
    longer
}

pub(crate) fn product<T: Clone>(prefixes: &[Vec<T>], options: &[T]) -> Vec<Vec<T>> {
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
