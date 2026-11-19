//! Names are interned once, and values print as the README's output section says.

use earnest_value::{Symbols, Value};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

fn text(symbols: &mut Symbols, name: &str) -> Value {
    Value::Str(symbols.intern(name))
}

#[test]
fn the_same_text_gets_the_same_name() {
    let mut symbols = Symbols::default();
    let first = symbols.intern("locked");
    let other = symbols.intern("open");
    assert_eq!(symbols.intern("locked"), first);
    assert_ne!(first, other);
    assert_eq!(symbols.name(first), "locked");
    assert_eq!(symbols.name(other), "open");
}

#[test]
fn finding_a_name_adds_nothing() {
    let mut symbols = Symbols::default();
    assert_eq!(symbols.find("locked"), None);
    let made = symbols.intern("locked");
    assert_eq!(symbols.find("locked"), Some(made));
    assert_eq!(symbols.find("open"), None);
}

#[test]
fn a_name_from_a_larger_table_has_no_text_in_a_smaller_one() {
    let mut large = Symbols::default();
    large.intern("first");
    let second = large.intern("second");
    assert_eq!(Symbols::default().name(second), "");
}

#[test]
fn plain_values_print_as_the_report_shows_them() {
    let mut symbols = Symbols::default();
    let locked = text(&mut symbols, "locked");
    assert_eq!(symbols.show(&locked), "\"locked\"");
    assert_eq!(symbols.show(&Value::Int(12)), "12");
    assert_eq!(symbols.show(&Value::Int(-3)), "-3");
    assert_eq!(symbols.show(&Value::Bool(true)), "true");
    assert_eq!(symbols.show(&Value::Bool(false)), "false");
    assert_eq!(symbols.show(&Value::Null), "null");
}

#[test]
fn collections_print_as_the_report_shows_them() {
    let mut symbols = Symbols::default();
    let a = text(&mut symbols, "a");
    let b = text(&mut symbols, "b");
    let set = Value::set(BTreeSet::from([b.clone(), a.clone()]));
    assert_eq!(symbols.show(&set), "{\"a\", \"b\"}");
    let sequence = Value::seq(vec![b.clone(), a.clone()]);
    assert_eq!(symbols.show(&sequence), "[\"b\", \"a\"]");
    let map = Value::map(BTreeMap::from([
        (a.clone(), Value::Int(1)),
        (b, Value::Int(2)),
    ]));
    assert_eq!(symbols.show(&map), "{\"a\" -> 1, \"b\" -> 2}");
    let record = Value::record(vec![
        (symbols.intern("name"), a),
        (symbols.intern("count"), Value::Int(3)),
    ]);
    assert_eq!(symbols.show(&record), "{name: \"a\", count: 3}");
}

#[test]
fn empty_collections_print_their_brackets() {
    let symbols = Symbols::default();
    assert_eq!(symbols.show(&Value::set(BTreeSet::new())), "{}");
    assert_eq!(symbols.show(&Value::seq(Vec::new())), "[]");
    assert_eq!(symbols.show(&Value::map(BTreeMap::new())), "{}");
    assert_eq!(symbols.show(&Value::record(Vec::new())), "{}");
}

#[test]
fn a_label_drops_the_quotes_of_a_text_value_only_at_the_top() {
    let mut symbols = Symbols::default();
    let adding = text(&mut symbols, "Adding");
    assert_eq!(symbols.plain(&adding), "Adding");
    assert_eq!(symbols.plain(&Value::Int(2)), "2");
    assert_eq!(symbols.plain(&Value::Null), "null");
    assert_eq!(symbols.plain(&Value::seq(vec![adding])), "[\"Adding\"]");
}

#[test]
fn values_become_the_json_the_output_documents() {
    let mut symbols = Symbols::default();
    let a = text(&mut symbols, "a");
    let b = text(&mut symbols, "b");
    assert_eq!(symbols.json(&a), json!("a"));
    assert_eq!(symbols.json(&Value::Int(12)), json!(12));
    assert_eq!(symbols.json(&Value::Bool(false)), json!(false));
    assert_eq!(symbols.json(&Value::Null), json!(null));
    let set = Value::set(BTreeSet::from([a.clone(), b.clone()]));
    assert_eq!(symbols.json(&set), json!(["a", "b"]));
    let sequence = Value::seq(vec![b.clone(), a.clone()]);
    assert_eq!(symbols.json(&sequence), json!(["b", "a"]));
    let map = Value::map(BTreeMap::from([
        (a.clone(), Value::Int(1)),
        (b, Value::Null),
    ]));
    assert_eq!(symbols.json(&map), json!([["a", 1], ["b", null]]));
    let record = Value::record(vec![(symbols.intern("name"), a)]);
    assert_eq!(symbols.json(&record), json!({ "name": "a" }));
}

#[test]
fn a_record_is_the_same_however_its_fields_were_listed() {
    let mut symbols = Symbols::default();
    let name = symbols.intern("name");
    let count = symbols.intern("count");
    let one = Value::record(vec![(name, Value::Int(1)), (count, Value::Int(2))]);
    let other = Value::record(vec![(count, Value::Int(2)), (name, Value::Int(1))]);
    assert_eq!(one, other);
}

#[test]
fn a_record_gives_its_fields_by_name_and_other_values_give_none() {
    let mut symbols = Symbols::default();
    let name = symbols.intern("name");
    let missing = symbols.intern("missing");
    let record = Value::record(vec![(name, Value::Int(1))]);
    assert_eq!(record.field(name), Some(&Value::Int(1)));
    assert_eq!(record.field(missing), None);
    assert_eq!(Value::Null.field(name), None);
    assert_eq!(Value::Int(1).field(name), None);
}
