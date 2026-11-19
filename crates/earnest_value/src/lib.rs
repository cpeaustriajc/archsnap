//! Values, interned names and finite domains.
//!
//! A [`Value`] is what a model's field holds in one state. A [`Domain`] is the finite set of
//! values a field or an input declares. [`Symbols`] holds the text behind every [`Sym`] and
//! prints values as the report shows them.

mod domain;
mod value;

pub use domain::{Domain, MOST_VALUES_LISTED};
pub use value::{Sym, Symbols, Value};
