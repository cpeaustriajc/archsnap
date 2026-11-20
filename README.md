# earnest

A model checker for models written in your app's own language. A coding agent states the rules of
a change as a model in TypeScript or Rust, `earnest verify` visits every reachable state, and the
result goes into the PR as a statechart and a report.

Status: version 1 is built and passes its own checks, and is not yet proven. Three adversarial
passes each found models with a broken rule that were reported as passing; those are fixed
except what "Where Node and the checker differ" in `npm/model/README.md` says, and a fourth pass
has not been run. TLC has not confirmed the TLA+ export in CI. The spec is the
Claude Doc "A formal verification suite for agents".

## Commands

```sh
earnest verify                       # every model in the repository
earnest verify src/cart.model.ts     # one file
earnest verify --model Cart --rule focusLands
earnest verify --json
earnest verify --base origin/main    # also list rule changes since that commit
earnest chart src/cart.model.ts --model Cart
earnest report --base origin/main    # Markdown for a PR body
earnest tla src/cart.model.ts --out build/tla
earnest confirm --tlc tla2tools.jar
```

| Flag | Default | Meaning |
|---|---|---|
| `PATH...` | the current directory | Files or folders to search for models |
| `--model <name>` | all | Check only the model with this name |
| `--rule <id>` | all | Check only this rule (needs `--model` when two models share the id) |
| `--json` | off | Print the results as JSON instead of text |
| `--max-states <n>` | 5000000 | Stop a model after this many distinct states. Every state that is counted has its rules judged |
| `--time-limit <seconds>` | none | Stop a model after this long |
| `--jobs <n>` | all cores | Worker threads |
| `--no-cache` | off | Check every model again, even when unchanged |
| `--base <git ref>` | none | Compare each model's rules with the same file at this commit |

`earnest verify` runs in one process, needs no network, no Java and no setup, and writes only to
`.earnest/` in the folder it is run from.

### Which files are models

A file is a model file when it is a `.ts`, `.mts` or `.tsx` file that imports `@earnest/model`, or
a `.rs` file that names the `earnest` crate. The file is judged by what its imports mean and not
by how they are written. In TypeScript the specifier is what JavaScript makes of it, so
`"@earnest\x2fmodel"`, a Unicode escape or a line continuation in it is the library, and so is
an `export ... from`, a dynamic `import()` or a `require`. In Rust the file is read as tokens, so
a path that starts with `earnest` counts however it is spaced, grouped or broken by a comment:
`use earnest::model::Model`, `use earnest::/* c */model::Model`, `use earnest as kit` and
`extern crate earnest`. A word in a comment or a text is not a use. A file that mentions
`earnest`, and that can't be parsed or reaches for a module in a way the checker can't follow,
is not left out: it is a model file, and the reader refuses what it can't read. Folders named
`node_modules`, `target`, `dist`, `build` and any folder whose name starts with a dot are
skipped. Models are checked and printed in order of file path, then of position in the file.

A file is either read with the meaning its language gives it, or refused as unreadable with a
sentence saying why, and it is never left out without a word. A file with one of those
extensions is `unreadable`, with a problem that names it, when it reaches the library and is not
UTF-8 text (the problem is at the first byte that is not, or at the top of a file that is UTF-16
text; a file that starts with a UTF-16 byte order mark and holds ASCII text after it is not
UTF-16 text, so its problem is its first byte), and when it can't be opened at all, as a file
without read permission or a link to nothing: the checker can't tell that it holds no model. So
is a folder that can't be listed. A folder whose name ends like a model file is searched as a
folder, and a link to a folder is not followed. A file with a NUL byte between its letters is
found and refused by the reader. `earnest report` names every unreadable file, with its
problems, in a section `Unreadable files` of the Markdown, and `earnest verify` lists it as
`unreadable`: no unreadable file is left out of either.

### Text output

One line per model, then indented detail lines for a model that did not pass, then a summary line.
Fields on a line are separated by two or more spaces. Column widths are not part of the contract.

