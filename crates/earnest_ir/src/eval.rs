use crate::{Expr, MapOp, Math, Op, Order, SeqOp, SetOp, Shelf, Sweep};
use earnest_value::{IntType, MOST_SAFE_INTEGER, Sym, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Instant;

const TICKS_BETWEEN_CLOCK_READS: u32 = 1024;
const NO_ROUND: u64 = 0;
const FIRST_ROUND: u64 = 1;
pub(crate) const NO_UNIT: u64 = 0;

/// Why an expression has no value in a state.
///
/// The search reports a fault as a finding with the path to the state. All but the last can
/// happen in a well-typed model. [`Fault::WrongKind`] means the expression itself is malformed,
/// which a model lowered by `earnest_semantic` never is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Fault {
    /// A value that must be present is `null`.
    NullRead,
    /// A number does not fit in 64 bits, or in a TypeScript model is past what a JavaScript
    /// number holds exactly.
    Overflow,
    /// A remainder with a negative number or a divisor below 1, where languages disagree.
    Remainder,
    /// A division with a negative number or a divisor below 1, where languages disagree on the
    /// result and Rust panics on a divisor of zero.
    Division,
    /// A number is outside the integer type the model file gives it, where Rust stops a test
    /// build and wraps around in a release build.
    Outside(IntType),
    /// An operation got a value of a kind it has no meaning for, too few arguments, or a field
    /// or slot that does not exist.
    WrongKind,
    /// The time limit passed while the expression was being evaluated. It says nothing about
    /// the model: the search drops the state it was evaluating.
    Stopped,
}

impl Fault {
    /// The fault as the end of a sentence that starts with what was evaluated, such as "the
    /// rule `total` reads a value that is null".
    #[must_use]
    pub fn describe(self) -> String {
        match self {
            Self::NullRead => "reads a value that is null".to_owned(),
            Self::Overflow => "computes a number too large to hold".to_owned(),
            Self::Remainder => {
                "takes a remainder with a negative number or a divisor below 1".to_owned()
            }
            Self::Division => {
                "takes a division with a negative number or a divisor below 1".to_owned()
            }
            Self::Outside(ty) => format!("computes a number outside `{}`", ty.name()),
            Self::WrongKind => "uses a value of the wrong kind".to_owned(),
            Self::Stopped => "was stopped by the time limit".to_owned(),
        }
    }
}

/// One state and the local values an expression is evaluated with.
///
/// Slots hold inputs, bindings and the current item of a test. Evaluation writes to them, so
/// reuse a frame only for expressions that were lowered together, such as one action's guard
/// and updates.
#[derive(Debug)]
pub struct Frame<'a> {
    pub(crate) state: &'a [Value],
    pub(crate) locals: Vec<Value>,
    deadline: Option<Instant>,
    ticks: u32,
    expired: bool,
    beyond_32_bits: bool,
    round: u64,
    writer: u64,
    kept_in: Vec<u64>,
}

impl<'a> Frame<'a> {
    /// A frame over `state` with `slots` local slots, all `null`.
    ///
    /// `slots` must be at least the `slots` of the action, rule, end or label being evaluated.
    #[must_use]
    pub fn new(state: &'a [Value], slots: usize) -> Self {
        Self {
            state,
            locals: std::iter::repeat_with(|| Value::Null).take(slots).collect(),
            deadline: None,
            ticks: 0,
            expired: false,
            beyond_32_bits: false,
            round: FIRST_ROUND,
            writer: NO_UNIT,
            kept_in: Vec::new(),
        }
    }

    /// Makes the frame stop evaluation at `deadline`: a long loop in an expression then gives
    /// [`Fault::Stopped`]. `None` is no limit and costs nothing.
    #[must_use]
    pub fn until(mut self, deadline: Option<Instant>) -> Self {
        self.deadline = deadline;
        self
    }

    /// Whether the deadline has passed. The clock is read once in every 1024 calls, and the
    /// answer stays true once it is true. A loop that may run long calls this on each pass.
    pub fn out_of_time(&mut self) -> bool {
        let Some(limit) = self.deadline else {
            return false;
        };
        self.ticks = self.ticks.wrapping_add(1);
        if !self.expired && self.ticks.is_multiple_of(TICKS_BETWEEN_CLOCK_READS) {
            self.expired = Instant::now() >= limit;
        }
        self.expired
    }

