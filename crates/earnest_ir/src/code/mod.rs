mod cond;
mod num;
mod val;

use crate::{Expr, Fault, Frame};
use cond::Cond;
use earnest_value::Value;
use num::Stop;
use std::fmt;
use val::Val;

/// An expression together with the form the search evaluates it in.
///
/// Build one with [`Code::new`] once, before the search, and evaluate it in every state with
/// [`Code::eval`] or [`Code::truth`]. Both give the value or the [`Fault`] that [`eval`] and
/// [`crate::truth`] give for [`Code::expr`], with the same effects on the frame; they are faster
/// because numbers and booleans are computed, and fields and records are read where they are,
/// without building a [`Value`] for each step.
#[derive(Clone)]
pub struct Code {
    expr: Expr,
    value: Val,
    test: Cond,
}

impl Code {
    /// Prepares `expr` for evaluation. The cost is one walk over the expression.
    #[must_use]
    pub fn new(expr: Expr) -> Self {
        let value = Val::of(&expr);
        let test = Cond::of(&expr);
        Self { expr, value, test }
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
        match &self.value {
            Val::Num(num) => num.eval(frame).map(Value::Int).map_err(Stop::fault),
            other => other.eval(frame),
        }
    }

    /// Whether the expression is true in the state and locals of `frame`, for a guard or a rule.
    ///
    /// # Errors
    ///
    /// A [`Fault`] when the expression has no value in this state, and [`Fault::NullRead`] or
    /// [`Fault::WrongKind`] when its value is not a boolean.
    pub fn truth(&self, frame: &mut Frame<'_>) -> Result<bool, Fault> {
        self.test.eval(frame)
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
