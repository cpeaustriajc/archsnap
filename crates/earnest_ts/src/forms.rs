use crate::read;
use earnest_ast::Draft;
use earnest_diagnostics::{Place, Problem};

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

fn without_places(draft: &Draft) -> String {
    const MARK: &str = "Place {";
    let mut text = format!("{draft:?}");
    while let Some(start) = text.find(MARK) {
        let end = text
            .get(start..)
            .and_then(|rest| rest.find('}'))
            .map_or(text.len(), |close| start + close + 1);
        text.replace_range(start..end, "Place");
    }
    text
}

fn where_declared(draft: &Draft) -> Vec<(String, Place)> {
    let actions = draft
        .actions
        .iter()
        .map(|part| (part.id.clone(), part.place));
    let rules = draft.rules.iter().map(|part| (part.id.clone(), part.place));
    let ends = draft.ends.iter().map(|part| (part.id.clone(), part.place));
    actions.chain(rules).chain(ends).collect()
}

fn same_draft(old: &str, new: &str) {
    let (read_new, read_old) = (drafts(new), drafts(old));
    assert_eq!(
        read_new.iter().map(without_places).collect::<Vec<_>>(),
        read_old.iter().map(without_places).collect::<Vec<_>>(),
        "`{new}` reads as `{old}`"
    );
    assert_eq!(
        read_new.iter().map(where_declared).collect::<Vec<_>>(),
        read_old.iter().map(where_declared).collect::<Vec<_>>(),
        "each part is declared at the same place"
    );
}

#[test]
fn an_action_with_only_its_change_reads_as_it_did() {
    same_draft(
        "export const act = M.action(\"the user acts\", { then: (s) => ({ n: s.n + 1 }) });",
        "export const act = m.action(M, \"the user acts\", { changes: (s) => ({ n: s.n + 1 }) });",
    );
}

#[test]
fn an_action_with_no_input_and_a_condition_reads_as_it_did() {
    same_draft(
        "export const act = M.action(\"a\", { when: (s) => s.n < 3, then: (s) => ({ n: s.n + 1 }) });",
        "export const act = m.action(M, \"a\", { when: (s) => s.n < 3, changes: (s) => ({ n: s.n + 1 }) });",
    );
}

#[test]
fn an_action_that_picks_its_input_reads_as_it_did() {
    same_draft(
        "export const act = M.action(\"a\", { input: { by: Press }, \
         when: (s, { by }) => s.n < 3 && by === \"key\", then: (_, { by }) => ({ last: by }) });",
        "export const act = m.action(M, \"a\", { input: { by: Press }, \
         when: (s, { by }) => s.n < 3 && by === \"key\", changes: (_, { by }) => ({ last: by }) });",
    );
}

#[test]
fn an_action_that_takes_its_whole_input_reads_as_it_did() {
    same_draft(
        "export const act = M.action(\"a\", { input: { by: Press }, \
         when: (s, input) => input.by === \"key\", \
         then: (state, given) => ({ last: given.by, n: state.n }) });",
        "export const act = m.action(M, \"a\", { input: { by: Press }, \
         when: (s, input) => input.by === \"key\", \
         changes: (state, given) => ({ last: given.by, n: state.n }) });",
    );
}

#[test]
fn an_action_whose_keys_are_in_another_order_reads_as_it_did() {
    let old = "export const act = M.action(\"a\", { input: { by: Press }, \
               when: (s, { by }) => by === \"key\", then: (_, { by }) => ({ last: by }) });";
    for new in [
        "export const act = m.action(M, \"a\", { changes: (_, { by }) => ({ last: by }), \
         when: (s, { by }) => by === \"key\", input: { by: Press } });",
        "export const act = m.action(M, \"a\", { when: (s, { by }) => by === \"key\", \
         changes: (_, { by }) => ({ last: by }), input: { by: Press } });",
        "export const act = m.action(M, \"a\", { changes: (_, { by }) => ({ last: by }), \
         input: { by: Press }, when: (s, { by }) => by === \"key\" });",
    ] {
        same_draft(old, new);
    }
}

