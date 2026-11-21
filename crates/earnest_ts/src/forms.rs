use crate::read;
use crate::show::show;
use earnest_ast::{Draft, ViewDraft};
use earnest_diagnostics::Problem;

const HEAD: &str = "import { m } from \"@earnest/model\";\n\
    const Press = m.enum([\"key\", \"mouse\"]);\n\
    export const M = m.model(\"M\", { n: m.int(0, 3), last: Press.optional() });\n";

fn source(rest: &str) -> String {
    format!("{HEAD}{rest}\n")
}

fn drafts(rest: &str) -> Vec<Draft> {
    let reading = read("m.model.ts", &source(rest));
    assert_eq!(reading.problems, [], "no problem in: {rest}");
    reading.drafts
}

fn problems(rest: &str) -> Vec<Problem> {
    read("m.model.ts", &source(rest)).problems
}

fn features(rest: &str) -> Vec<String> {
    problems(rest)
        .into_iter()
        .map(|problem| problem.feature)
        .collect()
}

#[test]
fn an_action_reads_its_keys_in_any_order() {
    for parts in [
        "input: { by: Press }, when: (s, { by }) => by === \"key\", changes: (_, { by }) => ({ last: by })",
        "changes: (_, { by }) => ({ last: by }), when: (s, { by }) => by === \"key\", input: { by: Press }",
        "when: (s, { by }) => by === \"key\", changes: (_, { by }) => ({ last: by }), input: { by: Press }",
        "changes: (_, { by }) => ({ last: by }), input: { by: Press }, when: (s, { by }) => by === \"key\"",
    ] {
        let found = drafts(&format!(
            "export const act = m.action(M, \"a\", {{ {parts} }});"
        ));
        let action = found
            .first()
            .and_then(|draft| draft.actions.first())
            .expect("one action");
        let inputs: Vec<&str> = action
            .inputs
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(inputs, ["by"], "the input of `{parts}`");
        let guard = action.guard.as_ref().expect("a guard");
        assert_eq!(
            show(&guard.term),
            "(=== by \"key\")",
            "the guard of `{parts}`"
        );
        assert_eq!(
            show(&action.change.term),
            "{last: by}",
            "the change of `{parts}`"
        );
    }
}

#[test]
fn an_action_without_a_condition_or_an_input_has_neither() {
    let found = drafts("export const act = m.action(M, \"a\", { changes: () => ({ n: 0 }) });");
    let action = found
        .first()
        .and_then(|draft| draft.actions.first())
        .expect("one action");
    assert_eq!(action.inputs, [], "no input");
    assert_eq!(action.guard, None, "no guard");
}

#[test]
fn a_view_with_no_part_is_an_empty_view() {
    let found = drafts("export const view = m.view(M, {});");
    assert_eq!(
        found.first().map(|draft| &draft.view),
        Some(&ViewDraft::default()),
        "nothing declared"
    );
}

#[test]
fn a_rule_and_an_end_read_a_function_with_no_parameter() {
    let found = drafts(
        "export const low = m.rule(M, \"low\", () => true);\n\
         export const full = m.end(M, \"Full\", () => false);",
    );
    let draft = found.first().expect("one model");
    let rule = draft.rules.first().expect("one rule");
    assert_eq!(rule.state_name, None, "the rule reads no state");
    assert_eq!(show(&rule.holds), "true", "the condition of the rule");
    let end = draft.ends.first().expect("one end");
    assert_eq!(end.state_name, None, "the end reads no state");
    assert_eq!(show(&end.when), "false", "the condition of the end");
}

#[test]
fn a_rule_keeps_its_text_as_written() {
    let found = drafts("export const low = m.rule(M, \"low\", (s) =>\n\ts.n <= 2 &&\n\ts.n >= 0);");
    let rule = found
        .first()
        .and_then(|draft| draft.rules.first())
        .expect("one rule");
    assert_eq!(rule.id, "low", "the exported name");
    assert_eq!(rule.says, "low", "the sentence");
    assert_eq!(
        rule.text, "s.n <= 2 && s.n >= 0",
        "as written, with each run of white space as one space"
    );
}

