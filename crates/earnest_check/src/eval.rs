use crate::ir::{Expr, MapOp, Math, Op, Order, SeqOp, SetOp, Shelf, Sweep};
use crate::value::{Sym, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    NullRead,
    Overflow,
    Remainder,
    WrongKind,
}

impl Fault {
    #[must_use]
    pub fn describe(self) -> &'static str {
        match self {
            Self::NullRead => "reads a value that is null",
            Self::Overflow => "computes a number too large to hold",
            Self::Remainder => "takes a remainder with a negative number or a divisor below 1",
            Self::WrongKind => "uses a value of the wrong kind",
        }
    }
}

pub(crate) struct Frame<'a> {
    state: &'a [Value],
    locals: Vec<Value>,
}

impl<'a> Frame<'a> {
    pub(crate) fn new(state: &'a [Value], slots: usize) -> Self {
        Self {
            state,
            locals: vec![Value::Null; slots],
        }
    }

    pub(crate) fn put(&mut self, slot: usize, value: Value) {
        if let Some(place) = self.locals.get_mut(slot) {
            *place = value;
        }
    }
}

type Evaluated = Result<Value, Fault>;

pub(crate) fn eval(expr: &Expr, frame: &mut Frame<'_>) -> Evaluated {
    match expr {
        Expr::Const(value) => Ok(value.clone()),
        Expr::Field(index) => frame.state.get(*index).cloned().ok_or(Fault::WrongKind),
        Expr::Local(slot) => frame.locals.get(*slot).cloned().ok_or(Fault::WrongKind),
        Expr::Apply(op, arguments) => apply(*op, arguments, frame),
        Expr::And(left, right) => Ok(Value::Bool(truth(left, frame)? && truth(right, frame)?)),
        Expr::Or(left, right) => Ok(Value::Bool(truth(left, frame)? || truth(right, frame)?)),
        Expr::Choose(test, yes, no) => {
            if truth(test, frame)? {
                eval(yes, frame)
            } else {
                eval(no, frame)
            }
        }
        Expr::Bind { slot, value, body } => {
            let bound = eval(value, frame)?;
            frame.put(*slot, bound);
            eval(body, frame)
        }
        Expr::Record(fields) => Ok(Value::record(named(fields, frame)?)),
        Expr::Get(inner, name) => eval(inner, frame)?
            .field(*name)
            .cloned()
            .ok_or(Fault::NullRead),
        Expr::With(base, changes) => with_fields(base, changes, frame),
        Expr::SetOf(items) => Ok(Value::set(each(items, frame)?.into_iter().collect())),
        Expr::SeqOf(items) => Ok(Value::seq(each(items, frame)?)),
        Expr::MapOf(entries) => {
            let mut built = BTreeMap::new();
            for (key, value) in entries {
                built.insert(eval(key, frame)?, eval(value, frame)?);
            }
            Ok(Value::map(built))
        }
        Expr::Sweep {
            sweep,
            shelf,
            slot,
            source,
            body,
        } => {
            let items = members(eval(source, frame)?)?;
            sweep_over(*sweep, *shelf, *slot, items, body, frame)
        }
    }
}

pub(crate) fn truth(expr: &Expr, frame: &mut Frame<'_>) -> Result<bool, Fault> {
    match eval(expr, frame)? {
        Value::Bool(flag) => Ok(flag),
        Value::Null
        | Value::Int(_)
        | Value::Str(_)
        | Value::Record(_)
        | Value::Set(_)
        | Value::Map(_)
        | Value::Seq(_) => Err(Fault::WrongKind),
    }
}

fn each(items: &[Expr], frame: &mut Frame<'_>) -> Result<Vec<Value>, Fault> {
    let mut built = Vec::with_capacity(items.len());
    for item in items {
        built.push(eval(item, frame)?);
    }
    Ok(built)
}

fn named(fields: &[(Sym, Expr)], frame: &mut Frame<'_>) -> Result<Vec<(Sym, Value)>, Fault> {
    let mut built = Vec::with_capacity(fields.len());
    for (name, inner) in fields {
        built.push((*name, eval(inner, frame)?));
    }
    Ok(built)
}