```
pass        Counter  4 states  1 ms
fail        Turnstile  3 states  1 ms
  broken  staysLocked  "The gate stays locked until the visitor pays"
    start  gate = "locked", paid = false
    1. push  gate: "locked" -> "open"
  stuck  no action can happen here, and the model does not mark it as an end
    start  gate = "locked", paid = false
    1. pay  paid: false -> true
unfinished  Ledger  1000 states so far  3 ms  (state limit of 1000)
unreadable  src/clock.model.ts
  src/clock.model.ts:12:17  `Date.now`  the system clock is outside the library
4 models: 1 pass, 1 fail, 1 unfinished, 1 unreadable
```

- The status word is `pass`, `fail`, `unfinished` or `unreadable`.
- A model line has the status, the model name, `<n> states` (`<n> states so far` when unfinished)
  and `<n> ms`. An unchanged model served from the cache ends with `(cached)`. A cached result
  is served only by a build of the checker made from the same sources as the build that stored
  it.
- `broken` names the rule's id and its sentence, and is followed by the shortest path to the first
  state that breaks it: a `start` line with every field, then one numbered line per action with
  the action's id, its input as `name = value` when it has one, and each field that changed as
  `field: before -> after`.
- `stuck`, `out of range` and `fault` findings have the same path form. A model is out of range
  when an action gives a field a value outside what the field declares. The state out of range
  is counted, and nothing is searched from it.
- Anything the model asks that can't be evaluated in a state is a `fault` in that state: a rule,
  the `when` or the `then` of an action, or the `when` of an end, for example because it reads a
  field of `null`. A state is `stuck` only when the `when` of every action is false and every
  end is false. A state where a `when` or an end can't be evaluated is a fault and is not stuck.
  An end is asked only in a state where no action can happen. `explore` in the TypeScript
  library follows the same rule.
- A model that stops at `--max-states` or `--time-limit` is `unfinished` with `<n> states so
  far`. Each of those states has had its rules judged, also the states the search found last
  and did not go on from, so a rule a counted state breaks is `broken` with its path. No action
  is tried from the states it did not go on from, so they add no stuck state and no fault of an
  action.
- An `unreadable` file lists each problem as `file:line:column`, the feature in backticks and a
  sentence. No model in that file is checked.
- The summary line counts each model and each unreadable file once in its total, then lists
  `pass`, `fail` and `unfinished`, and adds `<n> unreadable` only when there are unreadable
  files. In JSON, `summary.models` counts models only and `summary.unreadable` counts files.
- Values print as `"text"`, `12`, `true`, `null`, `{a, b}` for a set, `[a, b]` for a sequence or
  queue, `{k -> v}` for a map and `{name: value}` for a record. Inside the quotes of a text, `"`
  prints as `\"`, `\` as `\\`, a line feed as `\n`, a carriage return as `\r`, a tab as `\t` and
  any other control character or line separator as `\u{...}` with its code in hexadecimal, so a
  value is always on one line and reads only one way.

### Exit codes

| Code | Meaning |
|---|---|
| 0 | Every model passed |
| 1 | A rule broke, a model was stuck, out of range or faulted, a model was unfinished, or a file was unreadable |
| 2 | Bad flags, or no model matched `--model` or `--rule` |

### JSON output

```json
{
  "models": [
    {
      "name": "Turnstile",
      "file": "src/turnstile.model.ts",
      "line": 5,
      "status": "fail",
      "states": 3,
      "milliseconds": 1,
      "cached": false,
      "reason": null,
      "rules": [
        {
          "id": "staysLocked",
          "says": "The gate stays locked until the visitor pays",
          "verdict": "broken",
          "path": {
            "start": [{ "name": "gate", "value": "locked" }, { "name": "paid", "value": false }],
            "steps": [
              {
                "action": "push",
                "label": "the visitor pushes",
                "input": [],
                "changes": [{ "field": "gate", "before": "locked", "after": "open" }],
                "state": [{ "name": "gate", "value": "open" }, { "name": "paid", "value": false }]
              }
            ]
          }
        }
      ],
      "findings": [{ "kind": "stuck", "message": "...", "path": { "start": [], "steps": [] } }]
    }
  ],
  "unreadable": [
    { "file": "src/clock.model.ts", "line": 12, "column": 17, "feature": "`Date.now`", "message": "..." }
  ],
  "ruleChanges": [],
  "summary": { "models": 1, "pass": 0, "fail": 1, "unfinished": 0, "unreadable": 1 }
}
```

`status` is `pass`, `fail` or `unfinished`. `verdict` is `holds`, `broken` or `unknown` (the rule
was not seen broken, but the model was unfinished or the rule faulted in a state). `kind` is `stuck`, `out-of-range` or `fault`.
In JSON a set and a sequence are arrays, a map is an array of `[key, value]` pairs and a record is
an object. Two runs on the same files give the same JSON apart from `milliseconds` and `cached`.

### Rule changes

With `--base <ref>`, each model file is also read as it was at that commit. The text output gains
a section before the summary line, and the JSON gains entries in `ruleChanges`:

```
rule changes since origin/main
  added      Cart.totalNeverNegative  "The total is never below zero"
  removed    Cart.focusLands  "After a keyboard add lands, focus is on In cart"
  loosened   Cart.oneAddAtATime
    was  "At most one add is in flight"  s.inFlight <= 1
    now  "At most two adds are in flight"  s.inFlight <= 2