#[test]
fn a_block_in_a_change_reads_as_it_did() {
    same_draft(
        "export const act = M.action(\"a\", { then: (s) => { const next = s.n + 1; \
         if (next > 3) return { n: 0 }; return { n: next }; } });",
        "export const act = m.action(M, \"a\", { changes: (s) => { const next = s.n + 1; \
         if (next > 3) return { n: 0 }; return { n: next }; } });",
    );
}

#[test]
fn a_rule_reads_as_it_did() {
    same_draft(
        "export const low = M.rule({\n\tsays: \"The count is low\",\n\tholds: (s) =>\n\t\ts.n <= 2 &&\n\t\ts.n >= 0,\n});",
        "export const low = m.rule(M, \"The count is low\", (s) =>\n\t\ts.n <= 2 &&\n\t\ts.n >= 0,\n);\n\n",
    );
    same_draft(
        "export const low = M.rule({ says: \"low\", holds: () => true });",
        "export const low = m.rule(M, \"low\", () => true);",
    );
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
fn an_end_reads_as_it_did() {
    same_draft(
        "export const full = M.end({ says: \"Full\", when: (s) => s.n === 3 });",
        "export const full = m.end(M, \"Full\", (s) => s.n === 3);",
    );
}

#[test]
fn a_view_with_every_part_reads_as_it_did() {
    let act = "export const act = M.action(\"act\", { then: () => ({ n: 0 }) });\n";
    let new_act = "export const act = m.action(M, \"act\", { changes: () => ({ n: 0 }) });\n";
    same_draft(
        &format!(
            "{act}export const view = M.view({{ title: \"Counting\", \
             state: (s) => `at ${{s.n}}, by ${{s.last}}!`, group: (s) => s.last, \
             step: {{ act: \"go\" }}, colors: {{ broken: \"#c62828\", start: \"#fff\" }} }});"
        ),
        &format!(
            "{new_act}export const view = m.view(M, {{ title: \"Counting\", \
             state: (s) => `at ${{s.n}}, by ${{s.last}}!`, group: (s) => s.last, \
             step: {{ act: \"go\" }}, colors: {{ broken: \"#c62828\", start: \"#fff\" }} }});"
        ),
    );
}

#[test]
fn a_view_with_no_part_reads_as_it_did() {
    same_draft(
        "export const view = M.view({});",
        "export const view = m.view(M, {});",
    );
}

#[test]
fn a_model_written_partly_in_each_form_reads() {
    let old = "export const act = M.action(\"a\", { then: (s) => ({ n: s.n + 1 }) });\n\
               export const low = M.rule({ says: \"low\", holds: (s) => s.n <= 2 });\n\
               export const full = M.end({ says: \"Full\", when: (s) => s.n === 3 });\n\
               export const view = M.view({ step: { act: \"go\" } });";
    let new = "export const act = M.action(\"a\", { then: (s) => ({ n: s.n + 1 }) });\n\
               export const low = m.rule(M, \"low\", (s) => s.n <= 2);\n\
               export const full = M.end({ says: \"Full\", when: (s) => s.n === 3 });\n\
               export const view = m.view(M, { step: { act: \"go\" } });";
    same_draft(old, new);
    let other = "export const act = m.action(M, \"a\", { changes: (s) => ({ n: s.n + 1 }) });\n\
                 export const low = M.rule({ says: \"low\", holds: (s) => s.n <= 2 });\n\
                 export const full = m.end(M, \"Full\", (s) => s.n === 3);\n\
                 export const view = M.view({ step: { act: \"go\" } });";
    same_draft(old, other);
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
fn a_name_in_both_forms_is_declared_once() {
    let found = problems(
        "export const low = m.rule(M, \"low\", (s) => s.n < 1);\n\
         export const low = M.rule({ says: \"low\", holds: (s) => s.n < 1 });",
    );
    assert!(
        found
            .iter()
            .any(|problem| problem.message.contains("declared twice")),
        "a second `const` of one name is refused: {found:?}"
    );
}
