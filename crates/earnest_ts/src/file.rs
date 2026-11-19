use crate::Reader;
use crate::members::MemberKind;
use crate::names::LIBRARY_MODULE;
use crate::terms::bare;
use earnest_ast::{Draft, HelperDraft, ViewDraft};
use oxc_ast::ast::{
    Argument, ArrowFunctionExpression, BindingPattern, CallExpression, Declaration, Expression,
    ImportDeclaration, ImportDeclarationSpecifier, Program, Statement, VariableDeclaration,
    VariableDeclarationKind,
};
use oxc_span::GetSpan;

const ONLY_AT_THE_TOP: &str =
    "a model file has only imports, `const` declarations, `type` and `interface`";

pub(crate) enum Pending<'p, 'a> {
    Helper {
        name: String,
        arrow: &'p ArrowFunctionExpression<'a>,
    },
    Constant {
        name: String,
        value: &'p Expression<'a>,
    },
    Member {
        model: usize,
        kind: MemberKind,
        id: String,
        start: u32,
        call: &'p CallExpression<'a>,
    },
}

#[derive(Default)]
struct Top<'p, 'a> {
    drafts: Vec<Draft>,
    owners: Vec<(String, usize)>,
    pending: Vec<Pending<'p, 'a>>,
}

fn callee_parts<'p>(call: &'p CallExpression<'_>) -> Option<(&'p str, &'p str)> {
    let Expression::StaticMemberExpression(member) = bare(&call.callee) else {
        return None;
    };
    let Expression::Identifier(root) = bare(&member.object) else {
        return None;
    };
    Some((root.name.as_str(), member.property.name.as_str()))
}