```

```json
{ "model": "Cart", "id": "oneAddAtATime", "change": "loosened",
  "was": { "says": "...", "holds": "s.inFlight <= 1" },
  "now": { "says": "...", "holds": "s.inFlight <= 2" } }
```

`change` is `added`, `removed`, `loosened`, `tightened`, `changed` or `reworded`. A rule is
`removed` when the base has it and the run does not: deleted from a model that stays, gone with
its whole model, or gone with its file. The models of a file that was deleted, renamed or no
longer imports the library are found in the base commit under the paths the run searches, and
their rules are listed as `removed` after the changes of the files that are still there. A
file that has become unreadable since the base lists the rules of its base models as `removed`
too, because the run no longer has them, and the file is shown as `unreadable` beside them. A
file that is unreadable at the base as well has no rules to compare, and a warning on stderr
says so. A renamed file or a renamed model therefore shows each rule twice, as `removed` under
the old name and `added` under the new one. With `--model`, only removed models of that name
are listed, also when no model of that name is left: `--base <ref> --model Name` for a model
that was deleted lists its rules as `removed` and exits as a run with no model does, and only a
name that neither the run nor the base has is exit 2. A rule is
`loosened` when some reachable state of the new model passes the new rule and breaks the old one,
`tightened` when the reverse happens and no state loosens it, `changed` when its condition differs
but no reachable state tells the two apart or the old condition can't be read against the new
model, and `reworded` when only its sentence differs. A rule whose sentence and condition are
unchanged is not listed.

Every rule the two commits share is compared, not only those whose text was edited. The old
condition is read with the constants and helper functions of the file at the base commit, so a
rule is `loosened` when only a constant or a helper it uses changed. A condition differs when its
text does, or when it means something else through such a constant or helper. The text of a
condition is compared and printed with each run of white space as one space, in TypeScript and
in Rust, so moving white space is not a change.

### Statechart and report

`earnest chart` prints one fenced `mermaid` block holding a `stateDiagram-v2` chart. States and
arrows carry the labels from the model's view. A state that breaks a rule has the class `broken`.
A chart never has more than 20 arrows: when the model has more, the chart keeps the paths to the
broken states first and says how many states and arrows it left out. The states are counted
for what they are: a box of the chart stands for every state with its label, so the note counts
the states of the boxes it left out, and an arrow is one box to another however many actions it
carries. When the shortest path to a broken state is longer than what is left of the 20 arrows,
the chart keeps the first arrows of that path, does not draw the broken state, and the note
says so: `Not shown: 5 of 26 states and 5 of 25 arrows. A broken state lies beyond what is
shown.`

`earnest report` prints Markdown for a PR body: one section per model with its result line, its
chart, the path to each broken rule, a section `Unreadable files` that names each file the
checker could not read and why, and the rule changes when `--base` is given. A file is never
left out of the report without a word: a report of a run with an unreadable file says so.

### TLA+ export

`earnest tla` writes `<Model>.tla` and `<Model>.cfg` for each model. TLC finds the same number of
distinct states and the same verdict for each rule. `earnest confirm --tlc <jar>` exports every
model, runs TLC on each with Java, and exits 1 when a state count or a verdict differs.

A rule `id` is the invariant `Rule_id`, and the configuration names one invariant for each rule.
A rule that can't be evaluated in a state is true there in `Rule_id`, as an action that can't be
evaluated takes no step, so TLC counts every state and never stops on such a rule. The module
also defines `Judged_id` for a rule that can fault: a state where the rule has no value breaks
it. `earnest confirm` checks both for each rule and compares TLC's verdict, `broken` when a state
breaks `Rule_id`, else `unknown` when a state breaks `Judged_id`, else `holds`, with the
checker's.

## Writing a model in TypeScript

```ts
import { m } from "@earnest/model";