    /// Whether any arithmetic evaluated with this frame gave a number outside 32 bits, which
    /// TLC's integers can't hold.
    #[must_use]
    pub fn computed_beyond_32_bits(&self) -> bool {
        self.beyond_32_bits
    }

    pub(crate) fn note(&mut self, answer: &Result<i64, Fault>) {
        self.beyond_32_bits |= match answer {
            Ok(number) => i32::try_from(*number).is_err(),
            Err(fault) => *fault == Fault::Overflow,
        };
    }

    /// Puts `value` in `slot`, as the search does with an action's inputs. A slot the frame
    /// does not have is left out, and reading it later is [`Fault::WrongKind`].
    pub fn put(&mut self, slot: usize, value: Value) {
        self.writer = NO_UNIT;
        self.fill(slot, value);
    }

    pub(crate) fn write(&mut self, unit: u64, slot: usize, value: Value) {
        self.enter(unit);
        self.fill(slot, value);
    }

    pub(crate) fn kept(&mut self, unit: u64, slot: usize) -> bool {
        self.enter(unit);
        self.kept_in.get(slot) == Some(&self.round)
    }

    pub(crate) fn keep(&mut self, slot: usize) {
        if slot >= self.locals.len() {
            return;
        }
        if self.kept_in.len() < self.locals.len() {
            self.kept_in.resize(self.locals.len(), NO_ROUND);
        }
        if let Some(round) = self.kept_in.get_mut(slot) {
            *round = self.round;
        }
    }

    fn enter(&mut self, unit: u64) {
        if self.writer != unit {
            self.writer = unit;
            self.round = self.round.wrapping_add(1);
        }
    }

    fn fill(&mut self, slot: usize, value: Value) {
        if let Some(place) = self.locals.get_mut(slot) {
            *place = value;
        }
    }
}

type Evaluated = Result<Value, Fault>;

/// The value of `expr` in the state and locals of `frame`.
///
/// # Errors
///
/// A [`Fault`] when the expression has no value in this state.
pub fn eval(expr: &Expr, frame: &mut Frame<'_>) -> Result<Value, Fault> {
    match expr {
        Expr::Const(value) => Ok(value.clone()),
        Expr::Field(index) => frame.state.get(*index).cloned().ok_or(Fault::WrongKind),
        Expr::Local(slot) => frame.locals.get(*slot).cloned().ok_or(Fault::WrongKind),
        Expr::Apply(op, arguments) => applied(*op, arguments, frame, eval),
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
        Expr::Record(fields) => Ok(Value::record(named(fields, frame, eval)?)),
        Expr::Get(inner, name) => field_of(&eval(inner, frame)?, *name),
        Expr::With(base, changes) => {
            let fields = fields_of(eval(base, frame)?)?;
            Ok(changed(fields, named(changes, frame, eval)?))
        }
        Expr::SetOf(items) => Ok(Value::set(each(items, frame, eval)?.into_iter().collect())),
        Expr::SeqOf(items) => Ok(Value::seq(each(items, frame, eval)?)),
        Expr::MapOf(entries) => mapped(entries, frame, eval),
        Expr::Sweep {
            sweep,
            shelf,
            slot,
            source,
            body,
        } => {
            let test = Test {
                sweep: *sweep,
                shelf: *shelf,
                slot: *slot,
                body: &**body,
                unit: NO_UNIT,
            };
            test.over(&eval(source, frame)?, frame, truth)
        }
    }
}

/// Whether `expr` is true in the state and locals of `frame`, for a guard or a rule.
///
/// # Errors
///
/// A [`Fault`] when the expression has no value in this state, and [`Fault::NullRead`] or
/// [`Fault::WrongKind`] when its value is not a boolean.
pub fn truth(expr: &Expr, frame: &mut Frame<'_>) -> Result<bool, Fault> {
    as_bool(&eval(expr, frame)?)
}