#[test]
fn a_part_goes_to_the_model_named_by_its_first_argument() {
    let reading = read(
        "two.model.ts",
        "import { m } from \"@earnest/model\";\n\
         export const Porch = m.model(\"Porch\", { lit: m.boolean() });\n\
         export const Hall = m.model(\"Hall\", { level: m.int(0, 2) });\n\
         export const turn = m.action(Hall, \"turn\", { changes: (s) => ({ level: s.level }) });\n\
         export const flip = m.action(Porch, \"flip\", { changes: (s) => ({ lit: !s.lit }) });\n\
         export const known = m.rule(Hall, \"known\", (s) => s.level < 3);\n\
         export const done = m.end(Porch, \"done\", (s) => s.lit);\n",
    );
    assert_eq!(reading.problems, [], "the file is readable");
    let ids = |model: usize| -> (Vec<String>, Vec<String>, Vec<String>) {
        let draft = reading.drafts.get(model).expect("a model");
        (
            draft.actions.iter().map(|part| part.id.clone()).collect(),
            draft.rules.iter().map(|part| part.id.clone()).collect(),
            draft.ends.iter().map(|part| part.id.clone()).collect(),
        )
    };
    assert_eq!(
        ids(0),
        (vec!["flip".to_owned()], vec![], vec!["done".to_owned()]),
        "the porch"
    );
    assert_eq!(
        ids(1),
        (vec!["turn".to_owned()], vec!["known".to_owned()], vec![]),
        "the hall"
    );
}

#[test]
fn a_renamed_library_and_a_parenthesised_model_are_read() {
    let reading = read(
        "m.model.ts",
        "import { m as lib } from \"@earnest/model\";\n\
         const Inner = lib.model(\"Shown\", { on: lib.boolean() });\n\
         const stay = lib.end((Inner), \"stay\", (s) => s.on);\n\
         export { Inner, stay };\n",
    );
    assert_eq!(reading.problems, [], "read through the other name");
    let draft = reading.drafts.first().expect("one model");
    assert_eq!(draft.ends.len(), 1, "the end belongs to it");
}

#[test]
fn a_part_exported_by_a_list_is_read() {
    assert_eq!(
        problems(
            "const low = m.rule(M, \"low\", (s) => s.n <= 1);\n\
             const act = m.action(M, \"a\", { changes: () => ({ n: 0 }) });\n\
             export { low, act };"
        ),
        [],
        "the id is the exported name"
    );
}

#[test]
fn a_part_bound_to_a_constant_that_is_not_exported_is_refused() {
    for part in [
        "const low = m.rule(M, \"low\", (s) => s.n <= 1);",
        "const inc = m.action(M, \"inc\", { changes: (s) => ({ n: s.n + 1 }) });",
        "const top = m.end(M, \"top\", (s) => s.n === 3);",
    ] {
        let found = problems(part);
        assert_eq!(found.len(), 1, "`{part}` is refused once: {found:?}");
        assert_eq!(found[0].line, 4, "on the line of the constant");
        assert!(
            found[0].message.contains("exported constant"),
            "the sentence says why: {}",
            found[0].message
        );
    }
    assert_eq!(
        problems("const view = m.view(M, { title: \"t\" });"),
        [],
        "a view has no id to export"
    );
}

#[test]
fn a_part_made_in_any_other_place_than_the_whole_value_of_a_constant_is_refused() {
    let rule = "m.rule(M, \"low\", (s) => s.n <= 1)";
    for (made, expected) in [
        (
            format!("export const low = true && {rule};"),
            vec!["`m.rule`"],
        ),
        (
            format!("export const low = true ? {rule} : null;"),
            vec!["`m.rule`"],
        ),
        (
            format!("export const all = {{ low: {rule} }};"),
            vec!["`m.rule`"],
        ),
        (
            format!("const same = <T>(part: T) => part;\nexport const low = same({rule});"),
            vec!["`m.rule`"],
        ),
        (format!("const make = () => {rule};"), vec!["`m.rule`"]),
        (format!("export const low = [{rule}];"), vec!["`[]`"]),
        (
            "const make = () => m.action(M, \"a\", { changes: () => ({ n: 0 }) });".to_owned(),
            vec!["`m.action`"],
        ),
        (
            "const make = () => m.view(M, { title: \"t\" });".to_owned(),
            vec!["`m.view`"],
        ),
    ] {
        assert_eq!(features(&made), expected, "`{made}` is refused");
    }
}

#[test]
fn a_part_standing_alone_is_refused() {
    let found = problems("m.rule(M, \"low\", (s) => s.n <= 1);");
    assert_eq!(found.len(), 1, "one problem: {found:?}");
    assert_eq!(found[0].line, 4, "on the line of the statement");
}