const Press = m.enum(["keyboard", "virtual", "mouse"]);

export const AddToCartFocus = m.model("AddToCartFocus", {
	surface: m.enum(["results", "shortlist"]),
	cta: m.enum(["Add", "Adding", "InCart"]).initial("Add"),
	focus: m.enum(["Add", "InCart", "Heart", "Away", "Lost"]).initial("Add"),
	pressBy: Press.optional().initial(null),
	keyboardAdd: m.boolean().initial(false),
});

export const press = AddToCartFocus.action("the buyer presses Add", {
	input: { by: Press },
	when: (s, { by }) => s.cta === "Add" && (by === "mouse" || s.focus === "Add"),
	then: (_, { by }) => ({ cta: "Adding", pressBy: by, keyboardAdd: by === "keyboard" }),
});

export const focusLandsOnInCart = AddToCartFocus.rule({
	says: "After a keyboard add lands, focus is on In cart",
	holds: (s) => s.pressBy !== "keyboard" || s.cta !== "InCart" || s.focus === "InCart",
});

export const inCart = AddToCartFocus.end({
	says: "The item is in the cart",
	when: (s) => s.cta === "InCart",
});

export const view = AddToCartFocus.view({
	title: "Add to cart focus",
	state: (s) => `${s.cta}, focus on ${s.focus}`,
	group: (s) => s.surface,
	step: { press: "press Add" },
	colors: { broken: "#c62828" },
});

export type State = m.Infer<typeof AddToCartFocus>;
```

- A field with `.initial(value)` starts at that value. A boolean, number, enum, optional or record
  field without it starts at every value it declares, so `surface` above gives two start states.
  A set, map, sequence, queue, lock, timer, request or retry field without it starts empty, free,
  idle or at zero.
- An action has a sentence, an optional `input`, an optional `when` and a `then`. The checker
  tries every value of every input. `then` gives back only the fields that change.
- The id of an action, rule or end is the name of its exported constant.
- `end` marks states where stopping is correct. Any other state with no action is stuck.
- `view` is optional. `step` maps an action id to its arrow label; the default is the action's
  sentence. `colors` accepts `broken`, `start` and `end`, each with `#` and 3, 4, 6 or 8
  hexadecimal digits or a color name of letters; any other part, or any other color, makes the
  file unreadable, because the chart would leave it out without a word.

### What a model file may contain

- Imports from `@earnest/model`, and `import type` from anywhere.
- Top-level `const` declarations of declarations (`m.enum(...)`), numbers, text, booleans and
  helper functions written as arrow functions. A number, text or boolean is written out, or
  computed from constants declared before it with the operators below.
- A model, an action, a rule, an end or a view, each made by calling `m.model` or the model
  itself as the whole value of a top-level `const`: `export const low = Cart.rule({ ... })`.
  The model's name is refused anywhere else, so a rule can't be made through another name, in
  a condition, in a record, in a list or inside a function. A constant is exported under its
  own name.
