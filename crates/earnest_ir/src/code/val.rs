use super::cond::Cond;
use super::num::{Num, Stop};
use crate::eval::{
    Test, applied, changed, each, field_of, fields_of, mapped, named, wrong_or_null,
};
use crate::{Expr, Fault, Frame, Op, Shelf, Sweep};
use earnest_value::{Sym, Value};

#[derive(Clone, Debug)]
pub(super) enum Place {
    Field(usize),
    Local(usize),
    Get(Box<Self>, Sym),
}

impl Place {
    pub(super) fn of(expr: &Expr) -> Option<Self> {
        match plain(expr) {
            Expr::Field(index) => Some(Self::Field(*index)),
            Expr::Local(slot) => Some(Self::Local(*slot)),
            Expr::Get(inner, name) => Some(Self::Get(Box::new(Self::of(inner)?), *name)),
            _ => None,
        }
    }

    pub(super) fn find<'f>(&self, frame: &'f Frame<'_>) -> Result<&'f Value, Fault> {
        match self {
            Self::Field(index) => frame.state.get(*index).ok_or(Fault::WrongKind),
            Self::Local(slot) => frame.locals.get(*slot).ok_or(Fault::WrongKind),
            Self::Get(inner, name) => {
                let record = inner.find(frame)?;
                record.field(*name).ok_or_else(|| wrong_or_null(record))
            }
        }
    }
}

pub(super) fn plain(expr: &Expr) -> &Expr {
    match expr {
        Expr::Apply(Op::SomeOf, arguments) => match arguments.as_slice() {
            [inner] => plain(inner),
            _ => expr,
        },
        _ => expr,
    }
}

#[derive(Clone, Debug)]
pub(super) struct Binding<T> {
    pub(super) slot: usize,
    pub(super) value: Val,
    pub(super) body: T,
}

impl<T> Binding<T> {
    pub(super) fn enter(&self, frame: &mut Frame<'_>) -> Result<(), Fault> {
        let bound = self.value.eval(frame)?;
        frame.put(self.slot, bound);
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(super) struct Sweeping {
    sweep: Sweep,
    shelf: Shelf,
    slot: usize,
    source: Val,
    body: Cond,
}

#[derive(Clone, Debug)]
pub(super) enum Val {
    Const(Value),
    Read(Place),
    Num(Num),
    Cond(Cond),
    Choose(Box<(Cond, Self, Self)>),
    Bind(Box<Binding<Self>>),
    Get(Box<Self>, Sym),
    Record(Box<[(Sym, Self)]>),
    With(Box<Self>, Box<[(Sym, Self)]>),
    Apply(Op, Box<[Self]>),
    SetOf(Box<[Self]>),
    SeqOf(Box<[Self]>),
    MapOf(Box<[(Self, Self)]>),
    Sweep(Box<Sweeping>),
}

fn all_of(items: &[Expr]) -> Box<[Val]> {
    items.iter().map(Val::of).collect()
}

fn all_named(fields: &[(Sym, Expr)]) -> Box<[(Sym, Val)]> {
    fields
        .iter()
        .map(|(name, inner)| (*name, Val::of(inner)))
        .collect()
}

impl Val {
    pub(super) fn of(expr: &Expr) -> Self {
        match plain(expr) {
            Expr::Const(Value::Bool(flag)) => Self::Cond(Cond::Const(*flag)),
            Expr::Const(value) => Self::Const(value.clone()),
            Expr::Field(index) => Self::Read(Place::Field(*index)),
            Expr::Local(slot) => Self::Read(Place::Local(*slot)),
            Expr::Get(inner, name) => match Place::of(inner) {
                Some(place) => Self::Read(Place::Get(Box::new(place), *name)),
                None => Self::Get(Box::new(Self::of(inner)), *name),
            },
            Expr::Choose(test, yes, no) => {
                Self::Choose(Box::new((Cond::of(test), Self::of(yes), Self::of(no))))
            }
            Expr::Bind { slot, value, body } => Self::Bind(Box::new(Binding {
                slot: *slot,
                value: Self::of(value),
                body: Self::of(body),
            })),
            Expr::Record(fields) => Self::Record(all_named(fields)),
            Expr::With(base, changes) => Self::With(Box::new(Self::of(base)), all_named(changes)),
            Expr::SetOf(items) => Self::SetOf(all_of(items)),
            Expr::SeqOf(items) => Self::SeqOf(all_of(items)),
            Expr::MapOf(entries) => Self::MapOf(
                entries
                    .iter()
                    .map(|(key, value)| (Self::of(key), Self::of(value)))
                    .collect(),
            ),
            Expr::Sweep {
                sweep,
                shelf,
                slot,
                source,
                body,
            } => Self::Sweep(Box::new(Sweeping {
                sweep: *sweep,
                shelf: *shelf,
                slot: *slot,
                source: Self::of(source),
                body: Cond::of(body),
            })),
            shaped @ (Expr::And(..) | Expr::Or(..)) => Self::Cond(Cond::of(shaped)),
            shaped @ Expr::Apply(op, arguments) => {
                if let Some(num) = Num::shaped(shaped) {
                    Self::Num(num)
                } else if let Some(cond) = Cond::shaped(shaped) {
                    Self::Cond(cond)
                } else {
                    Self::Apply(*op, all_of(arguments))
                }
            }
        }
    }

    pub(super) fn eval(&self, frame: &mut Frame<'_>) -> Result<Value, Fault> {
        match self {
            Self::Const(value) => Ok(value.clone()),
            Self::Read(place) => place.find(frame).cloned(),
            Self::Num(num) => num.eval(frame).map(Value::Int).map_err(Stop::fault),
            Self::Cond(cond) => cond.eval(frame).map(Value::Bool),
            Self::Choose(branches) => {
                if branches.0.eval(frame)? {
                    branches.1.eval(frame)
                } else {
                    branches.2.eval(frame)
                }
            }
            Self::Bind(binding) => {
                binding.enter(frame)?;
                binding.body.eval(frame)
            }
            Self::Get(inner, name) => field_of(&inner.eval(frame)?, *name),
            Self::Record(fields) => Ok(Value::record(named(fields, frame, Self::eval)?)),
            Self::With(base, changes) => {
                let fields = fields_of(base.eval(frame)?)?;
                Ok(changed(fields, named(changes, frame, Self::eval)?))
            }
            Self::Apply(op, arguments) => applied(*op, arguments, frame, Self::eval),
            Self::SetOf(items) => Ok(Value::set(
                each(items, frame, Self::eval)?.into_iter().collect(),
            )),
            Self::SeqOf(items) => Ok(Value::seq(each(items, frame, Self::eval)?)),
            Self::MapOf(entries) => mapped(entries, frame, Self::eval),
            Self::Sweep(sweeping) => {
                let test = Test {
                    sweep: sweeping.sweep,
                    shelf: sweeping.shelf,
                    slot: sweeping.slot,
                    body: &sweeping.body,
                };
                test.over(&sweeping.source.eval(frame)?, frame, Cond::eval)
            }
        }
    }
}
