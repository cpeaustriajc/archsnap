use crate::eval::{arithmetic, as_bool, compare, eval, is_safe, wrong_or_null};
use crate::{Expr, Fault, Frame, Math, Op, Order};
use earnest_value::{IntType, Value};
use std::fmt;

/// An expression together with the form the search evaluates it in.
///
/// Build one with [`Code::new`] once, before the search, and evaluate it in every state with
/// [`Code::eval`] or [`Code::truth`]. Both give the value or the [`Fault`] that [`eval`] and
/// [`crate::truth`] give for [`Code::expr`], with the same effects on the frame; they are faster
/// because numbers and booleans are computed without building a [`Value`] for each step.
#[derive(Clone)]
pub struct Code {
    expr: Expr,
    top: Val,
}

impl Code {
    /// Prepares `expr` for evaluation. The cost is one walk over the expression.
    #[must_use]
    pub fn new(expr: Expr) -> Self {
        let top = Val::of(&expr);
        Self { expr, top }
    }

    /// The expression as it was given, for printing it or translating it.
    #[must_use]
    pub fn expr(&self) -> &Expr {
        &self.expr
    }

    /// The value of the expression in the state and locals of `frame`.
    ///
    /// # Errors
    ///
    /// A [`Fault`] when the expression has no value in this state.
    pub fn eval(&self, frame: &mut Frame<'_>) -> Result<Value, Fault> {
        self.top.eval(frame)
    }

    /// Whether the expression is true in the state and locals of `frame`, for a guard or a rule.
    ///
    /// # Errors
    ///
    /// A [`Fault`] when the expression has no value in this state, and [`Fault::NullRead`] or
    /// [`Fault::WrongKind`] when its value is not a boolean.
    pub fn truth(&self, frame: &mut Frame<'_>) -> Result<bool, Fault> {
        match &self.top {
            Val::Cond(cond) => cond.eval(frame),
            other => as_bool(&other.eval(frame)?),
        }
    }
}

impl From<Expr> for Code {
    fn from(expr: Expr) -> Self {
        Self::new(expr)
    }
}

impl PartialEq for Code {
    fn eq(&self, other: &Self) -> bool {
        self.expr == other.expr
    }
}

impl Eq for Code {}

impl fmt::Debug for Code {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.expr.fmt(formatter)
    }
}

#[derive(Clone, Copy, Debug)]
enum Stop {
    Fault(Fault),
    Kind(Fault),
}

impl Stop {
    fn fault(self) -> Fault {
        match self {
            Self::Fault(fault) | Self::Kind(fault) => fault,
        }
    }
}

impl From<Fault> for Stop {
    fn from(fault: Fault) -> Self {
        Self::Fault(fault)
    }
}

#[derive(Clone, Copy, Debug)]
enum Bound {
    Any,
    Safe,
    Fit(IntType),
}

