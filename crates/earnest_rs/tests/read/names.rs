use crate::{drafts, search};
use earnest_check::{Status, Verdict};
use earnest_rs::read;

const HEAD: &str = "\
use earnest::model::{Domain, Model, model, require, forall, exists};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
pub enum Level {
    Low,
    High,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Gate {
    #[range(0, 1)]
    #[initial(0)]
    n: u8,
}

#[model]
impl Gate {
    #[action(\"the gate opens\")]
    fn open(&self) -> Option<Self> {
        require!(self.n < 1);
        Some(Self { n: self.n + 1 })
    }

";

fn gate(uses: &str, items: &str) -> String {
    format!("{items}\n{HEAD}{uses}}}\n")
}

fn rule(body: &str) -> String {
    format!("    #[rule(\"The rule\")]\n    fn known(&self) -> bool {{\n        {body}\n    }}\n")
}

fn refused(source: &str) -> Vec<(u32, u32, String, String)> {
    let reading = read("src/case.rs", source);
    assert_eq!(reading.drafts, [], "an unreadable file gives no model");
    reading
        .problems
        .into_iter()
        .map(|problem| {
            (
                problem.line,
                problem.column,
                problem.feature,
                problem.message,
            )
        })
        .collect()
}

fn features(source: &str) -> Vec<String> {
    refused(source)
        .into_iter()
        .map(|(_, _, feature, _)| feature)
        .collect()
}

#[test]
fn an_item_of_the_library_imported_under_another_name_is_refused() {
    let source = "use earnest::model::{Model, model, forall as exists};\n";
    let found = refused(source);
    assert_eq!(found.len(), 1, "one import: {found:?}");
    assert_eq!(
        (found[0].0, found[0].1, found[0].2.as_str()),
        (1, 46, "`forall as exists`"),
        "the rename is where the file gives the other name"
    );
    assert!(
        found[0].3.contains("under another name"),
        "the sentence says why: {}",
        found[0].3
    );
}

#[test]
fn every_way_to_rename_a_library_item_is_refused() {
    for import in [
        "use earnest::model::forall as exists;",
        "use earnest::model::{Model, exists as forall};",
        "use earnest::model::Model as State;",
        "use earnest::model as kit;",
        "use earnest::{model::{forall as exists}};",
        "use earnest as kit;",
    ] {
        assert_eq!(
            features(&format!("{import}\n")).len(),
            1,
            "`{import}` gives another name to something of the library"
        );
    }
}

#[test]
fn a_rename_that_gives_no_name_and_a_glob_of_the_library_are_read() {
    let source = gate(
        &rule("exists::<Level>(|value| value == Level::Low)"),
        "use earnest::model::*;\nuse earnest::model::Model as _;\n",
    )
    .replace(
        "use earnest::model::{Domain, Model, model, require, forall, exists};\n",
        "",
    );
    assert_eq!(
        drafts("src/case.rs", &source).len(),
        1,
        "`as _` brings no name in, and a glob brings each name as it is written"
    );
}

#[test]
fn a_quantifier_imported_under_the_other_name_is_not_read_as_the_other() {
    let source = "use earnest::model::{Model, model, forall as exists};\n";
    assert_eq!(
        read("src/case.rs", source).drafts,
        [],
        "`exists::<T>(..)` would be the library's `forall`, so the file is refused"
    );
}

#[test]
fn an_item_named_like_a_library_name_is_refused() {
    for (item, feature) in [
        ("const Set: u8 = 1;", "const Set"),
        ("const None: u8 = 1;", "const None"),
        ("const forall: u8 = 1;", "const forall"),
        ("fn Some(x: u8) -> u8 { x }", "fn Some"),
        ("fn exists(x: u8) -> u8 { x }", "fn exists"),
        (
            "#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]\nenum Map { A }",
            "enum Map",
        ),
        (
            "#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]\nstruct Seq { a: bool }",
            "struct Seq",
        ),
    ] {
        let source = gate(&rule("true"), &format!("{item}\n"));
        assert!(
            features(&source).contains(&format!("`{feature}`")),
            "`{item}` is refused as `{feature}`"
        );
    }
}

#[test]
fn a_type_alias_a_macro_and_a_module_cannot_give_a_library_name_another_meaning() {
    for item in [
        "type Set = bool;",
        "macro_rules! require { ($e:expr) => { true }; }",
        "mod inner { pub fn forall() {} }",
    ] {
        let source = gate(&rule("true"), &format!("{item}\n"));
        assert_eq!(
            refused(&source).len(),
            1,
            "`{item}` is not part of a model, so the file is refused"
        );
    }
}

#[test]
fn a_use_inside_a_body_is_refused_before_it_can_rename_anything() {
    let source = gate(
        &rule(
            "{ use earnest::model::forall as exists; exists::<Level>(|value| value == Level::Low) }",
        ),
        "",
    );
    let found = features(&source);
    assert!(
        found.contains(&"`an item inside a body`".to_owned()),
        "the `use` is an item in a body: {found:?}"
    );
}

#[test]
fn a_value_named_like_a_function_makes_a_call_of_that_name_refused() {
    let items = "fn double(number: u8) -> u8 {\n    number\n}\n";
    for body in [
        "let double = 3u8; double(self.n) == 0",
        "let exists = 3u8; exists::<Level>(|value| value == Level::Low)",
        "let forall = |value: u8| value == self.n; forall(|value: u8| value == self.n)",
    ] {
        let source = gate(&rule(body), items);
        let found = refused(&source);
        assert_eq!(found.len(), 1, "`{body}`: {found:?}");
        assert!(
            found[0].3.contains("give the value another name"),
            "the sentence says what to do: {}",
            found[0].3
        );
    }
}

#[test]
fn a_closure_parameter_named_like_a_function_hides_it_inside_the_closure_only() {
    let items = "fn double(number: u8) -> u8 {\n    number\n}\n";
    let hidden = gate(
        &rule("exists::<Level>(|double| double(self.n) == 0)"),
        items,
    );
    assert_eq!(refused(&hidden).len(), 1, "the call is of the parameter");
    let after = gate(
        &rule("exists::<Level>(|double| double == Level::Low) && double(self.n) == 0"),
        items,
    );
    assert_eq!(
        drafts("src/case.rs", &after).len(),
        1,
        "after the closure the name is the function again"
    );
}

#[test]
fn a_value_named_like_a_function_that_is_never_called_is_read() {
    let items = "fn double(number: u8) -> u8 {\n    number\n}\n";
    let source = gate(&rule("let double = 1u8; double == self.n"), items);
    assert_eq!(drafts("src/case.rs", &source).len(), 1, "a plain value");
}

#[test]
fn an_earlier_use_of_a_name_is_the_outer_binding_and_not_the_later_let() {
    let items = "\
fn shadowed(n: u8) -> u8 {
    let kept = n;
    let n: u8 = 1;
    kept + n
}
";
    let source = gate(&rule("shadowed(2) == 3"), items);
    let outcome = search(&drafts("src/case.rs", &source)[0]);
    assert_eq!(
        outcome.rules[0].verdict,
        Verdict::Holds,
        "`kept` is the parameter 2, and the later `n` is 1: 2 + 1 is 3"
    );
    let reused = gate(&rule("shadowed(2) == 2"), items);
    let outcome = search(&drafts("src/case.rs", &reused)[0]);
    assert_eq!(
        outcome.rules[0].verdict,
        Verdict::Broken,
        "if `kept` had been read as the later `n` it would be 1 + 1 and this would hold"
    );
    assert_eq!(outcome.status, Status::Fail, "a rule is broken");
}

#[test]
fn a_let_may_use_the_value_it_shadows_in_its_own_initializer() {
    let items = "\
fn bumped(n: u8) -> u8 {
    let n: u8 = n + 1;
    n
}
";
    let source = gate(&rule("bumped(1) == 2"), items);
    let outcome = search(&drafts("src/case.rs", &source)[0]);
    assert_eq!(
        outcome.rules[0].verdict,
        Verdict::Holds,
        "in `let n = n + 1;` the right side is the parameter, as in Rust"
    );
}

