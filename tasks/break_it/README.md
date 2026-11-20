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
