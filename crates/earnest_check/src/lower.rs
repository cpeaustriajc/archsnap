mod methods;
mod terms;
mod types;

use crate::domain::{Domain, MOST_VALUES_LISTED};
use crate::eval::{Frame, eval};
use crate::ir::{Action, End, Expr, Field, Input, Label, Model, Piece, Rule, View};
use crate::surface::{
    ActionDraft, Draft, EndDraft, FieldDraft, LabelDraft, Place, Problem, RuleDraft, Shape,
    ShapeKind, TemplatePiece, Term, TermKind,
};
use crate::value::{Symbols, Value};
use std::collections::{BTreeMap, BTreeSet};
use types::{Binding, Scope, Type, Typed};

#[derive(Debug)]
pub struct Lowered {
    pub model: Option<Model>,
    pub problems: Vec<Problem>,
}

pub(crate) struct Stop;

type Lowering<T> = Result<T, Stop>;

struct Lowerer<'a> {
    draft: &'a Draft,
    symbols: Symbols,
    problems: Vec<Problem>,
    fields: Vec<(String, Type)>,
    in_flight: Vec<String>,
}

struct Shaped {
    domain: Domain,
    ty: Type,
    start: Option<Value>,
}

#[must_use]
pub fn lower(draft: &Draft) -> Lowered {
    let mut lowerer = Lowerer {
        draft,
        symbols: Symbols::default(),
        problems: Vec::new(),
        fields: Vec::new(),
        in_flight: Vec::new(),
    };
    let mut fields = Vec::with_capacity(draft.fields.len());
    let mut types = Vec::with_capacity(draft.fields.len());
    for field in &draft.fields {
        if let Ok((lowered, ty)) = lowerer.field(field) {
            types.push((lowered.name.clone(), ty));
            fields.push(lowered);
        }
    }
    lowerer.fields = types;
    let actions: Vec<Action> = draft
        .actions
        .iter()
        .filter_map(|action| lowerer.action(action).ok())
        .collect();
    let rules: Vec<Rule> = draft
        .rules
        .iter()
        .filter_map(|rule| lowerer.rule(rule).ok())
        .collect();
    let ends: Vec<End> = draft
        .ends
        .iter()
        .filter_map(|end| lowerer.end(end).ok())
        .collect();
    let view = View {
        title: draft.view.title.clone(),
        state: lowerer.label(draft.view.state.as_ref()),
        group: lowerer.label(draft.view.group.as_ref()),
        steps: draft.view.steps.clone(),
        colors: draft.view.colors.clone(),
    };
    if !lowerer.problems.is_empty() {
        return Lowered {
            model: None,
            problems: lowerer.problems,
        };
    }
    Lowered {
        model: Some(Model {
            name: draft.name.clone(),
            file: draft.file.clone(),
            line: draft.place.line,
            symbols: lowerer.symbols,
            fields,
            actions,
            rules,
            ends,
            view,
        }),
        problems: Vec::new(),
    }
}

