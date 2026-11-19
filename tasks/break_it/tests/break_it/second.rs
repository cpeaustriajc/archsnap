use crate::support::{Report, Sandbox, is_broken_after, rule};

const RUST_FILE: &str = "case.rs";
const TYPESCRIPT_FILE: &str = "case.model.ts";
const STRAY_BYTE_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/models/second/stray.model.ts");

fn checked_rust(source: &str) -> Report {
    Sandbox::with(&[(RUST_FILE, source)]).verify(&[])
}

fn checked_typescript(source: &str) -> Report {
    Sandbox::with(&[(TYPESCRIPT_FILE, source)]).verify(&[])
}

#[test]
fn a_helper_method_is_not_replaced_by_a_function_of_the_same_name() {
    let report = checked_rust(include_str!("../../models/second/twins.rs"));
    let kept = rule(report.model("Twins"), "kept");
    assert!(
        report.is_unreadable() || is_broken_after(kept, 0),
        "`self.pick(true)` calls the method, which gives `!true`, so the rule is broken in the \
         start state; the function `pick` outside the model is another function\n{report}"
    );
}

#[test]
fn an_optional_value_of_a_map_is_present_in_rust() {
    let report = checked_rust(include_str!("../../models/second/notes.rs"));
    let blank = rule(report.model("Notes"), "blank");
    assert!(
        report.is_unreadable() || is_broken_after(blank, 1),
        "after one `note` the key maps to `None`, so `get(key)` is `Some(None)` in Rust and \
         `is_none()` is false; the file is that or unreadable, never passed\n{report}"
    );
}

#[test]
fn an_optional_item_of_a_sequence_or_a_queue_is_present_in_rust() {
    let report = checked_rust(include_str!("../../models/second/gaps.rs"));
    let model = report.model("Gaps");
    let every_rule_breaks = ["headless", "tailless", "zeroless", "quiet"]
        .iter()
        .all(|id| is_broken_after(rule(model, id), 1));
    assert!(
        report.is_unreadable() || every_rule_breaks,
        "a list or an inbox that holds one `None` has a first, a last, an item at zero and a \
         head, each `Some(None)` in Rust; the file is that or unreadable, never passed\n{report}"
    );
}

#[test]
fn a_model_file_that_is_not_utf8_is_not_skipped() {
    let report = Sandbox::with(&[]).verify(&[STRAY_BYTE_FILE]);
    let low = rule(report.model("Stray"), "low");
    assert!(
        report.is_unreadable() || is_broken_after(low, 2),
        "the file imports the library and its rule breaks after two steps; one byte of its \
         action's sentence is Latin-1, so it is unreadable or checked, never left out\n{report}"
    );
}

fn assert_arithmetic_rust_refuses_is_not_a_pass(source: &str, name: &str, why: &str) {
    let report = checked_rust(source);
    let failed = report.model(name)["status"] == "fail";
    assert!(
        report.is_unreadable() || failed,
        "{why}; the file is unreadable or the model fails, never passes\n{report}"
    );
}

#[test]
fn unsigned_subtraction_below_zero_in_rust_is_not_a_pass() {
    assert_arithmetic_rust_refuses_is_not_a_pass(
        include_str!("../../models/second/wrap.rs"),
        "Wrap",
        "`self.n - 1` on a `u8` of 0 panics in a test build and is 255 in a release build, so \
         the rule can't be evaluated or is broken in the start state",
    );
}

#[test]
fn a_size_less_one_of_an_empty_list_in_rust_is_not_a_pass() {
    assert_arithmetic_rust_refuses_is_not_a_pass(
        include_str!("../../models/second/short.rs"),
        "Short",
        "`self.items.size() - 1` on an empty list is a `usize` below zero, which panics in a \
         test build and is the largest `usize` in a release build",
    );
}

#[test]
fn a_helper_method_in_rust_takes_a_number() {
    let report = checked_rust(include_str!("../../models/second/within.rs"));
    assert!(
        !report.is_unreadable(),
        "`fn below(&self, limit: u8)` is a helper method; Rust refuses `#[range]` on its \
         parameter, so the checker can't ask for one\n{report}"
    );
    let model = report.model("Within");
    assert_eq!(model["states"], 4, "the count goes 0, 1, 2, 3\n{report}");
    assert!(
        is_broken_after(rule(model, "low"), 2),
        "the count is two after two steps\n{report}"
    );
}

#[test]
fn a_rule_removed_with_its_model_is_listed_as_removed() {
    let sandbox = Sandbox::with(&[(
        TYPESCRIPT_FILE,
        include_str!("../../models/second/removed/before.model.ts"),
    )]);
    sandbox.commit();
    sandbox.write(
        TYPESCRIPT_FILE,
        include_str!("../../models/second/removed/after.model.ts"),
    );
    let report = sandbox.verify(&["--base", "HEAD"]);
    assert_eq!(
        report.change("dim")["change"],
        "removed",
        "the base commit has the model `Lamp` with the rule `dim`, and the file no longer \
         has either\n{report}"
    );
}

#[test]
fn a_choice_between_two_records_of_one_declaration_is_read() {
    let report = checked_typescript(include_str!("../../models/second/either.model.ts"));
    assert!(
        !report.is_unreadable(),
        "`{{ qty: 0, who: g }}` and `{{ qty: 1, who: null }}` are both values of the record \
         whose `who` is optional\n{report}"
    );
    let model = report.model("Either");
    assert_eq!(
        model["states"], 3,
        "nothing, Ann's item, an item without a name\n{report}"
    );
    assert!(
        is_broken_after(rule(model, "named"), 1),
        "Bob's pick leaves an item without a name\n{report}"
    );
}

#[test]
fn a_record_bound_to_a_name_is_spread_into_its_declaration() {
    let report = checked_typescript(include_str!("../../models/second/bound.model.ts"));
    assert!(
        !report.is_unreadable(),
        "`{{ ...fresh, qty: 1 }}` is a value of the record whose `who` is optional, with \
         `who` present\n{report}"
    );
    let model = report.model("Bound");
    assert_eq!(
        model["status"], "pass",
        "every chosen item is one item with a name\n{report}"
    );
    assert_eq!(
        model["states"], 3,
        "nothing, Ann's item, Bob's item\n{report}"
    );
}
