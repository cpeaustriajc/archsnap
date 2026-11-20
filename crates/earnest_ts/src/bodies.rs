use crate::Reader;
use crate::terms::bare;
use earnest_ast::{Term, TermKind};
use oxc_ast::ast::{
    ArrowFunctionBody, ArrowFunctionExpression, BindingPattern, Expression, IfStatement,
    ReturnStatement, Statement, VariableDeclaration, VariableDeclarationKind,
};
use oxc_span::{GetSpan, Span};

const ONLY_IN_A_BODY: &str =
    "a function body has only `const`, `if (...) return ...;` and a last `return`";
const NO_LOOPS: &str =
    "a loop is outside the library; use `m.forall`, `m.exists`, `every`, `some` or `filter`";

fn written_parameters(arrow: &ArrowFunctionExpression<'_>) -> Vec<(String, u32)> {
    let mut names = Vec::with_capacity(arrow.params.items.len());
    for parameter in &arrow.params.items {
        match &parameter.pattern {
            BindingPattern::BindingIdentifier(name) => {
                names.push((name.name.to_string(), name.span.start));
            }
            BindingPattern::ObjectPattern(pattern) => {
                for property in &pattern.properties {
                    if let BindingPattern::BindingIdentifier(local) = &property.value {
                        names.push((local.name.to_string(), local.span.start));
                    }
                }
            }
            _ => {}
        }
    }
    names
}

pub(crate) fn body_span(arrow: &ArrowFunctionExpression<'_>) -> Span {
    arrow.body.span()
}

