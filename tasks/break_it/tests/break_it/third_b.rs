use crate::support::{Report, Sandbox, is_broken_after, rule};

const RUST_FILE: &str = "case.rs";
const TYPESCRIPT_FILE: &str = "case.model.ts";
const UTF16_MARK_FILE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/models/third_b/utf16_mark.model.ts"
);
const DEPTH_OF_TYPESCRIPT: usize = 20_000;
const DEPTH_OF_RUST: usize = 3_000;

const GOOD_MODEL: &str = r#"import { m } from "@earnest/model";

export const Good = m.model("Good", { n: m.int(0, 2).initial(0) });

export const inc = m.action(Good, "one up", {
	when: (s) => s.n < 2,
	changes: (s) => ({ n: s.n + 1 }),
});

export const top = m.end(Good, "The count is two", (s) => s.n === 2);

export const low = m.rule(Good, "The count is at most two", (s) => s.n <= 2);
"#;

fn checked_typescript(source: &str) -> Report {
    Sandbox::with(&[(TYPESCRIPT_FILE, source)]).verify(&[])
}

fn checked_rust(source: &str) -> Report {
    Sandbox::with(&[(RUST_FILE, source)]).verify(&[])
}

fn typescript_with_rule(condition: &str) -> String {
    format!(
        r#"import {{ m }} from "@earnest/model";

export const Deep = m.model("Deep", {{ n: m.int(0, 3).initial(0) }});

export const inc = m.action(Deep, "one up", {{
	when: (s) => s.n < 3,
	changes: (s) => ({{ n: s.n + 1 }}),
}});

export const top = m.end(Deep, "The count is three", (s) => s.n === 3);

export const low = m.rule(Deep, "The count is at most three", (s) => {condition});
"#
    )
}

fn rust_with_rule(condition: &str) -> String {
    format!(
        r#"use earnest::model::{{Model, model, require}};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Deep {{
    #[range(0, 3)]
    #[initial(0)]
    n: u8,
}}

#[model]
impl Deep {{
    #[action("one up")]
    fn inc(&self) -> Option<Self> {{
        require!(self.n < 3);
        Some(Self {{ n: self.n + 1 }})
    }}

    #[rule("The count is at most three")]
    fn low(&self) -> bool {{
        {condition}
    }}

    #[end("The count is three")]
    fn top(&self) -> bool {{
        self.n == 3
    }}
}}
"#
    )
}

fn assert_the_deep_file_is_refused_or_checked_and_the_good_file_reported(
    deep_name: &str,
    deep_source: &str,
) {
    let report =
        Sandbox::with(&[(deep_name, deep_source), ("good.model.ts", GOOD_MODEL)]).verify(&[]);
    let deep = report.model("Deep");
    assert!(
        report.is_unreadable() || (deep["status"] == "pass" && deep["states"] == 4),
        "the rule holds in all four states, so the file is unreadable or passes with 4 states\n\
         {report}"
    );
    assert_eq!(
        report.model("Good")["status"],
        "pass",
        "one file that is too deeply nested must not take the other files' results with it\n\
         {report}"
    );
}

#[test]
fn a_number_beyond_two_to_the_53_is_not_added_exactly() {
    let report = checked_typescript(include_str!("../../models/third_b/round.model.ts"));
    let exact = rule(report.model("Round"), "exact");
    assert!(
        report.is_unreadable() || exact["verdict"] == "unknown" || is_broken_after(exact, 1),
        "Node adds 1 to 2^53 and gets 2^53, so `n + 2^53 - 2^53 === n` is broken at n = 1, after \
         one step; the file is that, or the number is a fault, or the file is unreadable, never \
         a rule that holds on exact integer arithmetic\n{report}"
    );
}

#[test]
fn a_range_the_library_throws_on_is_unreadable() {
    let report = checked_typescript(include_str!("../../models/third_b/wide_range.model.ts"));
    assert!(
        report.is_unreadable(),
        "`m.int(9007199254740990, 9007199254740996)` throws in Node because 2^53 + 3 is not a \
         safe integer, so the file does not run and is unreadable\n{report}"
    );
}

