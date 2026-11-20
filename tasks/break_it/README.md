# Break it

Models that `earnest` gets wrong, found by an adversarial pass over the build, each with a test
that states the right answer. The tests in `tests/break_it` copy a file from `models/` into a
temporary folder, run the real binary on it with `verify --json --no-cache` (and, for one case,
the TypeScript library's `explore` in Node), and assert what the contract in the root `README.md`
says. Every test fails today and passes once its finding is fixed.

```sh
cargo test --locked -p earnest_break_it
```

The tests build the `earnest` binary themselves, so they always run the current code. They need
`node` 22.18 or newer and `git` on the `PATH`. Every number below is derived by hand from the
model and the contract, never from the checker. Every `.model.ts` file here except
`dead_code.model.ts` passes `tsc` with `strict`.

## Rules that are dropped without a word

`models/dropped/*.model.ts` hold the same model `Climb`: `n` starts at 0, `inc` adds one while
`n < 3`, and 3 is a marked end, so the states are 0, 1, 2, 3. Each file makes the rule "The count
stays at most one" (`s.n <= 1`) in a way the reader does not recognise as a rule. `n = 2` breaks
it after two steps (`inc`, `inc`), which is what `explore` in Node reports for every file. The
root `README.md` allows a top-level `const` to be a declaration, a number, text, a boolean or a
helper function, and says anything else makes the file unreadable. So the right result is
`unreadable`, or the rule checked and `broken` after two steps. Today each file is `pass` with 4
states and no rule at all.

| File | How the rule is made | Test |
|---|---|---|
| `dropped/alias.model.ts` | `const Alias = Climb;` then `Alias.rule({...})` | `a_rule_made_through_another_name_for_the_model_is_not_dropped` |
| `dropped/and.model.ts` | `true && Climb.rule({...})` | `a_rule_made_on_the_right_of_a_logical_and_is_not_dropped` |
| `dropped/ternary.model.ts` | `true ? Climb.rule({...}) : null` | `a_rule_made_in_a_branch_of_a_choice_is_not_dropped` |
| `dropped/call.model.ts` | `Climb.rule.call(Climb, {...})` | `a_rule_made_with_call_on_the_method_is_not_dropped` |
| `dropped/record.model.ts` | `{ low: Climb.rule({...}) }` | `a_rule_made_inside_a_record_constant_is_not_dropped` |
| `dropped/wrapped.model.ts` | `same(Climb.rule({...}))` with the helper `same = (part) => part` | `a_rule_passed_through_a_helper_function_is_not_dropped` |

The same holds for an action or an end made in any of these ways.

## A constant replaced by an input of the same name

`models/shadow/*.model.ts` hold the model `Shadow`: a top-level `const limit = 5`, `n` from 0, and
the action `inc` with the input `limit` of `m.int(0, 2)`. Its `when` reads `limit` without taking
the input, so in TypeScript `limit` there is the constant 5: `inc` runs while `n < 5`, `n` goes 0
to 5 (6 states), every state from 2 on is a marked end, and the rule `low` (`s.n <= 2`) breaks at
`n = 3` after three steps. `explore` in Node reports exactly that. Today the checker reads `limit`
as the input, tries 0, 1 and 2, stops at `n = 2` and reports `pass` with 3 states and `low`
holding. The right result is `fail`, 6 states, `low` broken after 3 steps, or `unreadable` as the
reader already answers when an input is taken under another name.

| File | `when` | `then` | Test |
|---|---|---|---|
| `shadow/neither.model.ts` | `(s) => s.n < limit` | `(s) => ...` | `a_constant_is_not_replaced_by_an_input_of_the_same_name_that_no_function_takes` |
| `shadow/then_only.model.ts` | `(s) => s.n < limit` | `(s, { limit }) => ...` | `a_constant_in_when_is_not_replaced_by_an_input_that_only_then_takes` |
| `shadow/whole_input.model.ts` | `(s, input) => s.n < limit && input.limit >= 0` | `(s) => ...` | `a_constant_is_not_replaced_by_a_field_of_the_whole_input` |

## A rule loosened through a constant or a helper

`models/changes/before/tank.model.ts` is committed, then replaced by
`models/changes/after/tank.model.ts`, and the run is given `--base HEAD`. `level` goes 0, 1, 2, 3
in both. Between the two only `const MAX` goes from 2 to 3 and the helper `allowed` from
`level <= 2` to `level <= 3`; the text of both rules is the same.

| Rule | Condition | Change | Why | Test |
|---|---|---|---|---|
| `capped` | `s.level <= MAX` | `loosened` | Level 3 passes the new rule and breaks the old one, where `MAX` was 2 | `a_rule_loosened_by_changing_its_constant_is_listed_as_loosened` |
| `checked` | `allowed(s.level)` | `loosened` | Level 3 passes the new rule and breaks the old one, where the helper allowed at most 2 | `a_rule_loosened_by_changing_its_helper_is_listed_as_loosened` |

Today neither rule is listed: the comparison is by the text of the condition, and the old
condition is evaluated with the new file's constants and helpers.

## `explore` says a rule holds that it could not evaluate

`models/basket.model.ts`, model `Basket`, state `(chosen, n)`: `({qty: 0}, 0)`, `({qty: 0}, 1)`,
`({qty: 0}, 2)`, then `clear` gives `(null, 2)`, a marked end. 4 states. Both rules read
`s.chosen!.qty`, which can't be evaluated in `(null, 2)`, and neither is broken anywhere, so by
the root `README.md` both verdicts are `unknown` and there is a fault after three steps. The
checker reports that. `explore` in Node reports the fault and gives both rules `holds`.

Test: `explore_in_node_gives_unknown_for_a_rule_that_faults`.

## Legal files that are refused

| File | Model | States | Rules | What is refused today | Test |
|---|---|---|---|---|---|
| `box.model.ts` | `Box` | 7: `null`, and `qty` 0 to 2 by `hot` | `cold` broken after `pick`, `more`, `more`, `heat` in any order that starts with `pick` (4 steps) | `{ ...s.chosen!, qty: ... }`, a record with a spread of an optional field marked present with `!` | `a_record_is_spread_from_an_optional_field_marked_present` |
| `flag.model.ts` | `Flag` | 2: `(false, [])`, then `(true, ["Bill"])`, a marked end | `agree` holds | Every `s.inbox.isFull`, because the model also has a field named `isFull`. Any field or record field named like a camelCase primitive property (`isEmpty`, `isIdle`, `hasFired`, `canRetry`, ...) does the same to that property in the whole file | `a_field_may_have_the_name_of_a_primitive_property` |

## Files that are read and should not be

| File | What is wrong | Right result | Today | Test |
|---|---|---|---|---|
| `dead_code.model.ts` | The body of `low` goes on after its `return` with the assignment `s.n = 4`. A body ends in a `return`, and assignment makes a file unreadable | `unreadable` | `pass`, 4 states | `a_statement_after_the_last_return_makes_the_file_unreadable` |
| `order.rs` | The rule is `self.num < Some(5)` on an `Option<u8>`. `<` is not an operation of an optional value. In Rust `None < Some(5)` and `Some(1) < Some(5)` are true, so the rule holds in both states (`None`, then `Some(1)`) | `unreadable`, or `pass` with 2 states | `fail`: a fault "reads a value that is null" in the start state, and the rule `unknown` | `an_optional_number_compared_by_order_in_rust_is_not_a_fault` |

## Second pass

A second adversarial pass, made after the findings above were fixed. Its tests are in
`tests/break_it/second.rs` and its files in `models/second/`. Each Rust file here was also
compiled as real Rust and searched breadth first over `starts()` and the action methods, in a
test build and in a release build; "in Rust" below is what that search reported. Every
`.model.ts` file here passes `tsc` with `strict`. Each test fails today.

### Rust: a broken rule reported as `pass`

| File | Model | By hand | Today | Test |
|---|---|---|---|---|
| `second/twins.rs` | `Twins`: `on` goes false, true (2 states). The file has a function `pick(_first, second)` that gives `second`, and the model a helper method `pick(&self, second)` that gives `!second` | The rule `kept` is `self.pick(true)`, a call of the method: `!true`, broken in the start state (0 steps). The rule `outside` is `pick(false, true)`, the function: holds. Right result: `kept` broken after 0 steps, or `unreadable` | `pass`, 2 states, `kept` holds: the method call is read as a call of the function of the same name, with the state as its first argument | `a_helper_method_is_not_replaced_by_a_function_of_the_same_name` |
| `second/notes.rs` | `Notes`: `notes: Map<Key, Option<bool>>`, `note(key)` sets an absent key to `None` (4 states: no key, `Left`, `Right`, both) | The rule `blank` is `forall::<Key>(\|key\| self.notes.get(key).is_none())`. After one `note`, `get` gives `Some(None)` in Rust, so `blank` is broken after 1 step. Right result: that, or `unreadable` | `pass`, 4 states, `blank` holds: a present `None` and an absent key are one value, `null` | `an_optional_value_of_a_map_is_present_in_rust` |
| `second/gaps.rs` | `Gaps`: `items: Seq<Option<bool>, 1>` and `inbox: Queue<Option<bool>, 1>`, each filled once with `None` (4 states) | `first()`, `last()`, `get(0)` and `head()` give `Some(None)` in Rust once the `None` is in, so each of the four rules is broken after 1 step. Right result: that, or `unreadable` | `pass`, 4 states, four rules hold | `an_optional_item_of_a_sequence_or_a_queue_is_present_in_rust` |
| `second/wrap.rs` | `Wrap`: `n: u8` goes 0, 1, 2, 3 (4 states) | The rule `below` is `self.n - 1 <= 2`. At `n = 0` Rust panics with "attempt to subtract with overflow" in a test build, and gives 255, so `false`, in a release build. Calling `state.below()` on the start state is what a `cargo test` replay does. Right result: a fault or the rule broken in the start state, or `unreadable`; the root `README.md` does not say which | `pass`, 4 states, `below` holds: the checker computes -1 | `unsigned_subtraction_below_zero_in_rust_is_not_a_pass` |
| `second/short.rs` | `Short`: `items: Seq<bool, 2>` grows to two items (1 + 2 + 4 = 7 states) | The rule `small` is `self.items.size() - 1 < 2`, and a size is a `usize`: the same as above on the empty list | `pass`, 7 states, `small` holds | `a_size_less_one_of_an_empty_list_in_rust_is_not_a_pass` |

In TypeScript `null` is one value, so `s.notes.get(key) === null` means the same to the checker
and to `explore`; only the Rust reading of an `Option` inside a `Map`, `Seq` or `Queue` departs
from what the file does when it runs.

### A model file that is left out without a word

`second/stray.model.ts` holds the model `Stray`: `n` from 0, `inc` while `n < 3`, 3 is a marked
end, and the rule `low` (`s.n <= 1`) breaks at `n = 2` after two steps (`inc`, `inc`). `explore`
in Node reports exactly that. One byte in the sentence of `inc` is the Latin-1 `é` (0xE9), which
is not UTF-8. Today `earnest verify stray.model.ts` prints `0 models: 0 pass, 0 fail, 0
unfinished` and exits 0, also when the file is found in a folder; a `.rs` model file with such
a byte is left out the same way. The file imports `@earnest/model`, so it is a model file: the
right result is `unreadable` (exit 1), or the model checked with `low` broken after 2 steps.

Test: `a_model_file_that_is_not_utf8_is_not_skipped`. The test names the file in place, because
the sandbox writes only text.

### A removed rule that is not listed

`second/removed/before.model.ts` is committed with the models `Tank` and `Lamp`, then replaced
by `second/removed/after.model.ts`, which keeps `Tank` as it was and has no `Lamp`. The run is
given `--base HEAD`. `Lamp` had the rule `dim`, so `Lamp.dim` is `removed`. Today no change is
listed. A rule deleted from a model that stays is listed as `removed`; a rule that goes with
its whole model is not, and a renamed model lists its rules only as `added`.

Test: `a_rule_removed_with_its_model_is_listed_as_removed`.

### Legal files that are refused

| File | Model | States | Rules | What is refused today | Test |
|---|---|---|---|---|---|
| `second/either.model.ts` | `Either` | 3: `null`, `{qty: 0, who: "ann"}`, `{qty: 1, who: null}` | `named` broken after `pick` with `g = "bob"` (1 step) | `g === "ann" ? { qty: 0, who: g } : { qty: 1, who: null }`: "a choice between a record and a record". A record literal that gives an optional field a value is another kind of record than one that gives it `null`, or than the declared record | `a_choice_between_two_records_of_one_declaration_is_read` |
| `second/bound.model.ts` | `Bound` | 3: `null`, `{qty: 1, who: "ann"}`, `{qty: 1, who: "bob"}` | `one` holds | `const fresh = { qty: 0, who: g }; return { chosen: { ...fresh, qty: 1 } };`: "a record where a record is needed" | `a_record_bound_to_a_name_is_spread_into_its_declaration` |
| `second/within.rs` | `Within` | 4: `n` is 0, 1, 2, 3 | `low` (`self.below(double(1))`) broken after 2 steps | The helper method `fn below(&self, limit: u8)`: "a number needs `#[range(lo, hi)]`". Rust refuses `#[range]` there ("cannot find attribute `range`"), because `#[model]` takes it only off an action's parameters, so no helper method can take a number. The function `double(number: u8)` outside the model is read | `a_helper_method_in_rust_takes_a_number` |

## Third pass

A third adversarial pass, made after the second pass's findings were fixed. Its tests are in
`tests/break_it/third.rs` and its files in `models/third/`. Each Rust file was compiled as real
Rust and searched breadth first over `starts()` and `steps()` in a test build and a release
build, and each TypeScript file was run with `explore` in Node; "in Rust" and "in Node" below
are what those searches reported. Every `.model.ts` file here except `early.model.ts` passes
`tsc` with `strict`. Each test fails today.

### A broken rule reported as `pass`

| File | Model | By hand | Today | Test |
|---|---|---|---|---|
| `third/alias.rs` | `Alias`: `gate` goes `Locked`, `Open` (2 states) | The file has `use earnest::model::forall as exists;`, so the rule `known`, `exists::<Gate>(\|value\| value == self.gate)`, calls the library's `forall`, which is false for either gate. In Rust `known` is broken in the start state (0 steps). Right result: that, or `unreadable` | `pass`, 2 states, `known` holds: the call is read by its written name as `exists` | `a_quantifier_imported_under_the_other_name_is_not_read_as_the_other` |
| `third/huge.model.ts` | `Huge`: `n` goes 0, 1, 2 (3 states) | The rule `apart` is `s.n * HALF * 2 + 1 !== s.n * HALF * 2` with `HALF = 2^52`. At `n = 1` both sides are 2^53 in JavaScript, whose numbers are 64-bit floats, so `apart` is broken after 1 step, as Node reports. Every literal is exact; only the arithmetic differs | `pass`, 3 states: the checker computes in 64-bit integers, so 2^53 + 1 differs from 2^53 | `typescript_arithmetic_beyond_two_to_the_fifty_three_is_not_a_pass` |
| `third/unsafe.model.ts` | `Unsafe`: `n` is `m.int(9007199254740991, 9007199254740992)`, every state an end (2 states) | The library throws on the declaration (`Number.isSafeInteger` is false for 2^53), and the file is refused for any other declaration the library throws on. Read as JavaScript, the rule `sane` (`s.n + 1 > s.n`) is false at 2^53, broken in a start state. Right result: `unreadable`, or `sane` broken after 0 steps | `pass`, 2 states | `a_typescript_range_beyond_the_safe_integers_is_refused` |
| `third/early.model.ts` | `Early`: `n` goes 0 to 3 (4 states), a top-level `const LIMIT = 5` | The rule `low` is `{ const most = LIMIT; const LIMIT = 1; return s.n <= most + LIMIT - LIMIT; }`. In TypeScript the first `LIMIT` is the block's own constant, read before its declaration: `tsc` refuses it (TS2448) and Node throws a `ReferenceError`. It is never the top-level 5. Right result: `unreadable`, or the rule faulting | `pass`, 4 states: the first `LIMIT` is read as the top-level constant. `const a = a + 1;` in a block, and the same in a helper's block, read the outer constant the same way | `a_block_constant_read_before_its_declaration_is_not_the_top_level_one` |

The library itself treats a number beyond 2^53 in a next state as the fault "computes a number
too large to hold" (`npm/model/src/search.ts`); the checker does not, and neither checks a
number computed inside a rule or a `when`.

### A legal file read as something else

| File | Model | By hand | Today | Test |
|---|---|---|---|---|
| `third/lowest.rs` | `Lowest`: `x: i8` with `#[range(-128, -126)]`, from -127 down to -128 (2 states) | `-128` is an `i8` in Rust, so `self.x > -128` and the rule `above`, `self.x >= -128`, are plain comparisons: `above` holds in both states. Right result: `pass` with 2 states, or `unreadable` | `fail`, 1 state: a fault "the rule `above` computes a number outside `i8`" in the start state, because `128` is fitted to `i8` before it is negated. `#[initial(-128)]` on an `i8` is refused for the same reason ("a starting value computes a number outside `i8`"), and the TLA+ export carries the same reading (`128 \in (-128)..127`) | `the_smallest_i8_written_as_a_negative_literal_is_an_i8` |

### A legal file that is refused

| File | Model | States | Rules | What is refused today | Test |
|---|---|---|---|---|---|
| `third/keyed.model.ts` | `Keyed` | 2: `{}`, then `{"ann" -> {qty: 1, tag: {size: 1}}}` | `empty` broken after 1 step | `m.mapOf(["ann", { qty: 1, tag: { size: 1 } }])` where `tag` is an optional record: "a map where a map is needed". The same record is read in `m.setOf`, `m.seqOf` and `s.byWho.set("ann", ...)` | `a_map_literal_takes_a_record_whose_optional_field_has_a_value` |

### A run that aborts

A TypeScript condition nested in 6,000 brackets (4,000 are read), a chain of 20,000 `!`, of
10,000 `? :` or of 10,000 `+`, or a Rust condition nested in 2,000 brackets (1,000 are read),
overflows the checker's stack: the process aborts with `fatal runtime error: stack
overflow, aborting` (exit code 134), and no model of the run is reported, including the
readable files beside it. `rustc` also crashes on the Rust file, so only the TypeScript case is
a legal file. Right result: the deep file `unreadable` and the other files checked.

Test: `a_deeply_nested_condition_does_not_abort_the_run`, which writes a condition in 20,000
brackets beside `huge.model.ts`.

## Third pass, second tester

A third adversarial pass, made after the second round of fixes. Its tests are in
`tests/break_it/third_b.rs` and its files in `models/third_b/`. Each Rust file here was also
compiled as real Rust and searched breadth first over `starts()` and `steps()`, in a test build
and in a release build, and each `.model.ts` file was run through `explore` in Node. Every
number below is derived by hand. Each test fails today.

### A rule that holds in the checker and is broken in Node

| File | Model | By hand | Today | Test |
|---|---|---|---|---|
| `third_b/round.model.ts` | `Round`: `n` is 0, 1, 2, 3 (4 states) | The rule `exact` is `s.n + 9007199254740992 - 9007199254740992 === s.n`. JavaScript adds and subtracts in 64-bit floating point: `2^53 + 1` is half way between two numbers it can hold and rounds to `2^53`, so at `n = 1` the left side is 0 and the rule is broken after one step. `explore` in Node reports exactly that | `pass`, 4 states, `exact` holds: the checker adds exact integers | `a_number_beyond_two_to_the_53_is_not_added_exactly` |
| `third_b/wide_range.model.ts` | `Wide` | `m.int(9007199254740990, 9007199254740996)` throws in Node ("needs two whole numbers"), so the file does not run. The checker reads it, searches 7 states and reports `skips` broken | `fail`, 7 states | `a_range_the_library_throws_on_is_unreadable` |

The root `README.md` says a number in a model is "a whole number that fits in 64 bits" and that
TypeScript has one kind of number "held in 64 bits". It does not say how far a TypeScript number
is exact. Two readings: the model's numbers are JavaScript's (exact to 2^53, as `explore`
already treats a next state past `Number.MAX_SAFE_INTEGER` as a fault), or they are exact 64-bit
integers (then Node and `explore` are the ones that are wrong). The first is the one the file
runs with. The smallest fix is to make a number of a TypeScript model past 2^53 in magnitude
unreadable when written out and a fault when computed, as `explore` does for a next state.

### Legal Rust that is judged as a fault

`third_b/min.rs`, model `Min`: `y: i8` runs from -126 down to -128 (3 states). `least`
(`self.y >= -128`) holds, and `never` (`self.y != -128`) and `never_low` (`self.y != LOW`, with
`const LOW: i8 = -128`) are broken after two steps, in a test build and a release build. The
checker reads `-128` as the negation of `128`, which is outside `i8`: the rule is a fault in the
start state, the action's `when` too, and the search stops at 1 state with every rule `unknown`.
`#[initial(-128)]` on an `i8`, and `-2147483648` on an `i32`, are refused for the same reason
("computes a number outside `i8`"). Test: `the_smallest_i8_is_a_number_of_its_type`.

### Model files that are left out without a word

| File | What is wrong | Right result | Today | Test |
|---|---|---|---|---|
| `third_b/escaped.model.ts` | `from "@earnest\x2fmodel"`. The specifier is `@earnest/model` in JavaScript (Node runs the file and `explore` reports `low` broken after 2 steps). A Unicode escape for the `@` or for the `o`, and a line continuation inside the text, do the same | `low` broken after 2 steps, or unreadable | `0 models`, exit 0, also named explicitly | `a_typescript_model_imported_through_an_escaped_specifier_is_not_left_out` |
| `third_b/comment_path.rs` | `use earnest::/* the model module */model::{..}` compiles; so does a `//` comment on the line between the two segments. The file holds `earnest::` and `model::` but not `earnest::model` | the same | `0 models`, exit 0 | `a_rust_model_with_a_comment_inside_its_use_path_is_not_left_out` |
| `third_b/utf16_mark.model.ts` | The first two bytes are FF FE and the rest is ASCII text that imports the library. The file is not UTF-8, and the reader decodes it as UTF-16, finds no import in the nonsense and drops it. The same holds for a file that starts FE FF | unreadable (the contract says a model file that is not UTF-8 is `unreadable`) | `0 models`, exit 0 | `a_file_that_starts_with_a_utf16_mark_and_holds_ascii_text_is_not_left_out` |

### The process aborts

A rule that is a long chain makes the reader or the lowering recurse until the main thread's
stack is gone: "thread 'main' has overflowed its stack", exit 134, nothing printed, so the
other models of the same run print nothing either. Found at 20 000 `!`, 20 000 `+ 0`, and 10 000
parentheses or `? :` in TypeScript, and at 3 000 parentheses, `{ }` or `+ 0` in Rust (at
3 000 `+ 0` the abort is in a worker thread). 1 000 of each is read. A chain of `&&` or `else
if` is also quadratic in the Rust reader: 3 000 take 1.3 s and 10 000 take 17 s before they
abort. The right result is a refusal that names the depth, or the model checked.

Tests: `twenty_thousand_negations_in_a_typescript_rule_do_not_abort_the_run`,
`twenty_thousand_additions_in_a_typescript_rule_do_not_abort_the_run`,
`three_thousand_parentheses_in_a_rust_rule_do_not_abort_the_run` and
`three_thousand_additions_in_a_rust_rule_do_not_abort_the_run`. Each puts a second, good model
beside the deep one and asserts that it is still reported.

### `explore` and `%` of a negative number

`third_b/remainder.model.ts`, model `Rem`, `n` from 0 to 2 (3 states). The rule `below` is
`(s.n - 1) % 3 !== 99`. At `n = 0` the left side is `-1 % 3`. The root `README.md` makes `%` on
a negative number a fault, so `below` is `unknown` with a fault after 0 steps, which is what the
checker reports. `explore` runs the file as JavaScript and reports `holds`, and no finding.
`explore` cannot intercept `%`, so the smallest fix may be one sentence in the contract that it
does not. Test: `explore_in_node_gives_unknown_for_a_remainder_of_a_negative_number`.

### Suspected, no test

- `--model Name` for a model that was removed: the root `README.md` says "with `--model`, only
  removed models of that name are listed", and also that no model matching `--model` is exit 2.
  `earnest verify --base HEAD --model Cafe` for a deleted `Cafe` prints "no model is named `Cafe`"
  and exits 2, so the removed model can never be listed. Two readings: list it, or drop the
  sentence.
- `replay()` finds a step's action by id and then, when no action has that id, by its sentence. A
  path whose action id is renamed to a name no action has is replayed. A path is therefore not
  refused for a wrong id when the label is right. The interface comment says so; the contract
  does not.
- A model file saved as UTF-16 without a byte order mark is not found: it holds no
  `@earnest/model` in UTF-8, and it is valid UTF-8 (NUL bytes between letters).
- A TLA+ module has no bound check in an action's guard for a field of `i32`, `u32` above 2^31 - 1
  or `i64` (`inside` leaves out any bound that TLC's 32-bit integers can't hold), where the
  checker faults at the type's bound. `earnest confirm` would show a difference in a model whose
  number reaches 2^31 - 1. Not run: there is no `tla2tools.jar` in the repository or in
  `target/tools`, and this session has no way to fetch it.

### Attacked and found correct

- Rust integer types `u8 i8 u16 i16 u32 i32 u64 i64 usize` in rules and actions, at their
  bounds: add, subtract, multiply, negate (not the minimum, see above), remainder, `%` of a
  negative, a size or an `attempts()` less one, a `u64` product past `i64`; a literal typed
  through a helper's parameter, a constant, a record field, a map value, a set item, a `Seq`
  item, a closure over a collection, `Some(..)`, an `if` expression and a comparison; a `let`
  of a number with no stated type is refused. About 200 rules were compared with the checker
  against a test build (overflow panics) and a release build (wraps), and in every case the
  checker gave the verdict of a build that did not overflow, a fault where either did, or a
  refusal (`as`, `/`, `.abs()`, `.pow()`, `i32::MAX`, `matches!`, tuples, `|`, `&`, `^`).
- Arithmetic inside a record, `Seq`, `Set`, `Map` and `Option` literal, in a rule and in an
  action's result, is judged, with a fault at the first overflow in every action.
- A method and a function of one name with different arities, `self.helper(..)` calling a
  function that calls a method, two models in one file with helpers of the same names, a helper
  taking `&self` and a record, a helper returning an `Option`, a closure capturing a `let`.
  `Self::name(self, ..)` and mutual recursion are refused.
- TypeScript: about 180 rules over operators, precedence, `-` and `!` chains, string comparison
  (refused), `%`, `? :`, optionals, helpers with shadowed parameters, constants computed from
  constants, short-circuit around a fault: Node and the checker agree, or the checker is more
  careful (a fault where Node gives a value), or the file is refused (`/`, `**`, `|`, `==`,
  `??`, `?.`, `typeof`, default parameters, recursion, a call of a function literal).
- A TypeScript record with an optional field that is a record with an optional field, in a set,
  a map, a sequence and a queue, from a helper's branches, spread from an optional, and tested
  for membership of a set: 1 472 states, 8 verdicts, equal in Node and the checker. The TLA+
  module writes an optional as `<<>>` or `<<x>>`, guards `o[1]` behind `o = <<>>`, and keeps a
  record in a set as a record.
- `--max-states` from 1 to the exact count and one under it, on a three-flag model (4, 7, 8
  states), a five-start model, a 10 370-state model with several starts and a 4 879 681-state
  model: the counted states are judged, the paths are the shortest and replay in Node; a rule
  that breaks in the last counted state is `broken`. `--time-limit` counts what it saw.
- `--jobs` 1, 2, 3, 4 and 8 on 47^4 = 4 879 681 states: identical JSON, 13.5 s on one worker
  and 6.3 s on four, 937 MB.
- `replay()` on every path of a model with a record, optional, set, number and enum input, two
  start states and a path of zero steps: each replays; a changed state, a reversed path, a
  changed start, a dropped input and a repeated step are refused.
- Discovery: a folder named like a model file, a link to a folder, a broken link, a NUL byte, a
  UTF-8 byte order mark, a Latin-1 byte in a comment, UTF-16 with a mark in `.ts`, `.mts`, `.tsx`
  and `.rs`, an overlong, a surrogate and a too-large UTF-8 sequence, in a subfolder and named
  explicitly: each is listed or refused with the first bad byte, exit 1, and with `--model`,
  `--rule`, `chart`, `report` and `tla` too.
- `--base` with a deleted file, a renamed file, a moved file, a file that no longer imports the
  library, a renamed rule, a loosened rule, an added model and a file that became unreadable,
  in one commit in a real repository; file names with a space, an accent, a quotation mark and
  a tab; from a subfolder; a base that is not a commit (exit 2).
- The cache: four `earnest verify` runs at once on three files, 60 times: identical output, no
  stderr. A cache entry cut to half, to nothing, and by one byte is not trusted. A key holds the
  file, its text, the limits, the rule and a fingerprint of every crate's `Cargo.toml` and `src`.
  `--model` runs share entries by file and keep each model's own result.
- Hostile text that does not abort: a 5 MB comment, 20 000 rules, 5 000 actions, identifiers
  named `Next`, `Init`, `vars`, `MODULE`, `EXTENDS`, `VARIABLES`, `UNCHANGED`, `ENABLED`,
  `Rule_x`, `InRange`, `Stop`, `Fault`, `Judged_r`, `TRUE`, `BOOLEAN`, `Len`, `DOMAIN`, `class`,
  `function`, `constructor`, `__proto__`, `toString`, `prototype`, Unicode identifiers, CRLF in
  a template string, a lone surrogate, U+2028 and U+0000 in a text. 31 models equal in Node.
- The chart's left-out note for 5 broken states beyond 20 arrows: 10 of 31 states and 10 of 30
  arrows, as derived.
- `--rule` keeps the findings of the model (stuck, out of range, fault).

### Not covered

`earnest confirm --tlc` and TLC itself (no jar; the TLA+ modules were read by hand only). A
rebuild of the checker after a change in a crate (the fingerprint code was read, not run).
`report` with several unfinished models. A view with every label form against an independent
reading of the labels. `Timer` and `Lock` beyond one `Retry` model. A `.rs` file with `#[cfg]`
inside a model.

## Fourth pass (a)

A fourth adversarial pass, made on commit 90ac5e6 after the third round of fixes. Its tests are
in `tests/break_it/fourth_a.rs` and its files in `models/fourth_a/`. Each TypeScript file was run
with `explore` in Node and each Rust file compiled with `rustc` (a test build and a release
build, searched breadth first over `starts()` and `steps()`). Every `.model.ts` file here except
`twice.model.ts` passes `tsc` with `strict`. Each test fails today.

### A broken rule reported as `pass` in a file that does not run

| File | By hand | Today | Test |
|---|---|---|---|
| `fourth_a/twice.model.ts` | `Climb`: `n` goes 0 to 3 (4 states). The file has `const MOST = 3;` and then `const MOST = 1;`, and the rule `low` is `s.n <= MOST`. A second `const` of one name in one scope is a SyntaxError in JavaScript ("Identifier 'MOST' has already been declared", so Node never runs the file) and TS2451 in `tsc`. The file has no meaning to judge by: right result `unreadable` | `pass`, 4 states, `low` holds: the checker reads the first `MOST` and drops the second without a word. With the two lines swapped it reports `low` broken, so the verdict depends on which one comes first. The same holds for two helpers of one name, two `const`s of one name in a block, a block `const` with the name of the function's parameter and an arrow function with two parameters of one name (`(n, n) => ...`) | `a_typescript_constant_declared_twice_makes_the_file_unreadable` |
| `fourth_a/twice.rs` | `Climb` as above, with `const MOST: u8 = 3;` then `const MOST: u8 = 1;`, and the helper method `below` written twice, first giving `true` and then `self.n <= 1`. `rustc` refuses the file (E0428 and E0592), so by the contract ("agree with Rust or refuse") the right result is `unreadable` | `pass`, 4 states: the first `MOST` and the first `below` are read | `a_rust_constant_and_a_method_defined_twice_make_the_file_unreadable` |

### Model files that are left out without a word

Both files hold the model `Climb`: `n` from 0, `inc` while `n < 3`, 3 is a marked end, and the
rule `low` (`s.n <= 1`) breaks at `n = 2` after two steps. `explore` in Node reports exactly
that for both. Right result: `low` broken after two steps, or the file `unreadable`.

| File | How it reaches the library | Today | Test |
|---|---|---|---|
| `fourth_a/joined.model.ts` | `const { m } = await import("@ear" + "nest/model");`. The contract says a file is judged by what its imports mean and names a dynamic `import()`. The text never says `earnest` and holds no backslash, so discovery never parses it | `0 models`, exit 0, also when named explicitly | `a_model_that_imports_the_library_through_a_joined_specifier_is_not_left_out` |
| `fourth_a/through_kit.model.ts` with `fourth_a/kit.ts` | `import { m } from "./kit.ts";`, where `kit.ts` is `export { m } from "@earnest/model";`. Run on the folder, `kit.ts` is listed as unreadable ("a model file can't pass on another module") and the model beside it is not mentioned | named explicitly: `0 models`, exit 0 | `a_model_that_takes_the_library_from_a_module_that_passes_it_on_is_not_left_out` |

The Rust form of the second case, a model file that writes `use crate::kit::{Model, model,
require};` where `kit.rs` holds `pub use earnest::model::*;`, compiles and is left out too
(`0 models`, exit 0). The contract defines a Rust model file as one that names the crate, so it
says nothing a user could rely on there; no test.

### Text read as nesting

The nesting pre-scan (`crates/earnest_ast/src/nesting.rs`) counts brackets and runs of `!`, `-`,
`+`, `~`, `&` and `*` in the whole text outside comments, also inside quotes.

| Case | By hand | Today | Test |
|---|---|---|---|
| `Climb` above with the action sentence `"one up "` followed by 1 001 dashes | No expression of the file is nested; the dashes are letters of a text. Right result: `low` broken after two steps, as Node reports | `unreadable`, "the checker reads an expression nested up to 1000 levels deep" at the 1 001st dash | `a_sentence_of_a_thousand_and_one_dashes_is_not_an_expression_nested_too_deep` |
| `banner.ts`, `export const banner = "\n` and 1 001 dashes `";`, beside a model `Good` that passes | `banner.ts` imports nothing, so it is not a model file. Right result: `Good` passes, nothing is unreadable, exit 0 | `banner.ts` is listed as `unreadable`, exit 1: a backslash in a file makes discovery look further, the pre-scan finds the run, and a file too deep is taken as a model file | `a_file_that_does_not_import_the_library_is_not_refused_for_its_text` |

The reverse also holds: a deep expression after the text `"//"` on the same line is skipped by
the pre-scan. The parsers and the lowering then take it without a crash (20 000 brackets, 20 000
`!` and 20 000 `+ 0` in TypeScript, 100 000 brackets in Rust), so this side is not a finding.

### `explore` and a null number

`fourth_a/pick.model.ts`, model `Pick`: `pick` is an optional `m.int(0, 3)` that starts at
`null`, and `choose` sets it to any of 0 to 3 once (5 states). The rule `small` is `s.pick! <=
3`. The root `README.md` says a number compared or added with an optional number "is a fault in
a state where it is `null`", so `small` is `unknown` with a fault in the start state, which is
what the checker reports. `explore` runs `null <= 3` as JavaScript, gets `true`, and reports
`holds` with no finding. `npm/model/README.md` does not list this among the places where Node
and the checker differ. The same holds for `null + 1`, `null * 0`, `n - null` and `null < n` in
a rule, a `when`, an end and a helper, and in a `then`: `({ pick: s.pick! + 1 })` gives Node the
value 1 and a model that passes with 4 states, where the checker reports a fault and 1 state.
Test: `explore_in_node_gives_unknown_for_a_rule_that_compares_a_null_number`.

### Suspected, no test

- A file name with a line feed in it is printed as it is in the text output of `verify`, so a
  file named `bad\npass        Cart  4 states.model.ts` that is unreadable prints a line that
  reads as a passing model `Cart`. `earnest report` prints the name with the line feed as a
  space, which is not the file's name. The contract escapes control characters in a value but
  says nothing of file names. The tests only read JSON, so no test.
- With `--base`, a rule whose old condition faults in a reachable state where the new one holds
  (`s.pick! < 3` to `s.pick === null || s.pick < 3`) is listed as `changed`. The contract
  defines `loosened` by a state that "breaks the old one" and says nothing of a state where the
  old one has no value.
- A rule bound to a `const` that is not exported is checked under the name of the constant
  (`q`), where `explore` names it by its sentence. The contract says "A constant is exported
  under its own name" but not whether a rule must be exported.
- An out-of-range state is judged by no rule, in the checker and in Node alike: a queue of
  capacity 1 sent to twice, with the rule `s.inbox.size <= 1`, gives `holds`. The `--max-states`
  row says "Every state that is counted has its rules judged" and an out-of-range state "is
  counted", so the contract reads both ways.

### Attacked and found correct

- `Op::Safe`: constants folded at read time (`MAX + 2 - MAX` as an `.initial` is refused,
  in a rule it is a fault), products, `-0` (`s.n * -0 === 0`, `m.rem(-0, 2)`, `m.int(0,
  3).initial(-0)`, `m.oneOf([0, -0])` refused as a value listed twice), `1.0`, `1e0`, `0x1`,
  `1_0`; `0.5` and `1e21` refused.
- `m.rem`: a negative first number and a divisor of `-0 + 1`; Node and the checker agree.
- The nesting limit with brackets hidden after `"//"` and `"/*"` in a string, in TypeScript
  and in Rust; at most 100 000 brackets.
- About 60 declarations against the library: every one the library throws on is refused
  (negative `max` and `capacity`, reversed ranges, duplicate and empty choices, mixed kinds,
  starts outside the declaration, optionals of optionals, owned locks in maps); a map literal
  with a key twice keeps the later pair, in Node and the checker.
- About 170 TypeScript rules and actions against `explore`: callbacks that shadow the state's
  name, quantifiers over a declaration and a collection, `===` chains, sets, maps and sequences
  built in different orders, `shift` and `pop` on short sequences, `get(-1)`, spreads in a rule,
  inputs named like a field or like the state, an input with `.initial`, helpers with blocks and
  early returns, block constants in nested `if` blocks, helpers and callbacks, quoted and
  escaped keys (`"n"`, `n`), a key written twice through an escape (refused), `as const`,
  `satisfies`, `!` on a call result, a rule spread from a constant (refused). Apart from the
  null case above every one agrees or is refused.
- About 120 Rust rules against `rustc` in both builds: integer methods (`min`, `pow`,
  `saturating_sub`, `checked_sub`, ...), shifts, casts, `u8::MAX`, `matches!`, `!` on a
  number, `Option` methods other than the library's, `-(-128_i8)`, `u64` and `u128` literals
  above `i64::MAX`, hexadecimal, octal and binary literals, a helper method named like a
  field, enum variants named `Some` and `None`, `return Option::None;` (refused) and
  `#[cfg(not(test))]` (refused). Each agrees with Rust, faults where Rust overflows, or is
  refused.
- Queue, retry and sequence pushed one past their limit: 20 states, one out-of-range finding
  after two steps, equal in Node.
- `--jobs` 1, 2, 3, 8 and 16 on a 215 264-state model with three broken rules, and with
  `--max-states` 777, 5 000 and 100 001: byte-identical JSON apart from `milliseconds`.
- Discovery of a file whose name is not UTF-8 (checked), files in a folder that starts with a
  dot or is named `node_modules` when the folder or the file is named explicitly (checked), and
  a file name with ```` ``` ```` in the report's code block (the fence holds).

### Not covered

`earnest confirm --tlc` (no jar). The cache after a change in the TypeScript library (the key
holds a fingerprint of the crates only, and a TypeScript model's result does not depend on the
library's code). `report` and `chart` with views that use every label form.
## Fourth pass (b)

A fourth adversarial pass, by differential testing. Its tests are in `tests/break_it/fourth_b.rs`
and its files in `models/fourth_b/`. Every `.model.ts` file here passes `tsc` with `strict`.
Every number below is derived by hand from the model and the contract. Each test fails today.
Three tests need more than `node` and `git`: the TLC tests need `java` and the jar that `just tlc`
puts at `target/tools/tla2tools.jar` (or the path in `EARNEST_TLC_JAR`), and say so when the jar
is missing.

### `explore` in Node gives a verdict the contract makes a fault

| File | Model | By hand | Today | Test |
|---|---|---|---|---|
| `fourth_b/null_compare.model.ts` | `Opt`: `o` is `null`, then 0, 1 or 2 after `set` (4 states, `done` marks `o !== null`) | `below` is `s.o! < 3`, `sum` is `s.o! + 1 > 0` and `remainder` is `m.rem(s.o!, 5) >= 0`. The root `README.md` says a number compared or added with an optional number "is a fault in a state where it is null", and that `explore` follows the same rule. In the start state `o` is `null`, so each rule is `unknown` and there is a fault after 0 steps. Elsewhere each holds | The checker says exactly that. `explore` runs the file as JavaScript, where `null < 3` is true and `null + 1` is 1: `pass`, 4 states, all three rules `holds`, no finding | `explore_in_node_gives_unknown_for_a_number_read_from_null_by_order_sum_or_remainder` |
| `fourth_b/null_then.model.ts` | `Bump`: `o` is `null` in every state and `n` is 0 | The `then` of `bump` is `{ n: s.o! + 1 }`, which reads a number from `null`: a fault in the start state, and no state follows. 1 state, a fault after 0 steps | The checker says that. `explore` gets 1 from `null + 1`, takes the step to `n = 1` and stops there: `pass`, 2 states | `explore_in_node_does_not_take_a_step_whose_then_reads_null` |

`explore` only turns a `TypeError` into a fault, and JavaScript throws none for an order, a sum
or a remainder of `null`, nor when `null` is passed to a primitive (`s.sq.includes(s.o!)`,
`s.mp.get(s.o!)` differ the same way). `s.o! !== 7` is not a fault in either. `npm/model/README.md`
lists the differences it knows and not this one, so either `explore` must check the values it is
given or the list must say so, as it says for `%`.

### A Rust model file left out without a word

`fourth_b/raw_crate.rs`, model `Climb`: `use r#earnest::model::{Model, model, require};`. A raw
identifier is the identifier, so `r#earnest` is the crate `earnest` and the file compiles
(checked with `rustc`). In Rust `n` goes 0, 1, 2, 3 (4 states, `top` marks 3) and `low` (`n <= 1`)
is broken after two steps. The checker says `0 models`, exit 0, with no `unreadable` entry, also
when the file is named explicitly. The contract says a file that names the crate is a model file
or is refused. Test: `a_rust_model_that_names_the_crate_as_a_raw_identifier_is_not_left_out`.
Raw identifiers elsewhere in a model (`self.r#n`, `r#require!`, `r#Some`, `-> r#bool`) are
refused with a sentence, which is right.

### A raw identifier is part of an id

`fourth_b/raw_names.rs`, model `Raw`: a field `r#type`, a field `r#match`, an action `fn r#loop`
with an input `r#in`, a rule `fn r#fn` and an end `fn r#else`. In Rust `type` goes 0, 1, 2, 3 (4
states) and `fn` (`type <= 1`) is broken after two steps with `loop` twice. The library's
`Claim::id` is `fn` (run as real Rust), `Step::action` is `loop` and `Debug` prints the field as
`type`; the contract says ids are the method names. The checker's JSON has the rule `r#fn`, the
action `r#loop`, the input `r#in` and the fields `r#type` and `r#match`, so `--rule fn` is exit 2
("no model has a rule `fn`"), the TLA+ module names the invariant `Rule_r_fn`, and a path from the
report does not name the methods of the model as the library does. Test:
`a_raw_identifier_gives_the_plain_name_as_the_id_of_a_rule_an_action_and_a_field`.

### The text output is not one line per model

| File | What is wrong | Right result | Today | Test |
|---|---|---|---|---|
| `fourth_b/newline_name.model.ts` | The model is named `"First line\nSecond line"` (3 states, `low` broken after 2 steps) | The contract says "One line per model", and prints a rule's sentence on one line with a line break as one space: `fail  First line Second line  3 states  0 ms` | The line is cut in two: `fail  First line` and `Second line  3 states  0 ms`. `report` is right (it puts the name on one line) | `the_text_output_prints_a_model_name_with_a_line_break_on_its_own_line` |
| a file named `first\nsecond.model.ts` that reads `Date.now()` | The contract lists each problem as `file:line:column`, the feature and a sentence | One line: ``first second.model.ts:7:73  `Date.now`  the system clock is outside the library`` | `first` and `second.model.ts:7:73  ...` on two lines, and the `unreadable` line is cut the same way. The Markdown report puts the name on one line | `the_text_output_prints_an_unreadable_file_with_a_line_break_in_its_name_on_one_line` |

### `--time-limit` does not stop a model

`fourth_b/slow_rule.model.ts`, model `Slow`: `n` goes 0 to 3 (4 states). The rule `sums` is
`m.forall(Digit, (a) => m.forall(Digit, (b) => m.forall(Digit, (c) => a + b + c + s.n >= 0)))`
with `Digit = m.int(0, 999)`, which holds and takes 10^9 steps in each state. The contract says
`--time-limit <seconds>` stops a model after that long, so `--time-limit 1` ends the run within a
few seconds with the model `unfinished`. The limit is read between states: with 300 digits (2.7
* 10^7 steps) a run with `--time-limit 1` takes 10.2 s for 2 states (5.1 s a state, about 190 ns a
step), and with 1000 it would take about 190 s a state. No declaration here has more than a
million values, so the file is not refused. The same holds for an action with three inputs of
1000 values each: 10^9 combinations in each state, built before any is judged, which took the
process to 38 s and a kernel kill for memory (not a test: it takes the machine with it). Test:
`a_time_limit_stops_a_model_whose_rule_takes_a_billion_steps_in_one_state`, which gives the run
30 s.

### The TLA+ export that TLC can't run or counts wrongly

| File | Model | By hand | Today | Test |
|---|---|---|---|---|
| `fourth_b/big_number.model.ts` | `Big`: `n` is `m.int(2147483645, 2147483650)`, 6 states, `low` (`n < 2147483649`) broken after 4 steps, `prod` (`n * 2 > 0`) holds | The contract says TLC finds the same number of distinct states and the same verdict for each rule, and `earnest confirm` exits 1 only when one differs. Every number is exact in a JavaScript number (below 2^53), so the search holds them and the answer is 6 states, `low` broken, `prod` holds | `earnest tla` writes the module without a word and exit 0. `earnest confirm --tlc` says `differs`, exit 1: the state count did not finish and both rules are `unknown` ("TLC can't handle a number this big"), because TLC's integers are 32 bits | `tlc_finds_the_same_verdicts_for_a_number_beyond_32_bits` |
| `fourth_b/square.model.ts` | `Prod`: `n` goes 0 to 3, `square` is `(50000 + s.n) * (50000 + s.n) > 0` | 4 states and `square` holds: every number in the file is below 2^31, and the largest square is 50 003^2 = 2 500 300 009 | `confirm` says `differs`, exit 1: TLC says `unknown` ("Overflow when computing 50000*50000") | `tlc_finds_the_same_verdicts_for_a_product_past_32_bits_of_numbers_within_them` |
| `fourth_b/text_collision.model.ts` | `Pick`: `letter` is `m.enum(["Z", "Ś"])` and an action chooses either (2 states, no rule can break) | 2 distinct states: the texts differ | `confirm` says `differs`, exit 1: "the checker found 2, TLC found 1". The module holds both texts in a set and TLC fingerprints a text one byte for each character, so `Z` (U+005A) and `Ś` (U+015A) are one state. Found with `"a\u000bb"` and `"a\u200bb"` among 17 texts of control and separator characters, which TLC counts as 16 states; every one of them alone is fine. `-Dfile.encoding=UTF-8` changes nothing | `tlc_counts_two_texts_that_differ_only_above_their_first_byte_as_two_states` |

`crates/earnest_tla/src/lib.rs` documents the limit on numbers in the writer ("a number beyond 32
bits, which TLC can't hold") and the root `README.md` names no limit of either kind. Either the
export refuses such a model, or the contract names the limits, or the module avoids them (wider
arithmetic is not available in TLC; a text can be written as an ASCII name the writer makes
unique).

### Suspected, no test

- `--max-states 3` on the conformance `turnstile.model.ts` prints `fail  Turnstile  3 states so
  far  (state limit of 3)` and counts the model as `fail` in the summary. The contract says a
  model that stops at the limit "is `unfinished` with `<n> states so far`" and also that a rule a
  counted state breaks is `broken` with its path. Both readings fit the text; the status of a
  model that is both is not said.
- The checker orders the inputs of an action with the first input varying fastest (a path to the
  first broken state of a Rust model with inputs `a: Letter, b: u8` and a rule broken for `(C, 0)`
  and `(A, 2)` names `a = C, b = 0`), while `Behavior::steps()` of the library tries the last
  input fastest (it gives `(A, 2)` first). A path of the checker and the first path found by
  walking `steps()` therefore differ, which `npm/model/README.md` states for `explore` and the
  root `README.md` does not state for Rust.
- A constant that is not exported (`const low = Cnt.rule({...})`) is the id `low` in the checker
  and the sentence in `explore`, which only knows the exported names. The contract says the id is
  the name of the exported constant, so it does not say what an unexported one is.
- A file named exactly `.ts`, `.mts`, `.tsx` or `.rs` is not found, also when named, with no
  word: Rust's `Path::extension` and Node's `path.extname` give nothing for a name that is only
  an extension.
- `--base`: a comment inside a rule's condition is not part of its text (a change that only adds
  `/* c */` is not listed), and `ruleChanges` has no file, so two files that each hold a model of
  one name give two entries a reader cannot tell apart.
- Two arguments `./a.model.ts` and `a.model.ts` check the model twice and list it twice.

### Attacked and found correct

Differential runs (every verdict, state count, finding kind and the whole shortest path, with
sets compared as sets):

- TypeScript against `explore` in Node, 2 200 files: 1 000 files of 40 rules over every operator
  and primitive of the tables (numbers, `m.rem`, booleans, text enums, optionals, records, sets,
  sequences, maps, `forall` and `exists` over declarations and collections) with 3 to 6 of 14
  actions with inputs; 800 files of 14 rules and 4 to 7 of 20 actions over `Timer`, `Request`,
  `Retry`, `Lock`, a lock with an owner, `Queue` and `Seq`, with ends that hold or not,
  out-of-range pushes and sends, and stuck states; 400 files of 60 rules written with only the
  parentheses precedence needs (`||`, `&&`, `===`, `<`, `+`, `-`, `*`, unary `!` and `-`,
  `? :`). 0 differences. The first group left out a `!` on a value that can be `null`, after
  the differences above.
- 22 out-of-range values in a start field, a record, a set, a map, a sequence and a map of sets,
  by a literal, a sum, a product and `m.rem`: the same finding in both.
- 28 declarations with a start value outside what it declares, a seq past its `max`, a queue past
  its `capacity`, a repeated map key and a repeated set member: refused by the checker where
  Node throws, the same otherwise. `.initial(v).optional()` is refused by the checker and
  accepted by Node, with a sentence.
- 100 spellings of one rule (index access, hexadecimal, binary, octal, `1_0`, `.2e1`, a unicode
  escape in an identifier, `as`, `satisfies`, `!`, comments between tokens, line breaks, ten
  operators outside the library) and 23 block bodies (`return` then a line break, `if` with and
  without a block, `const` chains, empty statements, a repeated `const` name in an inner block):
  each is read as Node runs it or refused with a sentence.
- Rust against a real build of the same file, in a test build, 941 files of 30 rules: `u8`, `i8`
  and `usize` arithmetic with typed and untyped literals, `bool`, a `Domain` enum, a `Domain`
  struct, `Option`, `Set`, `Seq`, `Map`, closures, helper functions and methods, `forall::<T>` and
  `exists::<T>`, on models with and without `#[initial]` (several start states) and 3 to 6 of 14
  actions with inputs. The checker's overflow faults line up with the panics. 71 more rules of
  every spelling in the `Set`, `Seq`, `Map`, `Option` and closure tables, `if` and `let` forms:
  0 differences, and each refusal has a sentence. A model with enums that share a variant name
  and enums with explicit discriminants: 0 differences. 57 files of 25 rules over `u8`, `i8`,
  `u16`, `i16`, `u32`, `i32` and `i64` near their bounds: 0 differences. A `usize` sum above
  `i64` is a fault where Rust holds it, as the contract says.
- About 55 spellings of an attribute, a signature, a number and a sentence in Rust (spaced and
  trailing-comma attributes, raw and escaped sentences, doc comments and `#[allow]` between the
  attribute and the method, `pub(crate)`, `const fn`, `return` and `let` in a rule, binary and
  octal numbers, `if` forms of an action): each is checked as Rust runs it or refused.
- `earnest confirm` with TLC 1.8.0 on 84 generated models (63 TypeScript with `Timer`, `Request`,
  `Retry`, `Lock`, `Queue`, `Seq`, sets, maps, records and optionals, and 21 Rust) and on a model
  with sets of sets, maps keyed by sets and records, and sequences of sequences (1 040 states):
  all `same`. 24 single texts of control, separator, combining and escape characters among two
  ordinary ones: all `same`.
- Discovery: 25 spellings of the import in TypeScript and Rust (a template literal, a
  concatenation, `import =`, a namespace, a default import, `createRequire`, `/// <reference>`, a
  leading `::`, a nested `use {}`, a glob, a `#[cfg]` use): each is found or refused, or is not a
  model file by the contract (`.js`, `.mjs`, `.cts`, `.TS`). Folders `build`, `dist`, `target`,
  `node_modules` and `.git` skipped when searched and read when named; `.hidden.model.ts`, a link
  to a file, a broken link, a link to a folder, a folder named `x.model.ts`.
- File names with a newline, a tab, a backtick, a pipe, a quote, a `<`, a `#`, a `%`, a `\`, a
  leading `-` and a `U+2028`, in `verify`, `verify --json` and `report`: the report and the JSON
  are right, the text output is not for a line break (above).
- The report and chart for a model name, a rule sentence, an action sentence, a view title, a
  group, a state label and an enum value that hold backticks, `|`, `-->`, `---`, `%%`, `[*]`,
  `<script>`, `"`, `\`, `#`, `{`, `}` and line breaks (read by hand, 23 states in 3 groups): no
  line of the report starts a fence, a heading or a table. The chart's `Not shown` note counts 22
  of 36 states and 41 of 61 arrows for a 6 by 6 grid with a self-loop.
- `--base` with 16 pairs of conditions (equivalent rewrites, wider and narrower, both at once,
  `m.rem` that faults in a state, `true` to `false`): the classes are as defined; two files with
  models of one name each get their own entry.
- The CLI: `--max-states 0`, a negative or too large number, `--jobs 0`, `--time-limit 0` and a
  fraction, a repeated flag, `--rule` for two models of one id, `--model` for two files, a missing
  `--tlc`, `tla --out` into a folder that does not exist: each is exit 2 or as the contract says.
- 180 models of 180 files run with `--jobs 1`, 16 and the default: identical JSON. A cache entry
  is not stored for a time limit, and is not shared between two spellings of one path.

### Not covered

Mermaid itself (no renderer here: the chart was read, not drawn). The memory a huge product of
inputs takes (it killed the process, so it has no test). `report --base` for a Rust file. A
`confirm` over models with an `Option` of a record in a map of sets. Models run on Windows.

## Fifth pass (a)

A fifth adversarial pass, made on commit 50ef5ab against the code written since the fourth
pass: the test-file rule of discovery, the shared thread pool, Rust views, `#[one_of]`,
actions that return `Self`, `is_multiple_of`, `earnest::model::replay`, and the refusals of
`!`, `%` and re-exports. Its tests are in `tests/break_it/fifth_a.rs` and its files in
`models/fifth_a/`. Each Rust file was compiled with `rustc` against `crates/earnest`, and the
two that compile (`inputs.rs`, `quoted.rs`) are included in the tests as real Rust, which is
why `earnest` is a dev-dependency of this crate. Each TypeScript file was run with `explore`
in Node. Every `.ts` file here except `exported_twice.model.ts` passes `tsc` with `strict`.
Each test fails today.

`models/fifth_a/climb.ts` is the model `Climb` of the first section: `n` from 0, `inc` while
`n < 3`, 3 is a marked end, and `low` (`s.n <= 1`) breaks after two steps (4 states).

### A broken model left out and the run passed

`passes_on.model.ts` holds the model `Lamp` (off, then on and a marked end: 2 states, passes)
and the line `export { m };`, which passes the library on to any file that imports it.
`takes_from_model.ts` is `Climb` written with `import { m } from "./passes_on.model.ts";`; in
Node `explore` reports `low` broken after two steps. `export { m } from "@earnest/model"` is
refused ("a model file can't pass on another module"), and the Rust forms (`pub use`,
`pub(crate) use`, `pub(super) use`, `pub use ::earnest`, `pub extern crate`) are all refused,
but `export { m }` of the imported name is read. `earnest verify` on the folder prints `pass
Lamp`, `1 model: 1 pass`, exit 0: `takes_from_model.ts` never mentions `earnest`, so it is not
found, and nothing says so. Named on the command line it is refused, which is right. Right
result: `passes_on.model.ts` unreadable, or `Climb` checked with `low` broken. Test:
`a_model_that_takes_the_library_from_a_model_file_that_exports_m_is_not_left_out`.

### Files the language refuses, read and passed

| File | By hand | Today | Test |
|---|---|---|---|
| `fifth_a/self_require.rs` | `fn inc(&self) -> Self { require!(self.n < 1); Self { n: self.n + 1 } }`. `require!` expands to `return None`, so `rustc` refuses the file (E0308), and the contract says an action that returns `Self` has no `require!`. Read with the meaning the contract gives `Self` ("nothing can stop it"), `n` would climb past 1 and break `low`. Right result: `unreadable` | `pass`, 2 states: the `require!` is read as the action's `when` | `an_action_returning_self_with_require_is_unreadable` |
| `fifth_a/signed_multiple.rs` | `self.n.is_multiple_of(2)` on an `i8`. Rust has the method on unsigned integers only, so `rustc` refuses the file (E0599); the contract speaks of it on an unsigned number only. Right result: `unreadable` | `pass`, 2 states; on a negative `i8` it is the fault of `%` | `is_multiple_of_on_a_signed_number_is_unreadable` |
| `fifth_a/display_enum.rs` | The view's state label is `format!("the lamp is {}", self.mode)` and its group `self.mode.to_string()`, on an enum that derives `Domain` and has no `Display`. `rustc` refuses both (E0277, E0599); the contract says to write `{:?}` for an enum variant. Right result: `unreadable` | `pass`, and `earnest chart` draws the boxes `the lamp is Off` and `the lamp is On` in groups `Off` and `On` | `a_display_hole_on_an_enum_that_rust_refuses_is_unreadable` |
| `fifth_a/exported_twice.model.ts` | `Climb` with the rule `s.n <= 3` bound to `const low` and then `export { low };` twice. That is a SyntaxError in JavaScript ("Duplicate export of 'low'", Node does not run the file) and TS2300 in `tsc`. Right result: `unreadable` | `pass`, 4 states | `a_rule_exported_twice_makes_the_file_unreadable` |

### Node, Rust and the checker disagree

| File | By hand | Today | Test |
|---|---|---|---|
| `fifth_a/cast.model.ts` | `Opt`: `o` is `null`, then 0, 1 or 2 (4 states). The rule is `(s.o as number) < 3`, which `tsc` accepts. `as` changes nothing in JavaScript, so this is `null < 3` in the start state, as `s.o! < 3` was in the fourth pass; `!` is now refused for that reason and `as` is not. Right result: `unreadable`, or the checker and `explore` give one verdict | The checker: a fault in the start state, `below` `unknown`, `fail`. `explore`: `holds`, `pass`, no finding | `explore_and_the_checker_agree_on_a_null_number_read_through_as` |
| `fifth_a/inline_type.model.ts` | `const low = Climb.rule(...)` and `export { type low };`. The `type` modifier erases the export from the JavaScript, as `export type { low }` does, which is refused as a rule bound to a constant that is not exported. Right result: `unreadable` | Read, with the rule id `low`; `explore` knows no export of it and names the rule by its sentence | `a_rule_exported_only_as_a_type_is_unreadable` |
| `fifth_a/inputs.rs` | `Post`: `deliver(letter: Option<Letter>)` once, then `place(spot: Spot)` once, where `Spot` is a record of `row` (0 or 1) and `lit`. States: the start, 3 after `deliver`, 3 times 4 after `place`: 16. `kept` breaks where nothing came and the post is in the lit spot of row 1: `deliver` with `letter = null`, then `place` with `spot = {row: 1, lit: true}`, 2 steps, which is what the checker reports | `replay` with the inputs written as the report prints them refuses step 1: `steps()` lists the inputs as `Debug` prints them (`None`, `Some(Bill)`, `Spot { row: 1, lit: true }`), and `Move` matches only that text or a text in quotes. The contract says an input is written as the report prints it | `replay_takes_an_optional_and_a_record_input_as_the_report_prints_them` |
| `fifth_a/quoted.rs` | The view's state label is `format!("{:?} by {:?}", self.mode, if self.mode == Mode::On { self.who } else { "Kim" })`, where `who` is a `#[one_of("Kim", "Lee")]` text. When the file runs, `state_label()` is `Off by "Kim"` and then `On by "Lee"`: `Debug` puts a text in quotes. Right result: the chart's boxes read as Rust prints them, or `unreadable` as `{:?}` of a text field is | The chart draws `Off by Kim` and `On by Lee`: the refusal looks only at a `{:?}` whose value is a plain field read | `a_debug_hole_on_a_text_gives_the_chart_the_label_rust_prints` |

### Discovery and the test-file rule

| Case | By hand | Today | Test |
|---|---|---|---|
| `tests/climb.ts`, run with `--model Climb` | The only `Climb` is in a test file, which is not read, and the contract says the run says so; a file is never left out without a word | `error: no model is named Climb`, exit 2, with no word of the skipped file | `a_model_skipped_as_a_test_file_is_named_when_model_asks_for_it` |
| `tests/climb.ts`, run as `earnest verify . ./tests` | `./tests` is named, so below it `climb.ts` is no test file: it is read and `Climb` is checked. `skipped` lists the test files that were not read | `Climb` is checked through `./tests/climb.ts` and `skipped` also lists `tests/climb.ts`, with the line `skipped 1 test file that uses the library` | `a_test_file_read_through_its_own_folder_is_not_listed_as_skipped` |
| `src/climb.ts`, run as `earnest verify . ./src` | One file, reached through two folders: checked and listed once | `2 models`: `src/climb.ts` and `./src/climb.ts`. `earnest verify src ../here/src` does the same. Only files named on the command line are compared by their real path | `a_model_found_through_two_spellings_of_its_folder_is_checked_once` |

### Suspected, no test

- A test file that can't be opened (a broken link `tests/x.ts`) is listed as `unreadable` and
  makes the run exit 1 (read in the code, not run), though by its name it is a test file,
  which the contract says never changes the exit code. The contract says both.
- `earnest chart`, `tla` and `confirm` stop at `--model` with exit 2 before they print the
  skipped line, as `verify` does (the test above is for `verify`).
- `format!("{:?} {", a, b)`: Rust refuses an unclosed `{`; the reader takes the text to its end
  as a hole, so with as many values as holes it would be read. Not run.

### Settled after the pass

All twelve tests pass. The three suspected cases were run and settled: a test file that can't be
opened is skipped and listed as skipped, never unreadable, and does not change the exit code
(unit test `a_test_file_that_cannot_be_opened_is_skipped_and_never_unread` in
`apps/earnest/src/discovery.rs`); `chart`, `tla`, `confirm` and `report` print the skipped line
before the exit 2 of `--model` (tests in `apps/earnest/src/commands.rs`); and a `format!` with a
`{` never closed, or more or fewer values than holes, is refused by the reader (tests in
`crates/earnest_rs/tests/read/views.rs`). The state comparison of
`apps/earnest/tests/rust_models/replay.rs` still goes through `Debug`: it builds its own source
and writes each value with its own `value_text`, which panics on `null` and on a record, and it is
protected, so it was left as it is; `earnest::model::Report` and the matching of a `Move` by value
are tested in `crates/earnest/tests/model/replays.rs`.

### Attacked and found correct

- Discovery: `tests`, `test`, `__tests__`, `benches`, `examples` and `build.rs` by language; a
  `.model.ts` under `tests`; `earnest verify tests`, `./tests/`, an absolute path to `tests`,
  `verify` run from inside `tests` and `verify ..` from there; the skipped line on `verify`,
  `chart`, `report` (its `Skipped files` section), `tla` and `confirm` for a run whose only
  model is in a test file; `--base` with a model moved from `src` into `tests` (`removed`) and
  from `tests` into `src` (`added`).
- `--jobs` 1, 2, 3, 4, 7, 8 and 16, with no limit, `--max-states 777` and `--max-states 5000`,
  on 70 models at once (the conformance models, the benchmark grids of 1 000 and 100 000
  states, `Ledger`, `Wide`, the fourth pass's files and this pass's Rust files: models that
  pass, fail, are stuck, fault, go out of range or are unreadable): byte-identical JSON apart
  from `milliseconds`. The de-duplication of next states inside a worker can't change which
  parent a state gets or where the state limit falls: the merge goes in frontier order.
- `#[one_of]`: `1` and `0x1`, `"a"` and `"\x61"` in one list are refused as a value listed
  twice; `i64` at both ends, `i8` at -128 and 127, `u8` at 0 and 255; `Option<u8>` with
  `#[one_of(5, 2)]` starts at `None`, 5, 2 as `starts()` does; `#[one_of]` on the fields of a
  `Domain` record and on an action input beside a record input and a `#[range]` input: the
  order of `steps()` (first input fastest), 19 states and the paths agree with real Rust;
  text equality across two fields with different lists and with a literal in neither list.
  Order on texts and `match` on a text are refused.
- `replay` of a text with a quote (`a"b`) written with its quotes, as the text report prints it.
- Rust re-exports: `pub(super) use`, `pub(in crate) use`, `pub(self) use`, `pub use ::earnest`,
  a glob, a grouped `pub use {..}`, `pub extern crate earnest` and `extern crate earnest as
  kit` are refused; `use earnest::model::forall as _` is read.
- TypeScript: `%` in a view's template and in a sentence's template is refused; `m.some(x)!`,
  `Climb.rule(...)!`, `export default`, `export { low as high }`, `export let`, `export var`,
  `let x!: T`, `export declare const`, `export type { low }` and `export * from` are refused.
- `earnest confirm --tlc`: texts with quotes, `Option` and record inputs, `#[one_of]` on an
  `Option<u8>`, an `i8` sum past its type in a rule, `u32`, `i64` and `u64` with small ranges,
  a `u8` at 255 in an action that returns `Self`, a single text that is not ASCII: `same`;
  `i64` at its ends: `cannot`, with the reason.
- Views: a `Display` impl in the file and a helper that returns `&'static str` are refused.
- `npm/cli/src/launch.ts` was read: exit codes and signals are passed through.

### Not covered

`--time-limit` across `--jobs` (its cut is timed, so it is not expected to repeat). tla-rs. The
`@earnest/cli` launcher run from an installed package. Models run on Windows, where the test
folder rule meets `\` separators.
