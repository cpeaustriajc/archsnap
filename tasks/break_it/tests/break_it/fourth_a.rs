use crate::support::{Report, Sandbox, is_broken_after, rule};

const TYPESCRIPT_FILE: &str = "case.model.ts";
const RUST_FILE: &str = "case.rs";
const PAST_THE_LIMIT: usize = 1_001;

const GOOD_MODEL: &str = r#"import { m } from "@earnest/model";

export const Good = m.model("Good", { n: m.int(0, 2).initial(0) });

export const inc = Good.action("one up", {
	when: (s) => s.n < 2,
	then: (s) => ({ n: s.n + 1 }),
});

export const top = Good.end({ says: "The count is two", when: (s) => s.n === 2 });

export const low = Good.rule({ says: "The count is at most two", holds: (s) => s.n <= 2 });
"#;

fn climb_with_sentence(sentence: &str) -> String {
    format!(
        r#"import {{ m }} from "@earnest/model";

export const Climb = m.model("Climb", {{ n: m.int(0, 3).initial(0) }});

export const inc = Climb.action("{sentence}", {{
	when: (s) => s.n < 3,
	then: (s) => ({{ n: s.n + 1 }}),
}});

export const top = Climb.end({{ says: "The count is three", when: (s) => s.n === 3 }});

export const low = Climb.rule({{ says: "The count stays at most one", holds: (s) => s.n <= 1 }});
"#
    )
}

fn assert_low_is_checked_or_the_file_refused(report: &Report, why: &str) {
    let low = rule(report.model("Climb"), "low");
    assert!(
        report.is_unreadable() || is_broken_after(low, 2),
        "{why}; the count reaches 2 after two steps, so `low` is broken after two steps or the \
         file is unreadable, never left out\n{report}"
    );
}

#[test]
fn a_model_that_imports_the_library_through_a_joined_specifier_is_not_left_out() {
    let source = include_str!("../../models/fourth_a/joined.model.ts");
    let report = Sandbox::with(&[(TYPESCRIPT_FILE, source)]).verify(&[TYPESCRIPT_FILE]);
    assert_low_is_checked_or_the_file_refused(
        &report,
        "`await import(\"@ear\" + \"nest/model\")` imports `@earnest/model` in JavaScript, and \
         Node runs the file with `low` broken after two steps",
    );
}

#[test]
fn a_model_that_takes_the_library_from_a_module_that_passes_it_on_is_not_left_out() {
    let sandbox = Sandbox::with(&[
        ("kit.ts", include_str!("../../models/fourth_a/kit.ts")),
        (
            "through_kit.model.ts",
            include_str!("../../models/fourth_a/through_kit.model.ts"),
        ),
    ]);
    let report = sandbox.verify(&["through_kit.model.ts"]);
    assert_low_is_checked_or_the_file_refused(
        &report,
        "`import { m } from \"./kit.ts\"` takes the library's `m`, which `kit.ts` passes on \
         from `@earnest/model`, and Node runs the file with `low` broken after two steps",
    );
}

#[test]
fn a_sentence_of_a_thousand_and_one_dashes_is_not_an_expression_nested_too_deep() {
    let source = climb_with_sentence(&format!("one up {}", "-".repeat(PAST_THE_LIMIT)));
    let report = Sandbox::with(&[(TYPESCRIPT_FILE, source.as_str())]).verify(&[]);
    assert!(
        !report.is_unreadable(),
        "the dashes are letters of a text in quotes, not a thousand prefix operators, so no \
         expression of the file is nested at all\n{report}"
    );
    assert!(
        is_broken_after(rule(report.model("Climb"), "low"), 2),
        "the count reaches 2 after two steps\n{report}"
    );
}

#[test]
fn a_file_that_does_not_import_the_library_is_not_refused_for_its_text() {
    let banner = format!(
        "export const banner = \"\\n{}\";\n",
        "-".repeat(PAST_THE_LIMIT)
    );
    let report = Sandbox::with(&[
        ("banner.ts", banner.as_str()),
        ("good.model.ts", GOOD_MODEL),
    ])
    .verify(&[]);
    assert!(
        !report.is_unreadable(),
        "`banner.ts` imports nothing, so it is not a model file and is neither read nor listed \
         as unreadable\n{report}"
    );
    assert_eq!(
        report.model("Good")["status"],
        "pass",
        "the count stays at most two\n{report}"
    );
}

#[test]
fn explore_in_node_gives_unknown_for_a_rule_that_compares_a_null_number() {
    let source = include_str!("../../models/fourth_a/pick.model.ts");
    let sandbox = Sandbox::with(&[(TYPESCRIPT_FILE, source)]);
    let checked = sandbox.verify(&[]);
    let explored = sandbox.explore(TYPESCRIPT_FILE, "Pick");
    assert_eq!(
        rule(checked.model("Pick"), "small")["verdict"],
        "unknown",
        "a number compared with an optional number that is null is a fault, so the checker \
         can't judge `small` in the start state\n{checked}"
    );
    assert_eq!(
        rule(&explored.json, "small")["verdict"],
        "unknown",
        "`explore` runs `null <= 3` as JavaScript and gets true, but the contract makes the \
         comparison a fault where the pick is null, so `small` can't be judged\n{explored}"
    );
}

#[test]
fn a_typescript_constant_declared_twice_makes_the_file_unreadable() {
    let source = include_str!("../../models/fourth_a/twice.model.ts");
    let report = Sandbox::with(&[(TYPESCRIPT_FILE, source)]).verify(&[]);
    assert!(
        report.is_unreadable(),
        "`const MOST = 3; const MOST = 1;` is a SyntaxError in JavaScript and TS2451 in `tsc`, \
         so the file never runs and has no `MOST` to judge `low` by\n{report}"
    );
}

#[test]
fn a_rust_constant_and_a_method_defined_twice_make_the_file_unreadable() {
    let source = include_str!("../../models/fourth_a/twice.rs");
    let report = Sandbox::with(&[(RUST_FILE, source)]).verify(&[]);
    assert!(
        report.is_unreadable(),
        "Rust refuses two `const MOST` (E0428) and two methods `below` (E0592), so the file \
         does not compile and has no `MOST` or `below` to judge a rule by\n{report}"
    );
}
