//! The typed lowering: tree to checked model, where each primitive gets its meaning.
//!
//! [`lower`] takes one [`Draft`] from a reader and gives back a [`Model`] the search can run,
//! or every [`Problem`] that stops it: a name the file does not define, a primitive that does
//! not exist, a value of the wrong kind, a helper that calls itself, a starting value outside
//! its field, or a declaration with too many values to try.
//!
//! A draft's language decides two things. In a draft read from Rust every number is computed
//! in its integer type, and a result outside the type is a fault when the model runs; and an
//! optional value that would sit inside another optional value is a problem, because Rust
//! tells `Some(None)` from `None` and the model's values can't.

mod methods;
mod terms;
mod types;

use earnest_ast::{
    ActionBody, ActionDraft, Draft, EndDraft, FieldDraft, HelperKind, LabelDraft, MOST_NESTING,
    RuleDraft, Shape, ShapeKind, TOO_DEEP, TemplatePiece, Term, TermKind,
};
use earnest_diagnostics::{Place, Problem};
use earnest_ir::{
    Action, Code, End, Expr, Field, Frame, Input, Label, Model, Piece, Rule, View, eval,
};
use earnest_value::{Domain, MOST_VALUES_LISTED, Symbols, Value};
use std::collections::{BTreeMap, BTreeSet};
use types::{Binding, Scope, Type, Typed};

pub(crate) struct Stop;

type Lowering<T> = Result<T, Stop>;

struct Lowerer<'a> {
    draft: &'a Draft,
    symbols: Symbols,
    problems: Vec<Problem>,
    fields: Vec<(String, Type)>,
    in_flight: Vec<(Option<HelperKind>, String)>,
    depth: usize,
}

struct Shaped {
    domain: Domain,
    ty: Type,
    start: Option<Value>,
}

/// Checks every name and kind in `draft` and gives the model the search runs.
///
/// Fields keep the draft's order, and so do actions, rules and ends. A field without a
/// starting value starts at every value it declares, or empty, free, idle or at zero for a
/// collection, lock, timer, request or retry. Every top-level constant is checked, also one
/// that nothing reads. A record literal is a value of a declared record when each field is,
/// so a field declared optional takes a plain value or `null`.
///
/// # Errors
///
/// Every problem found, each at the place of the term or declaration that caused it, in the
/// draft's file. With any problem there is no model: a partly understood model would be
/// checked against the wrong rules.
pub fn lower(draft: &Draft) -> Result<Model, Vec<Problem>> {
    let met = lowered(draft, Symbols::default())?;
    lowered(draft, met.symbols.alphabetical())
}

fn lowered(draft: &Draft, symbols: Symbols) -> Result<Model, Vec<Problem>> {
    let mut lowerer = Lowerer::reading(draft, symbols);
    let parts = lowerer.parts();
    lowerer.finish(parts)
}

/// Lowers `draft` as [`lower`] does, and with it the rules of `earlier`, an older version of
/// the same model, so that an old rule and a new one can be evaluated in the same state.
///
/// Each rule of `earlier` is read against the fields of `draft`, but with the constants and
/// the helper functions of `earlier`: a rule whose text did not change still differs when a
/// constant or a helper it uses did. The rules come back in the order of `earlier.rules`,
/// with `None` for a rule that can't be read against the fields of `draft`. They share the
/// model's symbols, so they are only meaningful together with the model returned here.
///
/// # Errors
///
/// The problems of `draft`, exactly as [`lower`] gives them. A problem in `earlier` is never
/// an error.
pub fn lower_with_earlier(
    draft: &Draft,
    earlier: &Draft,
) -> Result<(Model, Vec<Option<Rule>>), Vec<Problem>> {
    let (met, _) = lowered_with_earlier(draft, earlier, Symbols::default())?;
    lowered_with_earlier(draft, earlier, met.symbols.alphabetical())
}