impl Reader<'_> {
    pub(crate) fn program(&mut self, program: &Program<'_>) -> Vec<Draft> {
        let mut top = Top::default();
        for statement in &program.body {
            self.statement(statement, &mut top);
        }
        let mut constants = Vec::new();
        let mut helpers = Vec::new();
        for item in top.pending {
            match item {
                Pending::Helper { name, arrow } => {
                    let (parameters, body) = self.function(arrow);
                    helpers.push(HelperDraft {
                        name,
                        parameters,
                        body,
                    });
                }
                Pending::Constant { name, value } => constants.push((name, self.term(value))),
                Pending::Member {
                    model,
                    kind,
                    id,
                    start,
                    call,
                } => {
                    if let Some(draft) = top.drafts.get_mut(model) {
                        self.member_of(draft, kind, id, start, call);
                    }
                }
            }
        }
        for draft in &mut top.drafts {
            draft.constants.clone_from(&constants);
            draft.helpers.clone_from(&helpers);
        }
        top.drafts
    }

    fn statement<'p, 'a>(&mut self, statement: &'p Statement<'a>, top: &mut Top<'p, 'a>) {
        match statement {
            Statement::ImportDeclaration(import) => self.import(import),
            Statement::ExportDeclaration(export) => {
                self.exported(&export.declaration, export.span.start, top);
            }
            Statement::VariableDeclaration(variables) => {
                self.constants(variables, variables.span.start, top);
            }
            Statement::TSTypeAliasDeclaration(_)
            | Statement::TSInterfaceDeclaration(_)
            | Statement::EmptyStatement(_)
            | Statement::ExportNamedDeclaration(_) => {}
            Statement::ExportFromDeclaration(from) if from.export_kind.is_type() => {}
            Statement::ExportAllDeclaration(all) if all.export_kind.is_type() => {}
            Statement::ExportFromDeclaration(_) | Statement::ExportAllDeclaration(_) => {
                self.refuse(
                    statement.span().start,
                    "export",
                    "a model file can't pass on another module; only `export type` may",
                );
            }
            Statement::ExportDefaultDeclaration(_) => self.refuse(
                statement.span().start,
                "export default",
                "an id is the name of an exported constant: write `export const name = ...`",
            ),
            other => self.refuse_statement(other, ONLY_AT_THE_TOP),
        }
    }

    fn import(&mut self, import: &ImportDeclaration<'_>) {
        if import.import_kind.is_type() {
            return;
        }
        let module = import.source.value.as_str();
        if module != LIBRARY_MODULE {
            self.refuse(
                import.span.start,
                &format!("import \"{module}\""),
                "a model file imports only `@earnest/model`, and types with `import type`",
            );
            return;
        }
        for specifier in import.specifiers.iter().flatten() {
            match specifier {
                ImportDeclarationSpecifier::ImportSpecifier(named) => {
                    if named.import_kind.is_value() && named.imported.name() == "m" {
                        self.library = Some(named.local.name.to_string());
                    }
                }
                other => self.refuse(
                    other.span().start,
                    &self.quoted(other.span()),
                    "import the library by name: `import { m } from \"@earnest/model\"`",
                ),
            }
        }
    }

    fn exported<'p, 'a>(
        &mut self,
        declaration: &'p Declaration<'a>,
        start: u32,
        top: &mut Top<'p, 'a>,
    ) {
        let (feature, message) = match declaration {
            Declaration::VariableDeclaration(variables) => {
                return self.constants(variables, start, top);
            }
            Declaration::TSTypeAliasDeclaration(_) | Declaration::TSInterfaceDeclaration(_) => {
                return;
            }
            Declaration::FunctionDeclaration(_) => (
                "function",
                "write the function as an arrow function in a top-level `const`",
            ),
            Declaration::ClassDeclaration(_) => ("class", "a class is outside the library"),
            Declaration::TSEnumDeclaration(_) => {
                ("enum", "declare the choices with `m.enum([...])`")
            }
            _ => ("export", ONLY_AT_THE_TOP),
        };
        self.refuse(start, feature, message);
    }

    fn constants<'p, 'a>(
        &mut self,
        variables: &'p VariableDeclaration<'a>,
        start: u32,
        top: &mut Top<'p, 'a>,
    ) {
        if variables.kind != VariableDeclarationKind::Const {
            self.refuse(
                variables.span.start,
                variables.kind.as_str(),
                "a model file declares only `const`, because nothing in a model changes in place",
            );
            return;
        }
        for declarator in &variables.declarations {
            let (BindingPattern::BindingIdentifier(name), Some(init)) =
                (&declarator.id, &declarator.init)
            else {
                let written = self.quoted(declarator.id.span());
                self.refuse(
                    declarator.span.start,
                    &written,
                    "a `const` binds one name to one value",
                );
                continue;
            };
            self.top_names.push(name.name.to_string());
            self.top_level(name.name.as_str(), init, start, top);
        }
    }

    fn top_level<'p, 'a>(
        &mut self,
        name: &str,
        init: &'p Expression<'a>,
        start: u32,
        top: &mut Top<'p, 'a>,
    ) {
        let value = bare(init);
        if let Expression::ArrowFunctionExpression(arrow) = value {
            if arrow.r#async {
                self.refuse(
                    arrow.span.start,
                    "async",
                    "waiting is outside the library; model the wait with `m.request()`",
                );
            }
            top.pending.push(Pending::Helper {
                name: name.to_owned(),
                arrow,
            });
            return;
        }
        if let Expression::CallExpression(call) = value
            && let Some((root, part)) = callee_parts(call)
        {
            if self.is_library(root) && part == "model" {
                self.model(name, call, start, top);
                return;
            }
            let owner = top.owners.iter().find(|(known, _)| known == root);
            if let Some((_, model)) = owner {
                match MemberKind::named(part) {
                    Some(kind) => top.pending.push(Pending::Member {
                        model: *model,
                        kind,
                        id: name.to_owned(),
                        start,
                        call,
                    }),
                    None => self.refuse(
                        call.span.start,
                        &format!("{root}.{part}"),
                        "a model has `action`, `rule`, `end` and `view`",
                    ),
                }
                return;
            }
        }
        if self.is_declaration(value) {
            let shape = self.shape(value);
            self.shapes.push((name.to_owned(), shape));
            return;
        }
        if let Some(number) = self.whole_number(value) {
            self.numbers.push((name.to_owned(), number));
        }
        top.pending.push(Pending::Constant {
            name: name.to_owned(),
            value: init,
        });
    }

    fn model(
        &mut self,
        constant: &str,
        call: &CallExpression<'_>,
        start: u32,
        top: &mut Top<'_, '_>,
    ) {
        let named = call.arguments.first().and_then(Argument::as_expression);
        let declared = call.arguments.get(1).and_then(Argument::as_expression);
        let (Some(name), Some(fields), 2) = (named, declared, call.arguments.len()) else {
            self.refuse(
                call.span.start,
                &self.quoted(call.callee.span()),
                "a model takes its name and its fields: `m.model(\"Name\", { field: ... })`",
            );
            return;
        };
        let draft = Draft {
            name: self.sentence(name),
            file: self.file.to_owned(),
            place: self.place(start),
            fields: self
                .fields(fields)
                .into_iter()
                .map(|(field, shape)| earnest_ast::FieldDraft { name: field, shape })
                .collect(),
            actions: Vec::new(),
            rules: Vec::new(),
            ends: Vec::new(),
            view: ViewDraft::default(),
            constants: Vec::new(),
            helpers: Vec::new(),
        };
        top.owners.push((constant.to_owned(), top.drafts.len()));
        top.drafts.push(draft);
    }
}

