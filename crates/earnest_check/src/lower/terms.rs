use super::types::{Binding, Scope, Type, Typed};
use super::{Lowerer, Lowering};
use crate::ir::{Expr, Math, Op, Order, Shelf, Sweep};
use crate::surface::{Binary, Place, Reach, Shape, Term, TermKind, Unary};
use crate::value::Value;

pub(super) fn apply(op: Op, arguments: Vec<Expr>) -> Expr {
    Expr::Apply(op, arguments)
}

impl Lowerer<'_> {
    pub(super) fn term(&mut self, term: &Term, scope: &mut Scope) -> Lowering<Typed> {
        let place = term.place;
        match &term.kind {
            TermKind::Null => Ok(Typed::new(Expr::Const(Value::Null), Type::Null)),
            TermKind::Bool(flag) => Ok(Typed::new(Expr::Const(Value::Bool(*flag)), Type::Bool)),
            TermKind::Int(number) => Ok(Typed::new(Expr::Const(Value::Int(*number)), Type::Int)),
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
            TermKind::Helper { name, arguments } => self.helper(name, arguments, place, scope),
            TermKind::Unary(unary, inner) => self.unary(*unary, inner, scope),
            TermKind::Binary(binary, left, right) => {
                self.binary(*binary, left, right, place, scope)
            }
            TermKind::Choose(test, yes, no) => self.choose(test, yes, no, place, scope),
            TermKind::Record { base, fields } => {
                self.record(base.as_deref(), fields, place, scope)
            }
            TermKind::SetOf(items) => self.listed(items, Shelf::Set, place, scope),
            TermKind::SeqOf(items) => self.listed(items, Shelf::Seq, place, scope),
            TermKind::MapOf(entries) => self.map_of(entries, place, scope),
            TermKind::Lambda { .. } => Err(self.stop(
                place,
                "a function used as a value",
                "a function can only be passed to every, some, filter, forall and exists",
            )),
            TermKind::Bind { name, value, body } => {
                let bound = self.term(value, scope)?;
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
        }
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

    fn number(&mut self, term: &Term, scope: &mut Scope) -> Lowering<Expr> {
        let lowered = self.term(term, scope)?;
        self.coerce(lowered, &Type::Int, term.place)
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
                &format!("the name `{name}`"),
                "it is not defined in the model file, so the checker can't know its value",
            ));
        };
        let mut fresh = Scope::starting_at(scope.slots);
        let lowered = self.term(constant, &mut fresh);
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
                    return self.method(unwrapped, name, &[], place, scope);
                }
            },
            other => return self.method(Typed::new(on.expr, other), name, &[], place, scope),
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
        name: &str,
        arguments: &[Term],
        place: Place,
        scope: &mut Scope,
    ) -> Lowering<Typed> {
        let draft = self.draft;
        let Some(found) = draft.helpers.iter().find(|helper| helper.name == name) else {
            return Err(self.stop(
                place,
                &format!("a call to `{name}`"),
                "it is not a library primitive or a function defined in the model file",
            ));
        };
        if self.in_flight.iter().any(|running| running == name) {
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
        self.in_flight.push(name.to_owned());
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
                let lowered = self.number(inner, scope)?;
                Ok(Typed::new(apply(Op::Negate, vec![lowered]), Type::Int))
            }
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
        let (op, ty) = match binary {
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
                return self.equality(binary == Binary::Differ, first, second, place);
            }
            Binary::Less => (Op::Order(Order::Less), Type::Bool),
            Binary::LessOrEqual => (Op::Order(Order::LessOrEqual), Type::Bool),
            Binary::Greater => (Op::Order(Order::Greater), Type::Bool),
            Binary::GreaterOrEqual => (Op::Order(Order::GreaterOrEqual), Type::Bool),
            Binary::Add => (Op::Math(Math::Add), Type::Int),
            Binary::Subtract => (Op::Math(Math::Subtract), Type::Int),
            Binary::Multiply => (Op::Math(Math::Multiply), Type::Int),
            Binary::Remainder => (Op::Math(Math::Remainder), Type::Int),
        };
        let first = self.number(left, scope)?;
        let second = self.number(right, scope)?;
        Ok(Typed::new(apply(op, vec![first, second]), ty))
    }

    pub(super) fn equality(
        &mut self,
        differ: bool,
        left: Typed,
        right: Typed,
        place: Place,
    ) -> Lowering<Typed> {
        let op = if differ { Op::Differ } else { Op::Equal };
        let (first, second) = if left.ty.fits(&right.ty) {
            (left.expr, right.expr)
        } else {
            match (&left.ty, &right.ty) {
                (Type::Optional(inner), other) if inner.fits(other) => {
                    (left.expr, apply(Op::SomeOf, vec![right.expr]))
                }
                (other, Type::Optional(inner)) if inner.fits(other) => {
                    (apply(Op::SomeOf, vec![left.expr]), right.expr)
                }
                (have, other) => {
                    let feature = format!("a {} compared with a {}", have.kind(), other.kind());
                    return Err(self.stop(
                        place,
                        &feature,
                        "the two sides can never be equal, so the comparison is a mistake",
                    ));
                }
            }
        };
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
            Expr::Choose(Box::new(tested), Box::new(first_expr), Box::new(second_expr)),
            ty,
        ))
    }

    fn join(&mut self, left: Typed, right: Typed, place: Place) -> Lowering<(Expr, Expr, Type)> {
        if left.ty.fits(&right.ty) {
            let ty = left.ty.merged(&right.ty).merged(&left.ty);
            return Ok((left.expr, right.expr, ty));
        }
        match (&left.ty, &right.ty) {
            (Type::Null, other) => Ok((
                left.expr,
                apply(Op::SomeOf, vec![right.expr]),
                Type::optional(other.clone()),
            )),
            (other, Type::Null) => Ok((
                apply(Op::SomeOf, vec![left.expr]),
                right.expr,
                Type::optional(other.clone()),
            )),
            (Type::Optional(inner), other) if inner.fits(other) => Ok((
                left.expr,
                apply(Op::SomeOf, vec![right.expr]),
                left.ty.clone(),
            )),
            (other, Type::Optional(inner)) if inner.fits(other) => Ok((
                apply(Op::SomeOf, vec![left.expr]),
                right.expr,
                right.ty.clone(),
            )),
            (have, other) => {
                let feature = format!("a choice between a {} and a {}", have.kind(), other.kind());
                Err(self.stop(place, &feature, "both branches must give the same kind of value"))
            }
        }
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
        let on = self.term(spread, scope)?;
        let Type::Record(mut known) = on.ty else {
            return Err(self.stop(
                place,
                &format!("a spread of a {}", on.ty.kind()),
                "only a record can be spread into a record",
            ));
        };
        let mut changes = Vec::with_capacity(exprs.len());
        for ((sym, expr), (name, ty)) in exprs.into_iter().zip(types) {
            match known.iter_mut().find(|(have, _)| *have == name) {
                Some((_, wanted)) => {
                    let coerced = self.coerce(Typed::new(expr, ty), &wanted.clone(), place)?;
                    changes.push((sym, coerced));
                }
                None => {
                    known.push((name, ty));
                    changes.push((sym, expr));
                }
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
        shelf: Shelf,
        place: Place,
        scope: &mut Scope,
    ) -> Lowering<Typed> {
        let mut lowered = Vec::with_capacity(items.len());
        for item in items {
            lowered.push(self.term(item, scope)?);
        }
        let item_type = lowered
            .iter()
            .map(|typed| &typed.ty)
            .find(|ty| !matches!(ty, Type::Unknown | Type::Null))
            .cloned()
            .unwrap_or(Type::Unknown);
        let mut exprs = Vec::with_capacity(lowered.len());
        for typed in lowered {
            exprs.push(self.coerce(typed, &item_type, place)?);
        }
        Ok(match shelf {
            Shelf::Set => Typed::new(Expr::SetOf(exprs), Type::Set(Box::new(item_type))),
            Shelf::Seq => Typed::new(Expr::SeqOf(exprs), Type::Seq(Box::new(item_type))),
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
        let (key_type, value_type) = lowered
            .first()
            .map_or((Type::Unknown, Type::Unknown), |(key, value)| {
                (key.ty.clone(), value.ty.clone())
            });
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
                sweep: match reach {
                    Reach::Every => Sweep::Every,
                    Reach::Some => Sweep::Some,
                },
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
            return Ok(have.expr);
        }
        match (have.expr, have.ty, want) {
            (expr, Type::Optional(inner), wanted) if !matches!(wanted, Type::Optional(_)) => {
                self.coerce(Typed::new(apply(Op::Unwrap, vec![expr]), *inner), wanted, place)
            }
            (expr, ty, Type::Optional(inner)) if !matches!(ty, Type::Optional(_)) => {
                let coerced = self.coerce(Typed::new(expr, ty), inner, place)?;
                Ok(apply(Op::SomeOf, vec![coerced]))
            }
            (Expr::Record(fields), Type::Record(have_types), Type::Record(want_types))
                if have_types.len() == want_types.len() =>
            {
                self.coerce_record(fields, &have_types, want_types, place)
            }
            (Expr::SetOf(items), Type::Set(item), Type::Set(wanted)) => Ok(Expr::SetOf(
                self.coerce_each(items, &item, wanted, place)?,
            )),
            (Expr::SeqOf(items), Type::Seq(item), Type::Seq(wanted) | Type::Queue(wanted, _)) => {
                Ok(Expr::SeqOf(self.coerce_each(items, &item, wanted, place)?))
            }
            (_, ty, wanted) => {
                let feature = format!("a {} where a {} is needed", ty.kind(), wanted.kind());
                Err(self.stop(place, &feature, "the value has the wrong kind for this place"))
            }
        }
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
        fields: Vec<(crate::value::Sym, Expr)>,
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