fn lowered_with_earlier(
    draft: &Draft,
    earlier: &Draft,
    symbols: Symbols,
) -> Result<(Model, Vec<Option<Rule>>), Vec<Problem>> {
    let mut lowerer = Lowerer::reading(draft, symbols);
    let parts = lowerer.parts();
    if !lowerer.problems.is_empty() {
        return Err(lowerer.problems);
    }
    lowerer.draft = earlier;
    let rules = earlier
        .rules
        .iter()
        .map(|rule| lowerer.rule(rule).ok())
        .collect();
    lowerer.problems.clear();
    lowerer.draft = draft;
    Ok((lowerer.finish(parts)?, rules))
}

struct Parts {
    fields: Vec<Field>,
    actions: Vec<Action>,
    rules: Vec<Rule>,
    ends: Vec<End>,
    view: View,
}

impl<'a> Lowerer<'a> {
    fn reading(draft: &'a Draft, symbols: Symbols) -> Self {
        Self {
            draft,
            symbols,
            problems: Vec::new(),
            fields: Vec::new(),
            in_flight: Vec::new(),
            depth: 0,
        }
    }

    fn parts(&mut self) -> Parts {
        let draft = self.draft;
        let mut fields = Vec::with_capacity(draft.fields.len());
        let mut types = Vec::with_capacity(draft.fields.len());
        for field in &draft.fields {
            if let Ok((lowered, ty)) = self.field(field) {
                types.push((lowered.name.clone(), ty));
                fields.push(lowered);
            }
        }
        self.fields = types;
        let actions: Vec<Action> = draft
            .actions
            .iter()
            .filter_map(|action| self.action(action).ok())
            .collect();
        let rules: Vec<Rule> = draft
            .rules
            .iter()
            .filter_map(|rule| self.rule(rule).ok())
            .collect();
        let ends: Vec<End> = draft
            .ends
            .iter()
            .filter_map(|end| self.end(end).ok())
            .collect();
        let view = View {
            title: draft.view.title.clone(),
            state: self.label(draft.view.state.as_ref()),
            group: self.label(draft.view.group.as_ref()),
            steps: draft.view.steps.clone(),
            colors: draft.view.colors.clone(),
        };
        for (_, constant) in &draft.constants {
            let known = self.problems.len();
            self.term(constant, &mut Scope::default()).ok();
            for problem in self.problems.split_off(known) {
                if !self.problems.contains(&problem) {
                    self.problems.push(problem);
                }
            }
        }
        Parts {
            fields,
            actions,
            rules,
            ends,
            view,
        }
    }

