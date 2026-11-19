//! A domain counts, lists and recognises the same values.

use earnest_value::{Domain, MOST_VALUES_LISTED, Sym, Symbols, Value};
use proptest::prelude::{Just, Strategy, any, prop_assert, prop_assert_eq, prop_oneof, proptest};
use proptest::sample::Index;
use std::collections::{BTreeMap, BTreeSet};

const MOST_VALUES_TRIED: u128 = 600;

fn names() -> [Sym; 3] {
    let mut symbols = Symbols::default();
    [
        symbols.intern("a"),
        symbols.intern("b"),
        symbols.intern("c"),
    ]
}

fn int(lo: i64, hi: i64) -> Domain {
    Domain::Int { lo, hi }
}

fn seq(item: Domain, max: usize) -> Domain {
    Domain::Seq {
        item: Box::new(item),
        max,
    }
}

fn set_of(items: &[Value]) -> Value {
    Value::set(items.iter().cloned().collect())
}

fn plain_domain() -> impl Strategy<Value = Domain> {
    let [a, b, c] = names();
    prop_oneof![
        Just(Domain::Bool),
        (-2_i64..=2, 0_i64..=3).prop_map(|(lo, span)| int(lo, lo + span - 1)),
        (1_usize..=3).prop_map(move |kept| {
            Domain::OneOf(
                [a, b, c]
                    .into_iter()
                    .take(kept)
                    .map(Value::Str)
                    .collect::<Vec<_>>(),
            )
        }),
    ]
}

fn small_domain() -> impl Strategy<Value = Domain> {
    let [a, b, _] = names();
    plain_domain()
        .prop_recursive(2, 8, 2, move |inner| {
            prop_oneof![
                inner.clone().prop_map(|item| Domain::Set(Box::new(item))),
                (inner.clone(), 0_usize..=2).prop_map(|(item, max)| seq(item, max)),
                (inner.clone(), inner.clone())
                    .prop_map(|(key, value)| Domain::Map(Box::new(key), Box::new(value))),
                (inner.clone(), inner.clone())
                    .prop_map(move |(first, second)| Domain::Record(vec![(a, first), (b, second)])),
                inner
                    .prop_filter("an optional value holds no optional value", |item| {
                        !matches!(item, Domain::Optional(_))
                    })
                    .prop_map(|item| Domain::Optional(Box::new(item))),
            ]
        })
        .prop_filter("few enough values to list", |domain| {
            domain.count() <= MOST_VALUES_TRIED
        })
}

fn picked(domain: &Domain, index: Index) -> Value {
    let values = domain.values();
    if values.is_empty() {
        Value::Null
    } else {
        values[index.index(values.len())].clone()
    }
}

proptest! {
    #[test]
    fn a_domain_lists_as_many_values_as_it_counts(domain in small_domain()) {
        let listed = u128::try_from(domain.values().len()).unwrap();
        prop_assert_eq!(listed, domain.count());
    }

    #[test]
    fn a_domain_lists_each_value_once(domain in small_domain()) {
        let values = domain.values();
        let distinct: BTreeSet<&Value> = values.iter().collect();
        prop_assert_eq!(distinct.len(), values.len());
    }

    #[test]
    fn a_domain_contains_every_value_it_lists(domain in small_domain()) {
        for value in domain.values() {
            prop_assert!(domain.contains(&value), "{value:?} is listed by {domain:?}");
        }
    }

    #[test]
    fn a_domain_contains_exactly_the_values_it_lists(
        domain in small_domain(),
        other in small_domain(),
        index in any::<Index>(),
    ) {
        let candidate = picked(&other, index);
        let listed = domain.values().contains(&candidate);
        prop_assert_eq!(domain.contains(&candidate), listed, "{:?} in {:?}", candidate, domain);
    }

    #[test]
    fn a_number_range_contains_exactly_the_numbers_between_its_bounds(
        lo in -5_i64..=5,
        hi in -5_i64..=5,
        number in -8_i64..=8,
    ) {
        let inside = lo <= number && number <= hi;
        prop_assert_eq!(int(lo, hi).contains(&Value::Int(number)), inside);
    }

    #[test]
    fn a_sequence_longer_than_its_max_is_outside(max in 0_usize..=3, length in 0_usize..=5) {
        let value = Value::seq(vec![Value::Bool(true); length]);
        prop_assert_eq!(seq(Domain::Bool, max).contains(&value), length <= max);
    }
}

#[test]
fn each_kind_of_domain_counts_what_its_declaration_says() {
    let [a, b, _] = names();
    assert_eq!(Domain::Bool.count(), 2);
    assert_eq!(int(0, 3).count(), 4);
    assert_eq!(int(-2, 2).count(), 5);
    assert_eq!(int(3, 0).count(), 0);
    assert_eq!(int(i64::MIN, i64::MAX).count(), 18_446_744_073_709_551_616);
    assert_eq!(Domain::OneOf(vec![Value::Str(a), Value::Str(b)]).count(), 2);
    assert_eq!(Domain::Optional(Box::new(Domain::Bool)).count(), 3);
    assert_eq!(Domain::Set(Box::new(int(0, 2))).count(), 8);
    assert_eq!(
        Domain::Map(Box::new(Domain::Bool), Box::new(int(0, 1))).count(),
        9
    );
    assert_eq!(seq(Domain::Bool, 2).count(), 7);
    assert_eq!(
        Domain::Record(vec![(a, Domain::Bool), (b, int(0, 2))]).count(),
        6
    );
    assert_eq!(Domain::Record(Vec::new()).count(), 1);
}