- `type` and `interface` declarations.
- In a function: literals, `null`, field and input reads, `===`, `!==`, `<`, `<=`, `>`, `>=`,
  `&&`, `||`, `!`, `+`, `-` (also as a sign: `-1`), `*`, `m.rem(a, b)`, the `? :` choice, the `!` after a
  value that TypeScript needs to read a field of an optional (`s.chosen!.qty`), object literals with spread, calls to the
  primitives below and to helper functions, and template strings in `view` labels. A function
  body is one expression, or a block of `const` declarations and `if (...) return ...;`
  statements that ends in a `return`. Nothing follows that `return`.
- A name means what TypeScript says it means. In an action, an input is in scope only in a
  function that takes it, as `(s, { by })` or as a field of the whole input, `(s, input) =>
  input.by`; `when` and `then` may take different ones. Where a function does not take it, the
  same name is the top-level constant. A `const` in a block is in scope in the whole block, so
  a name that is read before its declaration, or in its own initializer (`const a = a + 1;`),
  is refused: in JavaScript it is the block's own constant with no value yet, and the file
  throws, never the top-level constant of the same name.
- A number is a JavaScript number, so it is exact from -9007199254740991 to 9007199254740991.
  A literal outside that is refused, so are the bounds of an `m.int` and the numbers of an
  `m.oneOf` outside it (the library throws on them too), and a number a model computes past it
  is a fault, "computes a number too large to hold", in a rule, an end, a `when`, a `then` and
  a helper. A sum or a product past it would be rounded when the file runs, so it is no value
  to judge a rule by.
- An expression is read to a depth of 1000, counted as the brackets, prefix operators,
  operands of a chain and statements it nests. A file with a deeper one is unreadable, with the
  place where it passes the limit, and the other models of the run are still checked: the
  checker never stops with the file that is too deep.

- A record literal is a value of a declared record when each field is: a field declared
  optional takes a value or `null`. So `g === "ann" ? { qty: 0, who: g } : { qty: 1, who: null }`
  is one kind of record, and so is a record bound to a `const` and spread into the field that
  declares it, or given back by a helper function, or written as the value of an entry of
  `m.mapOf`, an item of `m.setOf` or `m.seqOf`, or a field of another record. The entries of
  `m.mapOf` join into one record type, as the items of `m.setOf` do. It prints, compares and
  exports to TLA+ as the declared record does.

Anything else makes the file unreadable: `await`, loops, `let`, assignment, `new`, classes,
`fetch`, `Date`, `Math.random`, other imports, a name written twice in one object, a value
listed twice in `m.enum` or `m.oneOf` (the library throws on it too), and any name the file does
not define, also in a constant that nothing reads. The operator `%` and `%=` are refused: write `m.rem(a, b)`, which is
the remainder, and a fault for a negative `a` or a `b` below 1, because languages disagree on
the result. The library's `m.rem` throws the same fault in Node, so `explore` and `replay` report
it as `earnest verify` does. Rust's `%` keeps its meaning, a fault on a negative number.

## Writing a model in Rust

```rust
use earnest::model::{Domain, Model, model, require};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
pub enum Gate {
    Locked,
    Open,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Turnstile {
    #[initial(Gate::Locked)]
    gate: Gate,
    #[initial(false)]
    paid: bool,
    #[range(0, 3)]
    #[initial(0)]
    visitors: u8,
}

#[model]
impl Turnstile {
    #[action("the visitor pays")]
    fn pay(&self) -> Option<Self> {
        require!(!self.paid);
        Some(Self { paid: true, ..self.clone() })
    }

    #[action("the visitor pushes")]
    fn push(&self) -> Option<Self> {
        require!(self.paid && self.visitors < 3);
        Some(Self { gate: Gate::Open, visitors: self.visitors + 1, ..self.clone() })
    }

    #[rule("The gate stays locked until the visitor pays")]
    fn stays_locked(&self) -> bool {
        self.gate == Gate::Locked || self.paid
    }

    #[end("Three visitors went through")]
    fn full(&self) -> bool {
        self.visitors == 3
    }
}
```

