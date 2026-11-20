use crate::{reaches_library, read};
use earnest_diagnostics::Problem;

const HEAD: &str = "import { m } from \"@earnest/model\";\nconst LIMIT = 5;\n";

fn with_room<T: Send>(work: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn_scoped(scope, work)
            .expect("a thread with room starts")
            .join()
            .expect("the work does not panic")
    })
}

fn problems(source: &str) -> Vec<Problem> {
    read("m.model.ts", source).problems
}

fn features(source: &str) -> Vec<String> {
    problems(source)
        .into_iter()
        .map(|problem| problem.feature)
        .collect()
}

fn model_with(parts: &str) -> String {
    format!("{HEAD}export const M = m.model(\"M\", {{ n: m.int(0, 3).initial(0) }});\n{parts}\n")
}

fn rule(holds: &str) -> String {
    model_with(&format!(
        "export const r = M.rule({{ says: \"r\", holds: {holds} }});"
    ))
}

#[test]
fn a_block_constant_read_before_its_declaration_is_refused_in_a_rule() {
    let source = rule("(s) => { const most = LIMIT; const LIMIT = 1; return s.n <= most; }");
    let found = problems(&source);
    assert_eq!(found.len(), 1, "one early read: {found:?}");
    assert_eq!(found[0].feature, "`LIMIT`", "the name");
    assert_eq!(
        (found[0].line, found[0].column),
        (4, 67),
        "at the first use, before the declaration"
    );
    assert!(
        found[0].message.contains("declared later in this block"),
        "the sentence says what is wrong: {}",
        found[0].message
    );
}

#[test]
fn a_block_constant_read_in_its_own_initializer_is_refused() {
    for body in [
        "(s) => { const a = a + 1; return s.n <= a; }",
        "(s) => { const LIMIT = LIMIT + 1; return s.n <= LIMIT; }",
    ] {
        assert_eq!(
            features(&rule(body)).len(),
            1,
            "`{body}` reads a constant that has no value yet"
        );
    }
}

#[test]
fn a_block_constant_read_before_its_declaration_is_refused_in_an_action_and_a_helper() {
    let action = model_with(
        "export const up = M.action(\"up\", { then: (s) => { const next = LIMIT; const LIMIT = 1; return { n: next }; } });",
    );
    assert_eq!(features(&action), ["`LIMIT`"], "in `then`");
    let guard = model_with(
        "export const up = M.action(\"up\", { when: (s) => { const x = LIMIT; const LIMIT = 1; return s.n < x; }, then: (s) => ({ n: s.n + 1 }) });",
    );
    assert_eq!(features(&guard), ["`LIMIT`"], "in `when`");
    let helper = model_with(
        "const bump = (n: number) => { const x = LIMIT; const LIMIT = n; return x; };\nexport const r = M.rule({ says: \"r\", holds: (s) => bump(s.n) > 0 });",
    );
    assert_eq!(features(&helper), ["`LIMIT`"], "in a helper");
}

#[test]
fn a_helper_called_before_its_declaration_in_a_block_is_refused() {
    let source = rule("(s) => { const x = twice(s.n); const twice = 2; return x > 0; }");
    assert!(
        features(&source).contains(&"`twice`".to_owned()),
        "calling a constant of the block before it exists: {:?}",
        features(&source)
    );
}

#[test]
fn a_nested_block_constant_read_early_is_refused_whatever_the_outer_block_holds() {
    let source = rule(
        "(s) => { const outer = LIMIT; if (s.n > 1) { const inner = LIMIT; const LIMIT = 2; return inner > 0; } return outer > 0; }",
    );
    let found = problems(&source);
    assert_eq!(
        found.len(),
        1,
        "only the early read in the inner block: {found:?}"
    );
    assert_eq!(found[0].column, 104, "the `LIMIT` of `const inner = LIMIT`");
}

#[test]
fn constants_read_after_their_declaration_or_shadowed_in_an_inner_block_are_read() {
    for body in [
        "(s) => { const a = LIMIT; const b = a + 1; return s.n <= b; }",
        "(s) => { const x = s.n; if (s.n > 1) { const y = x; const LIMIT = y; return LIMIT > 0; } return LIMIT > x; }",
        "(s) => { const x = LIMIT; return s.n <= x; }",
    ] {
        assert_eq!(problems(&rule(body)), [], "`{body}` is read");
    }
}

