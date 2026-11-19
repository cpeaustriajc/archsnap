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
