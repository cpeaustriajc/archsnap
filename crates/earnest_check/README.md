# earnest_check

The search over every reachable state of a checked model.

`explore` starts from every start state and visits the states level by level, so the path it
gives to a broken rule is a shortest one. In each state it checks that every field is in range,
evaluates every rule, and tries every action with every combination of its inputs. The `Outcome`
holds

- the status: pass, fail or unfinished,
- the number of distinct states,
- a verdict for each rule, with the path to the first state that breaks it,
- findings for a stuck state, a value out of range and a fault, each with its path, and
- a `Graph` of the states and actions for a chart, when the limits ask for one.

`Limits` bounds the states and the time, picks one rule, and sets the worker threads. A level of
512 states or more is split among the workers, and the outcome is the same for any number of
them.

The split of a level into one slice per worker is `cut_points` in `src/cut.rs`. Verus proves
that the cut points start at 0, end at the level's size, strictly increase, and are never further
apart than the slice size. `cargo verus focus` checks the proof.