impl Lowerer<'_> {
    fn stop(&mut self, place: Place, feature: &str, message: &str) -> Stop {
        self.problems
            .push(Problem::new(&self.draft.file, place, feature, message));
        Stop
    }

    fn field(&mut self, draft: &FieldDraft) -> Lowering<(Field, Type)> {
        let shaped = self.shape(&draft.shape)?;
        let starts = match (&draft.shape.initial, shaped.start) {
            (Some(term), _) => vec![self.constant(term, &shaped.ty, &shaped.domain)?],
            (None, Some(value)) => vec![value],
            (None, None) => self.all_values(&shaped.domain, draft.shape.place)?,
        };
        let field = Field {
            name: draft.name.clone(),
            sym: self.symbols.intern(&draft.name),
            domain: shaped.domain,
            starts,
        };
        Ok((field, shaped.ty))
    }

    fn constant(&mut self, term: &Term, ty: &Type, domain: &Domain) -> Lowering<Value> {
        let mut scope = Scope::default();
        let lowered = self.term(term, &mut scope)?;
        let expr = self.coerce(lowered, ty, term.place)?;
        let value = match eval(&expr, &mut Frame::new(&[], scope.slots)) {
            Ok(value) => value,
            Err(fault) => {
                return Err(self.stop(term.place, "a starting value", fault.describe()));
            }
        };
        if domain.contains(&value) {
            return Ok(value);
        }
        let shown = self.symbols.show(&value);
        Err(self.stop(
            term.place,
            &format!("the starting value {shown}"),
            "it is outside the values the field declares",
        ))
    }

    fn all_values(&mut self, domain: &Domain, place: Place) -> Lowering<Vec<Value>> {
        if domain.count() > MOST_VALUES_LISTED {
            return Err(self.stop(
                place,
                "a declaration with more than a million values",
                "give it a starting value or narrower bounds, so every value can be tried",
            ));
        }
        Ok(domain.values())
    }

    fn action(&mut self, draft: &ActionDraft) -> Lowering<Action> {
        let mut scope = Scope::default();
        let mut inputs = Vec::with_capacity(draft.inputs.len());
        for (name, shape) in &draft.inputs {
            let shaped = self.shape(shape)?;
            let values = self.all_values(&shaped.domain, shape.place)?;
            let slot = scope.slot();
            scope.inputs.push((name.clone(), slot, shaped.ty.clone()));
            scope
                .names
                .push((name.clone(), Binding::Local(slot, shaped.ty)));
            inputs.push(Input {
                name: name.clone(),
                slot,
                domain: shaped.domain,
                values,
            });
        }
        bind_state(&mut scope, draft.state_name.as_ref());
        if let Some(input) = &draft.input_name {
            scope.names.push((input.clone(), Binding::Inputs));
        }
        let guard = match &draft.guard {
            Some(term) => self.truth(term, &mut scope)?,
            None => Expr::Const(Value::Bool(true)),
        };
        let updates = self.changes(&draft.change, &mut scope)?;
        Ok(Action {
            id: draft.id.clone(),
            label: draft.label.clone(),
            line: draft.place.line,
            inputs,
            guard,
            updates,
            slots: scope.slots,
        })
    }

    fn changes(&mut self, term: &Term, scope: &mut Scope) -> Lowering<Vec<Option<Expr>>> {
        if let TermKind::Record { base, fields } = &term.kind {
            return self.changed_fields(base.as_deref(), fields, scope);
        }
        if let TermKind::Wrap(inner) = &term.kind {
            return self.changes(inner, scope);
        }
        if let TermKind::Bind { name, value, body } = &term.kind {
            let bound = self.term(value, scope)?;
            let slot = scope.slot();
            scope
                .names
                .push((name.clone(), Binding::Local(slot, bound.ty)));
            let inner = self.changes(body, scope);
            scope.names.pop();
            return Ok(inner?
                .into_iter()
                .map(|update| {
                    update.map(|expr| Expr::Bind {
                        slot,
                        value: Box::new(bound.expr.clone()),
                        body: Box::new(expr),
                    })
                })
                .collect());
        }
        if let TermKind::Choose(test, yes, no) = &term.kind {
            let tested = self.truth(test, scope)?;
            let first = self.changes(yes, scope)?;
            let second = self.changes(no, scope)?;
            return Ok(first
                .into_iter()
                .zip(second)
                .enumerate()
                .map(|(index, pair)| match pair {
                    (None, None) => None,
                    (yes_expr, no_expr) => Some(Expr::Choose(
                        Box::new(tested.clone()),
                        Box::new(yes_expr.unwrap_or(Expr::Field(index))),
                        Box::new(no_expr.unwrap_or(Expr::Field(index))),
                    )),
                })
                .collect());
        }
        Err(self.stop(
            term.place,
            "an action result that is not a record of fields",
            "an action gives back only the fields it changes, as a record",
        ))
    }

    fn changed_fields(
        &mut self,
        base: Option<&Term>,
        fields: &[(String, Term)],
        scope: &mut Scope,
    ) -> Lowering<Vec<Option<Expr>>> {
        if let Some(spread) = base {
            let whole_state = matches!(&spread.kind, TermKind::Name(owner)
                if matches!(scope.find(owner), Some(Binding::State)));
            if !whole_state {
                return Err(self.stop(
                    spread.place,
                    "a spread in an action result",
                    "an action gives back only the fields it changes; the rest stay as they were",
                ));
            }
        }
        let mut updates = vec![None; self.fields.len()];
        for (name, value) in fields {
            let found = self
                .fields
                .iter()
                .enumerate()
                .find(|(_, (known, _))| known == name)
                .map(|(index, (_, ty))| (index, ty.clone()));
            let Some((index, wanted)) = found else {
                return Err(self.stop(
                    value.place,
                    &format!("the state field `{name}`"),
                    "the model declares no field with this name",
                ));
            };
            let lowered = self.term(value, scope)?;
            let coerced = self.coerce(lowered, &wanted, value.place)?;
            if let Some(update) = updates.get_mut(index) {
                *update = Some(coerced);
            }
        }
        Ok(updates)
    }

    fn rule(&mut self, draft: &RuleDraft) -> Lowering<Rule> {
        let mut scope = Scope::default();
        bind_state(&mut scope, draft.state_name.as_ref());
        let holds = self.truth(&draft.holds, &mut scope)?;
        Ok(Rule {
            id: draft.id.clone(),
            says: draft.says.clone(),
            line: draft.place.line,
            text: draft.text.clone(),
            holds,
            slots: scope.slots,
        })
    }

    fn end(&mut self, draft: &EndDraft) -> Lowering<End> {
        let mut scope = Scope::default();
        bind_state(&mut scope, draft.state_name.as_ref());
        let when = self.truth(&draft.when, &mut scope)?;
        Ok(End {
            id: draft.id.clone(),
            says: draft.says.clone(),
            when,
            slots: scope.slots,
        })
    }

    fn label(&mut self, draft: Option<&LabelDraft>) -> Option<Label> {
        let found = draft?;
        let mut scope = Scope::default();
        bind_state(&mut scope, found.state_name.as_ref());
        let mut pieces = Vec::new();
        if let TermKind::Template(parts) = &found.term.kind {
            for part in parts {
                pieces.push(match part {
                    TemplatePiece::Text(text) => Piece::Text(text.clone()),
                    TemplatePiece::Value(term) => {
                        Piece::Shown(self.term(term, &mut scope).ok()?.expr)
                    }
                });
            }
        } else {
            pieces.push(Piece::Shown(self.term(&found.term, &mut scope).ok()?.expr));
        }
        Some(Label {
            pieces,
            slots: scope.slots,
        })
    }

    fn shape(&mut self, shape: &Shape) -> Lowering<Shaped> {
        match &shape.kind {
            ShapeKind::Bool => Ok(plain(Domain::Bool, Type::Bool)),
            ShapeKind::Int { lo, hi } => {
                if lo > hi {
                    return Err(self.stop(
                        shape.place,
                        &format!("a number range from {lo} to {hi}"),
                        "the lower bound is above the upper bound",
                    ));
                }
                Ok(plain(Domain::Int { lo: *lo, hi: *hi }, Type::Int))
            }
            ShapeKind::OneOf(options) => self.one_of(options, shape.place),
            ShapeKind::Optional(inner) => {
                let shaped = self.shape(inner)?;
                if matches!(shaped.domain, Domain::Optional(_)) {
                    return Err(self.stop(
                        shape.place,
                        "an optional value inside an optional value",
                        "an optional value can't hold another optional value or an owned lock",
                    ));
                }
                Ok(plain(
                    Domain::Optional(Box::new(shaped.domain)),
                    Type::optional(shaped.ty),
                ))
            }
            ShapeKind::Record(fields) => self.record_shape(fields),
            ShapeKind::Set(item) => {
                let shaped = self.shape(item)?;
                Ok(Shaped {
                    domain: Domain::Set(Box::new(shaped.domain)),
                    ty: Type::Set(Box::new(shaped.ty)),
                    start: Some(Value::set(BTreeSet::new())),
                })
            }
            ShapeKind::Map(key, value) => {
                let keys = self.shape(key)?;
                let values = self.shape(value)?;
                Ok(Shaped {
                    domain: Domain::Map(Box::new(keys.domain), Box::new(values.domain)),
                    ty: Type::Map(Box::new(keys.ty), Box::new(values.ty)),
                    start: Some(Value::map(BTreeMap::new())),
                })
            }
            ShapeKind::Seq { item, max } => {
                let shaped = self.shape(item)?;
                Ok(Shaped {
                    domain: Domain::Seq {
                        item: Box::new(shaped.domain),
                        max: *max,
                    },
                    ty: Type::Seq(Box::new(shaped.ty)),
                    start: Some(Value::seq(Vec::new())),
                })
            }
            ShapeKind::Queue { item, capacity } => {
                let shaped = self.shape(item)?;
                Ok(Shaped {
                    domain: Domain::Seq {
                        item: Box::new(shaped.domain),
                        max: *capacity,
                    },
                    ty: Type::Queue(Box::new(shaped.ty), *capacity),
                    start: Some(Value::seq(Vec::new())),
                })
            }
            ShapeKind::Lock(owner) => self.lock_shape(owner.as_deref(), shape.place),
            ShapeKind::Timer => Ok(self.staged(&["idle", "running", "fired"], Type::Timer)),
            ShapeKind::Request => {
                Ok(self.staged(&["idle", "pending", "succeeded", "failed"], Type::Request))
            }
            ShapeKind::Retry { max } => Ok(Shaped {
                domain: Domain::Int { lo: 0, hi: *max },
                ty: Type::Retry(*max),
                start: Some(Value::Int(0)),
            }),
        }
    }

    fn staged(&mut self, stages: &[&str], ty: Type) -> Shaped {
        let values: Vec<Value> = stages
            .iter()
            .map(|stage| Value::Str(self.symbols.intern(stage)))
            .collect();
        Shaped {
            start: values.first().cloned(),
            domain: Domain::OneOf(values),
            ty,
        }
    }

    fn lock_shape(&mut self, owner: Option<&Shape>, place: Place) -> Lowering<Shaped> {
        let Some(owned_by) = owner else {
            return Ok(Shaped {
                domain: Domain::Bool,
                ty: Type::Lock(None),
                start: Some(Value::Bool(false)),
            });
        };
        let shaped = self.shape(owned_by)?;
        if matches!(shaped.domain, Domain::Optional(_)) {
            return Err(self.stop(
                place,
                "a lock owned by an optional value",
                "a free lock already has no owner, so the owner can't be optional",
            ));
        }
        Ok(Shaped {
            domain: Domain::Optional(Box::new(shaped.domain)),
            ty: Type::Lock(Some(Box::new(shaped.ty))),
            start: Some(Value::Null),
        })
    }

    fn record_shape(&mut self, fields: &[(String, Shape)]) -> Lowering<Shaped> {
        let mut domains = Vec::with_capacity(fields.len());
        let mut types = Vec::with_capacity(fields.len());
        for (name, inner) in fields {
            let shaped = self.shape(inner)?;
            domains.push((self.symbols.intern(name), shaped.domain));
            types.push((name.clone(), shaped.ty));
        }
        domains.sort_by_key(|(sym, _)| *sym);
        Ok(plain(Domain::Record(domains), Type::record(types)))
    }

    fn one_of(&mut self, options: &[Term], place: Place) -> Lowering<Shaped> {
        let mut values = Vec::with_capacity(options.len());
        let mut ty = Type::Unknown;
        for option in options {
            let (value, found) = match &option.kind {
                TermKind::Text(text) => (Value::Str(self.symbols.intern(text)), Type::Text),
                TermKind::Int(number) => (Value::Int(*number), Type::Int),
                TermKind::Bool(flag) => (Value::Bool(*flag), Type::Bool),
                _ => {
                    return Err(self.stop(
                        option.place,
                        "a choice that is not a plain value",
                        "list text, numbers or booleans",
                    ));
                }
            };
            if !ty.fits(&found) {
                return Err(self.stop(
                    option.place,
                    "choices of different kinds",
                    "every choice in one list must be the same kind of value",
                ));
            }
            ty = found;
            if !values.contains(&value) {
                values.push(value);
            }
        }
        if values.is_empty() {
            return Err(self.stop(place, "an empty list of choices", "list at least one value"));
        }
        Ok(plain(Domain::OneOf(values), ty))
    }
}

fn plain(domain: Domain, ty: Type) -> Shaped {
    Shaped {
        domain,
        ty,
        start: None,
    }
}

fn bind_state(scope: &mut Scope, state_name: Option<&String>) {
    if let Some(name) = state_name {
        scope.names.push((name.clone(), Binding::State));
    }
}

impl From<Typed> for Expr {
    fn from(typed: Typed) -> Self {
        typed.expr
    }
}