pub(crate) fn each<T>(
    items: &[T],
    frame: &mut Frame<'_>,
    evaluate: impl Fn(&T, &mut Frame<'_>) -> Evaluated,
) -> Result<Vec<Value>, Fault> {
    let mut built = Vec::with_capacity(items.len());
    for item in items {
        built.push(evaluate(item, frame)?);
    }
    Ok(built)
}

pub(crate) fn named<T>(
    fields: &[(Sym, T)],
    frame: &mut Frame<'_>,
    evaluate: impl Fn(&T, &mut Frame<'_>) -> Evaluated,
) -> Result<Vec<(Sym, Value)>, Fault> {
    let mut built = Vec::with_capacity(fields.len());
    for (name, inner) in fields {
        built.push((*name, evaluate(inner, frame)?));
    }
    Ok(built)
}

pub(crate) fn mapped<T>(
    entries: &[(T, T)],
    frame: &mut Frame<'_>,
    evaluate: impl Fn(&T, &mut Frame<'_>) -> Evaluated,
) -> Evaluated {
    let mut built = BTreeMap::new();
    for (key, value) in entries {
        built.insert(evaluate(key, frame)?, evaluate(value, frame)?);
    }
    Ok(Value::map(built))
}

pub(crate) fn field_of(record: &Value, name: Sym) -> Evaluated {
    record
        .field(name)
        .cloned()
        .ok_or_else(|| wrong_or_null(record))
}

pub(crate) fn fields_of(record: Value) -> Result<Vec<(Sym, Value)>, Fault> {
    match record {
        Value::Record(fields) => Ok(Arc::unwrap_or_clone(fields)),
        other => Err(wrong_or_null(&other)),
    }
}

pub(crate) fn changed(mut fields: Vec<(Sym, Value)>, changes: Vec<(Sym, Value)>) -> Value {
    for (name, value) in changes {
        match fields.iter_mut().find(|(key, _)| *key == name) {
            Some((_, place)) => *place = value,
            None => fields.push((name, value)),
        }
    }
    Value::record(fields)
}

pub(crate) struct Test<'b, B> {
    pub(crate) sweep: Sweep,
    pub(crate) shelf: Shelf,
    pub(crate) slot: usize,
    pub(crate) body: &'b B,
    pub(crate) unit: u64,
}

impl<B> Test<'_, B> {
    pub(crate) fn over(
        &self,
        collection: &Value,
        frame: &mut Frame<'_>,
        judge: impl Fn(&B, &mut Frame<'_>) -> Result<bool, Fault>,
    ) -> Evaluated {
        match collection {
            Value::Set(items) => self.through(items.iter(), frame, judge),
            Value::Seq(items) => self.through(items.iter(), frame, judge),
            other => Err(wrong_or_null(other)),
        }
    }

    fn through<'v>(
        &self,
        items: impl Iterator<Item = &'v Value>,
        frame: &mut Frame<'_>,
        judge: impl Fn(&B, &mut Frame<'_>) -> Result<bool, Fault>,
    ) -> Evaluated {
        let mut kept = Vec::new();
        for item in items {
            if frame.out_of_time() {
                return Err(Fault::Stopped);
            }
            frame.write(self.unit, self.slot, item.clone());
            let passed = judge(self.body, frame)?;
            match self.sweep {
                Sweep::Every if !passed => return Ok(Value::Bool(false)),
                Sweep::Some if passed => return Ok(Value::Bool(true)),
                Sweep::Filter if passed => kept.push(item.clone()),
                Sweep::Every | Sweep::Some | Sweep::Filter => {}
            }
        }
        Ok(self.gathered(kept))
    }

    fn gathered(&self, kept: Vec<Value>) -> Value {
        match (self.sweep, self.shelf) {
            (Sweep::Every, _) => Value::Bool(true),
            (Sweep::Some, _) => Value::Bool(false),
            (Sweep::Filter, Shelf::Set) => Value::set(kept.into_iter().collect()),
            (Sweep::Filter, Shelf::Seq) => Value::seq(kept),
        }
    }
}

pub(crate) fn applied<T>(
    op: Op,
    arguments: &[T],
    frame: &mut Frame<'_>,
    evaluate: impl Fn(&T, &mut Frame<'_>) -> Evaluated,
) -> Evaluated {
    let mut remaining = arguments.iter();
    let first = match remaining.next() {
        Some(argument) => evaluate(argument, frame)?,
        None => return Err(Fault::WrongKind),
    };
    let second = remaining
        .next()
        .map(|argument| evaluate(argument, frame))
        .transpose()?;
    let third = remaining
        .next()
        .map(|argument| evaluate(argument, frame))
        .transpose()?;
    for argument in remaining {
        evaluate(argument, frame)?;
    }
    match op {
        Op::Not => Ok(Value::Bool(!as_bool(&first)?)),
        Op::Negate => as_int(&first)?
            .checked_neg()
            .map(Value::Int)
            .ok_or(Fault::Overflow),
        Op::Unwrap if first == Value::Null => Err(Fault::NullRead),
        Op::SomeOf | Op::Unwrap => Ok(first),
        Op::Safe if is_safe(as_int(&first)?) => Ok(first),
        Op::Safe => Err(Fault::Overflow),
        Op::Fit(ty) if ty.holds(as_int(&first)?) => Ok(first),
        Op::Fit(ty) => Err(Fault::Outside(ty)),
        Op::Equal => Ok(Value::Bool(first == second.ok_or(Fault::WrongKind)?)),
        Op::Differ => Ok(Value::Bool(first != second.ok_or(Fault::WrongKind)?)),
        Op::Order(order) => {
            let (left, right) = int_pair(&first, second.as_ref())?;
            Ok(Value::Bool(compare(order, left, right)))
        }
        Op::Math(math) => {
            let answer = int_pair(&first, second.as_ref())
                .and_then(|(left, right)| arithmetic(math, left, right));
            frame.note(&answer);
            answer.map(Value::Int)
        }
        Op::Set(set_op) => on_set(set_op, first, second),
        Op::Map(map_op) => on_map(map_op, first, second, third),
        Op::Seq(seq_op) => on_seq(seq_op, first, second),
    }
}

pub(crate) fn wrong_or_null(value: &Value) -> Fault {
    if *value == Value::Null {
        Fault::NullRead
    } else {
        Fault::WrongKind
    }
}

pub(crate) fn as_bool(value: &Value) -> Result<bool, Fault> {
    let Value::Bool(flag) = value else {
        return Err(wrong_or_null(value));
    };
    Ok(*flag)
}

fn as_int(value: &Value) -> Result<i64, Fault> {
    let Value::Int(number) = value else {
        return Err(wrong_or_null(value));
    };
    Ok(*number)
}

fn set_in(value: &Value) -> Result<&BTreeSet<Value>, Fault> {
    let Value::Set(shared) = value else {
        return Err(wrong_or_null(value));
    };
    Ok(shared)
}

fn map_in(value: &Value) -> Result<&BTreeMap<Value, Value>, Fault> {
    let Value::Map(shared) = value else {
        return Err(wrong_or_null(value));
    };
    Ok(shared)
}

fn seq_in(value: &Value) -> Result<&[Value], Fault> {
    let Value::Seq(shared) = value else {
        return Err(wrong_or_null(value));
    };
    Ok(shared)
}

fn as_set(value: Value) -> Result<BTreeSet<Value>, Fault> {
    let Value::Set(shared) = value else {
        return Err(wrong_or_null(&value));
    };
    Ok(Arc::unwrap_or_clone(shared))
}

fn as_map(value: Value) -> Result<BTreeMap<Value, Value>, Fault> {
    let Value::Map(shared) = value else {
        return Err(wrong_or_null(&value));
    };
    Ok(Arc::unwrap_or_clone(shared))
}

fn as_seq(value: Value) -> Result<Vec<Value>, Fault> {
    let Value::Seq(shared) = value else {
        return Err(wrong_or_null(&value));
    };
    Ok(Arc::unwrap_or_clone(shared))
}

fn int_pair(first: &Value, second: Option<&Value>) -> Result<(i64, i64), Fault> {
    Ok((as_int(first)?, as_int(second.ok_or(Fault::WrongKind)?)?))
}

pub(crate) fn compare(order: Order, left: i64, right: i64) -> bool {
    match order {
        Order::Less => left < right,
        Order::LessOrEqual => left <= right,
        Order::Greater => left > right,
        Order::GreaterOrEqual => left >= right,
    }
}

pub(crate) fn is_safe(number: i64) -> bool {
    number.unsigned_abs() <= MOST_SAFE_INTEGER.unsigned_abs()
}

pub(crate) fn arithmetic(math: Math, left: i64, right: i64) -> Result<i64, Fault> {
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
        Math::Divide => {
            if left < 0 || right < 1 {
                return Err(Fault::Division);
            }
            left.checked_div(right)
        }
    };
    answer.ok_or(Fault::Overflow)
}

fn size_of(count: usize) -> Evaluated {
    i64::try_from(count)
        .map(Value::Int)
        .or(Err(Fault::Overflow))
}

fn needed(argument: Option<Value>) -> Evaluated {
    argument.ok_or(Fault::WrongKind)
}

fn on_set(op: SetOp, first: Value, second: Option<Value>) -> Evaluated {
    let items = set_in(&first)?;
    match op {
        SetOp::Size => size_of(items.len()),
        SetOp::Has => Ok(Value::Bool(items.contains(&needed(second)?))),
        SetOp::Add => {
            let item = needed(second)?;
            let mut items = as_set(first)?;
            items.insert(item);
            Ok(Value::set(items))
        }
        SetOp::Delete => {
            let item = needed(second)?;
            let mut items = as_set(first)?;
            items.remove(&item);
            Ok(Value::set(items))
        }
        SetOp::Union => {
            let other = needed(second)?;
            Ok(Value::set(items.union(set_in(&other)?).cloned().collect()))
        }
        SetOp::Intersect => {
            let other = needed(second)?;
            Ok(Value::set(
                items.intersection(set_in(&other)?).cloned().collect(),
            ))
        }
        SetOp::Subtract => {
            let other = needed(second)?;
            Ok(Value::set(
                items.difference(set_in(&other)?).cloned().collect(),
            ))
        }
        SetOp::IsSubset => {
            let other = needed(second)?;
            Ok(Value::Bool(items.is_subset(set_in(&other)?)))
        }
    }
}

fn on_map(op: MapOp, first: Value, second: Option<Value>, third: Option<Value>) -> Evaluated {
    let entries = map_in(&first)?;
    match op {
        MapOp::Size => size_of(entries.len()),
        MapOp::Keys => Ok(Value::set(entries.keys().cloned().collect())),
        MapOp::Has => Ok(Value::Bool(entries.contains_key(&needed(second)?))),
        MapOp::Get => Ok(entries
            .get(&needed(second)?)
            .cloned()
            .unwrap_or(Value::Null)),
        MapOp::Delete => {
            let key = needed(second)?;
            let mut entries = as_map(first)?;
            entries.remove(&key);
            Ok(Value::map(entries))
        }
        MapOp::Set => {
            let key = needed(second)?;
            let value = needed(third)?;
            let mut entries = as_map(first)?;
            entries.insert(key, value);
            Ok(Value::map(entries))
        }
    }
}

fn on_seq(op: SeqOp, first: Value, second: Option<Value>) -> Evaluated {
    let items = seq_in(&first)?;
    match op {
        SeqOp::Size => size_of(items.len()),
        SeqOp::First => Ok(items.first().cloned().unwrap_or(Value::Null)),
        SeqOp::Last => Ok(items.last().cloned().unwrap_or(Value::Null)),
        SeqOp::Shift => Ok(Value::seq(items.iter().skip(1).cloned().collect())),
        SeqOp::Pop => {
            let mut items = as_seq(first)?;
            items.pop();
            Ok(Value::seq(items))
        }
        SeqOp::Get => {
            let position = as_int(&needed(second)?)?;
            let found = usize::try_from(position)
                .ok()
                .and_then(|index| items.get(index));
            Ok(found.cloned().unwrap_or(Value::Null))
        }
        SeqOp::Push => {
            let item = needed(second)?;
            let mut items = as_seq(first)?;
            items.push(item);
            Ok(Value::seq(items))
        }
        SeqOp::Includes => Ok(Value::Bool(items.contains(&needed(second)?))),
    }
}