#[test]
fn a_count_too_large_to_hold_is_the_largest_number() {
    let every_set = Domain::Set(Box::new(int(0, 200)));
    assert_eq!(every_set.count(), u128::MAX);
    assert!(every_set.count() > MOST_VALUES_LISTED);
}

#[test]
fn a_map_to_no_values_counts_only_the_empty_map_however_many_keys() {
    let no_values = Domain::OneOf(Vec::new());
    let map = Domain::Map(Box::new(int(i64::MIN, i64::MAX)), Box::new(no_values));
    assert_eq!(map.count(), 1);
}

#[test]
fn an_optional_domain_lists_null_first_then_the_inner_values() {
    let listed = Domain::Optional(Box::new(Domain::Bool)).values();
    assert_eq!(
        listed,
        vec![Value::Null, Value::Bool(false), Value::Bool(true)]
    );
}

#[test]
fn a_number_range_lists_its_numbers_upward() {
    assert_eq!(
        int(-1, 1).values(),
        vec![Value::Int(-1), Value::Int(0), Value::Int(1)]
    );
    assert_eq!(int(1, -1).values(), Vec::new());
}

#[test]
fn a_set_domain_lists_every_subset() {
    let listed: BTreeSet<Value> = Domain::Set(Box::new(Domain::Bool))
        .values()
        .into_iter()
        .collect();
    let no = Value::Bool(false);
    let yes = Value::Bool(true);
    let expected: BTreeSet<Value> = [
        set_of(&[]),
        set_of(std::slice::from_ref(&no)),
        set_of(std::slice::from_ref(&yes)),
        set_of(&[no, yes]),
    ]
    .into_iter()
    .collect();
    assert_eq!(listed, expected);
}

#[test]
fn a_queue_of_capacity_two_over_two_messages_has_seven_contents() {
    let [a, b, _] = names();
    let messages = Domain::OneOf(vec![Value::Str(a), Value::Str(b)]);
    let listed: BTreeSet<Value> = seq(messages, 2).values().into_iter().collect();
    let text = |items: &[Sym]| Value::seq(items.iter().copied().map(Value::Str).collect());
    let expected: BTreeSet<Value> = [
        text(&[]),
        text(&[a]),
        text(&[b]),
        text(&[a, a]),
        text(&[a, b]),
        text(&[b, a]),
        text(&[b, b]),
    ]
    .into_iter()
    .collect();
    assert_eq!(listed, expected);
}

#[test]
fn a_value_of_another_kind_is_outside() {
    let [a, b, c] = names();
    let one_of = Domain::OneOf(vec![Value::Str(a), Value::Str(b)]);
    assert!(!Domain::Bool.contains(&Value::Int(0)));
    assert!(!Domain::Bool.contains(&Value::Null));
    assert!(!int(0, 3).contains(&Value::Bool(true)));
    assert!(!int(0, 3).contains(&Value::Int(4)));
    assert!(!int(0, 3).contains(&Value::Int(-1)));
    assert!(one_of.contains(&Value::Str(b)));
    assert!(!one_of.contains(&Value::Str(c)));
    assert!(!one_of.contains(&Value::Null));
    assert!(!Domain::Set(Box::new(Domain::Bool)).contains(&Value::seq(Vec::new())));
    assert!(!seq(Domain::Bool, 2).contains(&set_of(&[])));
}

#[test]
fn an_optional_domain_accepts_null_and_its_inner_values_only() {
    let optional = Domain::Optional(Box::new(int(1, 2)));
    assert!(optional.contains(&Value::Null));
    assert!(optional.contains(&Value::Int(2)));
    assert!(!optional.contains(&Value::Int(3)));
}

#[test]
fn a_collection_with_one_item_outside_is_outside() {
    let numbers = int(0, 1);
    let set = Domain::Set(Box::new(numbers.clone()));
    assert!(set.contains(&set_of(&[Value::Int(0), Value::Int(1)])));
    assert!(!set.contains(&set_of(&[Value::Int(0), Value::Int(2)])));
    let sequence = seq(numbers.clone(), 3);
    assert!(sequence.contains(&Value::seq(vec![Value::Int(1), Value::Int(1)])));
    assert!(!sequence.contains(&Value::seq(vec![Value::Int(1), Value::Int(2)])));
    let map = Domain::Map(Box::new(Domain::Bool), Box::new(numbers));
    let entry = |key: Value, value: Value| Value::map(BTreeMap::from([(key, value)]));
    assert!(map.contains(&entry(Value::Bool(true), Value::Int(1))));
    assert!(!map.contains(&entry(Value::Int(1), Value::Int(1))));
    assert!(!map.contains(&entry(Value::Bool(true), Value::Int(2))));
}

#[test]
fn a_record_needs_exactly_the_declared_fields() {
    let [a, b, c] = names();
    let record = Domain::Record(vec![(a, Domain::Bool), (b, int(0, 2))]);
    let yes = Value::Bool(true);
    assert!(record.contains(&Value::record(vec![(b, Value::Int(2)), (a, yes.clone())])));
    assert!(!record.contains(&Value::record(vec![(a, yes.clone())])));
    assert!(!record.contains(&Value::record(vec![(a, yes.clone()), (b, Value::Int(3))])));
    assert!(!record.contains(&Value::record(vec![(a, yes.clone()), (c, Value::Int(2))])));
    assert!(!record.contains(&Value::record(vec![
        (a, yes),
        (b, Value::Int(2)),
        (c, Value::Int(2)),
    ])));
}
