# earnest_tla

The TLA+ writer: a checked model as a TLA+ module and the configuration TLC checks it with.

`write` takes a `Model` and gives `Files`: the module's name and the text of `<name>.tla` and
`<name>.cfg`. The export exists so that TLC, the reference TLA+ checker, can confirm the search
in `earnest_check`: the same number of distinct states and the same verdict for each rule.

## What the module holds

```
---- MODULE Counter ----
EXTENDS Integers, Sequences, FiniteSets, TLC

VARIABLES count

vars == <<count>>

InRange ==
    /\ (count \in 0..3)

Init ==
    /\ count = 0

Action_inc ==
    IF InRange /\ (count < 3)
    THEN /\ count' = (count + 1)
    ELSE FALSE

End_full ==
    (count = 3)

Stop ==
    IF (~InRange) \/ End_full
    THEN UNCHANGED vars
    ELSE FALSE

Next ==
    \/ Action_inc
    \/ Stop

Rule_bounded ==
    InRange => (count <= 3)

====
```

```
INIT Init
NEXT Next
CHECK_DEADLOCK FALSE
INVARIANT Rule_bounded
```

- **Start states and steps.** `Init` is every combination of the fields' start values. `Next` has
  one disjunct per action: for each combination of its input values, the guard, then every
  variable primed.
- **Counting.** `InRange` says each variable holds a value its field declares. Every action asks
  for it, so a state out of range is reached and counted and has no step of its own, as in the
  search. A rule is the invariant `InRange => rule`, because the search judges no rule in a state
  out of range.
- **Stuck and end.** `Stop` lets a state out of range or a marked end repeat itself, which adds
  no state. With it, the states TLC would call a deadlock are the ones the search calls stuck,
  apart from a state where a guard or an update faults. The configuration still turns TLC's
  deadlock check off, because TLC stops at the first deadlock and would count fewer states for
  a model that is stuck.
- **Guards as one condition.** A guard sits in the condition of an `IF`, not as a conjunct of the
  action. TLC tries both sides of an `\/` that is part of an action, and the `IF` keeps it to the
  left-to-right evaluation the search uses, where the second side is not read when the first
  decides.
- **Optional values.** An optional is `<<>>` when absent and `<<value>>` when present, so TLC
  never compares values of two kinds. `SomeOf` wraps and `Unwrap` reads `[1]`.
- **Collections.** A set is a set, a sequence or queue a tuple, a record a record and a map a
  function over its keys. Where the evaluator has an answer and the TLA+ operator has none, a
  helper defined in the module gives the evaluator's answer: `SeqFirst`, `SeqLast` and `SeqGet`
  give `<<>>` where there is no item, `SeqShift` and `SeqPop` keep an empty sequence, and `MapGet`
  gives `<<>>` for a missing key. Only the helpers a model uses are written.
- **Faults.** The search takes no step with an action whose guard or update has no value. The
  action's `IF` therefore first asks that every optional it unwraps is present, every remainder
  has a non-negative left side and a right side of at least 1, and every number that must fit
  an integer type is inside it. A rule is guarded the same way: `Rule_id` is
  `InRange => ((~HasAValue) \/ rule)`, true where the rule has no value, so TLC does not stop
  and counts every state. `Judged_id` is `InRange => HasAValue`: a state where the rule has no
  value breaks it. Only a rule that can fault has one. The configuration names `Rule_id` alone,
  one invariant per rule; `Files::config_judging` gives the configuration that checks both.
- **Quantifiers.** The search stops a `forall` or an `exists` at the first item that decides it,
  so a fault in a later item never happens. The module asks the same: an item must have a value
  unless an item before it in the search's order decides, and a quantifier over a sequence, a set
  of numbers or a set of booleans is written that way, with `Ok` and `Hit` defined by `LET`. The
  items of a declaration come in the order of the declaration (a sequence), the items of a set
  ascending. A set of anything else whose test can fault still asks for every item.
- **Names.** A name keeps its spelling when TLA+ can spell it and nothing else has it. A letter
  TLA+ lacks becomes `_`, a name that does not start with a letter gets a `v` in front, and a
  taken name gets `_2`, `_3` and so on. Text values are TLA+ strings and are never renamed.

## Running TLC on the files

What can make TLC and the search differ, and what to do about each:

- **TLC stops at the first broken rule.** Run TLC with `-continue`: without it TLC has counted
  fewer states than the search.
- **TLC reports one invariant per state.** In one state TLC reports only the first invariant it
  finds broken, so a rule that breaks only where an earlier rule breaks needs a run of its own.
  `Files::config_checking` gives the configuration that checks only the invariants it is given:
  none for a run that only counts the states.
- **A rule that has no value in a state.** With the written configuration TLC calls it true
  there, where the search says `unknown` and reports a fault. Check `Judged_id` to see such a
  state: `Files::rules` lists both invariants of each rule and `Files::config_judging` gives the
  configuration for one rule. The rule's verdict is broken when a state breaks `Rule_id`,
  else unknown when a state breaks `Judged_id`, else it holds. A state breaks at most one of
  the two, so one run with `-continue` is enough.
- **Numbers beyond 32 bits.** TLC holds a number in 32 bits and stops with an error past them,
  where the search goes on to 64. A bound of an integer type that TLC can't reach, such as the
  top of a `u32` or either end of an `i64`, is left out of the check that a number fits its
  type.
- **A construct the writer does not know.** It is written as a call of `Fault`, which stops TLC
  with an error that says what it was.
- **A view label.** Labels are not exported, so nothing in the module depends on them.

## Tests

`tests/write` holds the expected TLA+ for each operation, each expression, each kind of
declaration, each way a guard can fault and five whole modules, all written by hand, and
property tests that any generated model exports as text with balanced delimiters, declared
variables, and names that are in scope, hide no other name and are not words of TLA+. Nothing
here runs TLC: `earnest confirm` does.
