use crate::Reader;
use crate::members::MemberKind;
use crate::names::{DECLARATIONS, snake, why_outside};
use crate::shapes::WRITTEN_TWICE;
use earnest_ast::{Binary, HelperKind, Reach, TemplatePiece, Term, TermKind, Unary};
use earnest_value::MOST_SAFE_INTEGER;
use oxc_ast::ast::{
    Argument, BinaryExpression, CallExpression, Expression, LogicalExpression, ObjectExpression,
    ObjectPropertyKind, StaticMemberExpression, TemplateLiteral, UnaryExpression,
};
use oxc_span::GetSpan;
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, LogicalOperator, UnaryOperator};

const NOT_AN_OPERATOR: &str = "this operator is outside the library: the operators are `===`, `!==`, `<`, `<=`, `>`, `>=`, `&&`, `||`, `!`, `+`, `-`, `*`, `m.rem(a, b)` and `m.div(a, b)`";
pub(crate) const CLASS: &str = "a class is outside the library: declare the data as an `m.record({...})` and the behavior as the actions and rules of an `m.model`";
const NOT_IN_PLACE: &str = "changing a value in place is outside the library; every operation gives back a new value: write `const next = value + 1;`, or return the new field from `changes`";

const AN_ASSERTION: &str = "a type assertion hides a `null` from the type checker and changes \
    nothing in JavaScript, so a value that is optional would be read as `null` where \
    TypeScript saw a number: write `m.some(value)` for the value inside an optional, and \
    only `as const` is read";

pub(crate) fn bare<'p, 'a>(expr: &'p Expression<'a>) -> &'p Expression<'a> {
    let mut current = expr;
    loop {
        current = match current {
            Expression::ParenthesizedExpression(inner) => &inner.expression,
            Expression::TSAsExpression(inner)
                if inner.type_annotation.is_const_type_reference() =>
            {
                &inner.expression
            }
            _ => return current,
        };
    }
}

pub(crate) fn whole(value: f64) -> Option<i64> {
    let number: i64 = format!("{value}").parse().ok()?;
    (number.unsigned_abs() <= MOST_SAFE_INTEGER.unsigned_abs()).then_some(number)
}

