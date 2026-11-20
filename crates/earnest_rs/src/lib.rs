//! The Rust reader: model file to tree.
//!
//! [`read`] parses one Rust file written to `earnest::model` and gives back a draft for each
//! struct that derives `Model`, or a problem for each thing in the file that is outside the
//! library. It reads the text only: the file is never compiled or run, and names are checked
//! later by `earnest_semantic`.

mod bodies;
mod items;
mod library;
mod reader;
mod shapes;
mod terms;
mod views;

use earnest_ast::Reading;

pub use library::uses_library;

/// Reads every model in the Rust source text `source`.
///
/// `file` is the path the problems and the drafts carry; nothing is read from disk. A byte
/// order mark at the start of `source` is skipped, as `rustc` skips it. A model
/// is a struct that derives `Model` together with its `#[model]` block. In the drafts, a
/// field's shape comes from its Rust type, `#[range(lo, hi)]` and `#[initial(value)]`; an
/// enum that derives `Domain` is known by its variant names, so `Gate::Locked` is the text
/// `"Locked"`; ids are method names; the state is named `self`; and a rule's `text` is the
/// source of its body with each run of white space as one space. `x.clone()` and `&x` are
/// read as `x`. Top-level constants, functions, and methods of the block without an
/// attribute become the drafts' constants and helpers. A function is a helper of the kind
/// `Function` and a method one of the kind `Method`, so the two may share a name. An item
/// marked `#[cfg(test)]` that is not part of a model (a module, a function, a constant, a
/// plain `impl` block) is skipped. An `impl` block marked `#[view(...)]` gives the view of the
/// model of its type: its `title`, `step` labels and `colors`, and the labels of its `#[state]`
/// and `#[group]` methods, read from a `format!` or a `.to_string()` or `.to_owned()`. A number
/// or a `&'static str` marked `#[one_of(a, b)]` is a choice of the listed values.
///
/// Every draft has the language `Rust`. A number's shape carries its integer type, and a
/// value whose type the file writes out is a `Declared` term: a constant, a `let` with a
/// type, a number with a suffix, and each parameter and the result of a helper. A helper's
/// parameter needs no `#[range]`; a field and an action's input do, and the range must lie
/// inside the integer type.
///
/// An `Option` as the value of a `Map` or the item of a `Seq` or a `Queue` is a problem:
/// `get`, `first`, `last` and `head` would give `Some(None)`, which the checker can't tell
/// from `None`. So is a function named `forall`, `exists` or `Some`, whose calls are read as
/// the library's.
///
/// Any other `#[cfg]` or `#[cfg_attr]`, on an item, a method, a field, an input, a statement
/// or an expression, is a problem: the file is read once for every build, so a part kept in
/// one build only would be checked wrongly in the other. So is `#[action]`, `#[rule]` or
/// `#[end]` written with a path, which `#[model]` would not see.
///
/// When anything in the file is outside the library and the allowed forms (a loop, a clock,
/// a random number, input or output, `unsafe`, an unknown macro, a change in place, an item
/// that is not part of a model), the reading has no drafts and one problem for each such
/// thing, sorted by position. Each problem has the 1-based line and column, the construct in
/// backticks as its feature, and a sentence. Text that is not Rust at all gives one problem
/// at the place the parser stopped.
#[must_use]
pub fn read(file: &str, source: &str) -> Reading {
    items::read_file(file, source)
}