fn with_fields(base: &Expr, changes: &[(Sym, Expr)], frame: &mut Frame<'_>) -> Evaluated {
    let Value::Record(old) = eval(base, frame)? else {
        return Err(Fault::NullRead);
    };
    let mut fields = Arc::unwrap_or_clone(old);
    for (name, value) in named(changes, frame)? {
        match fields.iter_mut().find(|(key, _)| *key == name) {
            Some((_, place)) => *place = value,
            None => fields.push((name, value)),
        }
    }
    Ok(Value::record(fields))
}

fn members(source: Value) -> Result<Vec<Value>, Fault> {
    match source {
        Value::Set(items) => Ok(items.iter().cloned().collect()),
        Value::Seq(items) => Ok(Arc::unwrap_or_clone(items)),
        Value::Null => Err(Fault::NullRead),
        Value::Bool(_) | Value::Int(_) | Value::Str(_) | Value::Record(_) | Value::Map(_) => {
            Err(Fault::WrongKind)
        }
    }
}

fn sweep_over(
    sweep: Sweep,
    shelf: Shelf,
    slot: usize,
    items: Vec<Value>,
    body: &Expr,
    frame: &mut Frame<'_>,
) -> Evaluated {
    let mut kept = Vec::new();
    for item in items {
        frame.put(slot, item.clone());
        let passed = truth(body, frame)?;
        match sweep {
            Sweep::Every if !passed => return Ok(Value::Bool(false)),
            Sweep::Some if passed => return Ok(Value::Bool(true)),
            Sweep::Filter if passed => kept.push(item),
            Sweep::Every | Sweep::Some | Sweep::Filter => {}
        }
    }
    Ok(match (sweep, shelf) {
        (Sweep::Every, _) => Value::Bool(true),
        (Sweep::Some, _) => Value::Bool(false),
        (Sweep::Filter, Shelf::Set) => Value::set(kept.into_iter().collect()),
        (Sweep::Filter, Shelf::Seq) => Value::seq(kept),
    })
}

fn apply(op: Op, arguments: &[Expr], frame: &mut Frame<'_>) -> Evaluated {
    let mut values = each(arguments, frame)?.into_iter();
    let first = values.next().ok_or(Fault::WrongKind)?;
    let second = values.next();
    let third = values.next();
    match op {
        Op::Not => match first {
            Value::Bool(flag) => Ok(Value::Bool(!flag)),
            other => Err(wrong_or_null(&other)),
        },
        Op::Negate => match first {
            Value::Int(number) => number.checked_neg().map(Value::Int).ok_or(Fault::Overflow),
            other => Err(wrong_or_null(&other)),
        },
        Op::SomeOf => Ok(first),
        Op::Unwrap => match first {
            Value::Null => Err(Fault::NullRead),
            inner => Ok(inner),
        },
        Op::Equal => Ok(Value::Bool(Some(first) == second)),
        Op::Differ => Ok(Value::Bool(Some(first) != second)),
        Op::Order(order) => compare(order, &first, second.as_ref()),
        Op::Math(math) => arithmetic(math, &first, second.as_ref()),
        Op::Set(set_op) => on_set(set_op, first, second),
        Op::Map(map_op) => on_map(map_op, first, second, third),
        Op::Seq(seq_op) => on_seq(seq_op, first, second),
    }
}

fn wrong_or_null(value: &Value) -> Fault {
    if *value == Value::Null {
        Fault::NullRead
    } else {
        Fault::WrongKind
    }
}

fn int_pair(first: &Value, second: Option<&Value>) -> Result<(i64, i64), Fault> {
    match (first, second) {
        (Value::Int(left), Some(Value::Int(right))) => Ok((*left, *right)),
        (Value::Null, _) | (_, Some(Value::Null)) => Err(Fault::NullRead),
        _ => Err(Fault::WrongKind),
    }
}

fn compare(order: Order, first: &Value, second: Option<&Value>) -> Evaluated {
    let (left, right) = int_pair(first, second)?;
    Ok(Value::Bool(match order {
        Order::Less => left < right,
        Order::LessOrEqual => left <= right,
        Order::Greater => left > right,
        Order::GreaterOrEqual => left >= right,
    }))
}

