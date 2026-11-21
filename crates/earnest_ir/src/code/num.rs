use super::shared::Shared;
use super::val::Val;
use crate::eval::{arithmetic, is_safe, wrong_or_null};
use crate::{Expr, Fault, Frame, Math, Op};
use earnest_value::{IntType, Value};

#[derive(Clone, Copy, Debug)]
pub(super) enum Stop {
    Fault(Fault),
    Kind(Fault),
}

impl Stop {
    pub(super) fn fault(self) -> Fault {
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
pub(super) enum Bound {
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

#[derive(Clone, Debug)]
pub(super) enum Operand {
    Int(i64),
    Field(usize),
    Local(usize),
    Num(Box<Num>),
}

#[derive(Clone, Debug)]
pub(super) enum Num {
    Math {
        math: Math,
        bound: Bound,
        left: Operand,
        right: Operand,
    },
    FieldMath {
        math: Math,
        bound: Bound,
        field: usize,
        number: i64,
    },
    Bounded(Bound, Operand),
    Negate(Operand),
    Value(Box<Val>),
}

impl Operand {
    pub(super) fn of(expr: &Expr, shared: &Shared) -> Self {
        match expr {
            Expr::Const(Value::Int(number)) => Self::Int(*number),
            Expr::Field(index) => Self::Field(*index),
            Expr::Local(slot) => Self::Local(*slot),
            _ => Self::Num(Box::new(Num::of(expr, shared))),
        }
    }

    #[inline]
    pub(super) fn get(&self, frame: &mut Frame<'_>) -> Result<i64, Stop> {
        match self {
            Self::Int(number) => Ok(*number),
            Self::Field(index) => int_at(frame.state, *index),
            Self::Local(slot) => int_at(&frame.locals, *slot),
            Self::Num(num) => num.eval(frame),
        }
    }
}

pub(super) fn int_at(values: &[Value], at: usize) -> Result<i64, Stop> {
    match values.get(at) {
        Some(Value::Int(number)) => Ok(*number),
        Some(other) => Err(Stop::Kind(wrong_or_null(other))),
        None => Err(Stop::Fault(Fault::WrongKind)),
    }
}

pub(super) fn pair(
    left: &Operand,
    right: &Operand,
    frame: &mut Frame<'_>,
) -> Result<(i64, i64), Fault> {
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
    fn of(expr: &Expr, shared: &Shared) -> Self {
        Self::shaped(expr, shared).unwrap_or_else(|| Self::Value(Box::new(Val::of(expr, shared))))
    }

    pub(super) fn shaped(expr: &Expr, shared: &Shared) -> Option<Self> {
        let Expr::Apply(op, arguments) = expr else {
            return None;
        };
        Some(match (op, arguments.as_slice()) {
            (Op::Math(math), [Expr::Field(field), Expr::Const(Value::Int(number))]) => {
                Self::FieldMath {
                    math: *math,
                    bound: Bound::Any,
                    field: *field,
                    number: *number,
                }
            }
            (Op::Math(math), [left, right]) => Self::Math {
                math: *math,
                bound: Bound::Any,
                left: Operand::of(left, shared),
                right: Operand::of(right, shared),
            },
            (Op::Safe, [inner]) => Self::bounded(Bound::Safe, inner, shared),
            (Op::Fit(ty), [inner]) => Self::bounded(Bound::Fit(*ty), inner, shared),
            (Op::Negate, [inner]) => Self::Negate(Operand::of(inner, shared)),
            _ => return None,
        })
    }

    fn bounded(bound: Bound, inner: &Expr, shared: &Shared) -> Self {
        match Self::shaped(inner, shared) {
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
            Some(Self::FieldMath {
                math,
                bound: Bound::Any,
                field,
                number,
            }) => Self::FieldMath {
                math,
                bound,
                field,
                number,
            },
            Some(other) => Self::Bounded(bound, Operand::Num(Box::new(other))),
            None => Self::Bounded(bound, Operand::of(inner, shared)),
        }
    }

    pub(super) fn eval(&self, frame: &mut Frame<'_>) -> Result<i64, Stop> {
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
            Self::FieldMath {
                math,
                bound,
                field,
                number,
            } => {
                let first = int_at(frame.state, *field).map_err(Stop::fault)?;
                let answer = arithmetic(*math, first, *number);
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