fn found_features(source: &str) -> Vec<String> {
    read("src/case.rs", source)
        .problems
        .into_iter()
        .map(|problem| problem.feature)
        .collect()
}

#[test]
fn two_constants_functions_or_types_of_one_name_are_refused() {
    assert_eq!(
        found_features(&gate("", "const MOST: u8 = 3;\nconst MOST: u8 = 1;\n")),
        ["`item MOST`"],
        "two constants"
    );
    assert_eq!(
        found_features(&gate("", "fn f() -> u8 { 1 }\nfn f() -> u8 { 2 }\n")),
        ["`item f`"],
        "two functions"
    );
    assert_eq!(
        found_features(&gate("", "const f: u8 = 1;\nfn f() -> u8 { 2 }\n")),
        ["`item f`"],
        "a constant and a function share the value namespace"
    );
    assert_eq!(
        found_features(&gate(
            "",
            "#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]\npub enum Level { Low }\n"
        )),
        ["`type Level`"],
        "an enum and an enum"
    );
    assert_eq!(
        found_features(&gate("", "const Level: u8 = 1;\n")),
        Vec::<String>::new(),
        "a constant and a type do not share a namespace"
    );
}

#[test]
fn two_methods_of_one_name_are_refused_also_across_roles() {
    let twice = format!("{}{}", rule("true"), rule("false"));
    assert_eq!(found_features(&gate(&twice, "")), ["`method known`"]);
    let helper = "    fn known(&self) -> bool { true }\n";
    assert_eq!(
        found_features(&gate(&format!("{}{helper}", rule("true")), "")),
        ["`method known`"]
    );
}

