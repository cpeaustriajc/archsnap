use crate::Reader;
use crate::members::{Member, MemberKind};
use crate::names::LIBRARY_MODULE;
use crate::terms::{CLASS, bare};
use earnest_ast::{Draft, HelperDraft, HelperKind, Language, ViewDraft};
use oxc_ast::ast::{
    Argument, ArrowFunctionExpression, BindingPattern, CallExpression, Declaration,
    ExportSpecifier, Expression, ImportDeclaration, ImportDeclarationSpecifier, Program, Statement,
    VariableDeclaration, VariableDeclarationKind,
};
use oxc_span::GetSpan;

const ONLY_AT_THE_TOP: &str =
    "a model file has only imports, `const` declarations, `type` and `interface`";
const NOT_A_CONSTANT: &str = "a top-level `const` holds a model, an action, a rule, an end or a \
    view made by calling the library, a declaration, a helper function, or a number, text or \
    boolean written out or computed from earlier constants";
const THEN: &str = "then";
const THEN_EXPORTED: &str = "a module with an export named `then` can't be awaited by `import()`, because JavaScript takes the module for a promise: export the constant under another name";
const NO_METHODS: &str = "a model has no methods: write";

pub(crate) enum Pending<'p, 'a> {
    Helper {
        name: String,
        arrow: &'p ArrowFunctionExpression<'a>,
    },
    Constant {
        name: String,
        value: &'p Expression<'a>,
    },
    Refused {
        value: &'p Expression<'a>,
    },
    Member(Member<'p, 'a>),
}

#[derive(Default)]
struct Top<'p, 'a> {
    drafts: Vec<Draft>,
    owners: Vec<(String, usize)>,
    pending: Vec<Pending<'p, 'a>>,
    exported: Vec<String>,
}

fn exported_names(program: &Program<'_>) -> Vec<String> {
    let mut names = Vec::new();
    for statement in &program.body {
        match statement {
            Statement::ExportDeclaration(export) => {
                if let Declaration::VariableDeclaration(variables) = &export.declaration {
                    for declarator in &variables.declarations {
                        if let BindingPattern::BindingIdentifier(name) = &declarator.id {
                            names.push(name.name.to_string());
                        }
                    }
                }
            }
            Statement::ExportNamedDeclaration(listed) if !listed.export_kind.is_type() => {
                for specifier in &listed.specifiers {
                    if specifier.export_kind.is_value() {
                        names.push(specifier.local.name().to_string());
                    }
                }
            }
            _ => {}
        }
    }
    names
}