#[test]
fn a_part_with_a_wrong_number_of_arguments_names_its_callee() {
    for made in [
        "export const a = m.action();",
        "export const a = m.action(M);",
        "export const a = m.action(M, \"a\");",
        "export const a = m.action(M, \"a\", { changes: () => ({ n: 0 }) }, 3);",
        "export const r = m.rule();",
        "export const r = m.rule(M);",
        "export const r = m.rule(M, \"r\");",
        "export const r = m.rule(M, \"r\", (s) => s.n < 1, 3);",
        "export const r = m.rule(M, \"r\", ...conditions);",
        "export const e = m.end();",
        "export const e = m.end(M, \"e\");",
        "export const e = m.end(M, \"e\", (s) => s.n < 1, 3);",
        "export const v = m.view();",
        "export const v = m.view(M);",
        "export const v = m.view(M, {}, 3);",
    ] {
        let found = features(made);
        assert_eq!(found.len(), 1, "`{made}` is refused once: {found:?}");
        assert!(
            ["`m.action`", "`m.rule`", "`m.end`", "`m.view`"].contains(&found[0].as_str()),
            "`{made}` names its callee: {found:?}"
        );
    }
}

#[test]
fn a_part_that_is_written_wrongly_is_named() {
    assert_eq!(
        features("export const a = m.action(M, \"a\", { when: (s) => s.n < 1 });"),
        ["`m.action`"],
        "no `changes`"
    );
    assert_eq!(
        features("export const a = m.action(M, \"a\", { changes: () => ({ n: 1 }), until: 3 });"),
        ["`until`"],
        "an unknown key"
    );
    assert_eq!(
        features("export const a = m.action(M, \"a\", { changes: ({ n }) => ({ n }) });"),
        ["`{ n }`"],
        "a state taken apart"
    );
    assert_eq!(
        features("export const a = m.action(M, 3, { changes: () => ({ n: 1 }) });"),
        ["`3`"],
        "an action sentence that is not text"
    );
    assert_eq!(
        features("export const r = m.rule(M, \"r\", (s, t) => s.n < t.n);"),
        ["`(s, t)`"],
        "a rule with two parameters"
    );
    assert_eq!(
        features("export const r = m.rule(M, 3, (s) => s.n < 1);"),
        ["`3`"],
        "a sentence that is not text"
    );
    assert_eq!(
        features("export const r = m.rule(M, \"r\", true);"),
        ["`true`"],
        "a condition that is not a function"
    );
    assert_eq!(
        features("export const e = m.end(M, \"e\", (s, t) => s.n < t.n);"),
        ["`(s, t)`"],
        "an end with two parameters"
    );
    assert_eq!(
        features("export const v = m.view(M, { title: 3 });"),
        ["`3`"],
        "a view title that is not text"
    );
    assert_eq!(
        features("export const v = m.view(M, { colors: { sky: \"red\" } });"),
        ["`sky`"],
        "a color for no part"
    );
    assert_eq!(
        features("export const v = m.view(M, { until: 3 });"),
        ["`until`"],
        "an unknown key in a view"
    );
}

#[test]
fn a_view_step_for_an_action_the_model_does_not_have_is_refused() {
    let found = problems(
        "export const inc = m.action(M, \"inc\", { changes: (s) => ({ n: s.n + 1 }) });\n\
         export const v = m.view(M, { step: { inc: \"up\", dec: \"down\" } });",
    );
    assert_eq!(found.len(), 1, "one unknown id: {found:?}");
    assert_eq!(found[0].feature, "`dec`", "the id");
    assert!(
        found[0].message.contains("no action with this id"),
        "{}",
        found[0].message
    );
}

#[test]
fn a_view_step_may_come_before_the_action_it_labels() {
    assert_eq!(
        problems(
            "export const v = m.view(M, { step: { inc: \"up\" } });\n\
             export const inc = m.action(M, \"inc\", { changes: (s) => ({ n: s.n + 1 }) });"
        ),
        [],
        "the id is known by the end of the file"
    );
}

