# Conformance models

Model files with the state counts, verdicts and paths `earnest` must report for them. The
acceptance tests in `apps/earnest/tests/behavior` copy these files into a temporary folder and run
the real binary on them; `apps/earnest/tests/behavior/expected.rs` holds the same numbers as the
tables below. Every number here is derived by hand from the model and the contract in the root
`README.md`, never from the checker.

How the numbers are counted:

- Distinct states are every state reachable from a start state, start states included. A state
  that breaks a rule is counted and the search goes on from it.
- A state an action takes out of range is counted, and nothing is searched from it.
- A path is written as its action ids in order, with the input in brackets where the action has
  one. Every path below is the only shortest one unless the row says otherwise.

## Models that every run reads

All files are in `models/`. A state is written as its field values in declaration order.

| File | Model | States | Rules | Other findings | Shortest path | Why that many states |
|---|---|---|---|---|---|---|
| `basket.model.ts` | `Basket` | 4 | `capped` holds | fault in `price` | to the fault: `clear` | `(chosen, total)`: start `({qty: 1}, 0)`; `clear` gives `(null, 0)`; `price` gives `({qty: 1}, 1)`; `clear` from there gives `(null, 1)`. In `(null, 0)` `price` reads `qty` of `null`. Both `null` states are marked ends, so nothing is stuck. 1 + 2 + 1 = 4 |
| `cart.model.ts` | `Cart` | 6 | `focusLands` broken | none | `press`, `land` from the start with `surface = "shortlist"` | `surface` has no initial value, so 2 start states; each walks `Add` → `Adding` → `InCart`. 2 × 3 = 6. On `shortlist` the add lands with focus `Lost` |
| `counter.model.ts` | `Counter` | 4 | `bounded` holds, `even` holds | none | none | `count` goes 0, 1, 2, 3 and 3 is a marked end. 4 |
| `dial.model.ts` | `Dial` | 36 | `near` broken | none | 10 steps, five `east` and five `north` in any order (252 shortest paths; the tests assert the length and that two runs agree) | `x` and `y` each go 0 to 5 on their own. 6 × 6 = 36. Only `(5, 5)` breaks the rule and it is a marked end |
| `download.model.ts` | `Download` | 4 | `whole` holds | stuck | to the stuck state: `start`, `fail` | `(phase, saved)`: `(Idle, false)`, `(Loading, false)`, `(Done, true)`, `(Failed, false)`. `Done` is a marked end; `Failed` has no action and is not. 4 |
| `grid.model.ts` | `Grid` | 27000 | `inside` holds | none | none | `x`, `y` and `z` each go 0 to 29 on their own. 30 × 30 × 30 = 27000. The far corner is a marked end |
| `heater.model.ts` | `Heater` | 8 | `capped` holds | none | none | `(temp, heating)`: heating, `temp` climbs 0, 1, 2, 3 (4 states); `cut` at 2 or 3 stops the heating, then `cool` walks 3, 2, 1, 0 (4 states); `resume` at 0 returns to the start. 4 + 4 = 8 |
| `lamp.model.ts` | `Lamp` | 6 | `dim` broken | none | `brighten`, `brighten`, `toggle` from the start with `power = true` | `power` has no initial value, so 2 start states. `power` × `level` 0 to 2, all reachable. 2 × 3 = 6. Only `(false, 2)` breaks the rule; from the start with the lamp off it takes 4 steps |
| `mailbox.model.ts` | `Mailbox` | 7 | `bills` broken, `fronted` holds | none | `deliver[letter = "Bill"]`, `deliver[letter = "Bill"]` | A queue of capacity 2 over 2 letters: 1 + 2 + 4 = 7. Only `["Bill", "Bill"]` breaks the rule |
| `meter.model.ts` | `Meter` | 3 | `floor` holds | fault in `refill` | to the fault: `drain`, `drain` | `level` goes 1, 0, -1. At -1 the `when` of `refill` takes `%` of a negative number. -1 is a marked end. 3 |
| `oven.model.ts` | `Oven` | 4 | `safe` broken, `closed` broken | none | `safe`: `start`, `open`. `closed`: `open` | `door` × `heat`, all four reachable. 2 × 2 = 4. The heat only starts with the door closed, so `(Open, On)` needs `start` first |
| `party.model.ts` | `Party` | 4 | `algebra`, `members`, `counted` hold; `alone` broken | none | `arrive[guest = "Ann"]`, `arrive[guest = "Bob"]` | Subsets of two guests: `{}`, `{Ann}`, `{Ann, Bob}`, and `{Bob}` once Ann leaves. 4. Bob only arrives while Ann is inside, so the path is the only shortest one |
| `printer.model.ts` | `Printer` | 5 | `guarded` holds, `owned` holds | none | none | `(tray, job)`: both free (1); the job held by Ada or Bo with the tray free (2); the same with the tray held (2). 1 + 2 + 2 = 5 |
| `stock.model.ts` | `Stock` | 6 | `counted` holds, `known` holds, `single` broken | none | `stock[item = "Pen"]`, `restock[item = "Pen"]` | Pen is absent, 1 or 2; Ink is absent or 1 (only a pen is restocked). 3 × 2 = 6 |
| `switches.model.ts` | `Porch` | 2 | `porchKnown` holds | none | none | `lit` is false or true. 2 |
| `switches.model.ts` | `Hall` | 3 | `hallKnown` holds | none | none | `level` cycles 0, 1, 2. 3 |
| `tally.model.ts` | `Tally` | 5 | `bounded` holds | out of range | to the state out of range: `add`, `add`, `add`, `add` | `count` goes 0, 1, 2, 3 inside its range, then `add` gives 4, outside `m.int(0, 3)`. 4 + 1 = 5 |
| `ticket.model.ts` | `Ticket` | 12 | `seated` holds | none | none | `seat.row` 1 or 2 × `seat.aisle` × `owner` (none, Kim, Lee). 2 × 2 × 3 = 12 |
| `trail.model.ts` | `Trail` | 7 | `ends` holds, `kept` holds, `ordered` broken | none | `visit[stop = "South"]`, `visit[stop = "North"]` | A sequence of at most 2 over 2 stops: 1 + 2 + 4 = 7. Only `["South", "North"]` breaks the rule |
| `turnstile.model.ts` | `Turnstile` | 4 | `locked` broken | none | `push` | `gate` × `paid`, all four reachable. 2 × 2 = 4. `push` does not ask for payment, so `(Open, false)` is one step away |
| `upload.model.ts` | `Upload` | 16 | `quiet`, `waiting`, `counted` hold | none | none | By tries used. 0: idle, pending, succeeded, failed (4). 1 and 2: timer running, timer fired, idle, pending, succeeded, failed (6 each). `abort` returns to the start. 4 + 6 + 6 = 16. Succeeded, and failed with no tries left, are marked ends |

