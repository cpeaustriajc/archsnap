# Agreement fixtures

Rust model files whose meaning is compared with what `rustc` computes. The test
`apps/earnest/tests/rust_agreement` builds each file in a temporary crate that depends on
`earnest`, walks the model breadth first with the library's `steps()`, `rules()` and `ends()`,
and requires the distinct states, the rules that break with the length of the shortest path to
each, and whether a state is stuck to be what `earnest verify --json` reports for the same file.
The numbers come from the Rust walk, never from the checker.

| File | Model | Covers |
|---|---|---|
| `../twins/quotient.rs` | `Quotient` | `/` on `u8`. `../twins/quotient.model.ts` is its twin with `m.div`, and both give 58 states and break `smaller` after one step |
| `../twins/basket.rs` | `Basket` | The value inside an `Option`: `if let` in a helper and at the end of an action, `let x = value?;`, `unwrap_or`, `map_or`, `== Some(..)` and a record in an `Option` |
| `../twins/phases.rs` | `Phases` | `match` on an enum and on a pair of enums, with `|` and `_`, as a value, as a `let` and as the end of an action, `matches!`, and `#[initial(A, B)]` |
| `../twins/builder.rs` | `Builder` | Helpers that return `Self`, a record built with struct update and a `&'static str`; `u8::from(bool)` and `u16::from(u8)`; a `#[view]` block that calls the helpers of the `#[model]` block |
| `tried.rs` | `Tried` | `let Some(x) = value else { return None; };` and a function that returns the model by its name. Clippy asks for `value?` in place of the first, so the file is not one a project that denies warnings would keep |
| `division.model.ts`, `division_fault.rs` | `Split`, `Below` | The faults of `m.div` and of `/`: a divisor of zero, where Rust panics, and a negative number, where Rust truncates and the checker declines to guess. Node's `explore` reports the same sentence and path as the checker |

The files in `../twins` are also built in a crate by `apps/earnest/tests/rust_models`, with
`cargo clippy -- -D warnings` under the lint table of this repository and `rustfmt --check`, and
`just tlc` runs `earnest confirm --tlc` on each.
