//! The Rust reader: model file to tree.
//!
//! [`read`] parses one Rust file written to `earnest::model` and gives back a draft for each
//! struct that derives `Model`, or a problem for each thing in the file that is outside the
//! library. It reads the text only: the file is never compiled or run, and names are checked
//! later by `earnest_semantic`.

mod bodies;
mod items;
mod reader;
mod shapes;
mod terms;

use earnest_ast::Reading;

/// Reads every model in the Rust source text `source`.
///
/// `file` is the path the problems and the drafts carry; nothing is read from disk. A model
/// is a struct that derives `Model` together with its `#[model]` block. In the drafts, a
/// field's shape comes from its Rust type, `#[range(lo, hi)]` and `#[initial(value)]`; an
/// enum that derives `Domain` is known by its variant names, so `Gate::Locked` is the text
/// `"Locked"`; ids are method names; the state is named `self`; and a rule's `text` is the
/// source of its body with each run of white space as one space. `x.clone()` and `&x` are
/// read as `x`. Top-level constants, functions, and methods of the block without an
/// attribute become the drafts' constants and helpers. Items marked `#[cfg(test)]` are
/// skipped.
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
