# earnest_macros

The macros of the Rust model library. Depend on `earnest` and import them from `earnest::model`;
the code they generate names `::earnest::model`.

| Macro | Goes on | What it generates |
|---|---|---|
| `#[derive(Domain)]` | an enum of plain variants, or a struct with named fields | `earnest::model::Domain`: every value of the type |
| `#[derive(Model)]` | the struct that holds a model's state | `earnest::model::Model`: every start state, from `#[initial(value)]`, `#[range(lo, hi)]` and each field's type |
| `#[model]` | the model's `impl` block | `earnest::model::Behavior`: every step, rule and end of a state. The methods stay as written, without `#[action]`, `#[rule]`, `#[end]` and a parameter's `#[range]` |

An integer always needs `#[range(lo, hi)]`, on a field or on an action's parameter, and the macros
refuse one without it. Generated code is written to pass the workspace's lints in the crate that
uses it.