#[test]
fn a_first_argument_that_is_not_a_model_of_the_file_is_refused() {
    let body = "{ changes: () => ({ n: 0 }) }";
    for (made, feature) in [
        (
            format!("export const a = m.action(Other, \"a\", {body});"),
            "`Other`",
        ),
        (
            format!("export const a = m.action(\"a\", {body});"),
            "`\"a\"`",
        ),
        (
            format!("export const a = m.action(3, \"a\", {body});"),
            "`3`",
        ),
        (
            format!("export const a = m.action(M.n, \"a\", {body});"),
            "`M.n`",
        ),
        (
            format!("export const a = m.action({{}}, \"a\", {body});"),
            "`{}`",
        ),
        (
            "export const r = m.rule(Other, \"r\", (s) => s.n < 1);".to_owned(),
            "`Other`",
        ),
        (
            "export const r = m.rule(m, \"r\", (s) => s.n < 1);".to_owned(),
            "`m`",
        ),
        (
            "export const e = m.end(Press, \"e\", (s) => s.n < 1);".to_owned(),
            "`Press`",
        ),
        (
            "export const v = m.view({ title: \"t\" });".to_owned(),
            "`{ title: \"t\" }`",
        ),
        (
            "const N = 3;\nexport const v = m.view(N, { title: \"t\" });".to_owned(),
            "`N`",
        ),
    ] {
        let found = problems(&made);
        let last = found.last().map(|problem| problem.feature.as_str());
        assert_eq!(last, Some(feature), "`{made}` is refused: {found:?}");
        assert!(
            found
                .iter()
                .any(|problem| problem.message.contains("first argument")),
            "the sentence names the first argument: {found:?}"
        );
    }
}

#[test]
fn a_model_declared_after_the_part_is_not_yet_a_model_of_the_file() {
    let found = read(
        "m.model.ts",
        "import { m } from \"@earnest/model\";\n\
         export const r = m.rule(Later, \"r\", (s) => s.n < 1);\n\
         export const Later = m.model(\"Later\", { n: m.int(0, 1) });\n",
    )
    .problems;
    assert_eq!(found.len(), 1, "one problem: {found:?}");
    assert_eq!(found[0].line, 2, "on the part");
}

#[test]
fn a_models_name_used_anywhere_else_is_refused() {
    for made in [
        "const Alias = M;",
        "export const same = M;",
        "export const all = { model: M };",
        "const pick = (s: number) => M;",
        "export const r = m.rule(M, \"r\", (s) => M.n < 1);",
        "export const r = m.rule(M, \"r\", (s) => s.n < 1 && M);",
        "export const a = m.action(M, \"a\", { changes: () => M });",
        "export const a = m.action(M, M, { changes: () => ({ n: 0 }) });",
        "export const a = m.action(M, \"a\", { input: { by: M }, changes: () => ({ n: 0 }) });",
        "export const v = m.view(M, { title: M });",
        "const lib = m;",
    ] {
        let found = features(made);
        assert!(!found.is_empty(), "`{made}` is refused");
    }
    assert_eq!(
        features("const Alias = M;\nexport const low = m.rule(Alias, \"low\", (s) => s.n < 1);"),
        ["`M`", "`Alias`"],
        "the other name, and the part made through it"
    );
}

#[test]
fn a_name_given_to_two_parts_is_declared_once() {
    let found = problems(
        "export const low = m.rule(M, \"low\", (s) => s.n < 1);\n\
         export const low = m.end(M, \"low\", (s) => s.n < 1);",
    );
    assert!(
        found
            .iter()
            .any(|problem| problem.message.contains("declared twice")),
        "a second `const` of one name is refused: {found:?}"
    );
}

