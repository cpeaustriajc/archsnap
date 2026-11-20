//! The macros of the Rust model library, re-exported by `earnest::model`.
//!
//! `#[derive(Domain)]` lists every value of an enum or a struct, `#[derive(Model)]` lists every
//! start state of a model struct, and `#[model]` on the model's `impl` block reads the
//! `#[action]`, `#[rule]` and `#[end]` attributes of its methods, and `#[view(...)]` on a second
//! `impl` block gives the model's statechart labels. Depend on `earnest`, not on
//! this crate: the generated code names `::earnest::model`.

mod behavior;
mod choices;
mod domain;
mod starts;
mod view;

use proc_macro::TokenStream;

fn finish(expanded: syn::Result<proc_macro2::TokenStream>) -> TokenStream {
    expanded
        .unwrap_or_else(|refused| refused.to_compile_error())
        .into()
}

/// Implements `earnest::model::Domain`, so every value of the type can be listed.
///
/// On an enum, every variant must be a plain name; the values are the variants in source
/// order. On a struct with named fields, every field must itself have a `Domain`, or be an
/// integer with `#[range(lo, hi)]` (both bounds included), or an integer or a `&'static str`
/// with `#[one_of(a, b)]`, which lists the values the field takes, in that order; the values
/// are every combination of the fields' values. The type must also be `Clone`.
///
/// `earnest verify` refuses a few types this derive accepts, because it can't read them as
/// Rust runs them: an `Option` as the value of a `Map` or the item of a `Seq` or a `Queue`,
/// where `get`, `first`, `last` and `head` give `Some(None)`; a `Lock<Owner>` held the same
/// way; `Option<Option<T>>`; and a range that reaches outside its integer type.
#[proc_macro_derive(Domain, attributes(range, one_of))]
pub fn derive_domain(input: TokenStream) -> TokenStream {
    finish(domain::expand(input.into()))
}

/// Implements `earnest::model::Model`, so `Type::starts()` lists every start state.
///
/// Put it on the struct that holds the model's state. A field with `#[initial(value)]` starts
/// at that value. An integer field needs `#[range(lo, hi)]` (both bounds included) and
/// without `#[initial]` starts at every number in the range. Any other field without
/// `#[initial]` starts where its type says: at every value for a `bool`, an `Option` and a
/// type that derives `Domain`, and empty, free, idle or at zero for the library's collections
/// and parts. `#[one_of(a, b)]` lists the values of an integer or of a `&'static str` field,
/// also inside an `Option`, in place of `#[range]`; a text always needs it. A model's type
/// needs no `pub`: the derive uses it where `rustc` looks for dead code, so a model that
/// only `earnest verify` reads draws no warning. `#[range(lo, hi)]` on a field whose type
/// holds an integer, such as
/// `Option<u8>` or `Map<Item, u8>`, is the range of that integer. Every field type must be
/// `Clone`.
///
/// `earnest verify` refuses a few field types this derive accepts, because it can't read
/// them as Rust runs them: `Map<K, Option<V>>`, `Seq<Option<T>, MAX>` and
/// `Queue<Option<T>, CAPACITY>`, where `get`, `first`, `last` and `head` give `Some(None)`,
/// which the checker can't tell from `None`; a `Lock<Owner>` held the same way;
/// `Option<Option<T>>` and `Lock<Option<T>>`; and a range that reaches outside its integer
/// type. Hold a struct with an `Option` field instead, or leave the entry out. The checker
/// computes each number in its integer type, so a rule that subtracts below zero on a `u8`
/// is reported as a fault, where a test of the model panics.
#[proc_macro_derive(Model, attributes(initial, range, one_of))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    finish(starts::expand(input.into()))
}

/// Marks the `impl` block that holds a model's actions, rules and ends.
///
/// Inside it, `#[action("sentence")]` goes on a method that takes `&self` and its inputs and
/// returns `Option<Self>`, and `#[rule("sentence")]` or `#[end("sentence")]` on a method that
/// takes `&self` and returns `bool`. The methods stay as written, so a test can call them.
/// The macro also implements `earnest::model::Behavior` for the type, which tries every
/// action with every input value; for that, each input type must have a `Domain` and be
/// `Clone` and `Debug`, and an integer input needs `#[range(lo, hi)]` on the parameter.
/// An action that nothing can stop returns `Self` in place of `Option<Self>`, so `clippy`'s
/// `unnecessary_wraps` has nothing to say about it; it can't use `require!`. An input may be
/// listed with `#[one_of(a, b)]` in place of `#[range]`.
/// A method without one of the three attributes is a helper: it is left as it is, takes its
/// parameters as plain Rust, and needs no `#[range]`, which Rust would not accept there.
#[proc_macro_attribute]
pub fn model(arguments: TokenStream, input: TokenStream) -> TokenStream {
    finish(behavior::expand(&arguments.into(), input.into()))
}

/// Marks the `impl` block that holds a model's statechart labels, the Rust form of the
/// TypeScript library's `Model.view({ ... })`.
///
/// The arguments are all optional: `title = "..."` is the chart's title, `step(action = "...")`
/// gives each named action an arrow label in place of its sentence, and
/// `colors(broken = "#c62828")` colors `broken`, `start` or `end`, each with `#` and 3, 4, 6
/// or 8 hexadecimal digits or a color name of letters. Inside the block, a method marked
/// `#[state]` labels a state and one marked `#[group]` names the box a state is drawn in; each
/// takes `&self` and returns a `String`, written with `format!` or `.to_string()`. The block
/// holds nothing else, and a model has one view. The macro implements `earnest::model::View`
/// for the type, so a test can ask for the same labels the chart shows, and it refuses a
/// `step` for a method the type does not have. `earnest verify`, `earnest chart` and
/// `earnest report` read the labels from the file; one that can't be read, a `step` for an
/// action the model does not have and a color that is not one are refused there.
#[proc_macro_attribute]
pub fn view(arguments: TokenStream, input: TokenStream) -> TokenStream {
    finish(view::expand(arguments.into(), input.into()))
}