    fn finish(self, parts: Parts) -> Result<Model, Vec<Problem>> {
        if !self.problems.is_empty() {
            return Err(self.problems);
        }
        Ok(Model {
            name: self.draft.name.clone(),
            file: self.draft.file.clone(),
            line: self.draft.place.line,
            symbols: self.symbols,
            fields: parts.fields,
            actions: parts.actions,
            rules: parts.rules,
            ends: parts.ends,
            view: parts.view,
        })
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
            (
                Some(Term {
                    kind: TermKind::Choices(listed),
                    ..
                }),
                _,
            ) => self.listed_starts(listed, &shaped.ty, &shaped.domain)?,
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

    fn listed_starts(
        &mut self,
        listed: &[Term],
        ty: &Type,
        domain: &Domain,
    ) -> Lowering<Vec<Value>> {
        let mut starts: Vec<Value> = Vec::with_capacity(listed.len());
        for term in listed {
            let value = self.constant(term, ty, domain)?;
            if starts.contains(&value) {
                let shown = self.symbols.show(&value);
                return Err(self.stop(
                    term.place,
                    &format!("the starting value {shown} listed twice"),
                    "list each starting value once",
                ));
            }
            starts.push(value);
        }
        Ok(starts)
    }

    fn constant(&mut self, term: &Term, ty: &Type, domain: &Domain) -> Lowering<Value> {
        let mut scope = Scope::default();
        let lowered = self.term(term, &mut scope)?;
        let expr = self.coerce(lowered, ty, term.place)?;
        let value = match eval(&expr, &mut Frame::new(&[], scope.slots)) {
            Ok(value) => value,
            Err(fault) => {
                return Err(self.stop(term.place, "a starting value", &fault.describe()));
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
            scope.inputs.push((name.clone(), slot, shaped.ty));
            inputs.push(Input {
                name: name.clone(),
                slot,
                domain: shaped.domain,
                values,
            });
        }
        let guard = match &draft.guard {
            Some(body) => {
                self.enter(body, &mut scope)?;
                self.truth(&body.term, &mut scope)?
            }
            None => Expr::Const(Value::Bool(true)),
        };
        self.enter(&draft.change, &mut scope)?;
        let updates = self.changes(&draft.change.term, &mut scope)?;
        let changed: Vec<bool> = updates.iter().map(Option::is_some).collect();
        let mut prepared =
            Code::together(std::iter::once(guard).chain(updates.into_iter().flatten())).into_iter();
        let guard = prepared
            .next()
            .expect("the guard is prepared first, before the updates");
        Ok(Action {
            id: draft.id.clone(),
            label: draft.label.clone(),
            line: draft.place.line,
            inputs,
            guard,
            updates: changed
                .into_iter()
                .map(|present| if present { prepared.next() } else { None })
                .collect(),
            slots: scope.slots,
        })
    }

    fn enter(&mut self, body: &ActionBody, scope: &mut Scope) -> Lowering<()> {
        scope.names.clear();
        bind_state(scope, body.state_name.as_ref());
        if let Some(input) = &body.input_name {
            scope.names.push((input.clone(), Binding::Inputs));
        }
        for name in &body.taken {
            let Some((slot, ty)) = scope.input(name) else {
                return Err(self.stop(
                    body.term.place,
                    &format!("the input `{name}`"),
                    "the action declares no input with this name",
                ));
            };
            scope.names.push((name.clone(), Binding::Local(slot, ty)));
        }
        Ok(())
    }

    fn changes(&mut self, term: &Term, scope: &mut Scope) -> Lowering<Vec<Option<Expr>>> {
        if self.depth >= MOST_NESTING {
            return Err(self.stop(term.place, "a deeply nested expression", TOO_DEEP));
        }
        self.depth = self.depth.saturating_add(1);
        let lowered = self.changes_of(term, scope);
        self.depth = self.depth.saturating_sub(1);
        lowered
    }

    fn changes_of(&mut self, term: &Term, scope: &mut Scope) -> Lowering<Vec<Option<Expr>>> {
        if let TermKind::Record { base, fields, .. } = &term.kind {
            return self.changed_fields(base.as_deref(), fields, scope);
        }
        if let TermKind::Wrap(inner) | TermKind::Updates(inner) = &term.kind {
            return self.changes(inner, scope);
        }
        if let TermKind::Name(owner) = &term.kind
            && matches!(scope.find(owner), Some(Binding::State))
        {
            return Ok(vec![None; self.fields.len()]);
        }
        if let TermKind::Unpack {
            value,
            name,
            present,
            absent,
            absent_first: false,
        } = &term.kind
        {
            return self.unpack_changes((value, name, present, absent), scope);
        }
        if let TermKind::Helper {
            kind,
            name,
            arguments,
        } = &term.kind
        {
            return self.helper_changes(*kind, name, arguments, term.place, scope);
        }
        if let TermKind::Bind { name, value, body } = &term.kind {
            let bound = self.bound(name, value, scope)?;
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
        let mut updates = vec![None; self.fields.len()];
        if let Some(spread) = base {
            match &spread.kind {
                TermKind::Name(owner) if matches!(scope.find(owner), Some(Binding::State)) => {}
                TermKind::Helper {
                    kind,
                    name,
                    arguments,
                } => {
                    updates = self.helper_changes(*kind, name, arguments, spread.place, scope)?;
                }
                _ => {
                    return Err(self.stop(
                        spread.place,
                        "a spread in an action result",
                        "an action gives back only the fields it changes; spread the state, \
                         `..self.clone()`, or a helper that returns `Self`, `..self.helper(..)`",
                    ));
                }
            }
        }
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
                *update = Some(match update.take() {
                    Some(dropped) => Expr::Bind {
                        slot: scope.slot(),
                        value: Box::new(dropped),
                        body: Box::new(coerced),
                    },
                    None => coerced,
                });
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
            holds: Code::new(holds),
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
            when: Code::new(when),
            slots: scope.slots,
        })
    }

    fn label(&mut self, draft: Option<&LabelDraft>) -> Option<Label> {
        let found = draft?;
        let mut scope = Scope::default();
        bind_state(&mut scope, found.state_name.as_ref());
        let pieces = if let TermKind::Template(parts) = &found.term.kind {
            let mut pieces = Vec::with_capacity(parts.len());
            for part in parts {
                pieces.push(self.piece(part, found.term.place, &mut scope).ok()?);
            }
            pieces
        } else {
            vec![self.shown(&found.term, Shown::Display, &mut scope).ok()?]
        };
        Some(Label {
            pieces,
            slots: scope.slots,
        })
    }

    fn shown(&mut self, term: &Term, how: Shown, scope: &mut Scope) -> Lowering<Piece> {
        let typed = self.term(term, scope)?;
        if self.is_rust() {
            let (feature, why) = match (&typed.ty, how) {
                (Type::Bool | Type::Int(_), Shown::Display | Shown::Debug)
                | (Type::Text, Shown::Display | Shown::Owned)
                | (Type::Variant(_), Shown::Debug) => {
                    return Ok(Piece::Shown(Code::new(typed.expr)));
                }
                (Type::Bool | Type::Int(_), Shown::Owned) => (
                    "`.to_owned()`",
                    "`.to_owned()` of a boolean or a number is that boolean or number, not a \
                     `String`, and Rust refuses it: write `.to_string()`",
                ),
                (Type::Text, Shown::Debug) => (
                    "`{:?}`",
                    "`{:?}` prints a text in quotes and the chart prints it without: write `{}`",
                ),
                (Type::Variant(_), Shown::Display) => (
                    "`{}`",
                    "an enum has no `Display`, so `{}` and `.to_string()` of an enum variant do \
                     not compile: write `{:?}`",
                ),
                (Type::Variant(_), Shown::Owned) => (
                    "`.to_owned()`",
                    "`.to_owned()` of an enum variant is the variant, not a `String`, and Rust \
                     refuses it: write `format!(\"{:?}\", value)`",
                ),
                _ => (
                    "a value in a label",
                    "a Rust label shows a boolean, a number, an enum variant or a text: `Debug` \
                     prints anything else differently from the chart",
                ),
            };
            return Err(self.stop(term.place, feature, why));
        }
        Ok(Piece::Shown(Code::new(typed.expr)))
    }

    fn piece(&mut self, part: &TemplatePiece, place: Place, scope: &mut Scope) -> Lowering<Piece> {
        match part {
            TemplatePiece::Text(text) => Ok(Piece::Text(text.clone())),
            TemplatePiece::Value(term) => self.shown(term, Shown::Display, scope),
            TemplatePiece::Debug(term) => self.shown(term, Shown::Debug, scope),
            TemplatePiece::Owned(term) => self.shown(term, Shown::Owned, scope),
            _ => Err(self.stop(
                place,
                "a part of a template string",
                "this version of the checker does not know it",
            )),
        }
    }

    fn shape(&mut self, shape: &Shape) -> Lowering<Shaped> {
        match &shape.kind {
            ShapeKind::Bool => Ok(plain(Domain::Bool, Type::Bool)),
            ShapeKind::Text => Ok(plain(Domain::OneOf(Vec::new()), Type::Text)),
            ShapeKind::Int { lo, hi, ty } => {
                if lo > hi {
                    return Err(self.stop(
                        shape.place,
                        &format!("a number range from {lo} to {hi}"),
                        "the lower bound is above the upper bound",
                    ));
                }
                Ok(plain(Domain::Int { lo: *lo, hi: *hi }, Type::Int(*ty)))
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
            ShapeKind::Record { owner, fields } => self.record_shape(owner.as_deref(), fields),
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
                let values = self.held(value, "a map value")?;
                Ok(Shaped {
                    domain: Domain::Map(Box::new(keys.domain), Box::new(values.domain)),
                    ty: Type::Map(Box::new(keys.ty), Box::new(values.ty)),
                    start: Some(Value::map(BTreeMap::new())),
                })
            }
            ShapeKind::Seq { item, max } => {
                let shaped = self.held(item, "a sequence item")?;
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
                let shaped = self.held(item, "a queue item")?;
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
            ShapeKind::Retry { max } => {
                if *max < 0 {
                    return Err(self.stop(
                        shape.place,
                        &format!("a retry counter with a max of {max}"),
                        "the most attempts can't be below zero",
                    ));
                }
                Ok(Shaped {
                    domain: Domain::Int { lo: 0, hi: *max },
                    ty: Type::Retry(*max),
                    start: Some(Value::Int(0)),
                })
            }
            _ => Err(self.stop(
                shape.place,
                "a declaration",
                "this version of the checker does not know it",
            )),
        }
    }

    fn held(&mut self, shape: &Shape, what: &str) -> Lowering<Shaped> {
        let shaped = self.shape(shape)?;
        if matches!(shaped.ty, Type::Lock(Some(_))) {
            return Err(self.stop(
                shape.place,
                &format!("a lock with an owner as {what}"),
                "reading it gives an optional value, and a free lock is already the value \
                 `null`, so a free lock could not be told from no lock: keep the owner in an \
                 optional field of a record",
            ));
        }
        if self.is_rust() && matches!(shaped.ty, Type::Optional(_)) {
            return Err(self.stop(
                shape.place,
                &format!("an optional value as {what}"),
                "in Rust reading it gives `Some(None)`, an optional value inside an optional \
                 value, which the checker can't tell from `None`: hold a record with an \
                 optional field, or leave the entry out",
            ));
        }
        Ok(shaped)
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

    fn record_shape(
        &mut self,
        owner: Option<&str>,
        fields: &[(String, Shape)],
    ) -> Lowering<Shaped> {
        let mut domains = Vec::with_capacity(fields.len());
        let mut types = Vec::with_capacity(fields.len());
        for (name, inner) in fields {
            let shaped = self.shape(inner)?;
            domains.push((self.symbols.intern(name), shaped.domain));
            types.push((name.clone(), shaped.ty));
        }
        Ok(plain(
            Domain::Record(domains),
            Type::named(owner.map(str::to_owned), types),
        ))
    }

    fn one_of(&mut self, options: &[Term], place: Place) -> Lowering<Shaped> {
        let mut values = Vec::with_capacity(options.len());
        let mut ty = Type::Unknown;
        for option in options {
            let (value, found) = match &option.kind {
                TermKind::Text(text) => (Value::Str(self.symbols.intern(text)), Type::Text),
                TermKind::Variant { owner, name } => (
                    Value::Str(self.symbols.intern(name)),
                    Type::Variant(owner.clone()),
                ),
                TermKind::Int(number) => (Value::Int(*number), Type::Int(None)),
                TermKind::Declared { shape, value } => match (&shape.kind, &value.kind) {
                    (ShapeKind::Int { ty, .. }, TermKind::Int(number)) => {
                        (Value::Int(*number), Type::Int(*ty))
                    }
                    _ => {
                        return Err(self.stop(
                            option.place,
                            "a choice that is not a plain value",
                            "list text, numbers or booleans",
                        ));
                    }
                },
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
            if values.contains(&value) {
                let shown = self.symbols.show(&value);
                return Err(self.stop(
                    option.place,
                    &format!("the choice {shown} listed twice"),
                    "list each value once",
                ));
            }
            values.push(value);
        }
        if values.is_empty() {
            return Err(self.stop(place, "an empty list of choices", "list at least one value"));
        }
        Ok(plain(Domain::OneOf(values), ty))
    }
}

#[derive(Clone, Copy)]
enum Shown {
    Display,
    Debug,
    Owned,
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