#[test]
fn a_number_beyond_what_a_javascript_number_holds_exactly_is_refused() {
    assert_eq!(
        problems(&rule("(s) => s.n + 9007199254740991 > 0")),
        [],
        "2^53 - 1 is exact"
    );
    assert_eq!(
        problems(&rule("(s) => s.n + -9007199254740991 < 0")),
        [],
        "and so is -(2^53 - 1)"
    );
    for literal in [
        "9007199254740992",
        "9007199254740993",
        "-9007199254740992",
        "18446744073709551616",
    ] {
        let found = problems(&rule(&format!("(s) => s.n + {literal} > 0")));
        assert_eq!(found.len(), 1, "{literal} is refused: {found:?}");
        assert!(
            found[0].message.contains("9007199254740991"),
            "the sentence names the limit: {}",
            found[0].message
        );
    }
}

#[test]
fn a_range_bound_the_library_throws_on_is_refused() {
    for bounds in [
        "9007199254740991, 9007199254740992",
        "9007199254740990, 9007199254740996",
        "-9007199254740992, 0",
        "0, 2 * 9007199254740991",
    ] {
        let source = format!(
            "{HEAD}export const M = m.model(\"M\", {{ n: m.int({bounds}).initial(0) }});\n"
        );
        assert!(
            !features(&source).is_empty(),
            "`m.int({bounds})` is outside the safe integers"
        );
    }
    let allowed = format!(
        "{HEAD}export const M = m.model(\"M\", {{ n: m.int(-9007199254740991, -9007199254740990).initial(-9007199254740991) }});\n"
    );
    assert_eq!(problems(&allowed), [], "the safe integers themselves");
}

#[test]
fn a_constant_computed_past_the_safe_integers_cannot_bound_a_range() {
    let source = format!(
        "{HEAD}const BIG = 9007199254740991;\nconst BIGGER = BIG + 1;\nexport const M = m.model(\"M\", {{ n: m.int(0, BIGGER) }});\n"
    );
    assert_eq!(
        features(&source),
        ["`BIGGER`"],
        "a sum past 2^53 is not a number the library accepts"
    );
}

#[test]
fn an_expression_nested_past_the_limit_is_refused_with_one_sentence() {
    for holds in [
        format!("(s) => s.n {}<= 3", "+ 0 ".repeat(3_000)),
        format!("(s) => {}s.n <= 3", "true && ".repeat(3_000)),
        format!(
            "(s) => {}true{}",
            "(s.n <= 3) ? ".repeat(3_000),
            " : true".repeat(3_000)
        ),
    ] {
        let source = rule(&holds);
        let found = with_room(|| problems(&source));
        assert!(!found.is_empty(), "refused");
        assert!(
            found
                .iter()
                .all(|problem| problem.message.contains("1000 levels deep")),
            "one sentence: {found:?}"
        );
    }
}

#[test]
fn brackets_and_negations_nested_past_the_limit_are_refused_before_the_parser_runs() {
    for holds in [
        format!(
            "(s) => {}s.n <= 3{}",
            "(".repeat(20_000),
            ")".repeat(20_000)
        ),
        format!("(s) => {}(s.n <= 3)", "!".repeat(20_000)),
    ] {
        let found = problems(&rule(&holds));
        assert_eq!(found.len(), 1, "one problem: {found:?}");
        assert!(
            found[0].message.contains("1000 levels deep"),
            "the sentence names the limit: {}",
            found[0].message
        );
    }
}

#[test]
fn declarations_nested_past_the_limit_are_refused() {
    let nested = format!("{}m.boolean(){}", "m.set(".repeat(3_000), ")".repeat(3_000));
    let source = format!("{HEAD}export const M = m.model(\"M\", {{ n: {nested} }});\n");
    let found = with_room(|| problems(&source));
    assert!(
        !found.is_empty(),
        "the file is refused, not run out of stack"
    );
}

#[test]
fn an_expression_nested_as_deep_as_the_limit_allows_is_read() {
    let holds = format!("(s) => s.n {}<= 3", "+ 0 ".repeat(900));
    assert_eq!(with_room(|| problems(&rule(&holds))), [], "900 sums");
}

#[test]
fn the_specifier_of_an_import_is_what_javascript_makes_of_it() {
    for specifier in [
        "@earnest\\x2fmodel",
        "@earnest\\u002fmodel",
        "@earnest\\u{2f}model",
        "\\u0040earnest/model",
        "@earnest/mo\\\ndel",
        "@earnest\\/model",
    ] {
        let source = format!("import {{ m }} from \"{specifier}\";\nexport const a = 1;\n");
        assert!(
            reaches_library("a.model.ts", &source),
            "`{}` is `@earnest/model`",
            specifier.replace('\n', "\\n")
        );
    }
}

#[test]
fn an_escaped_import_is_read_like_a_plain_one() {
    let source = "import { m } from \"@earnest\\x2fmodel\";\nexport const M = m.model(\"M\", { n: m.int(0, 3).initial(0) });\n";
    let reading = read("m.model.ts", source);
    assert_eq!(reading.problems, [], "no problem");
    assert_eq!(reading.drafts.len(), 1, "the model is found");
}