#[cfg(test)]
mod tests {
    use crate::read;
    use crate::show::show;
    use earnest_diagnostics::Place;

    const HEAD: &str = "import { m } from \"@earnest/model\";\n";

    fn features(rest: &str) -> Vec<(u32, String)> {
        read("m.model.ts", &format!("{HEAD}{rest}"))
            .problems
            .into_iter()
            .map(|problem| (problem.line, problem.feature))
            .collect()
    }

    fn feature(line: u32, name: &str) -> (u32, String) {
        (line, name.to_owned())
    }

    #[test]
    fn models_keep_file_order_and_their_own_members() {
        let reading = read(
            "two.model.ts",
            "import { m } from \"@earnest/model\";\n\
             export const Porch = m.model(\"Porch\", { lit: m.boolean() });\n\
             export const Hall = m.model(\"Hall\", { level: m.int(0, 2) });\n\
             export const turn = Hall.action(\"turn\", { then: (s) => ({ level: s.level }) });\n\
             export const flip = Porch.action(\"flip\", { then: (s) => ({ lit: !s.lit }) });\n\
             export const known = Hall.rule({ says: \"known\", holds: (s) => s.level < 3 });\n",
        );
        assert_eq!(reading.problems, [], "the file is readable");
        let names: Vec<&str> = reading
            .drafts
            .iter()
            .map(|draft| draft.name.as_str())
            .collect();
        assert_eq!(names, ["Porch", "Hall"], "file order, not alphabetical");
        let porch = reading.drafts.first().expect("the first model");
        let hall = reading.drafts.get(1).expect("the second model");
        assert_eq!(porch.file, "two.model.ts", "the file is kept");
        assert_eq!(
            porch.place,
            Place { line: 2, column: 1 },
            "where `Porch` is declared"
        );
        assert_eq!(
            hall.place,
            Place { line: 3, column: 1 },
            "where `Hall` is declared"
        );
        assert_eq!(porch.actions.len(), 1, "one action on the porch");
        assert_eq!(porch.rules.len(), 0, "no rule on the porch");
        assert_eq!(
            hall.actions.first().map(|action| action.id.as_str()),
            Some("turn"),
            "id"
        );
        assert_eq!(
            hall.rules.first().map(|rule| rule.id.as_str()),
            Some("known"),
            "id"
        );
    }

    #[test]
    fn the_model_is_named_by_its_text_and_owned_by_its_constant() {
        let reading = read(
            "m.model.ts",
            "import { m as lib } from \"@earnest/model\";\n\
             const Inner = lib.model(\"Shown\", { on: lib.boolean() });\n\
             const stay = Inner.end({ says: \"stay\", when: (s) => s.on });\n\
             export { Inner, stay };\n",
        );
        assert_eq!(
            reading.problems,
            [],
            "a renamed library and a late export are read"
        );
        let draft = reading.drafts.first().expect("one model");
        assert_eq!(draft.name, "Shown", "the name given to `model`");
        assert_eq!(draft.ends.len(), 1, "the end belongs to it");
    }

