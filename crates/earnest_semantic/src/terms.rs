use super::types::{Binding, Scope, Type, Typed, fitted, fitted_when_present};
use super::{Lowerer, Lowering};
use earnest_ast::{
    Binary, HelperKind, Language, MOST_NESTING, Reach, Shape, TOO_DEEP, Term, TermKind, Unary,
};
use earnest_diagnostics::Place;
use earnest_ir::{Expr, Math, Op, Order, Shelf, Sweep};
use earnest_value::{IntType, Sym, Value};

pub(super) fn apply(op: Op, arguments: Vec<Expr>) -> Expr {
    Expr::Apply(op, arguments)
}

fn snake_case(name: &str) -> String {
    let mut snake = String::with_capacity(name.len() + 2);
    for letter in name.chars() {
        if letter.is_ascii_uppercase() {
            snake.push('_');
            snake.push(letter.to_ascii_lowercase());
        } else {
            snake.push(letter);
        }
    }
    snake
}

fn can_be_null(ty: &Type) -> bool {
    matches!(ty, Type::Optional(_) | Type::Null)
}

fn is_null(expr: Expr) -> Expr {
    apply(Op::Equal, vec![expr, Expr::Const(Value::Null)])
}

impl Lowerer<'_> {
    pub(super) fn term(&mut self, term: &Term, scope: &mut Scope) -> Lowering<Typed> {
        if self.depth >= MOST_NESTING {
            return Err(self.stop(term.place, "a deeply nested expression", TOO_DEEP));
        }
        self.depth = self.depth.saturating_add(1);
        let lowered = self.term_of(term, scope);
        self.depth = self.depth.saturating_sub(1);
        lowered
    }

    fn term_of(&mut self, term: &Term, scope: &mut Scope) -> Lowering<Typed> {
        let place = term.place;
        match &term.kind {
            TermKind::Null => Ok(Typed::new(Expr::Const(Value::Null), Type::Null)),
            TermKind::Bool(flag) => Ok(Typed::new(Expr::Const(Value::Bool(*flag)), Type::Bool)),
            TermKind::Int(number) => Ok(Typed::new(
                Expr::Const(Value::Int(*number)),
                Type::Int(None),
            )),
            TermKind::Text(text) => Ok(self.text(text)),
            TermKind::Name(name) => self.name(name, place, scope),
            TermKind::Member(target, name) => self.member(target, name, place, scope),
            TermKind::Call {
                target,
                method,
                arguments,
            } => {
                let on = self.term(target, scope)?;
                self.method(on, method, arguments, place, scope)
            }
            TermKind::Helper {
                kind,
                name,
                arguments,
            } => self.helper(*kind, name, arguments, place, scope),
            TermKind::Unary(unary, inner) => self.unary(*unary, inner, scope),
            TermKind::Binary(binary, left, right) => {
                self.binary(*binary, left, right, place, scope)
            }
            TermKind::Choose(test, yes, no) => self.choose(test, yes, no, place, scope),
            TermKind::Record { base, fields } => self.record(base.as_deref(), fields, place, scope),
            TermKind::SetOf(items) => self.listed(items, Shelf::Set, place, scope),
            TermKind::SeqOf(items) => self.listed(items, Shelf::Seq, place, scope),
            TermKind::MapOf(entries) => self.map_of(entries, place, scope),
            TermKind::Lambda { .. } => Err(self.stop(
                place,
                "a function used as a value",
                "a function can only be passed to every, some, filter, forall and exists",
            )),
            TermKind::Bind { name, value, body } => {
                let bound = self.bound(name, value, scope)?;
                let slot = scope.slot();
                scope
                    .names
                    .push((name.clone(), Binding::Local(slot, bound.ty)));
                let inner = self.term(body, scope);
                scope.names.pop();
                let lowered = inner?;
                Ok(Typed::new(
                    Expr::Bind {
                        slot,
                        value: Box::new(bound.expr),
                        body: Box::new(lowered.expr),
                    },
                    lowered.ty,
                ))
            }
            TermKind::Wrap(inner) => {
                let lowered = self.term(inner, scope)?;
                if matches!(lowered.ty, Type::Optional(_) | Type::Null) {
                    return Err(self.stop(
                        place,
                        "an optional value inside an optional value",
                        "an optional value can't hold another optional value",
                    ));
                }
                Ok(Typed::new(
                    apply(Op::SomeOf, vec![lowered.expr]),
                    Type::optional(lowered.ty),
                ))
            }
            TermKind::Across {
                reach,
                shape,
                parameter,
                body,
            } => self.across(*reach, shape, parameter, body, scope),
            TermKind::Template(_) => Err(self.stop(
                place,
                "a template string",
                "text is built only in a view's state and group labels",
            )),
            TermKind::Declared { shape, value } => {
                let declared = self.shape(shape)?.ty;
                let lowered = self.term(value, scope)?;
                let expr = self.coerce(lowered, &declared, value.place)?;
                Ok(Typed::new(expr, declared))
            }
            _ => Err(self.unknown_syntax(place, "an expression")),
        }
    }

    fn unknown_syntax(&mut self, place: Place, feature: &str) -> super::Stop {
        self.stop(
            place,
            feature,
            "this version of the checker does not know it",
        )
    }

    pub(super) fn text(&mut self, text: &str) -> Typed {
        Typed::new(
            Expr::Const(Value::Str(self.symbols.intern(text))),
            Type::Text,
        )
    }

    pub(super) fn truth(&mut self, term: &Term, scope: &mut Scope) -> Lowering<Expr> {
        let lowered = self.term(term, scope)?;
        self.coerce(lowered, &Type::Bool, term.place)
    }

    fn number_kind(&self) -> Numbers {
        if self.is_rust() {
            Numbers::Wide
        } else {
            Numbers::Javascript
        }
    }

    pub(super) fn is_rust(&self) -> bool {
        self.draft.language == Language::Rust
    }

    pub(super) fn bound(&mut self, name: &str, value: &Term, scope: &mut Scope) -> Lowering<Typed> {
        let bound = self.term(value, scope)?;
        if self.is_rust() && bound.ty.has_a_number_without_a_type() {
            return Err(self.stop(
                value.place,
                &format!("`let {name}` with a number of no stated type"),
                &format!(
                    "the checker can't tell which integer type the number has, and Rust may \
                     pick another than it would: write the type after the name, as in \
                     `let {name}: u8 = 1;`"
                ),
            ));
        }
        Ok(bound)
    }

    fn mixed(&mut self, left: &Type, right: &Type, place: Place) -> Lowering<()> {
        let Some((first, second)) = left.mixes_integer_types(right) else {
            return Ok(());
        };
        Err(self.stop(
            place,
            &format!(
                "a `{}` number with a `{}` number",
                first.name(),
                second.name()
            ),
            "Rust does not mix integer types: both numbers must have one type",
        ))
    }

    fn numbers(
        &mut self,
        left: (&Term, Typed),
        right: (&Term, Typed),
        place: Place,
        fallback: Option<IntType>,
    ) -> Lowering<(Expr, Expr, Option<IntType>)> {
        let ((left_term, first), (right_term, second)) = (left, right);
        self.mixed(&first.ty, &second.ty, place)?;
        let ty = first.ty.int_type().or(second.ty.int_type()).or(fallback);
        let lower = self.coerce(first, &Type::Int(ty), left_term.place)?;
        let upper = self.coerce(second, &Type::Int(ty), right_term.place)?;
        Ok((lower, upper, ty))
    }

    fn default_int_type(&self) -> Option<IntType> {
        self.is_rust().then_some(IntType::I32)
    }

    fn name(&mut self, name: &str, place: Place, scope: &mut Scope) -> Lowering<Typed> {
        match scope.find(name) {
            Some(Binding::Local(slot, ty)) => return Ok(Typed::new(Expr::Local(slot), ty)),
            Some(Binding::State) => {
                return Err(self.stop(
                    place,
                    &format!("the whole state `{name}` used as a value"),
                    "read one of its fields, or pass it to a helper function",
                ));
            }
            Some(Binding::Inputs) => {
                return Err(self.stop(
                    place,
                    &format!("the whole input `{name}` used as a value"),
                    "read one of its fields",
                ));
            }
            None => {}
        }
        let draft = self.draft;
        let Some((_, constant)) = draft.constants.iter().find(|(known, _)| known == name) else {
            return Err(self.stop(
                place,
                &format!("`{name}`"),
                "the model file does not define this name, so the checker can't know its value",
            ));
        };
        let running = (None, name.to_owned());
        if self.in_flight.contains(&running) {
            return Err(self.stop(
                place,
                &format!("recursion in `{name}`"),
                "a constant can't be defined by itself",
            ));
        }
        let mut fresh = Scope::starting_at(scope.slots);
        self.in_flight.push(running);
        let lowered = self.term(constant, &mut fresh);
        self.in_flight.pop();
        scope.slots = fresh.slots;
        lowered
    }

    fn member(
        &mut self,
        target: &Term,
        name: &str,
        place: Place,
        scope: &mut Scope,
    ) -> Lowering<Typed> {
        if let TermKind::Name(owner) = &target.kind {
            match scope.find(owner) {
                Some(Binding::State) => return self.state_field(name, place),
                Some(Binding::Inputs) => {
                    return match scope.input(name) {
                        Some((slot, ty)) => Ok(Typed::new(Expr::Local(slot), ty)),
                        None => Err(self.stop(
                            place,
                            &format!("the input `{name}`"),
                            "the action declares no input with this name",
                        )),
                    };
                }
                Some(Binding::Local(_, _)) | None => {}
            }
        }
        let on = self.term(target, scope)?;
        self.field_of(on, name, place, scope)
    }

    fn state_field(&mut self, name: &str, place: Place) -> Lowering<Typed> {
        let found = self
            .fields
            .iter()
            .position(|(known, _)| known == name)
            .and_then(|index| self.fields.get(index).map(|(_, ty)| (index, ty.clone())));
        match found {
            Some((index, ty)) => Ok(Typed::new(Expr::Field(index), ty)),
            None => Err(self.stop(
                place,
                &format!("the state field `{name}`"),
                "the model declares no field with this name",
            )),
        }
    }

    fn field_of(
        &mut self,
        on: Typed,
        name: &str,
        place: Place,
        scope: &mut Scope,
    ) -> Lowering<Typed> {
        let (base, fields) = match on.ty {
            Type::Record(fields) => (on.expr, fields),
            Type::Optional(inner) => match *inner {
                Type::Record(fields) => (apply(Op::Unwrap, vec![on.expr]), fields),
                other => {
                    let unwrapped = Typed::new(on.expr, Type::optional(other));
                    return self.method(unwrapped, &snake_case(name), &[], place, scope);
                }
            },
            other => {
                let primitive = Typed::new(on.expr, other);
                return self.method(primitive, &snake_case(name), &[], place, scope);
            }
        };
        let Some((_, ty)) = fields.iter().find(|(known, _)| known == name) else {
            return Err(self.stop(
                place,
                &format!("the field `{name}`"),
                "the record has no field with this name",
            ));
        };
        let sym = self.symbols.intern(name);
        Ok(Typed::new(Expr::Get(Box::new(base), sym), ty.clone()))
    }

    fn helper(
        &mut self,
        kind: HelperKind,
        name: &str,
        arguments: &[Term],
        place: Place,
        scope: &mut Scope,
    ) -> Lowering<Typed> {
        let draft = self.draft;
        let known = draft
            .helpers
            .iter()
            .find(|helper| helper.kind == kind && helper.name == name);
        let Some(found) = known else {
            return Err(self.stop(
                place,
                &format!("a call to `{name}`"),
                "it is not a library primitive or a function defined in the model file",
            ));
        };
        let running = (Some(kind), name.to_owned());
        if self.in_flight.contains(&running) {
            return Err(self.stop(
                place,
                &format!("recursion in `{name}`"),
                "a helper function can't call itself",
            ));
        }
        if found.parameters.len() != arguments.len() {
            return Err(self.stop(
                place,
                &format!("a call to `{name}`"),
                "the number of arguments differs from the function's parameters",
            ));
        }
        let mut passed = Vec::with_capacity(arguments.len());
        for argument in arguments {
            let whole_state = matches!(&argument.kind, TermKind::Name(owner)
                if matches!(scope.find(owner), Some(Binding::State)));
            passed.push(if whole_state {
                None
            } else {
                Some(self.term(argument, scope)?)
            });
        }
        let mut inner = Scope::starting_at(scope.slots);
        let mut bound = Vec::new();
        for (parameter, value) in found.parameters.iter().zip(passed) {
            match value {
                None => inner.names.push((parameter.clone(), Binding::State)),
                Some(lowered) => {
                    let slot = inner.slot();
                    inner
                        .names
                        .push((parameter.clone(), Binding::Local(slot, lowered.ty)));
                    bound.push((slot, lowered.expr));
                }
            }
        }
        self.in_flight.push(running);
        let body = self.term(&found.body, &mut inner);
        self.in_flight.pop();
        scope.slots = inner.slots;
        let lowered = body?;
        let expr = bound
            .into_iter()
            .rev()
            .fold(lowered.expr, |body_expr, (slot, value)| Expr::Bind {
                slot,
                value: Box::new(value),
                body: Box::new(body_expr),
            });
        Ok(Typed::new(expr, lowered.ty))
    }

    fn unary(&mut self, unary: Unary, inner: &Term, scope: &mut Scope) -> Lowering<Typed> {
        match unary {
            Unary::Not => {
                let lowered = self.truth(inner, scope)?;
                Ok(Typed::new(apply(Op::Not, vec![lowered]), Type::Bool))
            }
            Unary::Negate => {
                let lowered = self.term(inner, scope)?;
                let ty = lowered.ty.int_type();
                let number = self.coerce(lowered, &Type::Int(ty), inner.place)?;
                Ok(Typed::new(
                    in_type(apply(Op::Negate, vec![number]), ty, self.number_kind()),
                    Type::Int(ty),
                ))
            }
            _ => Err(self.unknown_syntax(inner.place, "an operator")),
        }
    }

    fn binary(
        &mut self,
        binary: Binary,
        left: &Term,
        right: &Term,
        place: Place,
        scope: &mut Scope,
    ) -> Lowering<Typed> {
        let op = match binary {
            Binary::And | Binary::Or => {
                let first = Box::new(self.truth(left, scope)?);
                let second = Box::new(self.truth(right, scope)?);
                let expr = if binary == Binary::And {
                    Expr::And(first, second)
                } else {
                    Expr::Or(first, second)
                };
                return Ok(Typed::new(expr, Type::Bool));
            }
            Binary::Equal | Binary::Differ => {
                let first = self.term(left, scope)?;
                let second = self.term(right, scope)?;
                let op = if binary == Binary::Equal {
                    Op::Equal
                } else {
                    Op::Differ
                };
                return self.equality(op, first, second, place);
            }
            Binary::Less => return self.order(Order::Less, left, right, place, scope),
            Binary::LessOrEqual => {
                return self.order(Order::LessOrEqual, left, right, place, scope);
            }
            Binary::Greater => return self.order(Order::Greater, left, right, place, scope),
            Binary::GreaterOrEqual => {
                return self.order(Order::GreaterOrEqual, left, right, place, scope);
            }
            Binary::Add => Op::Math(Math::Add),
            Binary::Subtract => Op::Math(Math::Subtract),
            Binary::Multiply => Op::Math(Math::Multiply),
            Binary::Remainder => Op::Math(Math::Remainder),
            _ => return Err(self.unknown_syntax(place, "an operator")),
        };
        let first = self.term(left, scope)?;
        let second = self.term(right, scope)?;
        let (lower, upper, ty) = self.numbers((left, first), (right, second), place, None)?;
        Ok(Typed::new(
            in_type(apply(op, vec![lower, upper]), ty, self.number_kind()),
            Type::Int(ty),
        ))
    }

    fn order(
        &mut self,
        order: Order,
        left: &Term,
        right: &Term,
        place: Place,
        scope: &mut Scope,
    ) -> Lowering<Typed> {
        let first = self.term(left, scope)?;
        let second = self.term(right, scope)?;
        if can_be_null(&first.ty) && can_be_null(&second.ty) {
            return Err(self.stop(
                place,
                "two optional values compared by order",
                "`<`, `<=`, `>` and `>=` compare numbers, and an optional value has no order: \
                 test that it is present, then compare the number it holds with a number (in \
                 Rust, `value.is_some_and(|number| number < 5)`)",
            ));
        }
        let fallback = self.default_int_type();
        let (lower, upper, _) = self.numbers((left, first), (right, second), place, fallback)?;
        Ok(Typed::new(
            apply(Op::Order(order), vec![lower, upper]),
            Type::Bool,
        ))
    }

    pub(super) fn equality(
        &mut self,
        op: Op,
        left: Typed,
        right: Typed,
        place: Place,
    ) -> Lowering<Typed> {
        self.mixed(&left.ty, &right.ty, place)?;
        let never_null = |ty: &Type| !matches!(ty, Type::Null | Type::Optional(_) | Type::Unknown);
        let both = match (&left.ty, &right.ty) {
            (Type::Null, other) | (other, Type::Null) if never_null(other) => None,
            (first, second) => first.union(second),
        };
        let Some(joined) = both else {
            let feature = format!("{} compared with {}", left.ty.kind(), right.ty.kind());
            return Err(self.stop(
                place,
                &feature,
                "the two sides can never be equal, so the comparison is a mistake",
            ));
        };
        let ty = match (joined, self.default_int_type()) {
            (Type::Int(None), Some(fallback)) => Type::Int(Some(fallback)),
            (kept, _) => kept,
        };
        let first = self.coerce(left, &ty, place)?;
        let second = self.coerce(right, &ty, place)?;
        Ok(Typed::new(apply(op, vec![first, second]), Type::Bool))
    }

    fn choose(
        &mut self,
        test: &Term,
        yes: &Term,
        no: &Term,
        place: Place,
        scope: &mut Scope,
    ) -> Lowering<Typed> {
        let tested = self.truth(test, scope)?;
        let first = self.term(yes, scope)?;
        let second = self.term(no, scope)?;
        let (first_expr, second_expr, ty) = self.join(first, second, place)?;
        Ok(Typed::new(
            Expr::Choose(
                Box::new(tested),
                Box::new(first_expr),
                Box::new(second_expr),
            ),
            ty,
        ))
    }

    fn join(&mut self, left: Typed, right: Typed, place: Place) -> Lowering<(Expr, Expr, Type)> {
        self.mixed(&left.ty, &right.ty, place)?;
        let Some(ty) = left.ty.union(&right.ty) else {
            let feature = format!(
                "a choice between {} and {}",
                left.ty.kind(),
                right.ty.kind()
            );
            return Err(self.stop(
                place,
                &feature,
                "both branches must give the same kind of value",
            ));
        };
        let first = self.coerce(left, &ty, place)?;
        let second = self.coerce(right, &ty, place)?;
        Ok((first, second, ty))
    }

    fn record(
        &mut self,
        base: Option<&Term>,
        fields: &[(String, Term)],
        place: Place,
        scope: &mut Scope,
    ) -> Lowering<Typed> {
        let mut exprs = Vec::with_capacity(fields.len());
        let mut types = Vec::with_capacity(fields.len());
        for (name, value) in fields {
            let lowered = self.term(value, scope)?;
            exprs.push((self.symbols.intern(name), lowered.expr));
            types.push((name.clone(), lowered.ty));
        }
        let Some(spread) = base else {
            exprs.sort_by_key(|(sym, _)| *sym);
            return Ok(Typed::new(Expr::Record(exprs), Type::record(types)));
        };
        let spread_in = self.term(spread, scope)?;
        let on = match spread_in.ty {
            Type::Optional(inner) if matches!(*inner, Type::Record(_)) => {
                Typed::new(apply(Op::Unwrap, vec![spread_in.expr]), *inner)
            }
            other => Typed::new(spread_in.expr, other),
        };
        let Type::Record(mut known) = on.ty else {
            return Err(self.stop(
                place,
                &format!("a spread of {}", on.ty.kind()),
                "only a record can be spread into a record",
            ));
        };
        let mut changes = Vec::with_capacity(exprs.len());
        for ((sym, expr), (name, ty)) in exprs.into_iter().zip(types) {
            let wanted = known
                .iter()
                .find(|(have, _)| *have == name)
                .map(|(_, wanted)| wanted.clone());
            if let Some(wanted_type) = wanted {
                let coerced = self.coerce(Typed::new(expr, ty), &wanted_type, place)?;
                changes.push((sym, coerced));
            } else {
                known.push((name, ty));
                changes.push((sym, expr));
            }
        }
        Ok(Typed::new(
            Expr::With(Box::new(on.expr), changes),
            Type::record(known),
        ))
    }

    fn listed(
        &mut self,
        items: &[Term],
        kind: Shelf,
        place: Place,
        scope: &mut Scope,
    ) -> Lowering<Typed> {
        let mut lowered = Vec::with_capacity(items.len());
        for item in items {
            lowered.push(self.term(item, scope)?);
        }
        let item_type = joined_type(lowered.iter().map(|typed| &typed.ty));
        let mut exprs = Vec::with_capacity(lowered.len());
        for typed in lowered {
            exprs.push(self.coerce(typed, &item_type, place)?);
        }
        Ok(if kind == Shelf::Set {
            Typed::new(Expr::SetOf(exprs), Type::Set(Box::new(item_type)))
        } else {
            Typed::new(Expr::SeqOf(exprs), Type::Seq(Box::new(item_type)))
        })
    }

    fn map_of(
        &mut self,
        entries: &[(Term, Term)],
        place: Place,
        scope: &mut Scope,
    ) -> Lowering<Typed> {
        let mut lowered = Vec::with_capacity(entries.len());
        for (key, value) in entries {
            lowered.push((self.term(key, scope)?, self.term(value, scope)?));
        }
        let key_type = joined_type(lowered.iter().map(|(key, _)| &key.ty));
        let value_type = joined_type(lowered.iter().map(|(_, value)| &value.ty));
        let mut exprs = Vec::with_capacity(lowered.len());
        for (key, value) in lowered {
            exprs.push((
                self.coerce(key, &key_type, place)?,
                self.coerce(value, &value_type, place)?,
            ));
        }
        Ok(Typed::new(
            Expr::MapOf(exprs),
            Type::Map(Box::new(key_type), Box::new(value_type)),
        ))
    }

    fn across(
        &mut self,
        reach: Reach,
        shape: &Shape,
        parameter: &str,
        body: &Term,
        scope: &mut Scope,
    ) -> Lowering<Typed> {
        let sweep = match reach {
            Reach::Every => Sweep::Every,
            Reach::Some => Sweep::Some,
            _ => return Err(self.unknown_syntax(body.place, "a choice over a declaration")),
        };
        let shaped = self.shape(shape)?;
        let values = self.all_values(&shaped.domain, shape.place)?;
        let slot = scope.slot();
        scope
            .names
            .push((parameter.to_owned(), Binding::Local(slot, shaped.ty)));
        let inner = self.truth(body, scope);
        scope.names.pop();
        Ok(Typed::new(
            Expr::Sweep {
                sweep,
                shelf: Shelf::Set,
                slot,
                source: Box::new(Expr::Const(Value::set(values.into_iter().collect()))),
                body: Box::new(inner?),
            },
            Type::Bool,
        ))
    }

    pub(super) fn coerce(&mut self, have: Typed, want: &Type, place: Place) -> Lowering<Expr> {
        if have.ty.fits(want) {
            return self.typed_as(have, want, place);
        }
        match (have.expr, have.ty, want) {
            (expr, Type::Optional(inner), wanted) if !matches!(wanted, Type::Optional(_)) => self
                .coerce(
                    Typed::new(apply(Op::Unwrap, vec![expr]), *inner),
                    wanted,
                    place,
                ),
            (expr, ty, Type::Optional(inner)) if !matches!(ty, Type::Optional(_)) => {
                let coerced = self.coerce(Typed::new(expr, ty), inner, place)?;
                Ok(apply(Op::SomeOf, vec![coerced]))
            }
            (expr, Type::Optional(inner), Type::Optional(wanted)) => {
                let present = Typed::new(apply(Op::Unwrap, vec![expr.clone()]), *inner);
                let coerced = self.coerce(present, wanted, place)?;
                Ok(Expr::Choose(
                    Box::new(is_null(expr)),
                    Box::new(Expr::Const(Value::Null)),
                    Box::new(apply(Op::SomeOf, vec![coerced])),
                ))
            }
            (Expr::Record(fields), Type::Record(have_types), Type::Record(want_types))
                if have_types.len() == want_types.len() =>
            {
                self.coerce_record(fields, &have_types, want_types, place)
            }
            (expr, Type::Record(have_types), Type::Record(want_types))
                if have_types.len() == want_types.len() =>
            {
                self.coerce_fields(expr, &have_types, want_types, place)
            }
            (Expr::SetOf(items), Type::Set(item), Type::Set(wanted)) => {
                Ok(Expr::SetOf(self.coerce_each(items, &item, wanted, place)?))
            }
            (Expr::SeqOf(items), Type::Seq(item), Type::Seq(wanted) | Type::Queue(wanted, _)) => {
                Ok(Expr::SeqOf(self.coerce_each(items, &item, wanted, place)?))
            }
            (Expr::MapOf(entries), Type::Map(key, value), Type::Map(wanted_key, wanted_value)) => {
                let mut coerced = Vec::with_capacity(entries.len());
                for (entry_key, entry_value) in entries {
                    coerced.push((
                        self.coerce(Typed::new(entry_key, (*key).clone()), wanted_key, place)?,
                        self.coerce(
                            Typed::new(entry_value, (*value).clone()),
                            wanted_value,
                            place,
                        )?,
                    ));
                }
                Ok(Expr::MapOf(coerced))
            }
            (_, ty, wanted) => {
                let feature = format!("{} where {} is needed", ty.kind(), wanted.kind());
                Err(self.stop(
                    place,
                    &feature,
                    "the value has the wrong kind for this place",
                ))
            }
        }
    }

    fn typed_as(&mut self, have: Typed, want: &Type, place: Place) -> Lowering<Expr> {
        self.mixed(&have.ty, want, place)?;
        Ok(match (&have.ty, want) {
            (Type::Int(None), Type::Int(Some(ty))) => fitted(have.expr, *ty),
            (Type::Optional(inner), Type::Optional(wanted)) => match (&**inner, &**wanted) {
                (Type::Int(None), Type::Int(Some(ty))) => fitted_when_present(have.expr, *ty),
                _ => have.expr,
            },
            _ => have.expr,
        })
    }

    fn coerce_fields(
        &mut self,
        record: Expr,
        have: &[(String, Type)],
        want: &[(String, Type)],
        place: Place,
    ) -> Lowering<Expr> {
        let mut changes = Vec::new();
        for (name, wanted_type) in want {
            let Some((_, had_type)) = have.iter().find(|(known, _)| known == name) else {
                return Err(self.stop(
                    place,
                    &format!("the record field `{name}`"),
                    "the record given here has no field with this name",
                ));
            };
            let sym = self.symbols.intern(name);
            let read = Expr::Get(Box::new(record.clone()), sym);
            let coerced = self.coerce(
                Typed::new(read.clone(), had_type.clone()),
                wanted_type,
                place,
            )?;
            if coerced != read {
                changes.push((sym, coerced));
            }
        }
        Ok(if changes.is_empty() {
            record
        } else {
            Expr::With(Box::new(record), changes)
        })
    }

    fn coerce_each(
        &mut self,
        items: Vec<Expr>,
        have: &Type,
        want: &Type,
        place: Place,
    ) -> Lowering<Vec<Expr>> {
        let mut coerced = Vec::with_capacity(items.len());
        for item in items {
            coerced.push(self.coerce(Typed::new(item, have.clone()), want, place)?);
        }
        Ok(coerced)
    }

    fn coerce_record(
        &mut self,
        fields: Vec<(Sym, Expr)>,
        have: &[(String, Type)],
        want: &[(String, Type)],
        place: Place,
    ) -> Lowering<Expr> {
        let mut coerced = Vec::with_capacity(fields.len());
        for (sym, expr) in fields {
            let name = self.symbols.name(sym).to_owned();
            let had = have.iter().find(|(known, _)| *known == name);
            let wanted = want.iter().find(|(known, _)| *known == name);
            let (Some((_, had_type)), Some((_, wanted_type))) = (had, wanted) else {
                return Err(self.stop(
                    place,
                    &format!("the record field `{name}`"),
                    "the record needed here has no field with this name",
                ));
            };
            let typed = Typed::new(expr, had_type.clone());
            coerced.push((sym, self.coerce(typed, wanted_type, place)?));
        }
        Ok(Expr::Record(coerced))
    }
}

fn joined_type<'t>(types: impl Iterator<Item = &'t Type> + Clone) -> Type {
    let first_known = types
        .clone()
        .find(|ty| !matches!(ty, Type::Unknown | Type::Null))
        .cloned()
        .unwrap_or(Type::Unknown);
    types.fold(first_known, |joined, ty| joined.union(ty).unwrap_or(joined))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Numbers {
    Wide,
    Javascript,
}

fn in_type(expr: Expr, ty: Option<IntType>, numbers: Numbers) -> Expr {
    match (ty, numbers) {
        (Some(known), _) => apply(Op::Fit(known), vec![expr]),
        (None, Numbers::Wide) => expr,
        (None, Numbers::Javascript) => apply(Op::Safe, vec![expr]),
    }
}
