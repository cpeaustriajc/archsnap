use super::layout::Place;
use earnest_value::{Domain, Value};
use rustc_hash::FxHashMap;

const MOST_LISTED_VALUES: u128 = 1 << 12;
const GROWN_BITS: u32 = 32;
const MOST_GROWN_VALUES: usize = 1 << GROWN_BITS;

#[derive(Debug, Default)]
pub(super) struct Dict {
    values: Vec<Value>,
    inside: Vec<bool>,
    codes: FxHashMap<Value, u64>,
}

impl Dict {
    fn add(&mut self, value: Value, domain: Option<&Domain>) -> Option<u64> {
        if self.values.len() >= MOST_GROWN_VALUES {
            return None;
        }
        let inside = domain.is_none_or(|domain| domain.contains(&value));
        let code = u64::try_from(self.values.len()).ok()?;
        self.codes.insert(value.clone(), code);
        self.values.push(value);
        self.inside.push(inside);
        Some(code)
    }
}

#[derive(Debug)]
pub(super) enum Kind {
    Int { lo: i64, hi: i64 },
    Bool,
    Listed(Dict),
    Grown { dict: Dict, domain: Domain },
}

#[derive(Debug)]
pub(super) struct Codec {
    pub(super) place: Place,
    pub(super) kind: Kind,
}

fn bits_for(count: u128) -> u32 {
    if count <= 1 {
        0
    } else {
        (count - 1).bit_width()
    }
}

impl Codec {
    pub(super) fn kind_of(domain: &Domain) -> (Kind, u32) {
        match domain {
            Domain::Int { lo, hi } => (Kind::Int { lo: *lo, hi: *hi }, bits_for(domain.count())),
            Domain::Bool => (Kind::Bool, 1),
            _ if domain.count() <= MOST_LISTED_VALUES => {
                let mut dict = Dict::default();
                for value in domain.values() {
                    if domain.contains(&value) && !dict.codes.contains_key(&value) {
                        dict.add(value, None);
                    }
                }
                let bits = bits_for(u128::try_from(dict.values.len()).unwrap_or(u128::MAX));
                (Kind::Listed(dict), bits)
            }
            _ => (
                Kind::Grown {
                    dict: Dict::default(),
                    domain: domain.clone(),
                },
                GROWN_BITS,
            ),
        }
    }

    pub(super) fn code(&self, value: &Value) -> Option<u64> {
        match (&self.kind, value) {
            (Kind::Int { lo, hi }, Value::Int(number)) if lo <= number && number <= hi => {
                Some(number.abs_diff(*lo))
            }
            (Kind::Bool, Value::Bool(truth)) => Some(u64::from(*truth)),
            (Kind::Listed(dict) | Kind::Grown { dict, .. }, found) => {
                dict.codes.get(found).copied()
            }
            (Kind::Int { .. } | Kind::Bool, _) => None,
        }
    }

    pub(super) fn code_or_add(&mut self, value: &Value) -> Option<u64> {
        if let Some(code) = self.code(value) {
            return Some(code);
        }
        match &mut self.kind {
            Kind::Grown { dict, domain } => dict.add(value.clone(), Some(domain)),
            Kind::Int { .. } | Kind::Bool | Kind::Listed(_) => None,
        }
    }

    pub(super) fn value(&self, code: u64) -> Value {
        match &self.kind {
            Kind::Int { lo, .. } => lo
                .checked_add_unsigned(code)
                .map_or(Value::Null, Value::Int),
            Kind::Bool => Value::Bool(code != 0),
            Kind::Listed(dict) | Kind::Grown { dict, .. } => usize::try_from(code)
                .ok()
                .and_then(|at| dict.values.get(at))
                .cloned()
                .unwrap_or(Value::Null),
        }
    }

    pub(super) fn read_into(&self, row: &[u64], slot: &mut Value) {
        let code = self.read(row);
        match (&self.kind, slot) {
            (Kind::Int { lo, .. }, Value::Int(number))
                if lo.checked_add_unsigned(code).is_some() =>
            {
                *number = lo.saturating_add_unsigned(code);
            }
            (Kind::Bool, Value::Bool(truth)) => *truth = code != 0,
            (_, slot) => *slot = self.value(code),
        }
    }

    pub(super) fn inside(&self, code: u64) -> bool {
        match &self.kind {
            Kind::Grown { dict, .. } => usize::try_from(code)
                .ok()
                .and_then(|at| dict.inside.get(at))
                .copied()
                .unwrap_or(false),
            Kind::Int { .. } | Kind::Bool | Kind::Listed(_) => true,
        }
    }

    pub(super) fn read(&self, row: &[u64]) -> u64 {
        self.place.read(row)
    }

    pub(super) fn write(&self, row: &mut [u64], code: u64) {
        self.place.write(row, code);
    }
}
