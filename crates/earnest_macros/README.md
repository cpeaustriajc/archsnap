# earnest_macros

The macros of the Rust model library. Depend on `earnest` and import them from `earnest::model`;
the code they generate names `::earnest::model`.

| Macro | Goes on | What it generates |
|---|---|---|
| `#[derive(Domain)]` | an enum of plain variants, or a struct with named fields | `earnest::model::Domain`: every value of the type |
| `#[derive(Model)]` | the struct that holds a model's state | `earnest::model::Model`: every start state, from `#[initial(value)]`, `#[range(lo, hi)]` and each field's type |
| `#[model]` | the model's `impl` block | `earnest::model::Behavior`: every step, rule and end of a state. The methods stay as written, without `#[action]`, `#[rule]`, `#[end]` and a parameter's `#[range]` or `#[one_of]`. An action that returns `Self` in place of `Option<Self>` always happens |
| `#[view(...)]` | a second `impl` block of the model's type | `earnest::model::View`: the title, the arrow labels, the colors and the `#[state]` and `#[group]` labels of the statechart. The block holds only those two methods, and the macro refuses an unknown setting, a color part other than `broken`, `start` and `end`, a step for a method the type does not have and a name given twice |

An integer always needs `#[range(lo, hi)]` or `#[one_of(a, b)]`, on a field or on an action's
parameter, and a `&'static str` always needs `#[one_of("a", "b")]`; the macros refuse either
without it, and refuse both on one field. `#[derive(Model)]` also uses the model's type in an
anonymous constant, so a model that only `earnest verify` reads is not dead code to `rustc`.
Generated code is written to pass the workspace's lints in the crate that uses it, and a model
file needs no `pub`, no documentation and no `allow`.