Totals for a run over these files: 21 models, 8 pass, 13 fail.

What each file is there for:

| File | Covers |
|---|---|
| `counter.model.ts` | A passing model; `*` and `%`; a marked end; shares the rule id `bounded` with `tally.model.ts` |
| `turnstile.model.ts` | One broken rule; a step that changes one field of two |
| `oven.model.ts` | Two broken rules, a path for each |
| `download.model.ts` | A stuck state and a marked end |
| `tally.model.ts` | Out of range |
| `basket.model.ts` | A fault from a field read on `null`; optional; record; a record value in a path |
| `meter.model.ts` | A fault from `%` on a negative number; a negative value in a path |
| `lamp.model.ts`, `cart.model.ts` | Several start states (a field with no `.initial`) |
| `mailbox.model.ts`, `party.model.ts`, `stock.model.ts`, `trail.model.ts`, `printer.model.ts`, `ticket.model.ts` | Inputs |
| `heater.model.ts` | Helper functions (one expression, and a block of `const` and `if (...) return`) and top-level constants |
| `party.model.ts` | Set, with every set operation; `m.forall` and `m.exists` over a declaration and over a collection; `m.setOf` |
| `stock.model.ts` | Map, with every map operation |
| `trail.model.ts` | Sequence, with every sequence operation |
| `mailbox.model.ts` | Queue, with every queue operation |
| `printer.model.ts` | Lock without an owner and lock with an owner |
| `upload.model.ts` | Timer, request and retry, with every operation of each |
| `ticket.model.ts` | Record with spread; optional; `m.oneOf` |
| `grid.model.ts` | A model large enough for `--max-states` and for several workers |
| `cart.model.ts` | A `view` with `title`, `state`, `group`, `step` and `colors` |
| `dial.model.ts` | A `view` on a model with 60 arrows, more than a chart may hold |
| `switches.model.ts` | Two models in one file, in an order that is not alphabetical |