impl Reader<'_> {
    pub(crate) fn term(&mut self, wrapped: &Expression<'_>) -> Term {
        let expr = bare(wrapped);
        let start = expr.span().start;
        if self.too_deep(expr.span()) {
            return Term::new(self.place(start), TermKind::Null);
        }
        self.depth = self.depth.saturating_add(1);
        let kind = self.term_kind(expr, start);
        self.depth = self.depth.saturating_sub(1);
        Term::new(self.place(start), kind)
    }

    fn term_kind(&mut self, expr: &Expression<'_>, start: u32) -> TermKind {
        match expr {
            Expression::BooleanLiteral(literal) => TermKind::Bool(literal.value),
            Expression::NullLiteral(_) => TermKind::Null,
            Expression::NumericLiteral(literal) => self.number(literal.value, expr),
            Expression::StringLiteral(literal) => TermKind::Text(literal.value.to_string()),
            Expression::TemplateLiteral(template) => self.template(template),
            Expression::Identifier(name) => self.name(name.name.as_str(), start),
            Expression::UnaryExpression(unary) => self.unary(unary),
            Expression::BinaryExpression(binary) => self.binary(binary),
            Expression::LogicalExpression(logical) => self.logical(logical),
            Expression::ConditionalExpression(choice) => TermKind::Choose(
                Box::new(self.term(&choice.test)),
                Box::new(self.term(&choice.consequent)),
                Box::new(self.term(&choice.alternate)),
            ),
            Expression::ObjectExpression(object) => self.record(object),
            Expression::CallExpression(call) => self.call(call),
            Expression::StaticMemberExpression(member) => self.member(member),
            Expression::ArrowFunctionExpression(arrow) => self.lambda(arrow),
            _ => self.outside(expr),
        }
    }

    pub(crate) fn local(&self, name: &str) -> Option<&str> {
        self.locals
            .iter()
            .rev()
            .find(|(written, _)| written == name)
            .map(|(_, emitted)| emitted.as_str())
    }

    pub(crate) fn is_library(&self, name: &str) -> bool {
        self.local(name).is_none() && self.library.as_deref() == Some(name)
    }

    fn is_read_early(&self, written: &str) -> bool {
        let Some((_, mark)) = self
            .uninitialised
            .iter()
            .rev()
            .find(|(waiting, _)| waiting == written)
        else {
            return false;
        };
        self.locals
            .iter()
            .rposition(|(source, _)| source == written)
            .is_none_or(|index| index < *mark)
    }

    fn name(&mut self, written: &str, offset: u32) -> TermKind {
        if self.is_read_early(written) {
            return self.refused(
                offset,
                written,
                &format!(
                    "`{written}` is declared later in this block, so reading it here is an \
                     error in JavaScript, never the constant of the same name outside the \
                     block: move the declaration above its first use"
                ),
            );
        }
        let found = self
            .locals
            .iter()
            .rposition(|(source, _)| source == written);
        let emitted = found
            .and_then(|index| self.locals.get(index))
            .map_or(written, |(_, shared)| shared.as_str())
            .to_owned();
        let inner = found.map_or(0, |index| index + 1);
        let hidden = self
            .locals
            .iter()
            .skip(inner)
            .any(|(_, shared)| *shared == emitted);
        if found.is_none() && self.models.iter().any(|model| model == written) {
            return self.refused(
                offset,
                written,
                "a model is not a value: its actions, rules, ends and view are made at the top \
                 level with the model as the first argument, as in `export const name = \
                 m.rule(Model, \"sentence\", (s) => ...)`",
            );
        }
        if hidden {
            return self.refused(
                offset,
                written,
                &format!(
                    "`{emitted}` already names another value in this function; \
                     give one of them a different name"
                ),
            );
        }
        TermKind::Name(emitted)
    }

    fn is_defined(&self, name: &str) -> bool {
        self.local(name).is_some() || self.top_names.iter().any(|known| known == name)
    }

    fn refused(&mut self, offset: u32, feature: &str, message: &str) -> TermKind {
        self.refuse(offset, feature, message);
        TermKind::Null
    }

    fn outside(&mut self, expr: &Expression<'_>) -> TermKind {
        let start = expr.span().start;
        let (feature, message) = match expr {
            Expression::AwaitExpression(awaited) => {
                self.term(&awaited.argument);
                (
                    "await",
                    "waiting is outside the library; model the wait with `m.request()`",
                )
            }
            Expression::NewExpression(_) => (
                "new",
                "making an object is outside the library: write an object literal, `{ field: value }`, or a library value such as `m.setOf(a, b)`",
            ),
            Expression::AssignmentExpression(assignment) => (
                assignment.operator.as_str(),
                if assignment.operator == AssignmentOperator::Remainder {
                    "changing a value in place is outside the library; write `m.rem(a, b)`"
                } else if assignment.operator == AssignmentOperator::Division {
                    "changing a value in place is outside the library; write `m.div(a, b)`"
                } else {
                    NOT_IN_PLACE
                },
            ),
            Expression::UpdateExpression(update) => (update.operator.as_str(), NOT_IN_PLACE),
            Expression::ArrayExpression(_) => (
                "[]",
                "a list is written with `m.seqOf(...)` or `m.setOf(...)`",
            ),
            Expression::ComputedMemberExpression(_) => (
                "[]",
                "reading by index is outside the library; use `get(i)`",
            ),
            Expression::FunctionExpression(_) => {
                ("function", "write the function as an arrow function")
            }
            Expression::ClassExpression(_) => ("class", CLASS),
            Expression::ThisExpression(_) => (
                "this",
                "`this` is outside the library: take the state as the first parameter, `(s) => s.field`",
            ),
            Expression::TSNonNullExpression(inner) => {
                self.term(&inner.expression);
                (
                    "!",
                    "a postfix `!` is a fault in the checker where the value is null, and nothing \
                     in JavaScript: write `m.some(value)`, which gives the value and is the same \
                     fault in Node",
                )
            }
            Expression::TSAsExpression(inner) => {
                self.term(&inner.expression);
                ("as", AN_ASSERTION)
            }
            Expression::TSSatisfiesExpression(inner) => {
                self.term(&inner.expression);
                ("satisfies", AN_ASSERTION)
            }
            Expression::TSTypeAssertion(inner) => {
                self.term(&inner.expression);
                ("<type>", AN_ASSERTION)
            }
            Expression::ChainExpression(_) => (
                "?.",
                "optional chaining is outside the library; compare with `null` first",
            ),
            Expression::YieldExpression(_) => (
                "yield",
                "a generator is outside the library: write a loop as `every`, `some` or `filter`, or as `m.forall` and `m.exists`",
            ),
            _ => {
                let written = self.quoted(expr.span());
                return self.refused(start, &written, "this is outside the library: a model file may contain only the forms \"What a model file may contain\" lists in the README");
            }
        };
        self.refused(start, feature, message)
    }

    fn number(&mut self, value: f64, expr: &Expression<'_>) -> TermKind {
        if let Some(number) = whole(value) {
            TermKind::Int(number)
        } else {
            let written = self.quoted(expr.span());
            self.refused(
                expr.span().start,
                &written,
                "a number in a model is a whole number from -9007199254740991 to 9007199254740991, as far \
                 as a JavaScript number is exact",
            )
        }
    }

    fn template(&mut self, template: &TemplateLiteral<'_>) -> TermKind {
        let mut pieces = Vec::new();
        let mut holes = template.expressions.iter();
        for quasi in &template.quasis {
            let text = quasi
                .value
                .cooked
                .as_ref()
                .map_or_else(|| quasi.value.raw.to_string(), ToString::to_string);
            if !text.is_empty() {
                pieces.push(TemplatePiece::Text(text));
            }
            if let Some(hole) = holes.next() {
                pieces.push(TemplatePiece::Value(self.term(hole)));
            }
        }
        if template.expressions.is_empty() {
            let text = match pieces.pop() {
                Some(TemplatePiece::Text(only)) => only,
                _ => String::new(),
            };
            return TermKind::Text(text);
        }
        TermKind::Template(pieces)
    }

    fn unary(&mut self, unary: &UnaryExpression<'_>) -> TermKind {
        match unary.operator {
            UnaryOperator::LogicalNot => {
                TermKind::Unary(Unary::Not, Box::new(self.term(&unary.argument)))
            }
            UnaryOperator::UnaryNegation => {
                if let Expression::NumericLiteral(literal) = bare(&unary.argument)
                    && let Some(number) = whole(literal.value)
                {
                    return TermKind::Int(-number);
                }
                TermKind::Unary(Unary::Negate, Box::new(self.term(&unary.argument)))
            }
            other => {
                self.term(&unary.argument);
                self.refused(unary.span.start, other.as_str(), NOT_AN_OPERATOR)
            }
        }
    }

    fn binary(&mut self, binary: &BinaryExpression<'_>) -> TermKind {
        let left = self.term(&binary.left);
        let right = self.term(&binary.right);
        let operator = match binary.operator {
            BinaryOperator::StrictEquality => Binary::Equal,
            BinaryOperator::StrictInequality => Binary::Differ,
            BinaryOperator::LessThan => Binary::Less,
            BinaryOperator::LessEqualThan => Binary::LessOrEqual,
            BinaryOperator::GreaterThan => Binary::Greater,
            BinaryOperator::GreaterEqualThan => Binary::GreaterOrEqual,
            BinaryOperator::Addition => Binary::Add,
            BinaryOperator::Subtraction => Binary::Subtract,
            BinaryOperator::Multiplication => Binary::Multiply,
            BinaryOperator::Remainder => {
                return self.refused(
                    binary.span.start,
                    "%",
                    "JavaScript's `%` can't be checked for a negative number: write `m.rem(a, b)`",
                );
            }
            BinaryOperator::Division => {
                return self.refused(
                    binary.span.start,
                    "/",
                    "JavaScript's `/` gives a fraction, where Rust's `/` on integers drops it, and \
                     the languages disagree for a negative number: write `m.div(a, b)`, the whole \
                     number of times `b` goes into `a`",
                );
            }
            BinaryOperator::Equality | BinaryOperator::Inequality => {
                return self.refused(
                    binary.span.start,
                    binary.operator.as_str(),
                    "compare with `===` or `!==`",
                );
            }
            other => return self.refused(binary.span.start, other.as_str(), NOT_AN_OPERATOR),
        };
        TermKind::Binary(operator, Box::new(left), Box::new(right))
    }

    fn logical(&mut self, logical: &LogicalExpression<'_>) -> TermKind {
        let left = self.term(&logical.left);
        let right = self.term(&logical.right);
        let operator = match logical.operator {
            LogicalOperator::And => Binary::And,
            LogicalOperator::Or => Binary::Or,
            LogicalOperator::Coalesce => {
                return self.refused(
                    logical.span.start,
                    "??",
                    "this operator is outside the library; compare with `null` and use `? :`",
                );
            }
        };
        TermKind::Binary(operator, Box::new(left), Box::new(right))
    }

    fn record(&mut self, object: &ObjectExpression<'_>) -> TermKind {
        let mut base = None;
        let mut fields = Vec::new();
        for property in &object.properties {
            match property {
                ObjectPropertyKind::SpreadProperty(spread) => {
                    let spread_in = self.term(&spread.argument);
                    if base.is_some() || !fields.is_empty() {
                        self.refuse(
                            spread.span.start,
                            "...",
                            "a record takes one spread, before the fields it changes",
                        );
                    }
                    base = Some(Box::new(spread_in));
                }
                ObjectPropertyKind::ObjectProperty(written) => {
                    let name = written.key.static_name();
                    let plain = !written.computed && !written.method;
                    match name {
                        Some(key) if plain => {
                            if fields.iter().any(|(earlier, _)| *earlier == key) {
                                self.refuse(written.span.start, &key, WRITTEN_TWICE);
                            }
                            fields.push((key.into_owned(), self.term(&written.value)));
                        }
                        _ => {
                            let feature = self.quoted(written.key.span());
                            self.refuse(
                                written.span.start,
                                &feature,
                                "a record field is written as `name: value`",
                            );
                        }
                    }
                }
            }
        }
        TermKind::Record {
            owner: None,
            base,
            fields,
        }
    }

    fn member(&mut self, member: &StaticMemberExpression<'_>) -> TermKind {
        let property = member.property.name.as_str();
        if member.optional {
            return self.refused(member.span.start, "?.", "compare with `null` first");
        }
        if let Expression::Identifier(root) = bare(&member.object) {
            let name = root.name.as_str();
            if self.is_library(name) {
                return self.refused(
                    member.span.start,
                    &format!("{name}.{property}"),
                    "the library's parts are called, not read",
                );
            }
            if !self.is_defined(name) {
                return self.refused(
                    member.span.start,
                    &format!("{name}.{property}"),
                    why_outside(name, Some(property)),
                );
            }
        }
        TermKind::Member(Box::new(self.term(&member.object)), property.to_owned())
    }

    fn call(&mut self, call: &CallExpression<'_>) -> TermKind {
        let start = call.span.start;
        if call.optional {
            return self.refused(start, "?.", "compare with `null` first");
        }
        match bare(&call.callee) {
            Expression::Identifier(callee) => {
                let name = callee.name.as_str();
                if self.is_read_early(name) {
                    return self.refused(
                        start,
                        name,
                        "this name is declared later in this block, so calling it here is an \
                         error in JavaScript: move the declaration above its first use",
                    );
                }
                if self.local(name).is_some() {
                    return self.refused(
                        start,
                        name,
                        "only a primitive or a helper function of the model file can be called",
                    );
                }
                if !self.is_defined(name) {
                    return self.refused(start, name, why_outside(name, None));
                }
                TermKind::Helper {
                    kind: HelperKind::Function,
                    name: name.to_owned(),
                    arguments: self.arguments(call),
                }
            }
            Expression::StaticMemberExpression(member) => self.method(member, call),
            other => {
                let written = self.quoted(other.span());
                self.refused(
                    start,
                    &written,
                    "only a primitive or a helper function of the model file can be called",
                )
            }
        }
    }

    fn method(
        &mut self,
        member: &StaticMemberExpression<'_>,
        call: &CallExpression<'_>,
    ) -> TermKind {
        let start = call.span.start;
        let property = member.property.name.as_str();
        if member.optional {
            return self.refused(start, "?.", "compare with `null` first");
        }
        if let Expression::Identifier(root) = bare(&member.object) {
            let name = root.name.as_str();
            if self.is_library(name) {
                return self.library_call(name, property, call);
            }
            if !self.is_defined(name) {
                return self.refused(
                    start,
                    &format!("{name}.{property}"),
                    why_outside(name, Some(property)),
                );
            }
        }
        TermKind::Call {
            target: Box::new(self.term(&member.object)),
            method: snake(property),
            arguments: self.arguments(call),
        }
    }

    fn arguments(&mut self, call: &CallExpression<'_>) -> Vec<Term> {
        let mut terms = Vec::with_capacity(call.arguments.len());
        for argument in &call.arguments {
            match argument.as_expression() {
                Some(value) => terms.push(self.term(value)),
                None => self.refuse(
                    argument.span().start,
                    "...",
                    "a spread in a call is outside the library; pass each value",
                ),
            }
        }
        terms
    }

    fn library_call(&mut self, library: &str, part: &str, call: &CallExpression<'_>) -> TermKind {
        match part {
            "setOf" => TermKind::SetOf(self.arguments(call)),
            "seqOf" => TermKind::SeqOf(self.arguments(call)),
            "mapOf" => self.map_of(call),
            "some" => match self.arguments(call).as_slice() {
                [value] => TermKind::Present(Box::new(value.clone())),
                _ => self.refused(
                    call.span.start,
                    &format!("{library}.some"),
                    "`m.some` takes one value, an optional: `m.some(s.chosen)`",
                ),
            },
            "rem" => match self.arguments(call).as_slice() {
                [left, right] => TermKind::Binary(
                    Binary::Remainder,
                    Box::new(left.clone()),
                    Box::new(right.clone()),
                ),
                _ => self.refused(
                    call.span.start,
                    &format!("{library}.rem"),
                    "`m.rem` takes the number and the divisor: `m.rem(a, b)`",
                ),
            },
            "div" => match self.arguments(call).as_slice() {
                [left, right] => TermKind::Binary(
                    Binary::Divide,
                    Box::new(left.clone()),
                    Box::new(right.clone()),
                ),
                _ => self.refused(
                    call.span.start,
                    &format!("{library}.div"),
                    "`m.div` takes the number and the divisor: `m.div(a, b)`",
                ),
            },
            _ if MemberKind::named(part).is_some() => self.refused(
                call.span.start,
                &format!("{library}.{part}"),
                "an action, a rule, an end or a view is the whole value of a top-level `const`: \
                 it can't be made in a function, a condition, a record or a list",
            ),
            "forall" => self.across(Reach::Every, call),
            "exists" => self.across(Reach::Some, call),
            _ => {
                let message = if DECLARATIONS.contains(&part) || part == "model" {
                    "a declaration is not a value; it belongs in `m.model`, in an `input` or \
                     in `m.forall` and `m.exists`"
                } else {
                    "the library has no part with this name"
                };
                self.refused(call.span.start, &format!("{library}.{part}"), message)
            }
        }
    }

    fn map_of(&mut self, call: &CallExpression<'_>) -> TermKind {
        let mut entries = Vec::with_capacity(call.arguments.len());
        for argument in &call.arguments {
            let pair = match argument.as_expression().map(bare) {
                Some(Expression::ArrayExpression(pair)) if pair.elements.len() == 2 => pair,
                _ => {
                    let written = self.quoted(argument.span());
                    self.refuse(
                        argument.span().start,
                        &written,
                        "each entry of `m.mapOf` is a pair: `[key, value]`",
                    );
                    continue;
                }
            };
            let key = pair.elements.first().and_then(|part| part.as_expression());
            let value = pair.elements.get(1).and_then(|part| part.as_expression());
            if let (Some(key_expr), Some(value_expr)) = (key, value) {
                entries.push((self.term(key_expr), self.term(value_expr)));
            } else {
                self.refuse(
                    pair.span.start,
                    "...",
                    "each entry of `m.mapOf` is a pair: `[key, value]`",
                );
            }
        }
        TermKind::MapOf(entries)
    }

    fn across(&mut self, reach: Reach, call: &CallExpression<'_>) -> TermKind {
        let over = call.arguments.first().and_then(Argument::as_expression);
        let test = call.arguments.get(1).and_then(Argument::as_expression);
        let (Some(over_expr), Some(test_expr), 2) = (over, test, call.arguments.len()) else {
            let written = self.quoted(call.callee.span());
            return self.refused(
                call.span.start,
                &written,
                "it takes a declaration or a collection, then a function of one value",
            );
        };
        if !self.is_declaration(over_expr) {
            let method = match reach {
                Reach::Some => "some",
                _ => "every",
            };
            return TermKind::Call {
                target: Box::new(self.term(over_expr)),
                method: method.to_owned(),
                arguments: vec![self.term(test_expr)],
            };
        }
        let shape = self.shape(over_expr);
        let Some(arrow) = self.arrow(test_expr) else {
            return TermKind::Null;
        };
        let (parameters, body) = self.function(arrow);
        let mut names = parameters.into_iter();
        let (Some(parameter), None) = (names.next(), names.next()) else {
            return self.refused(
                arrow.span.start,
                &self.quoted(arrow.params.span),
                "the function takes one value of the declaration",
            );
        };
        TermKind::Across {
            reach,
            shape: Box::new(shape),
            parameter,
            body: Box::new(body),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::read;
    use crate::show::show;
    use earnest_ast::Draft;
    use earnest_diagnostics::Place;

    const HEAD: &str = "import { m } from \"@earnest/model\";\n\
        const Kind = m.enum([\"a\", \"b\"]);\n\
        const twice = (n: number) => n * 2;\n\
        export const M = m.model(\"M\", { n: m.int(0, 3), box: m.record({ isFull: m.boolean() }), \
        seen: m.set(Kind), tray: m.queue(Kind, { capacity: 2 }) });\n";

    fn draft(rule: &str) -> Draft {
        let source = format!("{HEAD}export const r = m.rule(M, \"r\", {rule});\n");
        let reading = read("m.model.ts", &source);
        assert_eq!(reading.problems, [], "no problem in: {rule}");
        reading.drafts.into_iter().next().expect("one model")
    }

    fn holds(rule: &str) -> String {
        show(&draft(rule).rules.first().expect("one rule").holds)
    }

    fn features(rule: &str) -> Vec<String> {
        let source = format!("{HEAD}export const r = m.rule(M, \"r\", {rule});\n");
        read("m.model.ts", &source)
            .problems
            .into_iter()
            .map(|problem| {
                assert_eq!(problem.line, 5, "the problem is on the rule's line");
                problem.feature
            })
            .collect()
    }

    #[test]
    fn literals_and_names() {
        assert_eq!(holds("() => true"), "true", "a boolean");
        assert_eq!(holds("() => null"), "null", "null");
        assert_eq!(holds("() => 12"), "12", "a number");
        assert_eq!(holds("() => \"a\""), "\"a\"", "text");
        assert_eq!(
            holds("() => `plain`"),
            "\"plain\"",
            "a template without holes"
        );
        assert_eq!(holds("(s) => s"), "s", "a name");
    }

    #[test]
    fn a_minus_sign_on_a_number_is_a_negative_number() {
        assert_eq!(holds("() => -1"), "-1", "a literal");
        assert_eq!(holds("(s) => -s.n"), "(- s.n)", "a value");
    }

    #[test]
    fn every_allowed_operator() {
        assert_eq!(
            holds("(s) => s.n === 1 && s.n !== 2 || !(s.n < 3)"),
            "(|| (&& (=== s.n 1) (!== s.n 2)) (! (< s.n 3)))",
            "equality, logic and not"
        );
        assert_eq!(
            holds("(s) => s.n <= 1 && s.n > 2 && s.n >= 3"),
            "(&& (&& (<= s.n 1) (> s.n 2)) (>= s.n 3))",
            "comparison"
        );
        assert_eq!(
            holds("(s) => m.rem((s.n + 1 - 2) * 3, 4)"),
            "(% (* (- (+ s.n 1) 2) 3) 4)",
            "arithmetic"
        );
    }

    #[test]
    fn a_choice_and_a_non_null_mark() {
        assert_eq!(
            holds("(s) => s.n === 0 ? m.some(s.box).isFull : false"),
            "(? (=== s.n 0) present(s.box).isFull false)",
            "`? :` and `m.some` of a value"
        );
    }

    #[test]
    fn as_const_is_dropped_and_the_other_type_marks_are_refused() {
        assert_eq!(
            holds("(s) => s.n === (1 as const)"),
            "(=== s.n 1)",
            "`as const`"
        );
        for rule in [
            "(s) => (s.n as number) === 1",
            "(s) => s.n === (1 satisfies number)",
            "(s) => (<number>s.n) < 3",
            "(s) => (s.o as number) < 3",
            "(s) => (s.o as unknown as number) < 3",
            "(s) => ((s.o as number)) < 3",
            "(s) => s.o satisfies number < 3",
            "(s) => m.some(s.o as number) < 3",
            "(s) => (s.n as any) === 1",
        ] {
            let found = features(rule);
            assert!(!found.is_empty(), "`{rule}` hides a null from the checker");
        }
        let sentence = read(
            "m.model.ts",
            &format!("{HEAD}export const r = m.rule(M, \"r\", (s) => (s.o as number) < 3);\n"),
        )
        .problems;
        assert!(
            sentence
                .iter()
                .any(|problem| problem.message.contains("m.some(value)")),
            "the sentence names `m.some(value)`: {sentence:?}"
        );
    }

    #[test]
    fn an_assertion_on_a_constant_a_declaration_or_a_helper_is_refused() {
        for source in [
            "const LIMIT = 3 as number;\n",
            "const LIMIT = 3 satisfies number;\n",
            "const SAME = ((n) => n) as (n: number) => number;\n",
            "export const N = m.model(\"N\", { n: m.int(0, 3) as any });\n",
            "export const O = m.model(\"O\", { n: m.enum([\"a\", \"b\"] as const) });\n",
        ] {
            let found = read("m.model.ts", &format!("{HEAD}{source}")).problems;
            assert_eq!(
                found.is_empty(),
                source.contains("as const"),
                "`{source}`: only `as const` is read: {found:?}"
            );
        }
    }

    #[test]
    fn a_record_with_a_spread_and_a_shorthand() {
        assert_eq!(
            holds("(s) => ({ ...s.box, isFull: true, \"n\": 1, s })"),
            "{...s.box, isFull: true, n: 1, s: s}",
            "spread, fields and shorthand"
        );
    }

    #[test]
    fn a_primitive_call_is_in_snake_case() {
        assert_eq!(
            holds("(s) => s.seen.isSubset(s.seen)"),
            "s.seen.is_subset(s.seen)",
            "a method"
        );
        assert_eq!(
            holds("(s) => s.tray.receive()"),
            "s.tray.receive()",
            "no arguments"
        );
    }

    #[test]
    fn a_primitive_property_is_in_snake_case_unless_the_file_declares_the_name() {
        assert_eq!(
            holds("(s) => s.seen.isEmpty"),
            "s.seen.isEmpty",
            "a primitive's"
        );
        assert_eq!(
            holds("(s) => s.box.isFull"),
            "s.box.isFull",
            "a record field's"
        );
        assert_eq!(holds("(s) => s.seen.size"), "s.seen.size", "one word");
    }

    #[test]
    fn a_helper_call() {
        assert_eq!(
            holds("(s) => twice(s.n) === 2"),
            "(=== twice(s.n) 2)",
            "a helper"
        );
    }

    #[test]
    fn collection_values() {
        assert_eq!(
            holds("() => m.setOf(\"a\", \"b\")"),
            "set(\"a\", \"b\")",
            "a set"
        );
        assert_eq!(holds("() => m.seqOf(1)"), "seq(1)", "a sequence");
        assert_eq!(
            holds("() => m.mapOf([\"a\", 1], [\"b\", 2])"),
            "map(\"a\" -> 1, \"b\" -> 2)",
            "a map"
        );
    }

    #[test]
    fn forall_and_exists_over_a_declaration() {
        assert_eq!(
            holds("(s) => m.forall(Kind, (k) => s.seen.has(k))"),
            "(forall k in enum(\"a\", \"b\"): s.seen.has(k))",
            "a named declaration"
        );
        assert_eq!(
            holds("() => m.exists(m.int(0, 1), (i) => i === 0)"),
            "(exists i in int(0, 1): (=== i 0))",
            "a declaration written in place"
        );
    }

    #[test]
    fn forall_and_exists_over_a_collection() {
        assert_eq!(
            holds("(s) => m.forall(s.seen, (k) => k === \"a\")"),
            "s.seen.every((k) => (=== k \"a\"))",
            "forall is every"
        );
        assert_eq!(
            holds("(s) => m.exists(s.seen, (k) => k === \"a\")"),
            "s.seen.some((k) => (=== k \"a\"))",
            "exists is some"
        );
    }

    #[test]
    fn a_parameter_hides_a_declaration_of_the_same_name() {
        assert_eq!(
            holds("(Kind) => m.forall(Kind.seen, (k) => k === \"a\")"),
            "Kind.seen.every((k) => (=== k \"a\"))",
            "the state is named like the declaration"
        );
    }

    #[test]
    fn a_name_that_would_be_read_as_another_value_is_refused() {
        let source = format!(
            "{HEAD}const by = 1;\nexport const a = m.action(M, \"a\", {{ input: {{ by: Kind }}, \
             when: (s) => s.n < 3, \
             changes: (state, {{ by: who }}) => ({{ n: m.setOf(1).some((s) => s === state.n) ? 1 : 2, \
             seen: m.setOf(who).filter((by) => by === who) }}) }});\n"
        );
        let found: Vec<(String, String)> = read("m.model.ts", &source)
            .problems
            .into_iter()
            .map(|problem| (problem.feature, problem.message))
            .collect();
        let sentence = |name: &str| {
            format!(
                "`{name}` already names another value in this function; \
                 give one of them a different name"
            )
        };
        assert_eq!(
            found,
            [("`who`".to_owned(), sentence("by"))],
            "an input under the name of a parameter; `changes` reads the state by its own name"
        );
    }

    #[test]
    fn a_term_has_the_place_it_starts_at() {
        let found = draft("(s) =>\n\t\ts.n === 1");
        let rule = found.rules.first().expect("one rule");
        assert_eq!(
            rule.holds.place,
            Place { line: 6, column: 3 },
            "the comparison"
        );
    }

    #[test]
    fn features_outside_the_library_are_named() {
        assert_eq!(
            features("() => fetch(\"u\") !== null"),
            ["`fetch`"],
            "a global function"
        );
        assert_eq!(
            features("() => Date.now() > 0"),
            ["`Date.now`"],
            "a global method"
        );
        assert_eq!(
            features("() => Math.PI > 0"),
            ["`Math.PI`"],
            "a global property"
        );
        assert_eq!(features("() => new Date() !== null"), ["`new`"], "new");
        assert_eq!(
            features("async () => await 1"),
            ["`async`", "`await`"],
            "async and await"
        );
        assert_eq!(features("(s) => s.n == 1"), ["`==`"], "loose equality");
        assert_eq!(features("(s) => s.n / 2 === 1"), ["`/`"], "division");
        assert_eq!(features("(s) => (s.n ?? 1) === 1"), ["`??`"], "coalescing");
        assert_eq!(
            features("(s) => s.box?.isFull"),
            ["`?.`"],
            "optional chaining"
        );
        assert_eq!(
            features("(s) => typeof s.n === \"a\""),
            ["`typeof`"],
            "typeof"
        );
        assert_eq!(features("(s) => [1].length === s.n"), ["`[]`"], "an array");
        assert_eq!(features("(s) => s.seen[0] === 1"), ["`[]`"], "an index");
        assert_eq!(features("() => 1.5 > 1"), ["`1.5`"], "a fraction");
        assert_eq!(features("(s) => (s.n = 1) === 1"), ["`=`"], "assignment");
        assert_eq!(features("(s) => s(1)"), ["`s`"], "a call of a value");
        assert_eq!(
            features("() => m.int(0, 1) === 1"),
            ["`m.int`"],
            "a declaration as a value"
        );
    }

    #[test]
    fn every_problem_in_one_expression_is_reported() {
        assert_eq!(
            features("() => fetch(\"u\") !== null && Math.random() < Date.now()"),
            ["`fetch`", "`Math.random`", "`Date.now`"],
            "three globals in one rule"
        );
    }
}