#[test]
fn a_parameter_named_twice_is_refused() {
    assert_eq!(
        found_features(&gate("", "fn f(a: u8, a: u8) -> u8 { a }\n")),
        ["`parameter a`"]
    );
    assert!(
        found_features(&gate(&rule("[1_u8, 2].every(|x, x| true)"), ""))
            .contains(&"`parameter x`".to_owned())
    );
}

#[test]
fn a_let_may_shadow_and_an_unrelated_pair_of_names_is_read() {
    let body = "let n = 1_u8;\n        let n = n + 1;\n        n < 5";
    assert_eq!(found_features(&gate(&rule(body), "")), Vec::<String>::new());
}

#[test]
fn a_raw_identifier_is_the_plain_name_in_an_id_and_in_a_field() {
    let source = "\
use earnest::model::{Model, model};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Raw {
    #[range(0, 3)]
    #[initial(0)]
    r#type: u8,
}

#[model]
impl Raw {
    #[action(\"loop\")]
    fn r#loop(&self) -> Option<Self> {
        Some(Self { r#type: self.r#type + 1 })
    }

    #[rule(\"low\")]
    fn r#fn(&self) -> bool {
        self.r#type <= 1
    }
}
";
    let reading = read("raw.rs", source);
    assert_eq!(reading.problems, [], "no problem");
    let draft = &reading.drafts[0];
    assert_eq!(draft.fields[0].name, "type");
    assert_eq!(draft.actions[0].id, "loop");
    assert_eq!(draft.rules[0].id, "fn");
    let twice = source.replace("fn r#fn", "fn r#loop");
    assert!(
        found_features(&twice)
            .iter()
            .any(|found| found.contains("method loop")),
        "`r#loop` and `loop` are one name"
    );
}

#[test]
fn a_re_export_of_the_library_is_refused_with_the_way_to_import_it() {
    for import in [
        "pub use earnest::model::*;",
        "pub(crate) use earnest::model::{Model, model};",
        "pub use r#earnest::model::*;",
        "pub use ::earnest::model::Model;",
    ] {
        let source = format!("{import}\n");
        let found = read("src/kit.rs", &source).problems;
        assert_eq!(found.len(), 1, "`{import}` is refused once: {found:?}");
        assert!(
            found[0].message.contains("imports the library directly"),
            "the sentence says what to do: {}",
            found[0].message
        );
    }
    let plain = read("src/ok.rs", "use earnest::model::{Model, model};\n");
    assert_eq!(plain.problems, [], "a plain `use` is read");
}