#[test]
fn a_file_that_cannot_be_shown_to_leave_the_library_alone_reaches_it() {
    for source in [
        "import { m } from \"@earnest/model\";\n",
        "import { m } from '@earnest/model';\n",
        "import type { T } from \"@earnest/model\";\n",
        "export { m } from \"@earnest/model\";\n",
        "const m = require(\"@earnest/model\");\n",
        "const later = import(\"@earnest/model\");\n",
        "const broken = ( @earnest ;\n",
        "import { m } from \"@earnest/mo\" + \"del\";\n",
    ] {
        assert!(
            reaches_library("a.ts", source),
            "`{source}` may use the library"
        );
    }
}

#[test]
fn a_file_that_leaves_the_library_alone_does_not_reach_it() {
    for source in [
        "export const one = 1;\n",
        "import { m } from \"@earnest/models\";\n",
        "import { x } from \"earnest\";\n",
        "// earnest and its model\nexport const one = 1;\n",
        "const text = \"a\\nb\";\n",
        "const broken = ( ;\n",
    ] {
        assert!(!reaches_library("a.ts", source), "`{source}` does not");
    }
}

fn view_with(colors: &str) -> String {
    model_with(&format!("export const v = M.view({{ colors: {colors} }});"))
}

#[test]
fn a_color_the_chart_would_leave_out_makes_the_file_unreadable() {
    for colors in [
        "{ broken: \"#ff\" }",
        "{ broken: \"#c62828\", start: \"rgb(1, 2, 3)\" }",
        "{ end: \"\" }",
        "{ end: \"red;\" }",
    ] {
        let found = problems(&view_with(colors));
        assert_eq!(found.len(), 1, "`{colors}` has one bad color: {found:?}");
        assert!(
            found[0].message.contains("a color is `#` and 3, 4, 6 or 8"),
            "the sentence says what a color is: {}",
            found[0].message
        );
    }
    let found = problems(&view_with("{ broken: \"#ff\" }"));
    assert_eq!(
        (found[0].line, found[0].column, found[0].feature.as_str()),
        (4, 45, "`\"#ff\"`"),
        "at the value, as the file writes it"
    );
}

#[test]
fn a_part_of_the_chart_that_has_no_color_is_refused() {
    let found = problems(&view_with("{ broken: \"red\", lost: \"blue\" }"));
    assert_eq!(found.len(), 1, "only `lost`: {found:?}");
    assert_eq!(found[0].feature, "`lost`", "the key");
}

#[test]
fn colors_the_chart_can_draw_are_read() {
    let source = view_with("{ broken: \"#c62828\", start: \"#fff\", end: \"seagreen\" }");
    assert_eq!(
        problems(&source),
        [],
        "a hex color of 6 and of 3 digits, and a name"
    );
}

#[test]
fn a_top_level_constant_named_like_the_library_is_refused() {
    let source = format!("{HEAD}const m = 3;\n");
    assert_eq!(
        features(&source),
        ["`m`"],
        "`m` is already the library, and JavaScript throws on the second declaration"
    );
    let renamed = "import { m as kit } from \"@earnest/model\";\nconst kit = 3;\nconst m = 4;\n";
    assert_eq!(
        features(renamed),
        ["`kit`"],
        "the library's name is the one the file gives it"
    );
}

#[test]
fn the_remainder_operator_is_refused_and_the_library_call_is_read() {
    for holds in [
        "(s) => s.n % 2 === 0",
        "(s) => { const r = s.n % 2; return r === 0; }",
    ] {
        let found = problems(&rule(holds));
        assert_eq!(found.len(), 1, "`{holds}`: {found:?}");
        assert_eq!(found[0].feature, "`%`", "the operator");
        assert!(
            found[0].message.contains("m.rem(a, b)"),
            "the sentence names the function: {}",
            found[0].message
        );
    }
    let assigned = problems(&rule("(s) => { s.n %= 2; return true; }"));
    assert!(
        assigned
            .iter()
            .any(|problem| problem.message.contains("m.rem(a, b)")),
        "`%=` is refused with the function named: {assigned:?}"
    );
    assert_eq!(
        problems(&rule("(s) => m.rem(s.n, 2) === 0")),
        [],
        "the call is read"
    );
    let one = problems(&rule("(s) => m.rem(s.n) === 0"));
    assert_eq!(one.len(), 1, "one argument is refused: {one:?}");
}

#[test]
fn m_rem_is_the_remainder_operation() {
    let reading = read("m.model.ts", &rule("(s) => m.rem(s.n, 2) === 0"));
    let holds = &reading.drafts[0].rules[0].holds;
    assert_eq!(
        crate::show::show(holds),
        "(=== (% s.n 2) 0)",
        "the same tree the operator used to give"
    );
}
