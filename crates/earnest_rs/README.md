# earnest_rs

The Rust reader: model file to tree.

`read(file, source)` parses one Rust file with `syn` and gives back a `Reading` from `earnest_ast`:
a `Draft` for each struct that derives `Model`, or a `Problem` for each thing
that is outside the library. The file is never compiled or run.

What it reads:

| In the file | In the draft |
|---|---|
| A struct that derives `Model` | A model. Each field's shape is its type, with `#[range(lo, hi)]` for a number (also a number inside the type, as in `Option<u8>`) and `#[initial(value)]` for its start |
| An enum that derives `Domain` | Choices known by their variant names: `Gate::Locked` is the text `"Locked"` |
| A struct that derives `Domain` | A record |
| A method with `#[action("...")]` | An action. Parameters after `&self` are its inputs, each `require!(c);` is one part of its guard, and `Some(Self { changed, ..self.clone() })` is its result. `let` bindings and `if c { return Some(Self { .. }); }` may come before it |
| A method with `#[rule("...")]` or `#[end("...")]` | A rule or an end. A rule's text is the source of its body |
| A method of the block without an attribute, a top-level `fn` | A helper, called as `self.name(..)` or `name(..)` |
| A top-level `const` | A constant, also usable as a bound in `#[range]` and as a `MAX` or `CAPACITY` |
| `x.clone()`, `&x` | `x` |
| An item marked `#[cfg(test)]` | Skipped |

Anything else is a problem with its 1-based line and column, the construct in backticks and a
sentence: a loop, `match`, `if let`, a clock, a random number, input or output, `unsafe`, an
unknown macro or method, a change in place (`let mut`, `=`, `+=`, `&mut self`), a cast, text in
quotes, an import from outside `earnest`, and any other top-level item. Every problem in the file
is reported, and a file with a problem gives no drafts. Names and kinds are not checked here:
`earnest_semantic` does that when it lowers a draft.