impl Bound {
    fn check(self, number: i64) -> Result<i64, Fault> {
        match self {
            Self::Any => Ok(number),
            Self::Safe if is_safe(number) => Ok(number),
            Self::Safe => Err(Fault::Overflow),
            Self::Fit(ty) if ty.holds(number) => Ok(number),
            Self::Fit(ty) => Err(Fault::Outside(ty)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Wanted {
    Equal,
    Different,
}

impl Wanted {
    fn judge<T: PartialEq + ?Sized>(self, first: &T, second: &T) -> bool {
        match self {
            Self::Equal => first == second,
            Self::Different => first != second,
        }
    }

    fn of_other_kinds(self) -> bool {
        matches!(self, Self::Different)
    }
}

#[derive(Clone, Debug)]
enum Operand {
    Int(i64),
    Field(usize),
    Local(usize),
    Num(Box<Num>),
}

#[derive(Clone, Debug)]
enum Num {
    Math {
        math: Math,
        bound: Bound,
        left: Operand,
        right: Operand,
    },
    Bounded(Bound, Operand),
    Negate(Operand),
    Value(Box<Val>),
}

#[derive(Clone, Debug)]
enum Cond {
    Const(bool),
    Order {
        order: Order,
        left: Operand,
        right: Operand,
    },
    Is {
        operand: Operand,
        number: i64,
        wanted: Wanted,
    },
    FieldIs {
        field: usize,
        value: Value,
        wanted: Wanted,
    },
    Same(Box<(Val, Val)>, Wanted),
    Not(Box<Self>),
    All(Box<[Self]>),
    Any(Box<[Self]>),
    Choose(Box<(Self, Self, Self)>),
    Value(Box<Val>),
}

#[derive(Clone, Debug)]
enum Val {
    Const(Value),
    Field(usize),
    Local(usize),
    Num(Num),
    Cond(Cond),
    Choose(Box<(Cond, Self, Self)>),
    Reference(Box<Expr>),
}

impl Operand {
    fn of(expr: &Expr) -> Self {
        match expr {
            Expr::Const(Value::Int(number)) => Self::Int(*number),
            Expr::Field(index) => Self::Field(*index),
            Expr::Local(slot) => Self::Local(*slot),
            _ => Self::Num(Box::new(Num::of(expr))),
        }
    }

    #[inline]
    fn get(&self, frame: &mut Frame<'_>) -> Result<i64, Stop> {
        match self {
            Self::Int(number) => Ok(*number),
            Self::Field(index) => int_at(frame.state, *index),
            Self::Local(slot) => int_at(&frame.locals, *slot),
            Self::Num(num) => num.eval(frame),
        }
    }
}

fn int_at(values: &[Value], at: usize) -> Result<i64, Stop> {
    match values.get(at) {
        Some(Value::Int(number)) => Ok(*number),
        Some(other) => Err(Stop::Kind(wrong_or_null(other))),
        None => Err(Stop::Fault(Fault::WrongKind)),
    }
}

fn pair(left: &Operand, right: &Operand, frame: &mut Frame<'_>) -> Result<(i64, i64), Fault> {
    let first = left.get(frame);
    if let Err(Stop::Fault(fault)) = first {
        return Err(fault);
    }
    let second = right.get(frame);
    match (first, second) {
        (Ok(first), Ok(second)) => Ok((first, second)),
        (_, Err(Stop::Fault(fault))) => Err(fault),
        (Err(stop), _) | (_, Err(stop)) => Err(stop.fault()),
    }
}

impl Num {
    fn of(expr: &Expr) -> Self {
        Self::shaped(expr).unwrap_or_else(|| Self::Value(Box::new(Val::of(expr))))
    }

    fn shaped(expr: &Expr) -> Option<Self> {
        let Expr::Apply(op, arguments) = expr else {
            return None;
        };
        Some(match (op, arguments.as_slice()) {
            (Op::Math(math), [left, right]) => Self::Math {
                math: *math,
                bound: Bound::Any,
                left: Operand::of(left),
                right: Operand::of(right),
            },
            (Op::Safe, [inner]) => Self::bounded(Bound::Safe, inner),
            (Op::Fit(ty), [inner]) => Self::bounded(Bound::Fit(*ty), inner),
            (Op::Negate, [inner]) => Self::Negate(Operand::of(inner)),
            _ => return None,
        })
    }

    fn bounded(bound: Bound, inner: &Expr) -> Self {
        match Self::shaped(inner) {
            Some(Self::Math {
                math,
                bound: Bound::Any,
                left,
                right,
            }) => Self::Math {
                math,
                bound,
                left,
                right,
            },
            Some(other) => Self::Bounded(bound, Operand::Num(Box::new(other))),
            None => Self::Bounded(bound, Operand::of(inner)),
        }
    }

    fn eval(&self, frame: &mut Frame<'_>) -> Result<i64, Stop> {
        match self {
            Self::Math {
                math,
                bound,
                left,
                right,
            } => {
                let (first, second) = pair(left, right, frame)?;
                let answer = arithmetic(*math, first, second);
                frame.note(&answer);
                Ok(bound.check(answer?)?)
            }
            Self::Bounded(bound, inner) => {
                let number = inner.get(frame).map_err(Stop::fault)?;
                Ok(bound.check(number)?)
            }
            Self::Negate(inner) => {
                let number = inner.get(frame).map_err(Stop::fault)?;
                number.checked_neg().ok_or(Stop::Fault(Fault::Overflow))
            }
            Self::Value(inner) => match inner.eval(frame)? {
                Value::Int(number) => Ok(number),
                other => Err(Stop::Kind(wrong_or_null(&other))),
            },
        }
    }
}

impl Cond {
    fn of(expr: &Expr) -> Self {
        if let Some(cond) = Self::shaped(expr) {
            return cond;
        }
        match expr {
            Expr::Const(Value::Bool(flag)) => Self::Const(*flag),
            Expr::Choose(test, yes, no) => {
                Self::Choose(Box::new((Self::of(test), Self::of(yes), Self::of(no))))
            }
            _ => Self::Value(Box::new(Val::of(expr))),
        }
    }

    fn shaped(expr: &Expr) -> Option<Self> {
        match expr {
            Expr::And(..) => {
                let mut parts = Vec::new();
                every_part(expr, &mut parts);
                Some(Self::All(parts.into_boxed_slice()))
            }
            Expr::Or(..) => {
                let mut parts = Vec::new();
                any_part(expr, &mut parts);
                Some(Self::Any(parts.into_boxed_slice()))
            }
            Expr::Apply(op, arguments) => match (op, arguments.as_slice()) {
                (Op::Order(order), [left, right]) => Some(Self::Order {
                    order: *order,
                    left: Operand::of(left),
                    right: Operand::of(right),
                }),
                (Op::Equal, [left, right]) => Some(Self::equality(left, right, Wanted::Equal)),
                (Op::Differ, [left, right]) => Some(Self::equality(left, right, Wanted::Different)),
                (Op::Not, [inner]) => Some(Self::Not(Box::new(Self::of(inner)))),
                _ => None,
            },
            _ => None,
        }
    }

    fn equality(left: &Expr, right: &Expr, wanted: Wanted) -> Self {
        match (left, right) {
            (_, Expr::Const(Value::Int(number))) => Self::Is {
                operand: Operand::of(left),
                number: *number,
                wanted,
            },
            (Expr::Const(Value::Int(number)), _) => Self::Is {
                operand: Operand::of(right),
                number: *number,
                wanted,
            },
            (Expr::Field(index), Expr::Const(value)) => Self::FieldIs {
                field: *index,
                value: value.clone(),
                wanted,
            },
            _ => Self::Same(Box::new((Val::of(left), Val::of(right))), wanted),
        }
    }

    fn eval(&self, frame: &mut Frame<'_>) -> Result<bool, Fault> {
        match self {
            Self::Const(flag) => Ok(*flag),
            Self::Order { order, left, right } => {
                let (first, second) = pair(left, right, frame)?;
                Ok(compare(*order, first, second))
            }
            Self::Is {
                operand,
                number,
                wanted,
            } => match operand.get(frame) {
                Ok(found) => Ok(wanted.judge(&found, number)),
                Err(Stop::Kind(_)) => Ok(wanted.of_other_kinds()),
                Err(Stop::Fault(fault)) => Err(fault),
            },
            Self::FieldIs {
                field,
                value,
                wanted,
            } => {
                let found = frame.state.get(*field).ok_or(Fault::WrongKind)?;
                Ok(wanted.judge(found, value))
            }
            Self::Same(sides, wanted) => {
                let first = sides.0.eval(frame)?;
                let second = sides.1.eval(frame)?;
                Ok(wanted.judge(&first, &second))
            }
            Self::Not(inner) => Ok(!inner.eval(frame)?),
            Self::All(parts) => {
                for part in parts {
                    if !part.eval(frame)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Self::Any(parts) => {
                for part in parts {
                    if part.eval(frame)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Self::Choose(branches) => {
                if branches.0.eval(frame)? {
                    branches.1.eval(frame)
                } else {
                    branches.2.eval(frame)
                }
            }
            Self::Value(inner) => as_bool(&inner.eval(frame)?),
        }
    }
}

fn every_part(expr: &Expr, parts: &mut Vec<Cond>) {
    if let Expr::And(left, right) = expr {
        every_part(left, parts);
        every_part(right, parts);
    } else {
        parts.push(Cond::of(expr));
    }
}

fn any_part(expr: &Expr, parts: &mut Vec<Cond>) {
    if let Expr::Or(left, right) = expr {
        any_part(left, parts);
        any_part(right, parts);
    } else {
        parts.push(Cond::of(expr));
    }
}

impl Val {
    fn of(expr: &Expr) -> Self {
        match expr {
            Expr::Const(value) => Self::Const(value.clone()),
            Expr::Field(index) => Self::Field(*index),
            Expr::Local(slot) => Self::Local(*slot),
            Expr::Choose(test, yes, no) => {
                Self::Choose(Box::new((Cond::of(test), Self::of(yes), Self::of(no))))
            }
            _ => {
                if let Some(num) = Num::shaped(expr) {
                    Self::Num(num)
                } else if let Some(cond) = Cond::shaped(expr) {
                    Self::Cond(cond)
                } else {
                    Self::Reference(Box::new(expr.clone()))
                }
            }
        }
    }

    fn eval(&self, frame: &mut Frame<'_>) -> Result<Value, Fault> {
        match self {
            Self::Const(value) => Ok(value.clone()),
            Self::Field(index) => frame.state.get(*index).cloned().ok_or(Fault::WrongKind),
            Self::Local(slot) => frame.locals.get(*slot).cloned().ok_or(Fault::WrongKind),
            Self::Num(num) => num.eval(frame).map(Value::Int).map_err(Stop::fault),
            Self::Cond(cond) => cond.eval(frame).map(Value::Bool),
            Self::Choose(branches) => {
                if branches.0.eval(frame)? {
                    branches.1.eval(frame)
                } else {
                    branches.2.eval(frame)
                }
            }
            Self::Reference(expr) => eval(expr, frame),
        }
    }
}
