use crate::Reader;
use crate::names::DECLARATIONS;
use crate::terms::bare;
use earnest_ast::{Shape, ShapeKind, Term};
use earnest_diagnostics::Place;
use earnest_value::MOST_SAFE_INTEGER;
use oxc_ast::ast::{
    Argument, ArrayExpressionElement, CallExpression, Expression, ObjectPropertyKind,
};
use oxc_span::GetSpan;
use oxc_syntax::operator::{BinaryOperator, UnaryOperator};

const BEYOND_EXACT: &str = "a JavaScript number is exact only from -9007199254740991 to \
    9007199254740991, and the library throws on a bound outside that";
const NEEDS_A_NUMBER: &str = "a whole number or a top-level number constant is needed here";

pub(crate) const WRITTEN_TWICE: &str =
    "this name is written twice in one object, where only the last one would count";

impl Reader<'_> {
    pub(crate) fn is_declaration(&self, wrapped: &Expression<'_>) -> bool {
        let mut current = bare(wrapped);
        loop {
            match current {
                Expression::Identifier(name) => {
                    return self.local(&name.name).is_none()
                        && self.named_shape(&name.name).is_some();
                }
                Expression::CallExpression(call) => {
                    let Expression::StaticMemberExpression(member) = bare(&call.callee) else {
                        return false;
                    };
                    let property = member.property.name.as_str();
                    if let Expression::Identifier(root) = bare(&member.object)
                        && self.is_library(&root.name)
                    {
                        return DECLARATIONS.contains(&property);
                    }
                    if !matches!(property, "optional" | "initial") {
                        return false;
                    }
                    current = bare(&member.object);
                }
                _ => return false,
            }
        }
    }

    pub(crate) fn shape(&mut self, wrapped: &Expression<'_>) -> Shape {
        let expr = bare(wrapped);
        let place = self.place(expr.span().start);
        if self.too_deep(expr.span()) {
            return Shape {
                place,
                kind: ShapeKind::Bool,
                initial: None,
            };
        }
        self.depth = self.depth.saturating_add(1);
        let found = self.shape_of(expr, place);
        self.depth = self.depth.saturating_sub(1);
        found
    }

    fn shape_of(&mut self, expr: &Expression<'_>, place: Place) -> Shape {
        match expr {
            Expression::Identifier(name) => match self.named_shape(&name.name) {
                Some(found) => Shape {
                    place,
                    ..found.clone()
                },
                None => self.no_shape(expr),
            },
            Expression::CallExpression(call) => self.declared(call, expr, place),
            _ => self.no_shape(expr),
        }
    }

    pub(crate) fn whole_number(&self, wrapped: &Expression<'_>) -> Option<i64> {
        let number = self.exact_number(wrapped)?;
        i64::try_from(number)
            .ok()
            .filter(|found| found.unsigned_abs() <= MOST_SAFE_INTEGER.unsigned_abs())
    }

    fn exact_number(&self, wrapped: &Expression<'_>) -> Option<i128> {
        match bare(wrapped) {
            Expression::NumericLiteral(literal) => {
                format!("{}", literal.value).parse::<i128>().ok()
            }
            Expression::Identifier(name) => self
                .numbers
                .iter()
                .find(|(known, _)| known == name.name.as_str())
                .map(|(_, number)| i128::from(*number)),
            Expression::UnaryExpression(unary)
                if unary.operator == UnaryOperator::UnaryNegation =>
            {
                self.exact_number(&unary.argument)?.checked_neg()
            }
            Expression::BinaryExpression(binary) => {
                let left = self.exact_number(&binary.left)?;
                let right = self.exact_number(&binary.right)?;
                match binary.operator {
                    BinaryOperator::Addition => left.checked_add(right),
                    BinaryOperator::Subtraction => left.checked_sub(right),
                    BinaryOperator::Multiplication => left.checked_mul(right),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub(crate) fn entries<'p, 'a>(
        &mut self,
        wrapped: &'p Expression<'a>,
    ) -> Vec<(String, u32, &'p Expression<'a>)> {
        let expr = bare(wrapped);
        let Expression::ObjectExpression(object) = expr else {
            let written = self.quoted(expr.span());
            self.refuse(
                expr.span().start,
                &written,
                "an object written as `{ name: value }` is needed here",
            );
            return Vec::new();
        };
        let mut found = Vec::with_capacity(object.properties.len());
        for property in &object.properties {
            let start = property.span().start;
            let ObjectPropertyKind::ObjectProperty(written) = property else {
                self.refuse(start, "...", "each part is written out by name here");
                continue;
            };
            match written.key.static_name() {
                Some(name) if !written.computed && !written.method => {
                    if found.iter().any(|(earlier, _, _)| *earlier == name) {
                        self.refuse(start, &name, WRITTEN_TWICE);
                    }
                    found.push((name.into_owned(), start, &written.value));
                }
                _ => {
                    let feature = self.quoted(written.key.span());
                    self.refuse(start, &feature, "each part is written as `name: value`");
                }
            }
        }
        found
    }

    fn named_shape(&self, name: &str) -> Option<&Shape> {
        self.shapes
            .iter()
            .find(|(known, _)| known == name)
            .map(|(_, shape)| shape)
    }

    fn no_shape(&mut self, expr: &Expression<'_>) -> Shape {
        let written = self.quoted(expr.span());
        self.refuse(
            expr.span().start,
            &written,
            "a declaration from the library is needed here, such as `m.enum([...])`",
        );
        Shape {
            place: self.place(expr.span().start),
            kind: ShapeKind::Bool,
            initial: None,
        }
    }

    fn declared(
        &mut self,
        call: &CallExpression<'_>,
        expr: &Expression<'_>,
        place: Place,
    ) -> Shape {
        let Expression::StaticMemberExpression(member) = bare(&call.callee) else {
            return self.no_shape(expr);
        };
        let property = member.property.name.as_str();
        if let Expression::Identifier(root) = bare(&member.object)
            && self.is_library(&root.name)
        {
            let kind = self.kind(property, call);
            return Shape {
                place,
                kind,
                initial: None,
            };
        }
        match property {
            "optional" => {
                let inner = self.shape(&member.object);
                if inner.initial.is_some() {
                    self.refuse(
                        member.property.span.start,
                        "optional",
                        "write `.optional()` before `.initial(...)`, so the starting value \
                         belongs to the optional field",
                    );
                }
                Shape {
                    place,
                    kind: ShapeKind::Optional(Box::new(inner)),
                    initial: None,
                }
            }
            "initial" => {
                let inner = self.shape(&member.object);
                let initial = self.initial(call);
                Shape {
                    place,
                    kind: inner.kind,
                    initial,
                }
            }
            _ => self.no_shape(expr),
        }
    }

    fn initial(&mut self, call: &CallExpression<'_>) -> Option<Term> {
        let only = call.arguments.first().and_then(Argument::as_expression);
        match only {
            Some(value) if call.arguments.len() == 1 => Some(self.term(value)),
            _ => {
                self.refuse(
                    call.span.start,
                    "initial",
                    "`.initial(value)` takes the one value the field starts at",
                );
                None
            }
        }
    }

    fn kind(&mut self, declaration: &str, call: &CallExpression<'_>) -> ShapeKind {
        match declaration {
            "boolean" => ShapeKind::Bool,
            "int" => ShapeKind::Int {
                lo: self.number_argument(call, 0),
                hi: self.number_argument(call, 1),
                ty: None,
            },
            "enum" | "oneOf" => ShapeKind::OneOf(self.choices(call)),
            "record" => ShapeKind::Record {
                owner: None,
                fields: self.record_fields(call),
            },
            "set" => ShapeKind::Set(Box::new(self.shape_argument(call, 0))),
            "map" => ShapeKind::Map(
                Box::new(self.shape_argument(call, 0)),
                Box::new(self.shape_argument(call, 1)),
            ),
            "seq" => ShapeKind::Seq {
                item: Box::new(self.shape_argument(call, 0)),
                max: self.size_option(call, 1, "max"),
            },
            "queue" => ShapeKind::Queue {
                item: Box::new(self.shape_argument(call, 0)),
                capacity: self.size_option(call, 1, "capacity"),
            },
            "lock" => ShapeKind::Lock(
                call.arguments
                    .first()
                    .map(|_| Box::new(self.shape_argument(call, 0))),
            ),
            "timer" => ShapeKind::Timer,
            "request" => ShapeKind::Request,
            "retry" => ShapeKind::Retry {
                max: self.number_option(call, 0, "max"),
            },
            _ => {
                self.refuse(
                    call.span.start,
                    &self.quoted(call.callee.span()),
                    "the library has no declaration with this name",
                );
                ShapeKind::Bool
            }
        }
    }

    pub(crate) fn argument<'p, 'a>(
        &mut self,
        call: &'p CallExpression<'a>,
        index: usize,
        needed: &str,
    ) -> Option<&'p Expression<'a>> {
        let found = call.arguments.get(index).and_then(Argument::as_expression);
        if found.is_none() {
            self.refuse(call.span.start, &self.quoted(call.callee.span()), needed);
        }
        found
    }

    fn number_argument(&mut self, call: &CallExpression<'_>, index: usize) -> i64 {
        let Some(value) = self.argument(call, index, NEEDS_A_NUMBER) else {
            return 0;
        };
        self.needed_number(value)
    }

    fn needed_number(&mut self, value: &Expression<'_>) -> i64 {
        if let Some(number) = self.whole_number(value) {
            return number;
        }
        let written = self.quoted(value.span());
        let message = if self.exact_number(value).is_some() {
            BEYOND_EXACT
        } else {
            NEEDS_A_NUMBER
        };
        self.refuse(value.span().start, &written, message);
        0
    }

    fn number_option(&mut self, call: &CallExpression<'_>, index: usize, key: &str) -> i64 {
        let needed = format!("an object with `{key}` is needed here: `{{ {key}: 2 }}`");
        let Some(options) = self.argument(call, index, &needed) else {
            return 0;
        };
        let entries = self.entries(options);
        let wanted = entries.iter().find(|(name, _, _)| name == key);
        if let Some((_, _, value)) = wanted {
            self.needed_number(value)
        } else {
            let written = self.quoted(options.span());
            self.refuse(options.span().start, &written, &needed);
            0
        }
    }

    fn size_option(&mut self, call: &CallExpression<'_>, index: usize, key: &str) -> usize {
        let before = self.problems.len();
        let number = self.number_option(call, index, key);
        if let Ok(size) = usize::try_from(number) {
            size
        } else {
            if self.problems.len() == before {
                self.refuse(
                    call.span.start,
                    key,
                    "the most items a collection holds can't be below zero",
                );
            }
            0
        }
    }

    fn shape_argument(&mut self, call: &CallExpression<'_>, index: usize) -> Shape {
        let needed = "a declaration is needed here, such as `m.enum([...])`";
        match self.argument(call, index, needed) {
            Some(value) => self.shape(value),
            None => Shape {
                place: self.place(call.span.start),
                kind: ShapeKind::Bool,
                initial: None,
            },
        }
    }

    fn choices(&mut self, call: &CallExpression<'_>) -> Vec<Term> {
        let needed = "a list of choices is needed here: `[\"a\", \"b\"]`";
        let Some(list) = self.argument(call, 0, needed) else {
            return Vec::new();
        };
        let Expression::ArrayExpression(array) = bare(list) else {
            let written = self.quoted(list.span());
            self.refuse(list.span().start, &written, needed);
            return Vec::new();
        };
        let mut choices = Vec::with_capacity(array.elements.len());
        for element in &array.elements {
            match element {
                ArrayExpressionElement::SpreadElement(_) | ArrayExpressionElement::Elision(_) => {
                    self.refuse(element.span().start, "...", "write each choice out");
                }
                other => {
                    if let Some(choice) = other.as_expression() {
                        choices.push(self.term(choice));
                    }
                }
            }
        }
        choices
    }

    fn record_fields(&mut self, call: &CallExpression<'_>) -> Vec<(String, Shape)> {
        let needed = "the record's fields are needed here: `{ name: m.boolean() }`";
        let Some(fields) = self.argument(call, 0, needed) else {
            return Vec::new();
        };
        self.fields(fields)
    }

    pub(crate) fn fields(&mut self, fields: &Expression<'_>) -> Vec<(String, Shape)> {
        let mut found = Vec::new();
        for (name, _, value) in self.entries(fields) {
            let shape = self.shape(value);
            found.push((name, shape));
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use crate::read;
    use crate::show::show_shape;
    use earnest_ast::Shape;
    use earnest_diagnostics::Place;

    const HEAD: &str = "import { m } from \"@earnest/model\";\n\
        const MAX = 3;\n\
        const LOW = -MAX + 1;\n\
        const Kind = m.enum([\"a\", \"b\"]);\n\
        const Maybe = Kind.optional();\n";

    fn source(declaration: &str) -> String {
        format!("{HEAD}export const M = m.model(\"M\", {{\n\tfield: {declaration},\n}});\n")
    }

    fn field(declaration: &str) -> Shape {
        let reading = read("m.model.ts", &source(declaration));
        assert_eq!(reading.problems, [], "no problem in: {declaration}");
        let draft = reading.drafts.into_iter().next().expect("one model");
        let found = draft.fields.into_iter().next().expect("one field");
        assert_eq!(found.name, "field", "the field's name");
        found.shape
    }

    fn shown(declaration: &str) -> String {
        show_shape(&field(declaration))
    }

    fn features(declaration: &str) -> Vec<String> {
        read("m.model.ts", &source(declaration))
            .problems
            .into_iter()
            .map(|problem| {
                assert_eq!(problem.line, 7, "the problem is on the field's line");
                problem.feature
            })
            .collect()
    }

    #[test]
    fn plain_declarations() {
        assert_eq!(shown("m.boolean()"), "bool", "a boolean");
        assert_eq!(shown("m.int(0, 3)"), "int(0, 3)", "a range");
        assert_eq!(shown("m.int(-2, -1)"), "int(-2, -1)", "a negative range");
        assert_eq!(
            shown("m.enum([\"x\", \"y\"])"),
            "enum(\"x\", \"y\")",
            "an enum"
        );
        assert_eq!(
            shown("m.oneOf([1, 2])"),
            "enum(1, 2)",
            "one of some numbers"
        );
        assert_eq!(shown("m.timer()"), "timer", "a timer");
        assert_eq!(shown("m.request()"), "request", "a request");
        assert_eq!(shown("m.retry({ max: 2 })"), "retry(2)", "a retry");
        assert_eq!(shown("m.lock()"), "lock", "a lock");
    }

    #[test]
    fn declarations_that_hold_declarations() {
        assert_eq!(shown("m.set(m.boolean())"), "set(bool)", "a set");
        assert_eq!(
            shown("m.map(Kind, m.int(1, 2))"),
            "map(enum(\"a\", \"b\"), int(1, 2))",
            "a map"
        );
        assert_eq!(
            shown("m.seq(Kind, { max: 2 })"),
            "seq(enum(\"a\", \"b\"), 2)",
            "a sequence"
        );
        assert_eq!(
            shown("m.queue(m.boolean(), { capacity: MAX })"),
            "queue(bool, 3)",
            "a queue"
        );
        assert_eq!(
            shown("m.lock(Kind)"),
            "lock(enum(\"a\", \"b\"))",
            "an owned lock"
        );
        assert_eq!(
            shown("m.record({ row: m.int(1, 2), aisle: m.boolean() })"),
            "record(row: int(1, 2), aisle: bool)",
            "a record"
        );
    }

    #[test]
    fn optional_and_initial() {
        assert_eq!(
            shown("m.boolean().optional()"),
            "optional(bool)",
            "optional"
        );
        assert_eq!(shown("m.int(0, 3).initial(1)"), "int(0, 3) = 1", "initial");
        assert_eq!(
            shown("Kind.optional().initial(null)"),
            "optional(enum(\"a\", \"b\")) = null",
            "an optional that starts empty"
        );
        assert_eq!(
            shown("Maybe.initial(\"a\")"),
            "optional(enum(\"a\", \"b\")) = \"a\"",
            "a named optional"
        );
    }

    #[test]
    fn a_bound_may_be_a_number_constant() {
        assert_eq!(
            shown("m.int(LOW, MAX)"),
            "int(-2, 3)",
            "constants as bounds"
        );
        assert_eq!(
            shown("m.int(0, MAX * 2 - 1)"),
            "int(0, 5)",
            "a sum of constants"
        );
    }

    #[test]
    fn a_shape_has_the_place_it_is_used_at() {
        let found = field("Kind.optional()");
        assert_eq!(
            found.place,
            Place { line: 7, column: 9 },
            "the field's declaration"
        );
    }

    #[test]
    fn what_is_not_a_declaration_is_named() {
        assert_eq!(features("3"), ["`3`"], "a number");
        assert_eq!(features("Missing"), ["`Missing`"], "an unknown name");
        assert_eq!(
            features("m.float()"),
            ["`m.float`"],
            "an unknown declaration"
        );
        assert_eq!(
            features("m.int(0, 1.5)"),
            ["`1.5`"],
            "a fraction as a bound"
        );
        assert_eq!(features("m.int(0)"), ["`m.int`"], "a missing bound");
        assert_eq!(features("m.seq(Kind)"), ["`m.seq`"], "a missing size");
        assert_eq!(
            features("m.seq(Kind, { max: -1 })"),
            ["`max`"],
            "a size below zero"
        );
        assert_eq!(
            features("m.enum(\"a\")"),
            ["`\"a\"`"],
            "choices that are not a list"
        );
        assert_eq!(
            features("m.int(0, 1).initial(0).optional()"),
            ["`optional`"],
            "optional after initial"
        );
        assert_eq!(
            features("m.int(0, 1).initial()"),
            ["`initial`"],
            "an empty initial"
        );
    }
}