    #[test]
    fn constants_and_helpers_reach_every_draft() {
        let reading = read(
            "m.model.ts",
            "import { m } from \"@earnest/model\";\n\
             import type { Other } from \"./other\";\n\
             const MAX = 3;\n\
             const NAME = \"n\", ON = true;\n\
             const isTop = (n: number) => n === MAX;\n\
             type Alias = Other;\n\
             interface Shape { n: number }\n\
             export type { Alias };\n\
             export const A = m.model(\"A\", { n: m.int(0, MAX) });\n\
             export const B = m.model(\"B\", { n: m.int(0, MAX) });\n",
        );
        assert_eq!(reading.problems, [], "types and type imports are skipped");
        assert_eq!(reading.drafts.len(), 2, "two models");
        for draft in &reading.drafts {
            let constants: Vec<(&str, String)> = draft
                .constants
                .iter()
                .map(|(name, term)| (name.as_str(), show(term)))
                .collect();
            assert_eq!(
                constants,
                [
                    ("MAX", "3".to_owned()),
                    ("NAME", "\"n\"".to_owned()),
                    ("ON", "true".to_owned())
                ],
                "the constants of the file"
            );
            let helper = draft.helpers.first().expect("one helper");
            assert_eq!(helper.name, "isTop", "the helper's name");
            assert_eq!(show(&helper.body), "(=== n MAX)", "the helper's body");
        }
    }

    #[test]
    fn a_helper_may_use_a_declaration_written_after_it() {
        let reading = read(
            "m.model.ts",
            "import { m } from \"@earnest/model\";\n\
             const any = (n: number) => m.exists(Small, (x) => x === n);\n\
             const Small = m.int(0, 1);\n",
        );
        assert_eq!(
            reading.problems,
            [],
            "the order of constants is free in a function"
        );
    }

    #[test]
    fn what_a_model_file_may_not_contain_is_named() {
        assert_eq!(
            features("import { readFileSync } from \"node:fs\";\n"),
            [feature(2, "`import \"node:fs\"`")],
            "another module"
        );
        assert_eq!(
            features("import * as lib from \"@earnest/model\";\n"),
            [feature(2, "`* as lib`")],
            "the whole library under one name"
        );
        assert_eq!(features("let count = 0;\n"), [feature(2, "`let`")], "let");
        assert_eq!(
            features("export var count = 0;\n"),
            [feature(2, "`var`")],
            "var"
        );
        assert_eq!(
            features("function twice(n: number) { return n * 2; }\n"),
            [feature(2, "`function`")],
            "a function declaration"
        );
        assert_eq!(
            features("export class Box {}\n"),
            [feature(2, "`class`")],
            "a class"
        );
        assert_eq!(
            features("enum Kind { A }\n"),
            [feature(2, "`enum`")],
            "an enum"
        );
        assert_eq!(
            features("for (const x of [1]) { x; }\n"),
            [feature(2, "`for`")],
            "a loop"
        );
        assert_eq!(
            features("console.log(\"hello\");\n"),
            [feature(2, "`console.log`")],
            "a statement that runs"
        );
        assert_eq!(
            features("export default 3;\n"),
            [feature(2, "`export default`")],
            "a default export"
        );
        assert_eq!(
            features("export * from \"./other\";\n"),
            [feature(2, "`export`")],
            "passing on a module"
        );
        assert_eq!(
            features("const [a, b] = [1, 2];\n"),
            [feature(2, "`[a, b]`")],
            "a pattern"
        );
    }

    #[test]
    fn every_problem_in_the_file_is_reported_in_order() {
        assert_eq!(
            features(
                "let a = 1;\n\
                 export const M = m.model(\"M\", { n: m.int(0, 1) });\n\
                 export const go = M.action(\"go\", { then: () => ({ n: Date.now() }) });\n\
                 export const ok = M.rule({ says: \"ok\", holds: () => fetch(\"u\") === null });\n\
                 export const odd = M.check({});\n"
            ),
            [
                feature(2, "`let`"),
                feature(4, "`Date.now`"),
                feature(5, "`fetch`"),
                feature(6, "`M.check`")
            ],
            "four problems on four lines"
        );
    }
}
