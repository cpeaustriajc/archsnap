//! Values, interned names and finite domains.
//!
//! A [`Value`] is what a model's field holds in one state. A [`Domain`] is the finite set of
//! values a field or an input declares. [`Symbols`] holds the text behind every [`Sym`] and
//! prints values as the report shows them. An [`IntType`] is the integer type a number has in
//! a Rust model file, with the numbers it reaches.

mod domain;
mod int_type;
mod value;

pub use domain::{Domain, MOST_VALUES_LISTED};
pub use int_type::IntType;
pub use value::{Sym, Symbols, Value};