impl Reader<'_> {
    pub(crate) fn arrow<'p, 'a>(
        &mut self,
        wrapped: &'p Expression<'a>,
    ) -> Option<&'p ArrowFunctionExpression<'a>> {
        let expr = bare(wrapped);
        match expr {
            Expression::ArrowFunctionExpression(arrow) => {
                self.refuse_async(arrow);
                Some(arrow)
            }
            Expression::FunctionExpression(_) => {
                self.refuse(
                    expr.span().start,
                    "function",
                    "write the function as an arrow function",
                );
                None
            }
            _ => {
                let written = self.quoted(expr.span());
                self.refuse(
                    expr.span().start,
                    &written,
                    "an arrow function is needed here",
                );
                None
            }
        }
    }

    pub(crate) fn parameters(&mut self, arrow: &ArrowFunctionExpression<'_>) -> Vec<String> {
        let mut names = Vec::with_capacity(arrow.params.items.len());
        for parameter in &arrow.params.items {
            if let Some(default) = &parameter.initializer {
                self.refuse(
                    default.span().start,
                    "=",
                    "a parameter with a default value is outside the library",
                );
            }
            match &parameter.pattern {
                BindingPattern::BindingIdentifier(name) => names.push(name.name.to_string()),
                other => {
                    let written = self.quoted(other.span());
                    self.refuse(
                        other.span().start,
                        &written,
                        "this parameter is read by one name",
                    );
                }
            }
        }
        if let Some(rest) = &arrow.params.rest {
            self.refuse(
                rest.span.start,
                "...",
                "a rest parameter is outside the library",
            );
        }
        names
    }

    pub(crate) fn function(&mut self, arrow: &ArrowFunctionExpression<'_>) -> (Vec<String>, Term) {
        let parameters = self.parameters(arrow);
        let mark = self.locals.len();
        for name in &parameters {
            self.locals.push((name.clone(), name.clone()));
        }
        let body = self.body(arrow);
        self.locals.truncate(mark);
        (parameters, body)
    }

    pub(crate) fn lambda(&mut self, arrow: &ArrowFunctionExpression<'_>) -> TermKind {
        self.refuse_async(arrow);
        let (parameters, body) = self.function(arrow);
        TermKind::Lambda {
            parameters,
            body: Box::new(body),
        }
    }

    pub(crate) fn body(&mut self, arrow: &ArrowFunctionExpression<'_>) -> Term {
        let parameters = written_parameters(arrow);
        self.refuse_repeats(
            &parameters,
            "this parameter is named twice, and JavaScript refuses an arrow function whose \
             parameters share a name",
        );
        match &arrow.body {
            ArrowFunctionBody::FunctionBody(block) => {
                let own: Vec<String> = parameters.into_iter().map(|(name, _)| name).collect();
                self.block(&block.statements, block.span.end.saturating_sub(1), &own)
            }
            other => match other.as_expression() {
                Some(value) => self.term(value),
                None => self.nothing(other.span().start),
            },
        }
    }

    fn refuse_async(&mut self, arrow: &ArrowFunctionExpression<'_>) {
        if arrow.r#async {
            self.refuse(
                arrow.span.start,
                "async",
                "waiting is outside the library; model the wait with `m.request()`",
            );
        }
    }

    fn nothing(&self, offset: u32) -> Term {
        Term::new(self.place(offset), TermKind::Null)
    }

    pub(crate) fn refuse_repeats(&mut self, names: &[(String, u32)], message: &str) {
        for (position, (name, offset)) in names.iter().enumerate() {
            if names
                .iter()
                .take(position)
                .any(|(earlier, _)| earlier == name)
            {
                self.refuse(*offset, name, message);
            }
        }
    }

    fn block(&mut self, statements: &[Statement<'_>], closing: u32, parameters: &[String]) -> Term {
        let mark = self.locals.len();
        let waiting = self.uninitialised.len();
        let mut declared: Vec<(String, u32)> = Vec::new();
        for statement in statements {
            if let Statement::VariableDeclaration(variables) = statement
                && variables.kind == VariableDeclarationKind::Const
            {
                for declarator in &variables.declarations {
                    if let BindingPattern::BindingIdentifier(name) = &declarator.id {
                        declared.push((name.name.to_string(), name.span.start));
                        self.uninitialised.push((name.name.to_string(), mark));
                    }
                }
            }
        }
        self.refuse_repeats(
            &declared,
            "this name is declared twice in one block, and JavaScript refuses a second `const` \
             of one name",
        );
        for (name, offset) in &declared {
            if parameters.contains(name) {
                self.refuse(
                    *offset,
                    name,
                    "this name is a parameter of the function, and JavaScript refuses a `const` \
                     of the same name in its body",
                );
            }
        }
        let folded = self.rest(statements, closing);
        self.locals.truncate(mark);
        self.uninitialised.truncate(waiting);
        folded
    }

    fn rest(&mut self, statements: &[Statement<'_>], closing: u32) -> Term {
        if self.too_deep(Span::new(closing, closing.saturating_add(1))) {
            return self.nothing(closing);
        }
        self.depth = self.depth.saturating_add(1);
        let folded = self.statements(statements, closing);
        self.depth = self.depth.saturating_sub(1);
        folded
    }

    fn statements(&mut self, statements: &[Statement<'_>], closing: u32) -> Term {
        let Some((first, later)) = statements.split_first() else {
            self.refuse(
                closing,
                "return",
                "a function body ends in a `return` with a value",
            );
            return self.nothing(closing);
        };
        match first {
            Statement::EmptyStatement(_) => self.rest(later, closing),
            Statement::ReturnStatement(returned) => {
                let value = self.returned(returned);
                self.refuse_after_return(later);
                value
            }
            Statement::VariableDeclaration(variables) => self.bound(variables, later, closing),
            Statement::IfStatement(branch) => self.branch(branch, later, closing),
            other => {
                self.refuse_statement(other, ONLY_IN_A_BODY);
                self.rest(later, closing)
            }
        }
    }

    fn refuse_after_return(&mut self, later: &[Statement<'_>]) {
        for unreached in later {
            if !matches!(unreached, Statement::EmptyStatement(_)) {
                let written = self.quoted(unreached.span());
                self.refuse(
                    unreached.span().start,
                    &written,
                    "a function body ends at its `return`, so nothing may follow it",
                );
            }
        }
    }

    fn returned(&mut self, returned: &ReturnStatement<'_>) -> Term {
        if let Some(value) = &returned.argument {
            self.term(value)
        } else {
            self.refuse(
                returned.span.start,
                "return",
                "a function body ends in a `return` with a value",
            );
            self.nothing(returned.span.start)
        }
    }

    fn bound(
        &mut self,
        variables: &VariableDeclaration<'_>,
        later: &[Statement<'_>],
        closing: u32,
    ) -> Term {
        if variables.kind != VariableDeclarationKind::Const {
            self.refuse(
                variables.span.start,
                variables.kind.as_str(),
                "a function declares only `const`, because nothing in a model changes in place",
            );
            return self.rest(later, closing);
        }
        let mut bindings = Vec::with_capacity(variables.declarations.len());
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
            let value = self.term(init);
            let bound = name.name.to_string();
            if let Some(index) = self
                .uninitialised
                .iter()
                .rposition(|(waiting, _)| *waiting == bound)
            {
                self.uninitialised.remove(index);
            }
            self.locals.push((bound.clone(), bound.clone()));
            bindings.push((bound, value, self.place(variables.span.start)));
        }
        let mut body = self.rest(later, closing);
        for (name, value, place) in bindings.into_iter().rev() {
            body = Term::new(
                place,
                TermKind::Bind {
                    name,
                    value: Box::new(value),
                    body: Box::new(body),
                },
            );
        }
        body
    }

    fn branch(&mut self, branch: &IfStatement<'_>, later: &[Statement<'_>], closing: u32) -> Term {
        let test = self.term(&branch.test);
        if let Some(alternate) = &branch.alternate {
            self.refuse(
                alternate.span().start,
                "else",
                "write the other case after the `if`, as the rest of the function body",
            );
        }
        let yes = match &branch.consequent {
            Statement::ReturnStatement(returned) => self.returned(returned),
            Statement::BlockStatement(inner) => {
                self.block(&inner.body, inner.span.end.saturating_sub(1), &[])
            }
            other => {
                self.refuse(
                    other.span().start,
                    "if",
                    "an `if` in a function body returns a value: `if (...) return ...;`",
                );
                self.nothing(other.span().start)
            }
        };
        let no = self.rest(later, closing);
        Term::new(
            self.place(branch.span.start),
            TermKind::Choose(Box::new(test), Box::new(yes), Box::new(no)),
        )
    }

    pub(crate) fn refuse_statement(&mut self, statement: &Statement<'_>, otherwise: &str) {
        let start = statement.span().start;
        let (feature, message) = match statement {
            Statement::ForStatement(_)
            | Statement::ForInStatement(_)
            | Statement::ForOfStatement(_) => ("for", NO_LOOPS),
            Statement::WhileStatement(_) => ("while", NO_LOOPS),
            Statement::DoWhileStatement(_) => ("do", NO_LOOPS),
            Statement::SwitchStatement(_) => ("switch", otherwise),
            Statement::TryStatement(_) => ("try", otherwise),
            Statement::ThrowStatement(_) => ("throw", otherwise),
            Statement::BreakStatement(_) => ("break", otherwise),
            Statement::ContinueStatement(_) => ("continue", otherwise),
            Statement::IfStatement(_) => ("if", otherwise),
            Statement::ReturnStatement(_) => ("return", otherwise),
            Statement::FunctionDeclaration(_) => (
                "function",
                "write the function as an arrow function in a top-level `const`",
            ),
            Statement::ClassDeclaration(_) => ("class", "a class is outside the library"),
            Statement::TSEnumDeclaration(_) => ("enum", "declare the choices with `m.enum([...])`"),
            Statement::VariableDeclaration(variables) => (variables.kind.as_str(), otherwise),
            Statement::ExpressionStatement(alone) => {
                let before = self.problems.len();
                self.term(&alone.expression);
                if self.problems.len() == before {
                    let written = self.quoted(alone.span);
                    self.refuse(start, &written, otherwise);
                }
                return;
            }
            _ => {
                let written = self.quoted(statement.span());
                self.refuse(start, &written, otherwise);
                return;
            }
        };
        self.refuse(start, feature, message);
    }
}

#[cfg(test)]
mod tests {
    use crate::read;
    use crate::show::show;
    use earnest_ast::{Draft, HelperDraft};
    use earnest_diagnostics::{Place, Problem};

    const HEAD: &str = "import { m } from \"@earnest/model\";\n\
        export const M = m.model(\"M\", { n: m.int(0, 3) });\n";

    fn draft(helper: &str) -> Draft {
        let reading = read("m.model.ts", &format!("{HEAD}const helper = {helper};\n"));
        assert_eq!(reading.problems, [], "no problem in: {helper}");
        reading.drafts.into_iter().next().expect("one model")
    }

    fn helper(written: &str) -> HelperDraft {
        draft(written)
            .helpers
            .into_iter()
            .next()
            .expect("one helper")
    }

    fn problems(helper: &str) -> Vec<Problem> {
        read("m.model.ts", &format!("{HEAD}const helper = {helper};\n")).problems
    }

    const NO_FEATURES: [&str; 0] = [];

    fn features(helper: &str) -> Vec<String> {
        problems(helper)
            .into_iter()
            .map(|problem| problem.feature)
            .collect()
    }

    #[test]
    fn a_helper_with_one_expression() {
        let found = helper("(a: number, b: number): boolean => a < b");
        assert_eq!(found.name, "helper", "the constant's name");
        assert_eq!(
            found.parameters,
            ["a", "b"],
            "the parameters, without types"
        );
        assert_eq!(show(&found.body), "(< a b)", "the expression");
    }

    #[test]
    fn a_block_folds_into_binds_and_choices() {
        let found = helper(
            "(n: number) => {\n\tconst next = n + 1, top = 3;\n\tif (next > top) return top;\n\treturn next;\n}",
        );
        assert_eq!(
            show(&found.body),
            "(let next = (+ n 1) in (let top = 3 in (? (> next top) top next)))",
            "two bindings, an early return and the last return"
        );
        assert_eq!(
            found.body.place,
            Place { line: 4, column: 2 },
            "a binding is placed at its `const`"
        );
    }

    #[test]
    fn an_if_with_a_block_returns_from_the_block() {
        let found =
            helper("(n: number) => { if (n > 1) { const half = n - 1; return half; } return n; }");
        assert_eq!(
            show(&found.body),
            "(? (> n 1) (let half = (- n 1) in half) n)",
            "the block's binding stays inside the first case"
        );
    }

    #[test]
    fn a_lambda_may_have_a_block() {
        let found = helper("(n: number) => m.setOf(n).some((x) => { return x === n; })");
        assert_eq!(
            show(&found.body),
            "set(n).some((x) => (=== x n))",
            "the block is folded"
        );
    }

    #[test]
    fn statements_outside_the_library_are_named() {
        assert_eq!(
            features("(n: number) => { let x = n; return x; }"),
            ["`let`"],
            "let"
        );
        assert_eq!(
            features("(n: number) => { var x = n; return x; }"),
            ["`var`"],
            "var"
        );
        assert_eq!(
            features("(n: number) => { for (const x of m.setOf(n)) return x; return n; }"),
            ["`for`"],
            "for of"
        );
        assert_eq!(
            features("(n: number) => { for (;;) { return n; } }"),
            ["`for`", "`return`"],
            "a loop is not a return"
        );
        assert_eq!(
            features("(n: number) => { while (n > 0) return n; return 0; }"),
            ["`while`"],
            "while"
        );
        assert_eq!(
            features("(n: number) => { if (n > 0) return 1; else return 2; }"),
            ["`else`", "`return`"],
            "else, and then no last return"
        );
        assert_eq!(
            features("(n: number) => { n + 1; return n; }"),
            ["`n + 1;`"],
            "a bare value"
        );
        assert_eq!(
            features("(n: number) => { return; }"),
            ["`return`"],
            "an empty return"
        );
        assert_eq!(
            features("(n: number) => { throw n; }"),
            ["`throw`", "`return`"],
            "throw"
        );
        assert_eq!(
            features("({ n }: { n: number }) => n"),
            ["`{ n }`"],
            "a pattern"
        );
        assert_eq!(features("(n = 1) => n"), ["`=`"], "a default");
        assert_eq!(
            features("(...n: number[]) => n"),
            ["`...`"],
            "a rest parameter"
        );
        assert_eq!(features("async (n: number) => n"), ["`async`"], "async");
    }

    #[test]
    fn a_body_without_a_return_is_reported_at_its_closing_brace() {
        let found = problems("(n: number) => {\n\tconst x = n;\n}");
        let only = found.first().expect("one problem");
        assert_eq!(only.feature, "`return`", "the missing statement");
        assert_eq!((only.line, only.column), (5, 1), "the closing brace");
    }

    #[test]
    fn a_statement_after_the_last_return_is_refused() {
        assert_eq!(
            features("(n: number) => { return n; n + 1; }"),
            ["`n + 1;`"],
            "an expression that is never reached"
        );
        assert_eq!(
            features("(n: number) => { if (n > 1) { return 1; const a = 2; } return n; }"),
            ["`const a = 2;`"],
            "inside the block of an `if` too"
        );
        assert_eq!(
            features("(n: number) => { return n; return 2; }"),
            ["`return 2;`"],
            "a second return"
        );
        assert_eq!(
            features("(n: number) => { return n;; }"),
            NO_FEATURES,
            "an empty statement is nothing"
        );
    }
}