## Rust twins

Each twin is the model of the same name written to the Rust API, with the same field names, ids
and sentences, and gives the same states, verdicts and paths as its TypeScript original.

| File | Model | States | Rules | Other findings | Shortest path |
|---|---|---|---|---|---|
| `counter.rs` | `Counter` | 4 | `bounded` holds, `even` holds | none | none |
| `download.rs` | `Download` | 4 | `whole` holds | stuck | `start`, `fail` |
| `mailbox.rs` | `Mailbox` | 7 | `bills` broken, `fronted` holds | none | `deliver[letter = "Bill"]`, `deliver[letter = "Bill"]` |
| `oven.rs` | `Oven` | 4 | `safe` broken, `closed` broken | none | `safe`: `start`, `open`. `closed`: `open` |
| `turnstile.rs` | `Turnstile` | 4 | `locked` broken | none | `push` |

## Limits

| File | Model | States | Rules | How it is run |
|---|---|---|---|---|
| `grid.model.ts` | `Grid` | 27000 in full | `inside` holds | With `--max-states 1000` it is `unfinished` with `1000 states so far` and `inside` is `unknown` |
| `limits/slow.model.ts` | `Slow` | 1000 in full (three wheels of 10 notches: 10 × 10 × 10) | `heavy` holds | Only with `--time-limit 1`. Its rule runs 100 × 100 × 100 = 1000000 comparisons in each state, a thousand million in all, so the model takes well over a second and is `unfinished` with fewer than 1000 states; `heavy` is `unknown`. It sits in its own folder because a run without a limit takes minutes |

## Rule changes

`changes/before/tank.model.ts` is committed, then replaced in the working tree by
`changes/after/tank.model.ts`, and the run is given `--base HEAD`. `level` goes 0, 1, 2, 3 in
both, and 3 is a marked end.

| File | Model | States | Rules | Shortest path |
|---|---|---|---|---|
| `changes/before/tank.model.ts` | `Tank` | 4 | `capped` broken; `floor`, `steady`, `named`, `gone`, `kept` hold | `capped`: `fill`, `fill`, `fill` |
| `changes/after/tank.model.ts` | `Tank` | 4 | `floor` broken; `capped`, `steady`, `named`, `kept`, `fresh` hold | `floor`: no steps, the start state breaks it |

| Rule | Was | Now | Change | Why |
|---|---|---|---|---|
| `capped` | "The level is at most two", `s.level <= 2` | "The level is at most three", `s.level <= 3` | `loosened` | Level 3 passes the new rule and breaks the old one |
| `floor` | "The level is at least zero", `s.level >= 0` | "The level is at least one", `s.level >= 1` | `tightened` | Level 0 breaks the new rule and passes the old one; no level does the reverse |
| `steady` | "The level stays within the tank", `s.level <= 3` | the same sentence, `s.level < 4` | `changed` | The condition differs and no level from 0 to 3 tells the two apart |
| `named` | "The level is small", `s.level < 4` | "The level is tiny", `s.level < 4` | `reworded` | Only the sentence differs |
| `gone` | "The level is never five", `s.level !== 5` | | `removed` | |
| `fresh` | | "The level is never seven", `s.level !== 7` | `added` | |
| `kept` | "The level is never six", `s.level !== 6` | the same | not listed | Nothing differs |

## Unreadable files

Each file holds one whole model and one construct from outside the library. No model in it is
checked.

| File | Model that is not checked | Line | Feature the report names |
|---|---|---|---|
| `unreadable/network.model.ts` | `Quote` | 13 | `fetch` |
| `unreadable/clock.model.ts` | `Stamp` | 8 | `Date.now` |
| `unreadable/dice.model.ts` | `Dice` | 10 | `Math.random` |
| `unreadable/loop.model.ts` | `Pile` | 13 | a `for` loop (the feature has `for` in it) |
| `unreadable/variable.model.ts` | `Score` | 11 | `let` (the feature has `let` in it) |
| `unreadable/undefined.model.ts` | `Bill` | 9 | the undefined name `surcharge` (the feature has `surcharge` in it) |