- The model is a struct that derives `Model`. A field's declaration is its Rust type: `bool`, an
  integer with `#[range(lo, hi)]`, an enum or struct that derives `Domain`, `Option<T>`, and the
  library's `Set<T>`, `Map<K, V>`, `Seq<T, MAX>`, `Queue<T, CAPACITY>`, `Lock`, `Lock<Owner>`,
  `Timer`, `Request` and `Retry<MAX>`. `#[initial(value)]` works as in TypeScript.
- An action is a method in the `#[model]` block that takes `&self` and its inputs and returns
  `Option<Self>`. Each `require!(condition);` is part of its `when`. The last expression is
  `Some(Self { changed fields, ..self.clone() })`. Before it, `let` bindings and
  `if condition { return Some(Self { .. }); }` are allowed.
- A rule or an end is a method that returns `bool`. Ids are the method names. An enum value is
  known to the checker by its variant name, so `Gate::Locked` prints as `"Locked"`.
- A method without `#[action]`, `#[rule]` or `#[end]` is a helper function, called as
  `self.name(..)`. A function outside the block is one too, called as `name(..)`. The two are
  kept apart, so a method and a function may share a name, and so may methods of two models.
  A helper's parameters need no `#[range]`, which Rust would refuse there: a helper is only
  called with values. The checker reads `forall`, `exists`, `Some`, `None`, `Set`, `Seq` and
  `Map` by the name they are written with, so nothing of the file can have such a name: not a
  function, a constant, a struct or an enum, and no value of a body can have the name of a
  function or of `forall` or `exists` that the body calls. An item of the library is imported
  by its own name: `use earnest::model::forall as exists;` is refused, as is any other `as`
  except `as _`. A `use` inside a body, a `macro_rules!` and a `type` alias are refused too, as a
  model file holds no such item. The three attributes are written plainly, never with a path. No part of a model is under `#[cfg]` or
  `#[cfg_attr]`: the checker reads one text for every build. Only an item that is not part of
  a model (a test module, a function, a plain `impl` block) may be `#[cfg(test)]`, and it is
  skipped.
- A model is ordinary Rust: a test calls `state.pay()` to take one step, which is how a path from
  the report is replayed with `cargo test`. `Turnstile::starts()` lists every start state, and
  a rule or an end is called the same way: `state.stays_locked()`.

Where the checker's reading of a Rust file could differ from what Rust does when the file runs,
the checker agrees with Rust or refuses the file. It never reads the file as something else.

- **A number has its integer type.** A result outside the type is a fault in that state, printed
  as "the rule `below` computes a number outside `u8` in this state": `self.n - 1` on a `u8` of
  0, `self.items.size() - 1` on an empty list (a size and a position are `usize`), and a sum or
  a product above the largest number of any type, signed ones included. Rust stops a test build
  there and wraps around in a release build, so neither is a value the rule can be judged by.
  The type comes from the field, the input, the constant, the helper's parameter or result, a
  `let` that states it, or a suffix as in `1_u8`; a number written out takes the type of the
  number it meets, and `i32` when it meets none. A `let` must state the type of a number that
  nothing else types: `let most: u8 = 3;`. Two integer types don't mix, as in Rust. A
  `#[range(lo, hi)]` must lie inside its type. A `u64`, a `usize` and the 128-bit types
  reach as far as an `i64` does, and beyond that the fault is "computes a number too large to
  hold". A negated literal is one number, so `-128` is an `i8` and `-9223372036854775808` an
  `i64`, in an expression, in `#[initial]`, in `#[range]`, in a constant and in the TLA+ export.
  TypeScript has one kind of number, a JavaScript number, exact to 2^53 - 1 in either
  direction, so there `s.n - 1` at 0 is -1 and a number past 2^53 - 1 is a fault. Rust keeps
  its integer types, whose arithmetic is exact in 64 bits.
- **A `let` is as Rust reads it.** The right side of `let n = n + 1;` is the `n` before it, and
  a use of a name before a later `let` of the same name is the name outside, as in Rust.