#[test]
fn the_smallest_i8_is_a_number_of_its_type() {
    let report = checked_rust(include_str!("../../models/third_b/min.rs"));
    assert!(
        !report.is_unreadable(),
        "`-128` is an `i8` in Rust, a start value, a comparison and a constant\n{report}"
    );
    let model = report.model("Min");
    assert_eq!(model["states"], 3, "-126, -127 and -128\n{report}");
    assert_eq!(
        rule(model, "least")["verdict"],
        "holds",
        "no `i8` is below -128\n{report}"
    );
    for id in ["never", "never_low"] {
        assert!(
            is_broken_after(rule(model, id), 2),
            "`{id}` is broken at -128, after two steps\n{report}"
        );
    }
    assert!(
        model["findings"].as_array().is_some_and(Vec::is_empty),
        "nothing in the model leaves `i8`\n{report}"
    );
}

#[test]
fn a_typescript_model_imported_through_an_escaped_specifier_is_not_left_out() {
    let report = checked_typescript(include_str!("../../models/third_b/escaped.model.ts"));
    let low = rule(report.model("Climb"), "low");
    assert!(
        report.is_unreadable() || is_broken_after(low, 2),
        "`\"@earnest\\x2fmodel\"` is the specifier `@earnest/model` in JavaScript, so the file \
         imports the library and Node reports `low` broken after two steps; it is checked or \
         unreadable, never left out\n{report}"
    );
}

#[test]
fn a_rust_model_with_a_comment_inside_its_use_path_is_not_left_out() {
    let report = checked_rust(include_str!("../../models/third_b/comment_path.rs"));
    let low = rule(report.model("Climb"), "low");
    assert!(
        report.is_unreadable() || is_broken_after(low, 2),
        "`use earnest::/* .. */model::{{..}}` compiles, so the file uses the library and `low` \
         is broken after two steps; it is checked or unreadable, never left out\n{report}"
    );
}

#[test]
fn a_file_that_starts_with_a_utf16_mark_and_holds_ascii_text_is_not_left_out() {
    let report = Sandbox::with(&[]).verify(&[UTF16_MARK_FILE]);
    let low = rule(report.model("Climb"), "low");
    assert!(
        report.is_unreadable() || is_broken_after(low, 2),
        "the file imports the library in plain text and is not UTF-8 (it starts with the bytes \
         FF FE), so it is unreadable or checked, never left out\n{report}"
    );
}

#[test]
fn twenty_thousand_negations_in_a_typescript_rule_do_not_abort_the_run() {
    let condition = format!("{}(s.n <= 3)", "!".repeat(DEPTH_OF_TYPESCRIPT));
    assert_the_deep_file_is_refused_or_checked_and_the_good_file_reported(
        "deep.model.ts",
        &typescript_with_rule(&condition),
    );
}

#[test]
fn twenty_thousand_additions_in_a_typescript_rule_do_not_abort_the_run() {
    let condition = format!("s.n {}<= 3", "+ 0 ".repeat(DEPTH_OF_TYPESCRIPT));
    assert_the_deep_file_is_refused_or_checked_and_the_good_file_reported(
        "deep.model.ts",
        &typescript_with_rule(&condition),
    );
}

#[test]
fn three_thousand_parentheses_in_a_rust_rule_do_not_abort_the_run() {
    let condition = format!(
        "{}self.n <= 3{}",
        "(".repeat(DEPTH_OF_RUST),
        ")".repeat(DEPTH_OF_RUST)
    );
    assert_the_deep_file_is_refused_or_checked_and_the_good_file_reported(
        "deep.rs",
        &rust_with_rule(&condition),
    );
}

#[test]
fn three_thousand_additions_in_a_rust_rule_do_not_abort_the_run() {
    let condition = format!("self.n {}<= 3", "+ 0 ".repeat(DEPTH_OF_RUST));
    assert_the_deep_file_is_refused_or_checked_and_the_good_file_reported(
        "deep.rs",
        &rust_with_rule(&condition),
    );
}

#[test]
fn explore_in_node_gives_unknown_for_a_remainder_of_a_negative_number() {
    let source = include_str!("../../models/third_b/remainder.model.ts");
    let sandbox = Sandbox::with(&[(TYPESCRIPT_FILE, source)]);
    let checked = sandbox.verify(&[]);
    let explored = sandbox.explore(TYPESCRIPT_FILE, "Rem");
    assert_eq!(
        rule(checked.model("Rem"), "below")["verdict"],
        "unknown",
        "`%` of -1 is a fault in the contract, so the checker can't judge `below`\n{checked}"
    );
    assert_eq!(
        rule(&explored.json, "below")["verdict"],
        "unknown",
        "`explore` runs `(-1) % 3` as JavaScript and gets -1, but the contract makes `%` on a \
         negative number a fault, so `below` can't be judged\n{explored}"
    );
}
