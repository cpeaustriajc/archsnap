//! The macros of the Rust model library, re-exported by `earnest::model`.
//!
//! `#[derive(Domain)]` lists every value of an enum or a struct, `#[derive(Model)]` lists every
//! start state of a model struct, and `#[model]` on the model's `impl` block reads the
//! `#[action]`, `#[rule]` and `#[end]` attributes of its methods. Depend on `earnest`, not on
//! this crate: the generated code names `::earnest::model`.

mod behavior;
mod choices;
mod domain;
mod starts;

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
/// integer with `#[range(lo, hi)]` (both bounds included); the values are every combination
/// of the fields' values. The type must also be `Clone`.
#[proc_macro_derive(Domain, attributes(range))]
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
/// and parts. `#[range(lo, hi)]` on a field whose type holds an integer, such as
/// `Option<u8>` or `Map<Item, u8>`, is the range of that integer. Every field type must be
/// `Clone`.
#[proc_macro_derive(Model, attributes(initial, range))]
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
#[proc_macro_attribute]
pub fn model(arguments: TokenStream, input: TokenStream) -> TokenStream {
    finish(behavior::expand(&arguments.into(), input.into()))
}