fn arithmetic(math: Math, first: &Value, second: Option<&Value>) -> Evaluated {
    let (left, right) = int_pair(first, second)?;
    let answer = match math {
        Math::Add => left.checked_add(right),
        Math::Subtract => left.checked_sub(right),
        Math::Multiply => left.checked_mul(right),
        Math::Remainder => {
            if left < 0 || right < 1 {
                return Err(Fault::Remainder);
            }
            left.checked_rem(right)
        }
    };
    answer.map(Value::Int).ok_or(Fault::Overflow)
}

fn size_of(count: usize) -> Evaluated {
    i64::try_from(count)
        .map(Value::Int)
        .map_err(|_| Fault::Overflow)
}

fn as_set(value: Value) -> Result<BTreeSet<Value>, Fault> {
    match value {
        Value::Set(shared) => Ok(Arc::unwrap_or_clone(shared)),
        other => Err(wrong_or_null(&other)),
    }
}

fn on_set(op: SetOp, first: Value, second: Option<Value>) -> Evaluated {
    let mut items = as_set(first)?;
    if op == SetOp::Size {
        return size_of(items.len());
    }
    let other = second.ok_or(Fault::WrongKind)?;
    Ok(match op {
        SetOp::Size => Value::Null,
        SetOp::Has => Value::Bool(items.contains(&other)),
        SetOp::Add => {
            items.insert(other);
            Value::set(items)
        }
        SetOp::Delete => {
            items.remove(&other);
            Value::set(items)
        }
        SetOp::Union => Value::set(items.union(&as_set(other)?).cloned().collect()),
        SetOp::Intersect => Value::set(items.intersection(&as_set(other)?).cloned().collect()),
        SetOp::Subtract => Value::set(items.difference(&as_set(other)?).cloned().collect()),
        SetOp::IsSubset => Value::Bool(items.is_subset(&as_set(other)?)),
    })
}

fn on_map(op: MapOp, first: Value, second: Option<Value>, third: Option<Value>) -> Evaluated {
    let mut entries = match first {
        Value::Map(shared) => Arc::unwrap_or_clone(shared),
        other => return Err(wrong_or_null(&other)),
    };
    match op {
        MapOp::Size => return size_of(entries.len()),
        MapOp::Keys => return Ok(Value::set(entries.keys().cloned().collect())),
        MapOp::Has | MapOp::Get | MapOp::Set | MapOp::Delete => {}
    }
    let key = second.ok_or(Fault::WrongKind)?;
    Ok(match op {
        MapOp::Size | MapOp::Keys => Value::Null,
        MapOp::Has => Value::Bool(entries.contains_key(&key)),
        MapOp::Get => entries.get(&key).cloned().unwrap_or(Value::Null),
        MapOp::Delete => {
            entries.remove(&key);
            Value::map(entries)
        }
        MapOp::Set => {
            entries.insert(key, third.ok_or(Fault::WrongKind)?);
            Value::map(entries)
        }
    })
}

fn on_seq(op: SeqOp, first: Value, second: Option<Value>) -> Evaluated {
    let mut items = match first {
        Value::Seq(shared) => Arc::unwrap_or_clone(shared),
        other => return Err(wrong_or_null(&other)),
    };
    match op {
        SeqOp::Size => size_of(items.len()),
        SeqOp::First => Ok(items.first().cloned().unwrap_or(Value::Null)),
        SeqOp::Last => Ok(items.last().cloned().unwrap_or(Value::Null)),
        SeqOp::Shift => Ok(Value::seq(items.into_iter().skip(1).collect())),
        SeqOp::Pop => {
            items.pop();
            Ok(Value::seq(items))
        }
        SeqOp::Get => {
            let Some(Value::Int(position)) = second else {
                return Err(Fault::WrongKind);
            };
            let found = usize::try_from(position)
                .ok()
                .and_then(|index| items.get(index));
            Ok(found.cloned().unwrap_or(Value::Null))
        }
        SeqOp::Push => {
            items.push(second.ok_or(Fault::WrongKind)?);
            Ok(Value::seq(items))
        }
        SeqOp::Includes => Ok(Value::Bool(items.contains(&second.ok_or(Fault::WrongKind)?))),
    }
}
