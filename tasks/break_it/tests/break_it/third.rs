use crate::support::{Report, Sandbox, is_broken_after, rule};

const RUST_FILE: &str = "case.rs";
const TYPESCRIPT_FILE: &str = "case.model.ts";
const NESTING: usize = 20_000;

fn checked_rust(source: &str) -> Report {
    Sandbox::with(&[(RUST_FILE, source)]).verify(&[])
}

fn checked_typescript(source: &str) -> Report {
    Sandbox::with(&[(TYPESCRIPT_FILE, source)]).verify(&[])
}

#[test]
fn a_quantifier_imported_under_the_other_name_is_not_read_as_the_other() {
    let report = checked_rust(include_str!("../../models/third/alias.rs"));
    let known = rule(report.model("Alias"), "known");
    assert!(
        report.is_unreadable() || is_broken_after(known, 0),
        "`use earnest::model::forall as exists;` makes `exists::<Gate>(..)` the library's \
         `forall`, which is false for a locked gate, so the rule is broken in the start state; \
         the file is that or unreadable, never passed\n{report}"
    );
}

#[test]
fn typescript_arithmetic_beyond_two_to_the_fifty_three_is_not_a_pass() {
    let report = checked_typescript(include_str!("../../models/third/huge.model.ts"));
    let model = report.model("Huge");
    assert!(
        report.is_unreadable() || model["status"] == "fail",
        "at n = 1 the rule compares 2^53 + 1 with 2^53, and JavaScript holds both as 2^53, so \
         the rule is false there, as `explore` in Node reports; the file is unreadable or the \
         model fails, never passes\n{report}"
    );
}

#[test]
fn a_typescript_range_beyond_the_safe_integers_is_refused() {
    let report = checked_typescript(include_str!("../../models/third/unsafe.model.ts"));
    let sane = rule(report.model("Unsafe"), "sane");
    assert!(
        report.is_unreadable() || is_broken_after(sane, 0),
        "the library throws on `m.int(9007199254740991, 9007199254740992)`, and in JavaScript \
         2^53 + 1 > 2^53 is false; the file is unreadable or the rule broken in a start state, \
         never passed\n{report}"
    );
}

#[test]
fn a_block_constant_read_before_its_declaration_is_not_the_top_level_one() {
    let report = checked_typescript(include_str!("../../models/third/early.model.ts"));
    assert!(
        report.is_unreadable() || report.model("Early")["status"] == "fail",
        "in `const most = LIMIT; const LIMIT = 1;` the first `LIMIT` is the block's own \
         constant before its declaration: `tsc` refuses it and Node throws a ReferenceError, \
         so it is never the top-level 5\n{report}"
    );
}

#[test]
fn the_smallest_i8_written_as_a_negative_literal_is_an_i8() {
    let report = checked_rust(include_str!("../../models/third/lowest.rs"));
    let model = report.model("Lowest");
    let as_rust_runs = model["status"] == "pass" && model["states"] == 2;
    assert!(
        report.is_unreadable() || as_rust_runs,
        "`-128` is an `i8`, so `self.x > -128` and `self.x >= -128` are plain comparisons: the \
         number goes -127, -128 and the rule holds in both, as the file does when it runs; \
         the file is that or unreadable, never a fault\n{report}"
    );
}

#[test]
fn a_map_literal_takes_a_record_whose_optional_field_has_a_value() {
    let report = checked_typescript(include_str!("../../models/third/keyed.model.ts"));
    assert!(
        !report.is_unreadable(),
        "`{{ qty: 1, tag: {{ size: 1 }} }}` is a value of the record whose `tag` is optional, \
         as it is in a set, a sequence or `set(k, v)`\n{report}"
    );
    let model = report.model("Keyed");
    assert_eq!(
        model["states"], 2,
        "nobody, then Ann with her line\n{report}"
    );
    assert!(
        is_broken_after(rule(model, "empty"), 1),
        "Ann has a line after one step\n{report}"
    );
}

#[test]
fn a_deeply_nested_condition_does_not_abort_the_run() {
    let deep = format!(
        "import {{ m }} from \"@earnest/model\";\n\n\
         export const Deep = m.model(\"Deep\", {{ n: m.int(0, 1).initial(0) }});\n\n\
         export const low = m.rule(Deep, \"low\", (s) => {}s.n <= 1{});\n",
        "(".repeat(NESTING),
        ")".repeat(NESTING),
    );
    let sandbox = Sandbox::with(&[
        ("deep.model.ts", &deep),
        (
            TYPESCRIPT_FILE,
            include_str!("../../models/third/huge.model.ts"),
        ),
    ]);
    let report = sandbox.verify(&[]);
    assert!(
        !report.model("Huge").is_null(),
        "a file whose condition is nested in {NESTING} brackets is unreadable or checked; the \
         other file in the folder is still reported\n{report}"
    );
}
