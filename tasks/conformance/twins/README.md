# Rust twins with a view, listed values and parts

Rust models written to the Rust model API, each with a TypeScript twin that gives the same chart,
states, verdicts or paths. They are in their own folder so that the run over
`tasks/conformance/models` keeps its 21 models. The tests in `apps/earnest/tests/rust_models` copy
them into a temporary folder and run the real binary on both twins; every number below is derived
by hand from the model and the contract in the root `README.md`, never from the checker.

None of these files has a `pub` item, a comment or a line that `rustfmt` would change, and
`tests/rust_models/crates.rs` builds all of them in a real crate that depends on `earnest`, with
`cargo clippy -- -D warnings` under the lint table of this repository and `rustfmt --check`.

| File | Twin | Model | States | Rules | Shortest path |
|---|---|---|---|---|---|
| `cart.rs` | `../models/cart.model.ts` | `Cart` | 6 | `focus_lands` broken (the TypeScript rule is `focusLands`: a Rust id is a snake case name) | `press`, `land` from the start with `surface = "shortlist"` |
| `dial.rs` | `../models/dial.model.ts` | `Dial` | 36 | `near` broken | 10 steps, five `east` and five `north` |
| `upload.rs` | `../models/upload.model.ts` | `Upload` | 16 | `quiet`, `waiting`, `counted` hold | none |
| `ticket.rs` | `../models/ticket.model.ts` | `Ticket` | 12 | `seated` holds | none |
| `pick.rs` | `pick.model.ts` | `Pick` | 9 | `fits` broken, `listed` holds | `sell[who = "Kim"]` from the start with `size = 9` |

What each file is there for:

| File | Covers |
|---|---|
| `cart.rs` | A `#[view(...)]` block with `title`, `step`, `colors`, a `#[state]` label and a `#[group]` label, on a model whose `surface` is a `#[one_of("results", "shortlist")]` text, so the chart has the same group boxes as the TypeScript chart. The chart of the twin and of `cart.model.ts` are the same text, from `earnest chart` and inside `earnest report` |
| `dial.rs` | A `#[view(title = ...)]` block with only a state label, on a model with 60 arrows, more than a chart may hold, so the note of what the chart left out is the same too |
| `upload.rs` | `Timer`, `Request` and `Retry` with every operation of each. The test also tightens `counted` to `attempts <= 1` in both languages and compares the path, `send`, `fail`, `wait`, `wake`, `rearm`, `send`, `fail`, `wait`, and the whole result |
| `ticket.rs` | A `#[one_of("Kim", "Lee")]` text in an `Option` and in an action's parameter, a record with a ranged number, and an action that nothing can stop, which returns `Self` |
| `pick.rs` | A `#[one_of(1, 5, 9)]` number as a field and as a parameter, next to a listed text. It has no TypeScript model in `../models`, so `pick.model.ts` is its twin. 3 sizes times 3 owners (none, Kim, Lee) are 9 states. `fits` breaks in the 2 states that have size 9 and an owner, and the start with size 9 is the one from which one `sell` reaches it |

The TLA+ export of `pick.rs` and of `upload.rs` is the same text as the export of their twins, and
`earnest confirm --tlc` finds the same states and verdicts for each file here. The others differ
in the text of the module only where Rust differs: `cart.rs` names its rule and its end
`focus_lands` and `in_cart`; `dial.rs` and `ticket.rs` keep an addition inside its
`u8`, so `x + 1` has a guard `(x + 1) \in 0..255`; and `ticket.rs` reads `seat.row <= 2` where the
TypeScript rule reads `s.seat.row >= 1 && s.seat.row <= 2`, which `clippy::manual_range_contains`
would flag.
