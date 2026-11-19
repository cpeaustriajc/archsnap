# earnest

The Rust model library: what a model file imports with `use earnest::model::{...}`.

A model is a struct that derives `Model`, with its actions, rules and ends in a `#[model]` block.
`earnest verify` reads the file and visits every reachable state. The file is also ordinary Rust,
so a test can replay a path from the report:

```rust
let start = Turnstile::starts().remove(0);
let pushed = start.push().expect("a locked gate can be pushed");
assert!(!pushed.locked());
```

`Turnstile::starts()` lists every start state, an action method takes one step or gives `None`,
and a rule method judges one state.

| Item | What it is |
|---|---|
| `Model` | The derive for the state struct, and the trait it implements: `Type::starts()` lists every start state. A field with `#[initial(value)]` starts there; a `bool`, a ranged integer, an `Option`, an enum or a record without it starts at every value; a collection or part starts empty, free, idle or at zero |
| `Domain` | The derive for an enum or a record, and the trait it implements: `Type::values()` lists every value |
| `model` | The attribute for the `impl` block. It reads `#[action("...")]`, `#[rule("...")]` and `#[end("...")]` and implements `Behavior` |
| `Behavior` | `state.steps()`, `state.rules()` and `state.ends()`: every action tried with every input, and every rule and end judged, as `Step` and `Claim` values |
| `require!` | `require!(condition);` in an action: the action returns `None` when the condition is false |
| `Set<T>`, `Map<K, V>`, `Seq<T, MAX>`, `Queue<T, CAPACITY>` | The collections, written with `Set::of([a, b])`, `Seq::of([a, b])` and `Map::of([(k, v)])` |
| `Lock`, `Lock<Owner>`, `Timer`, `Request`, `Retry<MAX>` | The parts |
| `forall::<T>(f)`, `exists::<T>(f)` | A test over every value of a type that has a `Domain` |
| `Start` | Where a field of a type starts. The derive uses it; a model file does not |

Every operation is a method that gives back a new value and leaves the one it was called on
unchanged. The operations are the ones in the primitives table of the repository's `README.md`,
with `snake_case` names. Sizes and counts are `usize`. An operation that only looks at an argument
takes it by value or by reference. Going past a `MAX` or a `CAPACITY` is not an error here: the
checker reports the state as out of range.

The five Rust conformance models in `tasks/conformance/models` are compiled and stepped along
their reported paths in this crate's doc tests, and the library's own tests are in `tests/model`.