- **`Some(None)` is not `None`.** `get`, `first`, `last` and `head` give an `Option`, so a `Map`
  value and a `Seq` or `Queue` item can't be an `Option`: the file is unreadable. Hold a struct
  with an `Option` field, or leave the entry out. `Option<Option<T>>`, `Some(None)`,
  `Lock<Option<T>>` and `first()` of a list written with `None` in it are refused the same way.
  A `Set` of options and a `Map` keyed by one are read. In TypeScript `null` is one value, so
  `m.map(K, V.optional())` is read and `get` gives `null` for a missing key and for a key that
  maps to `null`.

## The primitives

Each primitive has the same meaning in TypeScript, in Rust and in the TLA+ export. TypeScript
names are camelCase and Rust names are snake_case: `isEmpty` is `is_empty()`. In TypeScript a
name without parentheses below is a property; in Rust everything is a method call. Every
operation gives back a new value and changes nothing in place.

| Group | Declared with | Operations |
|---|---|---|
| Boolean | `m.boolean()` / `bool` | `&&`, `\|\|`, `!`, `===` |
| Number | `m.int(lo, hi)` / an integer type with `#[range(lo, hi)]` | `+`, `-`, `*`, `m.rem(a, b)` / `%`, `<`, `<=`, `>`, `>=`, `===` |
| Enum | `m.enum([...])`, `m.oneOf([...])` / `enum` | `===`, `!==` |
| Optional | `.optional()` / `Option<T>` | `=== null`, `!== null` / `is_some()`, `is_none()`, `is_some_and(f)`, `== Some(x)` |
| Record | `m.record({...})` / `struct` | field read, `{ ...r, field: v }` / `R { field: v, ..r.clone() }` |
| Set | `m.set(T)` / `Set<T>` | `has(x)`, `add(x)`, `delete(x)`, `union(s)`, `intersect(s)`, `subtract(s)`, `isSubset(s)`, `size`, `isEmpty`, `every(f)`, `some(f)`, `filter(f)` |
| Map | `m.map(K, V)` / `Map<K, V>` | `has(k)`, `get(k)` (optional), `set(k, v)`, `delete(k)`, `size`, `isEmpty`, `keys` |
| Sequence | `m.seq(T, { max })` / `Seq<T, MAX>` | `size`, `isEmpty`, `get(i)`, `first`, `last` (optional), `push(x)`, `shift()`, `pop()`, `includes(x)`, `every(f)`, `some(f)`, `filter(f)` |
| Choice | `input` / method parameters | `m.forall(T, f)`, `m.exists(T, f)` over a declaration or a collection / `forall::<T>(f)`, `exists::<T>(f)` |
| Queue | `m.queue(T, { capacity })` / `Queue<T, CAPACITY>` | `size`, `isEmpty`, `isFull`, `head` (optional), `send(x)`, `receive()`, `includes(x)`, `clear()`, `every(f)`, `some(f)` |
| Lock | `m.lock()`, `m.lock(Owner)` / `Lock`, `Lock<Owner>` | `isFree`, `isHeld`, `acquire()`, `release()`; with an owner `acquire(o)`, `isHeldBy(o)`, `holder` (optional) |
| Timer | `m.timer()` / `Timer` | `isIdle`, `isRunning`, `hasFired`, `start()`, `fire()`, `cancel()`, `reset()` |
| Async request | `m.request()` / `Request` | `isIdle`, `isPending`, `hasSucceeded`, `hasFailed`, `isSettled`, `send()`, `succeed()`, `fail()`, `reset()` |
| Retry | `m.retry({ max })` / `Retry<MAX>` | `attempts`, `canRetry`, `isExhausted`, `again()`, `reset()` |

Collection values are written with `m.setOf(a, b)`, `m.seqOf(a, b)` and `m.mapOf([k, v])` in
TypeScript, and `Set::of([a, b])`, `Seq::of([a, b])` and `Map::of([(k, v)])` in Rust. Sending to
a full queue, pushing past a sequence's `max`, or counting past a range is not an error in the
primitive: the checker reports the state as out of range, with the path to it.

A lock with an owner can't be a map value or a sequence or queue item, in either language: a
free lock is already `null`, so reading it could not be told from reading nothing. The checker
refuses the file and the TypeScript library throws. A lock without an owner can be held, and a
lock with an owner can be a field, a record field or a set item.