fn refusals(rest: &str) -> Vec<(u32, u32, String, String)> {
    problems(rest)
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

const WRITE_CHANGES: &str = "an object with a `then` is a promise to JavaScript, and an action that was exported would hang an `async` function: write `changes`";

#[test]
fn a_then_in_an_action_is_refused_and_names_changes() {
    for parts in [
        "{ when: (s) => s.n < 3, then: (s) => ({ n: s.n + 1 }) }",
        "{ then: (s) => ({ n: s.n + 1 }), when: (s) => s.n < 3 }",
        "{ then: (s) => ({ n: s.n + 1 }) }",
    ] {
        let found = refusals(&format!("export const act = m.action(M, \"a\", {parts});"));
        let at =
            parts.find("then").expect("a then") + "export const act = m.action(M, \"a\", ".len();
        assert_eq!(
            found,
            [(
                4,
                u32::try_from(at + 1).expect("a short line"),
                "`then`".to_owned(),
                WRITE_CHANGES.to_owned()
            )],
            "`{parts}` is refused once, at the key"
        );
    }
}

#[test]
fn an_action_with_a_then_still_reports_what_else_is_wrong_in_it() {
    assert_eq!(
        features(
            "export const act = m.action(M, \"a\", { when: () => fetch(\"u\"), then: () => ({ n: 0 }) });"
        ),
        ["`fetch`", "`then`"],
        "the condition is read, the change is not"
    );
}

#[test]
fn a_file_with_a_then_still_reads_its_other_parts() {
    let reading = read(
        "m.model.ts",
        &source(
            "export const old = m.action(M, \"old\", { then: () => ({ n: 0 }) });\n\
             export const act = m.action(M, \"a\", { changes: () => ({ n: 1 }) });\n\
             export const low = m.rule(M, \"low\", (s) => s.n < 3);",
        ),
    );
    assert_eq!(
        reading.problems.len(),
        1,
        "one refusal: {:?}",
        reading.problems
    );
    let draft = reading.drafts.first().expect("the model");
    let actions: Vec<&str> = draft.actions.iter().map(|part| part.id.as_str()).collect();
    assert_eq!(actions, ["act"], "the action that is written well");
    assert_eq!(draft.rules.len(), 1, "the rule");
}

#[test]
fn a_call_on_the_model_is_refused_and_names_the_function_to_write() {
    for (made, feature, message) in [
        (
            "export const a = M.action(\"a\", { then: () => ({ n: 0 }) });",
            "`M.action`",
            "a model has no methods: write `m.action(M, \"sentence\", { when: ..., changes: ... })`, with `changes` where an action had `then`",
        ),
        (
            "export const r = M.rule({ says: \"r\", holds: (s) => s.n < 1 });",
            "`M.rule`",
            "a model has no methods: write `m.rule(M, \"sentence\", (s) => condition)`, with the sentence and the condition as arguments in place of `says` and `holds`",
        ),
        (
            "export const e = M.end({ says: \"e\", when: (s) => s.n < 1 });",
            "`M.end`",
            "a model has no methods: write `m.end(M, \"sentence\", (s) => condition)`, with the sentence and the condition as arguments in place of `says` and `when`",
        ),
        (
            "export const v = M.view({ title: \"t\" });",
            "`M.view`",
            "a model has no methods: write `m.view(M, { ... })`",
        ),
        (
            "export const c = M.check({});",
            "`M.check`",
            "a model has no methods: its parts are made with `m.action(M, ...)`, `m.rule(M, ...)`, `m.end(M, ...)` and `m.view(M, ...)`",
        ),
    ] {
        assert_eq!(
            refusals(made),
            [(4, 18, feature.to_owned(), message.to_owned())],
            "`{made}` is refused once, at the call"
        );
    }
}

#[test]
fn a_call_on_the_model_that_is_not_exported_is_refused_once() {
    assert_eq!(
        features("const r = M.rule({ says: \"r\", holds: (s) => s.n < 1 });"),
        ["`M.rule`"],
        "the call is what is wrong"
    );
}

#[test]
fn a_file_with_an_old_call_still_reads_the_other_models_and_parts() {
    let reading = read(
        "two.model.ts",
        "import { m } from \"@earnest/model\";\n\
         export const Porch = m.model(\"Porch\", { lit: m.boolean() });\n\
         export const Hall = m.model(\"Hall\", { level: m.int(0, 2) });\n\
         export const flip = Porch.action(\"flip\", { then: (s) => ({ lit: !s.lit }) });\n\
         export const turn = m.action(Hall, \"turn\", { changes: (s) => ({ level: s.level }) });\n\
         export const known = m.rule(Hall, \"known\", (s) => s.level < 3);\n\
         export const done = m.end(Porch, \"done\", (s) => s.lit);\n",
    );
    let found: Vec<(u32, &str)> = reading
        .problems
        .iter()
        .map(|problem| (problem.line, problem.feature.as_str()))
        .collect();
    assert_eq!(found, [(4, "`Porch.action`")], "only the old call");
    let ids = |model: usize| -> Vec<String> {
        let draft = reading.drafts.get(model).expect("a model");
        let actions = draft.actions.iter().map(|part| part.id.clone());
        let rules = draft.rules.iter().map(|part| part.id.clone());
        let ends = draft.ends.iter().map(|part| part.id.clone());
        actions.chain(rules).chain(ends).collect()
    };
    assert_eq!(ids(0), ["done"], "the porch keeps its end");
    assert_eq!(ids(1), ["turn", "known"], "the hall keeps all it has");
}

#[test]
fn a_key_for_the_sentence_or_the_condition_is_refused_and_names_the_arguments() {
    for (made, column, written, replacement) in [
        (
            "export const r = m.rule(M, { says: \"r\", holds: (s) => s.n < 1 });",
            28,
            "{ says: \"r\", holds: (s) => s.n < 1 }",
            "m.rule(M, \"sentence\", (s) => condition)",
        ),
        (
            "export const r = m.rule(M, \"r\", { holds: (s) => s.n < 1 });",
            33,
            "{ holds: (s) => s.n < 1 }",
            "m.rule(M, \"sentence\", (s) => condition)",
        ),
        (
            "export const e = m.end(M, { says: \"e\", when: (s) => s.n < 1 });",
            27,
            "{ says: \"e\", when: (s) => s.n < 1 }",
            "m.end(M, \"sentence\", (s) => condition)",
        ),
        (
            "export const e = m.end(M, \"e\", { when: (s) => s.n < 1 });",
            32,
            "{ when: (s) => s.n < 1 }",
            "m.end(M, \"sentence\", (s) => condition)",
        ),
    ] {
        assert_eq!(
            refusals(made),
            [(
                4,
                column,
                format!("`{written}`"),
                format!(
                    "the sentence and the function are arguments, not keys: write `{replacement}`"
                )
            )],
            "`{made}` is refused once, at the object"
        );
    }
}

#[test]
fn a_file_with_a_key_for_the_condition_still_reads_its_other_parts() {
    let reading = read(
        "m.model.ts",
        &source(
            "export const old = m.rule(M, \"old\", { holds: (s) => s.n < 3 });\n\
             export const low = m.rule(M, \"low\", (s) => s.n < 3);\n\
             export const full = m.end(M, \"Full\", (s) => s.n === 3);",
        ),
    );
    assert_eq!(
        reading.problems.len(),
        1,
        "one refusal: {:?}",
        reading.problems
    );
    let draft = reading.drafts.first().expect("the model");
    let rules: Vec<&str> = draft.rules.iter().map(|part| part.id.as_str()).collect();
    assert_eq!(rules, ["low"], "the rule that is written well");
    assert_eq!(draft.ends.len(), 1, "the end");
}

#[test]
fn a_then_that_a_module_exports_is_refused_in_each_way_a_name_is_exported() {
    let message = "a module with an export named `then` can't be awaited by `import()`, because JavaScript takes the module for a promise: export the constant under another name";
    for (made, line, column, feature) in [
        ("export const then = 3;", 4, 14, "`then`"),
        ("export const a = 1, then = 2;", 4, 21, "`then`"),
        ("const x = 3;\nexport { x as then };", 5, 10, "`x as then`"),
        ("const then = 3;\nexport { then };", 5, 10, "`then`"),
        (
            "const x = 3;\nexport { x as \"then\" };",
            5,
            10,
            "`x as \"then\"`",
        ),
    ] {
        assert_eq!(
            refusals(made),
            [(line, column, feature.to_owned(), message.to_owned())],
            "`{made}` is refused once"
        );
    }
}

#[test]
fn a_then_that_is_only_a_type_is_not_an_export_of_the_module() {
    assert_eq!(
        problems("type then = number;\nexport type { then };\nconst x = 3;\nexport { x };"),
        [],
        "a type has no value in the module"
    );
}

#[test]
fn a_file_with_an_exported_then_still_reads_its_parts() {
    let reading = read(
        "m.model.ts",
        &source(
            "export const then = 3;\n\
             export const low = m.rule(M, \"low\", (s) => s.n < 3);",
        ),
    );
    assert_eq!(
        reading.problems.len(),
        1,
        "one refusal: {:?}",
        reading.problems
    );
    let draft = reading.drafts.first().expect("the model");
    assert_eq!(draft.rules.len(), 1, "the rule");
}

#[test]
fn a_call_on_the_model_outside_a_top_level_constant_names_the_model_as_a_value() {
    for made in [
        "M.rule({ says: \"r\", holds: (s) => s.n < 1 });",
        "const make = () => M.action(\"a\", { then: () => ({ n: 0 }) });",
        "export const all = { low: M.rule({ says: \"r\", holds: (s) => s.n < 1 }) };",
    ] {
        let found = problems(made);
        assert!(
            found
                .iter()
                .any(|problem| problem.message.contains("m.rule(Model, \"sentence\"")),
            "`{made}` is refused with what to write: {found:?}"
        );
    }
}
