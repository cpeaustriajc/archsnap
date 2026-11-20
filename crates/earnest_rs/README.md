# earnest_rs

The Rust reader: model file to tree.

`read(file, source)` parses one Rust file with `syn` and gives back a `Reading` from `earnest_ast`:
a `Draft` for each struct that derives `Model`, or a `Problem` for each thing
that is outside the library. The file is never compiled or run.

What it reads:

| In the file | In the draft |
|---|---|
| A struct that derives `Model` | A model. Each field's shape is its type, with `#[range(lo, hi)]` for a number (also a number inside the type, as in `Option<u8>`) and `#[initial(value)]` for its start. A number's shape carries its integer type, and the range must lie inside it |
| An enum that derives `Domain` | Choices known by their variant names: `Gate::Locked` is the text `"Locked"` |
| A struct that derives `Domain` | A record |
| A method with `#[action("...")]` | An action. Parameters after `&self` are its inputs, each `require!(c);` is one part of its guard, and `Some(Self { changed, ..self.clone() })` is its result. `let` bindings and `if c { return Some(Self { .. }); }` may come before it. A result that is not wrapped in `Some` is a problem (E0308), and so is `Some(Some(..))` |
| A method with `#[rule("...")]` or `#[end("...")]` | A rule or an end. A rule's text is the source of its body |
| An action that returns `Self` | An action with no guard whose result is the `Self { .. }` it gives back. A `require!`, a `None`, a `Some(..)` or a `?` in it is a problem: `rustc` refuses each (E0308, E0277) |
| `#[one_of(a, b)]` on a field, a parameter or a record field of an integer type, a `&'static str` or an `Option` of one | A choice of the listed values in the order listed. A number's choices are `Declared` terms of its integer type and must lie inside it; a text's are `Text` terms. A string literal is a `Text` term wherever it is written |
| An `impl` block with `#[view(title = "...", step(action = "..."), colors(part = "..."))]` | The view of the model of that type: its title, its step labels and its colors, and the methods marked `#[state]` and `#[group]` as `LabelDraft`s. A label is `format!("...{}...{:?}", values)` as a `Template` of `Value` and `Debug` pieces, or one value followed by `.to_string()` (a `Value` piece) or `.to_owned()` (an `Owned` piece) as a one-piece `Template`. An enum variant is a `Variant` term, so the lowering can tell it from a text and read each form only where `rustc` accepts it and the chart draws what Rust prints A step for an action the model does not have, a color that is not one, a part other than `broken`, `start` and `end`, a name given twice, a second view, a hole with a width or a name, a `{` never closed, more or fewer values than holes, and anything else in the block are problems |
| `n.is_multiple_of(d)` | A call of the primitive `is_multiple_of`, which the lowering reads as `n == 0` where `d == 0`, else `n % d == 0`, and only on an unsigned integer type (`rustc` refuses it on any other, E0599) |
| A method of the block without an attribute | A helper of the kind `Method`, called as `self.name(..)` |
| A top-level `fn` | A helper of the kind `Function`, called as `name(..)`. It may share its name with a method |
| A parameter of a helper and its result | The value with its type, as a `Declared` term. A number needs no `#[range]` here: its type is enough. A parameter of the model's own type is the state |
| A top-level `const`, a `let` with a type, a number with a suffix as in `1_u8` | The value with its type, as a `Declared` term. A constant is also usable as a bound in `#[range]` and as a `MAX` or `CAPACITY` |
| `x.clone()`, `&x` | `x` |
| An item marked `#[cfg(test)]` that is not part of a model: a module, a function, a constant, a plain `impl` block | Skipped |

Anything else is a problem with its 1-based line and column, the construct in backticks and a
sentence: a loop, `match`, `if let`, a clock, a random number, input or output, `unsafe`, an
unknown macro or method, a change in place (`let mut`, `=`, `+=`, `&mut self`), a cast, a character,
a byte or a number with a decimal point, `format!` anywhere but in a view's labels, an import from outside `earnest`, any other top-level item, an `Option` as the value of a
`Map` or the item of a `Seq` or a `Queue` (reading it would give `Some(None)`, which the checker
can't tell from `None`), a `#[range]` that reaches outside its integer type, an item or a value
named `forall`, `exists`, `Some`, `None`, `Set`, `Seq` or `Map` that the file calls or uses as the
library's, an item of the library imported under another name, a nesting deeper than 1000 levels,
any other `#[cfg]` or
`#[cfg_attr]` (a part of a model kept in one build only can't be checked for every build), and
`#[action]`, `#[rule]` or `#[end]` written with a path, which `#[model]` would not see. Every
problem in the file is reported, and a file with a problem gives no drafts. Names and kinds are not checked here:
`earnest_semantic` does that when it lowers a draft.

Every draft has the language `Rust`, which tells the lowering to compute each number in its
integer type. `tests/read/models` holds model files that are also compiled as part of the tests,
so what the reader accepts is what `rustc` accepts: `shelf.rs` has a method and a function of one
name and a helper that takes a number, and its own test shows the subtraction the checker
reports as a fault stopping a test build.