An optional value has no order. `<`, `<=`, `>` and `>=` between two optional values make the
file unreadable, in Rust too, where `None < Some(5)` compiles: write
`value.is_some_and(|number| number < 5)`. A number compared or added with an optional number
reads the number inside, and is a fault in a state where it is `null`. A record may be spread
from an optional record the same way: `{ ...s.chosen!, qty: 1 }`. A field may have the name of
a primitive's property, such as `isFull`: what `value.isFull` reads is decided by what `value`
is.

The TypeScript library also runs as ordinary code. `explore(Model)` searches a model in Node and
returns its distinct state count, each rule's verdict and the shortest path to each broken rule.
`replay(Model, path)` takes a path from `earnest verify --json`, applies it one action at a time
and returns the states it passes through. Node can't see one fault of the checker: a number computed past 2^53 - 1 inside a rule, an end
or a `when`. JavaScript's operators can't be intercepted from a library, so `explore` gives the
value JavaScript computes there, where `earnest verify` reports the fault. Only a next state
that holds such a number is seen. `earnest verify` is the authority on these; see
`npm/model/README.md`.

## Layout

The repository is laid out as oxc is: one crate for each thing the tool is made of, named for what
it is, with apps, npm packages and tasks beside them.

| Path | What it is |
|---|---|
| `apps/earnest` | The CLI: crate `earnest-cli`, binary `earnest`. It only wires the crates together |
| `crates/earnest_diagnostics` | A place in a file and a problem at that place |
| `crates/earnest_value` | Values, interned names and finite domains |
| `crates/earnest_ast` | The language-neutral tree both readers produce |
| `crates/earnest_ts` | The TypeScript reader: model file to tree |
| `crates/earnest_rs` | The Rust reader: model file to tree |
| `crates/earnest_ir` | The typed expression language, its evaluator and the checked model |
| `crates/earnest_semantic` | The typed lowering: tree to checked model, where each primitive gets its meaning |
| `crates/earnest_check` | The search over every reachable state |
| `crates/earnest_tla` | The TLA+ writer |
| `crates/earnest_chart` | The Mermaid chart and Markdown report writer |
| `crates/earnest`, `crates/earnest_macros` | The Rust model library and its macros |
| `npm/model` | The TypeScript model library, `@earnest/model` |
| `npm/cli` | `@earnest/cli`, which ships the binary |
| `tasks/conformance` | Model files with their expected state counts, verdicts and paths, shared by every test |
| `tasks/benchmark` | The speed targets and the models that measure them |

Data flows one way: a reader turns a file into a tree, `earnest_semantic` lowers the tree to a
checked model, `earnest_check` searches it, and a writer prints the result. A crate depends only
on crates earlier in that line. Each crate has a `README.md` saying what it is, takes its lints
and package fields from the workspace, keeps unit tests beside the code and tests of its public
functions in `tests/`.

## Development

One command runs every check:

```sh
just check
```

The `justfile` lists the checks: `cargo fmt --check`, clippy with warnings denied, `cargo deny
check`, `cargo machete`, the tests and the Verus proofs (`cargo verus focus`). It needs `just`,
`cargo-deny`, `cargo-machete` and Verus on your `PATH`; `.github/workflows/check.yml` pins the
version of each. The lint table in `Cargo.toml` and the settings in `clippy.toml` are
[BeautifulRust](https://github.com/morganlinton/BeautifulRust)'s `lints/lints.toml` and
`lints/clippy.toml`, taken whole; update them from there, never by hand. The tests also need Node
22.18 or newer and `tsc` on your `PATH`. `just fix` formats the code and applies clippy's fixes.

The TypeScript library in `npm/model` has its own commands, run from that folder after
`pnpm install --frozen-lockfile`: `pnpm run typecheck` (tsrs), `pnpm run format` (oxfmt, check
only), `pnpm run format:fix` and `pnpm test` (vitest).

To run the checks before each commit, enable the hook once per clone:

```sh
git config core.hooksPath .githooks
```
