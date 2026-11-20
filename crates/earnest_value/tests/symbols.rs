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

#[test]
fn a_text_value_prints_on_one_line_and_can_be_read_back() {
    let mut symbols = Symbols::default();
    let awkward = text(&mut symbols, "a\"b\\c\nd\re\tf\u{2028}g\u{0}h");
    assert_eq!(
        symbols.show(&awkward),
        r#""a\"b\\c\nd\re\tf\u{2028}g\u{0}h""#
    );
    assert_eq!(
        symbols.json(&awkward),
        json!("a\"b\\c\nd\re\tf\u{2028}g\u{0}h")
    );
    assert_eq!(symbols.plain(&awkward), "a\"b\\c\nd\re\tf\u{2028}g\u{0}h");
}

#[test]
fn one_text_with_a_quote_and_a_comma_does_not_print_as_two_texts() {
    let mut symbols = Symbols::default();
    let one = text(&mut symbols, "a\", \"b");
    let a = text(&mut symbols, "a");
    let b = text(&mut symbols, "b");
    let single = Value::seq(vec![one]);
    let pair = Value::seq(vec![a, b]);
    assert_eq!(symbols.show(&single), r#"["a\", \"b"]"#);
    assert_eq!(symbols.show(&pair), r#"["a", "b"]"#);
}

#[test]
fn an_alphabetical_table_hands_out_names_in_the_order_of_the_code_points() {
    let listed = ["pear", "Fig", "apple", "\u{1f600}", "\u{ff5e}", "b"];
    let mut met = Symbols::default();
    for each in listed {
        met.intern(each);
    }
    let mut sorted = met.alphabetical();
    let mut names: Vec<_> = listed.into_iter().map(|each| sorted.intern(each)).collect();
    names.sort();
    let texts: Vec<&str> = names.iter().map(|name| sorted.name(*name)).collect();
    assert_eq!(
        texts,
        ["Fig", "apple", "b", "pear", "\u{ff5e}", "\u{1f600}"],
        "the names sort as the texts do, a capital first and a character past U+FFFF last"
    );
    assert_eq!(sorted.texts().count(), 6, "no text is added or lost");
}

#[test]
fn a_set_of_values_from_an_alphabetical_table_is_walked_in_the_order_of_the_texts() {
    let mut met = Symbols::default();
    for each in ["pear", "apple", "fig"] {
        met.intern(each);
    }
    let mut sorted = met.alphabetical();
    let items: BTreeSet<Value> = ["pear", "apple", "fig"]
        .into_iter()
        .map(|each| text(&mut sorted, each))
        .collect();
    assert_eq!(
        sorted.show(&Value::set(items)),
        r#"{"apple", "fig", "pear"}"#
    );
}
