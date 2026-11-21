use super::num::{Operand, Stop, int_at, pair};
use super::shared::Shared;
use super::val::{Binding, Place, Val};
use crate::eval::{as_bool, compare};
use crate::{Expr, Fault, Frame, Op, Order};
use earnest_value::Value;

#[derive(Clone, Copy, Debug)]
pub(super) enum Wanted {
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
pub(super) enum Cond {
    Const(bool),
    Order {
        order: Order,
        left: Operand,
        right: Operand,
    },
    FieldOrder {
        order: Order,
        field: usize,
        number: i64,
    },
    Is {
        operand: Operand,
        number: i64,
        wanted: Wanted,
    },
    Matches {
        place: Place,
        value: Value,
        wanted: Wanted,
    },
    Alike(Box<(Place, Place)>, Wanted),
    Same(Box<(Val, Val)>, Wanted),
    Read(Place),
    Not(Box<Self>),
    All(Box<[Self]>),
    Any(Box<[Self]>),
    Choose(Box<(Self, Self, Self)>),
    Bind(Box<Binding<Self>>),
    Value(Box<Val>),
}

impl Cond {
    pub(super) fn of(expr: &Expr, shared: &Shared) -> Self {
        if let Some(cond) = Self::shaped(expr, shared) {
            return cond;
        }
        match expr {
            Expr::Const(Value::Bool(flag)) => Self::Const(*flag),
            Expr::Choose(test, yes, no) => Self::Choose(Box::new((
                Self::of(test, shared),
                Self::of(yes, shared),
                Self::of(no, shared),
            ))),
            _ => match Place::of(expr) {
                Some(place) => Self::Read(place),
                None => Self::Value(Box::new(Val::of(expr, shared))),
            },
        }
    }

    pub(super) fn shaped(expr: &Expr, shared: &Shared) -> Option<Self> {
        match expr {
            Expr::And(..) => {
                let mut parts = Vec::new();
                every_part(expr, &mut parts, shared);
                Some(Self::All(parts.into_boxed_slice()))
            }
            Expr::Or(..) => {
                let mut parts = Vec::new();
                any_part(expr, &mut parts, shared);
                Some(Self::Any(parts.into_boxed_slice()))
            }
            Expr::Bind { slot, value, body } => Some(Self::Bind(Binding::of(
                *slot,
                value,
                Self::of(body, shared),
                shared,
            ))),
            Expr::Apply(op, arguments) => match (op, arguments.as_slice()) {
                (Op::Order(order), [Expr::Field(field), Expr::Const(Value::Int(number))]) => {
                    Some(Self::FieldOrder {
                        order: *order,
                        field: *field,
                        number: *number,
                    })
                }
                (Op::Order(order), [left, right]) => Some(Self::Order {
                    order: *order,
                    left: Operand::of(left, shared),
                    right: Operand::of(right, shared),
                }),
                (Op::Equal, [left, right]) => {
                    Some(Self::equality(left, right, Wanted::Equal, shared))
                }
                (Op::Differ, [left, right]) => {
                    Some(Self::equality(left, right, Wanted::Different, shared))
                }
                (Op::Not, [inner]) => Some(Self::Not(Box::new(Self::of(inner, shared)))),
                _ => None,
            },
            _ => None,
        }
    }

    fn equality(left: &Expr, right: &Expr, wanted: Wanted, shared: &Shared) -> Self {
        match (left, right) {
            (_, Expr::Const(Value::Int(number))) => {
                return Self::Is {
                    operand: Operand::of(left, shared),
                    number: *number,
                    wanted,
                };
            }
            (Expr::Const(Value::Int(number)), _) => {
                return Self::Is {
                    operand: Operand::of(right, shared),
                    number: *number,
                    wanted,
                };
            }
            _ => {}
        }
        match (
            Place::of(left),
            Place::of(right),
            left.plain(),
            right.plain(),
        ) {
            (Some(place), None, _, Expr::Const(value))
            | (None, Some(place), Expr::Const(value), _) => Self::Matches {
                place,
                value: value.clone(),
                wanted,
            },
            (Some(first), Some(second), _, _) => Self::Alike(Box::new((first, second)), wanted),
            _ => Self::Same(
                Box::new((Val::of(left, shared), Val::of(right, shared))),
                wanted,
            ),
        }
    }

    pub(super) fn eval(&self, frame: &mut Frame<'_>) -> Result<bool, Fault> {
        match self {
            Self::FieldOrder {
                order,
                field,
                number,
            } => Ok(compare(
                *order,
                int_at(frame.state, *field).map_err(Stop::fault)?,
                *number,
            )),
            Self::All(parts) => {
                for part in parts {
                    if !part.eval(frame)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => self.rest(frame),
        }
    }

    #[inline(never)]
    fn rest(&self, frame: &mut Frame<'_>) -> Result<bool, Fault> {
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
            Self::Matches {
                place,
                value,
                wanted,
            } => Ok(wanted.judge(place.find(frame)?, value)),
            Self::Alike(places, wanted) => {
                let first = places.0.find(frame)?;
                let second = places.1.find(frame)?;
                Ok(wanted.judge(first, second))
            }
            Self::Same(sides, wanted) => {
                let first = sides.0.eval(frame)?;
                let second = sides.1.eval(frame)?;
                Ok(wanted.judge(&first, &second))
            }
            Self::Read(place) => as_bool(place.find(frame)?),
            Self::Not(inner) => Ok(!inner.eval(frame)?),
            Self::FieldOrder { .. } | Self::All(_) => self.eval(frame),
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
            Self::Bind(binding) => {
                binding.enter(frame)?;
                binding.body.eval(frame)
            }
            Self::Value(inner) => as_bool(&inner.eval(frame)?),
        }
    }
}

fn every_part(expr: &Expr, parts: &mut Vec<Cond>, shared: &Shared) {
    if let Expr::And(left, right) = expr {
        every_part(left, parts, shared);
        every_part(right, parts, shared);
    } else {
        parts.push(Cond::of(expr, shared));
    }
}

fn any_part(expr: &Expr, parts: &mut Vec<Cond>, shared: &Shared) {
    if let Expr::Or(left, right) = expr {
        any_part(left, parts, shared);
        any_part(right, parts, shared);
    } else {
        parts.push(Cond::of(expr, shared));
    }
}
