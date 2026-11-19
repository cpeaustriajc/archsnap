//! One failing test for each confirmed finding of the adversarial pass over `earnest`.
//!
//! Every test asserts what the contract in the root `README.md` says, so it fails today and
//! passes once the finding is fixed. `tasks/break_it/README.md` has the hand derivation of each
//! expected result.

mod second;
mod support;

use support::{Sandbox, is_broken_after, rule};

const FILE: &str = "case.model.ts";

fn assert_the_rule_is_checked_or_the_file_refused(source: &str) {
    let report = Sandbox::with(&[(FILE, source)]).verify(&[]);
    let broken = report.model("Climb")["rules"]
        .as_array()
        .is_some_and(|rules| rules.iter().any(|found| is_broken_after(found, 2)));
    assert!(
        report.is_unreadable() || broken,
        "the rule that the count stays at most one is broken after two steps, so the file is \
         either unreadable or reported with that rule broken, never passed without it\n{report}"
    );
}

#[test]
fn a_rule_made_through_another_name_for_the_model_is_not_dropped() {
    assert_the_rule_is_checked_or_the_file_refused(include_str!(
        "../../models/dropped/alias.model.ts"
    ));
}

#[test]
fn a_rule_made_on_the_right_of_a_logical_and_is_not_dropped() {
    assert_the_rule_is_checked_or_the_file_refused(include_str!(
        "../../models/dropped/and.model.ts"
    ));
}

#[test]
fn a_rule_made_in_a_branch_of_a_choice_is_not_dropped() {
    assert_the_rule_is_checked_or_the_file_refused(include_str!(
        "../../models/dropped/ternary.model.ts"
    ));
}

#[test]
fn a_rule_made_with_call_on_the_method_is_not_dropped() {
    assert_the_rule_is_checked_or_the_file_refused(include_str!(
        "../../models/dropped/call.model.ts"
    ));
}

#[test]
fn a_rule_made_inside_a_record_constant_is_not_dropped() {
    assert_the_rule_is_checked_or_the_file_refused(include_str!(
        "../../models/dropped/record.model.ts"
    ));
}

#[test]
fn a_rule_passed_through_a_helper_function_is_not_dropped() {
    assert_the_rule_is_checked_or_the_file_refused(include_str!(
        "../../models/dropped/wrapped.model.ts"
    ));
}

fn assert_the_constant_is_read_not_the_input(source: &str) {
    let report = Sandbox::with(&[(FILE, source)]).verify(&[]);
    let model = report.model("Shadow");
    let as_written = model["states"] == 6 && is_broken_after(rule(model, "low"), 3);
    assert!(
        report.is_unreadable() || as_written,
        "`limit` in `when` is the constant 5, so the count reaches 5 through 6 states and the \
         rule breaks after 3 steps; the file is that or unreadable, never passed\n{report}"
    );
}

#[test]
fn a_constant_is_not_replaced_by_an_input_of_the_same_name_that_no_function_takes() {
    assert_the_constant_is_read_not_the_input(include_str!("../../models/shadow/neither.model.ts"));
}

#[test]
fn a_constant_in_when_is_not_replaced_by_an_input_that_only_then_takes() {
    assert_the_constant_is_read_not_the_input(include_str!(
        "../../models/shadow/then_only.model.ts"
    ));
}

#[test]
fn a_constant_is_not_replaced_by_a_field_of_the_whole_input() {
    assert_the_constant_is_read_not_the_input(include_str!(
        "../../models/shadow/whole_input.model.ts"
    ));
}

fn changes_since_the_commit() -> support::Report {
    let sandbox = Sandbox::with(&[(
        "tank.model.ts",
        include_str!("../../models/changes/before/tank.model.ts"),
    )]);
    sandbox.commit();
    sandbox.write(
        "tank.model.ts",
        include_str!("../../models/changes/after/tank.model.ts"),
    );
    sandbox.verify(&["--base", "HEAD"])
}

#[test]
fn a_rule_loosened_by_changing_its_constant_is_listed_as_loosened() {
    let report = changes_since_the_commit();
    assert_eq!(
        report.change("capped")["change"],
        "loosened",
        "level 3 passes `s.level <= MAX` now and broke it at the base, where MAX was 2\n{report}"
    );
}

#[test]
fn a_rule_loosened_by_changing_its_helper_is_listed_as_loosened() {
    let report = changes_since_the_commit();
    assert_eq!(
        report.change("checked")["change"],
        "loosened",
        "level 3 passes `allowed(s.level)` now and broke it at the base, where the helper \
         allowed at most 2\n{report}"
    );
}

#[test]
fn explore_in_node_gives_unknown_for_a_rule_that_faults() {
    let source = include_str!("../../models/basket.model.ts");
    let sandbox = Sandbox::with(&[(FILE, source)]);
    let checked = sandbox.verify(&[]);
    let explored = sandbox.explore(FILE, "Basket");
    for id in ["few", "early"] {
        assert_eq!(
            rule(checked.model("Basket"), id)["verdict"],
            "unknown",
            "the checker can't evaluate `{id}` once the basket is null\n{checked}"
        );
        assert_eq!(
            rule(&explored.json, id)["verdict"],
            "unknown",
            "`explore` can't evaluate `{id}` once the basket is null, so it is not seen to \
             hold\n{explored}"
        );
    }
}

#[test]
fn a_record_is_spread_from_an_optional_field_marked_present() {
    let report = Sandbox::with(&[(FILE, include_str!("../../models/box.model.ts"))]).verify(&[]);
    assert!(
        !report.is_unreadable(),
        "`{{ ...s.chosen!, qty: ... }}` is a record with a spread of a value marked with `!`\n\
         {report}"
    );
    let model = report.model("Box");
    assert_eq!(
        model["states"], 7,
        "null and two flags by three counts\n{report}"
    );
    assert!(
        is_broken_after(rule(model, "cold"), 4),
        "two hot items take pick, more, more and heat\n{report}"
    );
}

#[test]
fn a_field_may_have_the_name_of_a_primitive_property() {
    let report = Sandbox::with(&[(FILE, include_str!("../../models/flag.model.ts"))]).verify(&[]);
    assert!(
        !report.is_unreadable(),
        "a boolean field named `isFull` leaves `s.inbox.isFull` the queue's own property\n\
         {report}"
    );
    let model = report.model("Flag");
    assert_eq!(
        model["status"], "pass",
        "the flag follows the inbox\n{report}"
    );
    assert_eq!(model["states"], 2, "empty, then full\n{report}");
}

#[test]
fn a_statement_after_the_last_return_makes_the_file_unreadable() {
    let source = include_str!("../../models/dead_code.model.ts");
    let report = Sandbox::with(&[(FILE, source)]).verify(&[]);
    assert!(
        report.is_unreadable(),
        "the body of `low` goes on after its `return` with the assignment `s.n = 4`\n{report}"
    );
}

#[test]
fn an_optional_number_compared_by_order_in_rust_is_not_a_fault() {
    let report = Sandbox::with(&[("order.rs", include_str!("../../models/order.rs"))]).verify(&[]);
    let model = report.model("Order");
    let as_rust_orders = model["status"] == "pass" && model["states"] == 2;
    assert!(
        report.is_unreadable() || as_rust_orders,
        "in Rust `None < Some(5)` and `Some(1) < Some(5)` are both true, so the rule holds in \
         both states; the file is that or unreadable, never a fault\n{report}"
    );
}