fn library_bindings(program: &Program<'_>) -> Vec<String> {
    let mut bindings = Vec::new();
    for statement in &program.body {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        if import.import_kind.is_type() || import.source.value.as_str() != LIBRARY_MODULE {
            continue;
        }
        for specifier in import.specifiers.iter().flatten() {
            let local = match specifier {
                ImportDeclarationSpecifier::ImportSpecifier(listed) => {
                    if listed.import_kind.is_type() {
                        continue;
                    }
                    &listed.local
                }
                ImportDeclarationSpecifier::ImportDefaultSpecifier(default) => &default.local,
                ImportDeclarationSpecifier::ImportNamespaceSpecifier(all) => &all.local,
            };
            bindings.push(local.name.to_string());
        }
    }
    bindings
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

fn old_form(model: &str, part: &str) -> String {
    match part {
        "action" => format!(
            "{NO_METHODS} `m.action({model}, \"sentence\", {{ when: ..., changes: ... }})`, with \
             `changes` where an action had `then`"
        ),
        "rule" => format!(
            "{NO_METHODS} `m.rule({model}, \"sentence\", (s) => condition)`, with the sentence \
             and the condition as arguments in place of `says` and `holds`"
        ),
        "end" => format!(
            "{NO_METHODS} `m.end({model}, \"sentence\", (s) => condition)`, with the sentence \
             and the condition as arguments in place of `says` and `when`"
        ),
        "view" => format!("{NO_METHODS} `m.view({model}, {{ ... }})`"),
        _ => format!(
            "a model has no methods: its parts are made with `m.action({model}, ...)`, \
             `m.rule({model}, ...)`, `m.end({model}, ...)` and `m.view({model}, ...)`"
        ),
    }
}

impl Reader<'_> {
    pub(crate) fn program(&mut self, program: &Program<'_>) -> Vec<Draft> {
        let mut top = Top {
            exported: exported_names(program),
            ..Top::default()
        };
        for statement in &program.body {
            self.statement(statement, &mut top);
        }
        self.exports(program);
        let mut constants = Vec::new();
        let mut helpers = Vec::new();
        let mut steps: Vec<(usize, String, u32)> = Vec::new();
        for item in top.pending {
            match item {
                Pending::Helper { name, arrow } => {
                    let (parameters, body) = self.function(arrow);
                    helpers.push(HelperDraft {
                        kind: HelperKind::Function,
                        name,
                        parameters,
                        body,
                    });
                }
                Pending::Constant { name, value } => constants.push((name, self.term(value))),
                Pending::Refused { value } => self.refuse_constant(value),
                Pending::Member(member) => {
                    let model = member.model;
                    if let Some(draft) = top.drafts.get_mut(model) {
                        self.step_places.clear();
                        self.member_of(draft, member);
                        steps.extend(
                            self.step_places
                                .drain(..)
                                .map(|(name, offset)| (model, name, offset)),
                        );
                    }
                }
            }
        }
        for (model, name, offset) in steps {
            let known = top
                .drafts
                .get(model)
                .is_some_and(|draft| draft.actions.iter().any(|action| action.id == name));
            if !known {
                self.refuse(
                    offset,
                    &name,
                    "a view's `step` labels an action of the model by its id, and this model has \
                     no action with this id: the chart would leave the label out",
                );
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
            Declaration::ClassDeclaration(_) => ("class", CLASS),
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
            if self.library.as_deref() == Some(name.name.as_str()) {
                self.refuse(
                    declarator.span.start,
                    name.name.as_str(),
                    "this name is the library's: a model file would throw on it, and the checker \
                     would read every `m.` after it as the library",
                );
                continue;
            }
            if self
                .top_names
                .iter()
                .any(|known| known == name.name.as_str())
            {
                self.refuse(
                    name.span.start,
                    name.name.as_str(),
                    "this name is declared twice in the file, and JavaScript refuses a second \
                     `const` of one name, so the file never runs and has no value to read",
                );
                continue;
            }
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
            if self.is_library(root)
                && let Some(kind) = MemberKind::named(part)
            {
                self.library_part(name, kind, call, start, top);
                return;
            }
            if top.owners.iter().any(|(known, _)| known == root) {
                self.refuse(
                    call.span.start,
                    &format!("{root}.{part}"),
                    &old_form(root, part),
                );
                return;
            }
        }
        if self.is_declaration(value) {
            let shape = self.shape(value);
            self.shapes.push((name.to_owned(), shape));
            return;
        }
        if !self.is_plain_value(value) {
            top.pending.push(Pending::Refused { value: init });
            return;
        }
        if let Some(number) = self.whole_number(value) {
            self.numbers.push((name.to_owned(), number));
        }
        self.plain_values.push(name.to_owned());
        top.pending.push(Pending::Constant {
            name: name.to_owned(),
            value: init,
        });
    }

    fn library_part<'p, 'a>(
        &mut self,
        name: &str,
        kind: MemberKind,
        call: &'p CallExpression<'a>,
        start: u32,
        top: &mut Top<'p, 'a>,
    ) {
        let first = call.arguments.first().and_then(Argument::as_expression);
        let owner = first.map(bare).and_then(|written| match written {
            Expression::Identifier(model) => top
                .owners
                .iter()
                .find(|(known, _)| known == model.name.as_str()),
            _ => None,
        });
        let Some((_, model)) = owner else {
            let (offset, feature) = first.map_or_else(
                || (call.span.start, self.quoted(call.callee.span())),
                |written| (written.span().start, self.quoted(written.span())),
            );
            self.refuse(
                offset,
                &feature,
                &format!(
                    "the first argument of `{}` is the model the part belongs to, a model \
                     declared in this file with `m.model`",
                    self.quoted(call.callee.span())
                ),
            );
            return;
        };
        self.queue(
            Member {
                model: *model,
                kind,
                id: name.to_owned(),
                start,
                call,
            },
            top,
        );
    }

    fn queue<'p, 'a>(&mut self, member: Member<'p, 'a>, top: &mut Top<'p, 'a>) {
        let unexported = !top.exported.iter().any(|known| known == &member.id);
        if unexported && !matches!(member.kind, MemberKind::View) {
            self.refuse(
                member.call.span.start,
                &member.id,
                "a rule, an action or an end is an exported constant, because its id is \
                 the name it is exported under: write `export const name = ...`",
            );
            return;
        }
        top.pending.push(Pending::Member(member));
    }

    fn is_plain_value(&self, wrapped: &Expression<'_>) -> bool {
        match bare(wrapped) {
            Expression::BooleanLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::StringLiteral(_) => true,
            Expression::TemplateLiteral(template) => template.expressions.is_empty(),
            Expression::Identifier(name) => self
                .plain_values
                .iter()
                .any(|known| known == name.name.as_str()),
            Expression::UnaryExpression(unary) => self.is_plain_value(&unary.argument),
            Expression::BinaryExpression(binary) => {
                self.is_plain_value(&binary.left) && self.is_plain_value(&binary.right)
            }
            Expression::LogicalExpression(logical) => {
                self.is_plain_value(&logical.left) && self.is_plain_value(&logical.right)
            }
            Expression::ConditionalExpression(choice) => {
                self.is_plain_value(&choice.test)
                    && self.is_plain_value(&choice.consequent)
                    && self.is_plain_value(&choice.alternate)
            }
            _ => false,
        }
    }

    fn refuse_constant(&mut self, value: &Expression<'_>) {
        let before = self.problems.len();
        self.term(value);
        if self.problems.len() == before {
            let written = self.quoted(value.span());
            self.refuse(value.span().start, &written, NOT_A_CONSTANT);
        }
    }

    fn exports(&mut self, program: &Program<'_>) {
        let from_library = library_bindings(program);
        let mut seen: Vec<String> = Vec::new();
        for statement in &program.body {
            match statement {
                Statement::ExportDeclaration(export) => {
                    if let Declaration::VariableDeclaration(variables) = &export.declaration {
                        for declarator in &variables.declarations {
                            if let BindingPattern::BindingIdentifier(name) = &declarator.id {
                                if name.name.as_str() == THEN {
                                    self.refuse(name.span.start, THEN, THEN_EXPORTED);
                                } else {
                                    self.export_once(
                                        &mut seen,
                                        name.name.as_str(),
                                        name.span.start,
                                    );
                                }
                            }
                        }
                    }
                }
                Statement::ExportNamedDeclaration(listed) if !listed.export_kind.is_type() => {
                    for specifier in &listed.specifiers {
                        self.export_specifier(&from_library, &mut seen, specifier);
                    }
                }
                _ => {}
            }
        }
    }

    fn export_specifier(
        &mut self,
        from_library: &[String],
        seen: &mut Vec<String>,
        specifier: &ExportSpecifier<'_>,
    ) {
        let at = specifier.span.start;
        if specifier.exported.name() == THEN && !specifier.export_kind.is_type() {
            self.refuse(at, &self.quoted(specifier.span), THEN_EXPORTED);
            return;
        }
        if specifier.export_kind.is_type() {
            self.refuse(
                at,
                &self.quoted(specifier.span),
                "a `type` modifier erases this export from the JavaScript, so the name is not \
                 exported where the model file runs: write `export type { name }` for a type \
                 and leave the modifier off a value",
            );
            return;
        }
        if from_library
            .iter()
            .any(|known| known == specifier.local.name().as_str())
        {
            self.refuse(
                at,
                &self.quoted(specifier.span),
                "a model file can't pass on the library: a file that imports it from here \
                 would build its models where the checker does not look for them",
            );
            return;
        }
        if specifier.local.name() != specifier.exported.name() {
            self.refuse(
                at,
                &self.quoted(specifier.span),
                "an id is the name of the constant, so a constant is exported under its own \
                 name",
            );
        }
        self.export_once(seen, specifier.exported.name().as_str(), at);
    }

    fn export_once(&mut self, seen: &mut Vec<String>, name: &str, at: u32) {
        if seen.iter().any(|known| known == name) {
            self.refuse(
                at,
                name,
                "this name is exported twice, and JavaScript refuses a second export of one \
                 name with a SyntaxError, so the file never runs and has no value to read",
            );
        } else {
            seen.push(name.to_owned());
        }
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
            language: Language::TypeScript,
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
        self.models.push(constant.to_owned());
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
             export const turn = m.action(Hall, \"turn\", { changes: (s) => ({ level: s.level }) });\n\
             export const flip = m.action(Porch, \"flip\", { changes: (s) => ({ lit: !s.lit }) });\n\
             export const known = m.rule(Hall, \"known\", (s) => s.level < 3);\n",
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
             const stay = lib.end(Inner, \"stay\", (s) => s.on);\n\
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
                 export const go = m.action(M, \"go\", { changes: () => ({ n: Date.now() }) });\n\
                 export const ok = m.rule(M, \"ok\", () => fetch(\"u\") === null);\n\
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

    const CLIMB: &str = "export const Climb = m.model(\"Climb\", { n: m.int(0, 3) });\n";
    const LOW: &str = "\"low\", (s) => s.n <= 1";

    fn climb_features(rest: &str) -> Vec<(u32, String)> {
        features(&format!("{CLIMB}{}", rest.replace("LOW", LOW)))
    }

    #[test]
    fn a_rule_made_any_other_way_than_as_the_whole_value_of_a_constant_is_refused() {
        let not_a_value = |line: u32| feature(line, "`m.rule`");
        assert_eq!(
            climb_features("export const low = true && m.rule(Climb, LOW);\n"),
            [not_a_value(3)],
            "on the right of a logical and"
        );
        assert_eq!(
            climb_features("export const low = true ? m.rule(Climb, LOW) : null;\n"),
            [not_a_value(3)],
            "in a branch of a choice"
        );
        assert_eq!(
            climb_features("export const low = m.rule.call(Climb, LOW);\n"),
            [not_a_value(3), feature(3, "`Climb`")],
            "with `call` on the function: the function is read, and the model is named as a value"
        );
        assert_eq!(
            climb_features("export const all = { low: m.rule(Climb, LOW) };\n"),
            [not_a_value(3)],
            "inside a record"
        );
        assert_eq!(
            climb_features(
                "const same = <T>(part: T) => part;\nexport const low = same(m.rule(Climb, LOW));\n"
            ),
            [not_a_value(4)],
            "passed through a helper function"
        );
        assert_eq!(
            climb_features("const make = () => m.rule(Climb, LOW);\n"),
            [not_a_value(3)],
            "inside a helper function"
        );
        assert_eq!(
            climb_features("export const low = [m.rule(Climb, LOW)];\n"),
            [feature(3, "`[]`")],
            "inside a list, which is refused before the model is read"
        );
    }

    #[test]
    fn a_rule_made_through_another_name_for_the_model_or_the_library_is_refused() {
        let lines = |rest: &str| -> Vec<u32> {
            climb_features(rest)
                .into_iter()
                .map(|(line, _)| line)
                .collect()
        };
        assert_eq!(
            lines("const Alias = Climb;\nexport const low = m.rule(Alias, LOW);\n"),
            [3, 4],
            "the other name, and the rule made through it"
        );
        assert_eq!(
            climb_features("const lib = m;\n"),
            [feature(3, "`m`")],
            "another name for the library"
        );
    }

    #[test]
    fn a_constant_is_a_plain_value_written_out_or_computed_from_earlier_constants() {
        let reading = read(
            "m.model.ts",
            "import { m } from \"@earnest/model\";\n\
             const MAX = 3;\n\
             const LOW = -MAX + 1;\n\
             const NAME = `n`;\n\
             const ON = !false && MAX > 2;\n\
             const PICK = (ON ? MAX : LOW);\n\
             export const A = m.model(\"A\", { n: m.int(LOW, MAX) });\n",
        );
        assert_eq!(reading.problems, [], "every one is a plain value");
        let draft = reading.drafts.first().expect("one model");
        let constants: Vec<(&str, String)> = draft
            .constants
            .iter()
            .map(|(name, term)| (name.as_str(), show(term)))
            .collect();
        assert_eq!(
            constants,
            [
                ("MAX", "3".to_owned()),
                ("LOW", "(+ (- MAX) 1)".to_owned()),
                ("NAME", "\"n\"".to_owned()),
                ("ON", "(&& (! false) (> MAX 2))".to_owned()),
                ("PICK", "(? ON MAX LOW)".to_owned())
            ],
            "the constants of the file"
        );
    }

    #[test]
    fn a_constant_that_holds_anything_else_is_refused_even_when_nothing_reads_it() {
        assert_eq!(
            features("const LATE = AFTER + 1;\nconst AFTER = 1;\n"),
            [feature(2, "`AFTER + 1`")],
            "a constant declared later has no value yet"
        );
        assert_eq!(
            features("const BOX = { n: 1 };\n"),
            [feature(2, "`{ n: 1 }`")],
            "a record"
        );
        assert_eq!(
            features("const NOTHING = null;\n"),
            [feature(2, "`null`")],
            "null"
        );
        assert_eq!(
            features("const twice = (n: number) => n * 2;\nconst FOUR = twice(2);\n"),
            [feature(3, "`twice(2)`")],
            "a call of a helper function"
        );
        assert_eq!(
            features("const NOW = Date.now();\n"),
            [feature(2, "`Date.now`")],
            "what is outside the library keeps its own sentence"
        );
        assert_eq!(
            features("const LOUD = `${1}!`;\n"),
            [feature(2, "`'${1}!'`")],
            "a template string with a hole"
        );
    }

    #[test]
    fn a_constant_is_exported_under_its_own_name() {
        assert_eq!(
            climb_features("const low = m.rule(Climb, LOW);\nexport { low as high };\n"),
            [feature(4, "`low as high`")],
            "the id would be `low` here and `high` for whoever imports the file"
        );
        assert_eq!(
            climb_features("const low = m.rule(Climb, LOW);\nexport { low as low };\n"),
            [],
            "the same name on both sides"
        );
    }

    #[test]
    fn a_part_that_stands_alone_is_refused() {
        let found = climb_features("m.rule(Climb, LOW);\n");
        assert_eq!(found.len(), 1, "one problem: {found:?}");
        assert_eq!(found[0].0, 3, "on the line of the statement");
    }

    #[test]
    fn a_part_written_twice_is_refused_because_only_the_last_would_count() {
        assert_eq!(
            climb_features(
                "export const low = m.action(Climb, \"low\", { changes: () => ({ n: 1 }), \
                 changes: () => ({ n: 2 }) });\n"
            ),
            [feature(3, "`changes`")],
            "the reader would check the first function and the program would run the second"
        );
        assert_eq!(
            climb_features(
                "export const inc = m.action(Climb, \"inc\", { changes: (s) => ({ n: 1, n: s.n }) });\n"
            ),
            [feature(3, "`n`")],
            "a field of a record"
        );
        assert_eq!(
            features(
                "export const Twice = m.model(\"Twice\", { n: m.int(0, 1), n: m.boolean() });\n"
            ),
            [feature(2, "`n`")],
            "a field of a model"
        );
    }
}
