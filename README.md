# earnest

A model checker for models written in your app's own language. A coding agent states the rules of
a change as a model in TypeScript or Rust, `earnest verify` visits every reachable state, and the
result goes into the PR as a statechart and a report.

Status: version 1 is built and passes its own checks, and is not yet proven. Six adversarial
passes each found models with a broken rule that were reported as passing, and no pass has come
back clean. TLC 1.8.0 confirms the TLA+ export of every conformance model in CI.
[Limits of the current build](docs/limits.md) says what earnest does not do, what was checked and
how far that evidence goes. The spec is the Claude Doc "A formal verification suite for agents".

## Why the app's own language

A model is an ordinary TypeScript or Rust file, in the same language as the app it describes.
That is a deliberate choice, made to keep verification from adding anything new to a project.

- **No second language.** Whoever writes the app can write the model. There is no TLA+ or Quint
  to learn, and none to review.
- **No second toolchain.** The project's own type checker, linter and formatter already accept a
  model file. Nothing new is installed to write one.
- **No second place for the truth.** The model sits next to the code, uses the same names, and
  changes in the same pull request.

Earnest still checks what a TLA+ checker checks. Every model can be exported as TLA+, and TLC
finds the same states and the same verdicts.

## Install

In a project that holds models, from the package manager the project already uses:

```sh
npm install --save-dev @earnest/cli @earnest/model
npx earnest verify
```

`pnpm add -D` and `yarn add -D` do the same. `@earnest/cli` is one program: a small Node
launcher and, in an optional dependency chosen by operating system and cpu, the binary
(`@earnest/cli-linux-x64-gnu`, `-linux-arm64-gnu`, `-darwin-arm64`, `-darwin-x64` and
`-win32-x64-msvc`). It has no install script and downloads nothing, so after the install
`earnest` needs no network, no Java and no setup step on its first run. `@earnest/model` is the
library a TypeScript model imports; it ships compiled JavaScript and declarations, so it runs in
Node and type-checks under strict `tsc` from `node_modules`. See `npm/cli/README.md` and
`npm/model/README.md`. A Rust model needs no npm package: it uses the `earnest` crate.

To install the checker from source, with no npm and no registry:

```sh
cargo install --git https://github.com/cpeaustriajc/earnest earnest-cli
cargo install --path apps/earnest      # from a clone of this repository
```

Either puts the program `earnest` in `~/.cargo/bin`. The checker never resolves the
`@earnest/model` import on disk: it reads the file's text and never looks for the package, so
`earnest verify` works with no `node_modules` at all. The import matters only to the editor, the
type checker and Node, which need the package to find `m` and its types.

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
earnest confirm --tla-rs ~/.cargo/bin/tla-rs
```

| Flag | Default | Meaning |
|---|---|---|
| `PATH...` | the current directory | Files or folders to search for models |
| `--model <name>` | all | Check only the model with this name |
| `--rule <id>` | all | Check only this rule (needs `--model` when two models share the id) |
| `--json` | off | Print the results as JSON instead of text |
| `--max-states <n>` | 5000000 | Stop a model after this many distinct states. Every state that is counted has its rules judged |
| `--time-limit <seconds>` | none | Stop a model after this long, also inside one state: a rule or an action that takes very long in one state is stopped too |
| `--jobs <n>` | all cores | Worker threads |
| `--no-cache` | off | Check every model again, even when unchanged. The results it leaves unchecked stay stored |
| `--max-arrows <n>` | 20 | `chart` and `report` only: the most arrows a chart draws, at least 1 (see "Statechart and report") |
| `--base <git ref>` | none | Compare each model's rules with the same file at this commit |

`earnest verify` runs in one process, needs no network, no Java and no setup, and writes only to
`.earnest/` in the folder it is run from.

`--jobs` is the number of worker threads for the whole run. Models are checked side by side, and
the threads a model is not using go to the others: the levels of one large model are spread
over every thread that is free, and when a model finishes and no model waits to start, its
threads go to the models still running, also to one that started with a single thread, and a
model takes up such a thread before it starts its next part of a level (a level is cut in at
most 64 parts), not only at its next level. The results, their order and the path of each
broken rule are the same for every `--jobs` and on every run.

### Which files are models

The simple answer: a TypeScript file named `*.model.ts` (or `.model.mts`, `.model.tsx`), or any
`.ts`, `.mts` or `.tsx` file that imports `@earnest/model`, is a model file, and so is a Rust file
that derives `Model`. The rest of this section is the exceptions and the edge cases. A file is a
model file when any of these holds, and a model file is read or refused as unreadable, never
left out:

- it is named on the command line, whatever it is called and whatever it imports;
- in TypeScript, its name ends in `.model.ts`, `.model.mts` or `.model.tsx`, whatever it imports;
- it is a `.ts`, `.mts` or `.tsx` file that imports `@earnest/model` by the plain specifier, or
  a `.rs` file that names the `earnest` crate, and it is not a test file (below).

The plain specifier is judged by what it means and not by how it is written. In TypeScript the
specifier is what JavaScript makes of it, so `"@earnest\x2fmodel"`, a Unicode escape or a line
continuation in it is the library, and so is an `export ... from`, a dynamic `import()` or a
`require` in a file that spells `earnest`. In Rust the file is read as tokens, so a path that
starts with `earnest` counts however it is spaced, grouped or broken by a comment: `use
earnest::model::Model`, `use earnest::/* c */model::Model`, `use earnest as kit` and `extern
crate earnest`. A raw identifier is the same name as its plain spelling everywhere in Rust, so
`use r#earnest::model::Model` names the crate. A word in a comment or a text is not a use.

A Rust file found by walking a folder that names the `earnest` crate but declares no model, such
as a test helper that walks `Model::starts()` and `steps()`, is not a model file. It is left out,
and the run says so: a line `skipped  1 file that uses the library and declares no model` in
the text output of `earnest verify` (on stderr for the other commands), a section `Files that
declare no model` that lists the paths in `earnest report`, and the paths in the `noModel` list
of the JSON. The file is judged by its tokens, so nothing hides a model from the test: a
`derive` of `Model` or `Domain`, an attribute `#[model]`, `#[view]`, `#[action]`, `#[rule]`,
`#[end]`, `#[state]`, `#[group]`, `#[initial]`, `#[range]` or `#[one_of]`, a call of `require!`
and an `impl` of one of the library's traits count wherever they stand, inside a `mod`, a
function or a macro, behind `cfg` or `cfg_attr`, with a path or a raw identifier, and a file that
can't be parsed or is nested too deep to be is read and refused, never left out.

The rule is that a file is left out as declaring no model only when every use of the library in
it is one that the token test understands completely: names written as the library has them,
plainly or by a path that starts with `earnest`. A file that uses the library and has any
other construct through which a model could be declared without those names showing is a model
file, and is read or refused: a `use` or an `extern crate` that renames, with `as`, the crate
or a name of the library (`use earnest::model::{Model as Starts, model as checked}`, `use
earnest::model as lib`, `use earnest as kit`, but not `as _`, and a rename of a name that a module of
the file re-exports from it), an `include!`, and any `macro_rules!`. Such a file is refused
whatever it declares, because the reader refuses a renamed library name, an `include!` and a
macro of a model file; an included file that does not name the crate is not a model file of its
own, and the file that includes it is the one refused. A file named on
the command line is read or refused whatever it declares. TypeScript has no such rule: a file
that imports `@earnest/model` is a model file even when it only calls `explore`, because telling
that no model is made through an alias, a namespace import or a destructured `m` takes the
binding analysis that the checker applies only to refuse those files, and a model missed there
would be dropped without a word. Name such a script's folder `tests`, or call it `*.test.ts`, to
have it skipped as a test file.

A file named on the command line is always read or refused, whatever it is called: a name that
is only an extension (`.ts`, `.mts`, `.tsx`, `.rs`) is a file of that language, and a file of
another kind is `unreadable` with a sentence saying the checker does not know how to read it.
The same file reached by two spellings of its path (`a.model.ts` and `./a.model.ts`, a path
through `..`), through two roots (`earnest verify . ./src`), by a link or by a root that is a
folder and another that names a file in it, is checked and listed once: every file found is
compared by its real path, whichever way it was found. It keeps the spelling of the first
root that reached it.

A file found by walking a folder is a test file, not a model file, when it follows the test
convention of its language, so an app's own tests that import the library (a vitest test, a Node
script that replays a bug path, a Rust integration test) do not stop a bare `earnest verify`:

| Language | A test file is |
|---|---|
| TypeScript | a file named `*.test.ts`, `*.spec.ts` or the `.mts` and `.tsx` forms of those, or any file under a folder named `test`, `tests` or `__tests__` |
| Rust | any file under a folder named `tests`, `benches` or `examples`, or a file named `build.rs` |

The folders are judged by the names below the folder or file the run was given, so
`earnest verify tests/models` searches `tests/models` as it is told to. A name ending in
`.model.ts` (or `.model.mts`, `.model.tsx`) always wins: it is a model file wherever it is, and
a test file that is one is read. A test file that uses the library is not read and does not
change the exit code, and the run says so: a line `skipped  3 test files that use the library`
in the text output of `earnest verify` (on stderr for the other commands), a section `Skipped
files` that lists the paths in `earnest report`, and the paths in the `skipped` list of the
JSON. A test file that does not use the library is not a model either way and is not listed.
A file named on the command line is read or refused whatever it is called and wherever it is,
so it is never skipped. A root that names a test folder (`earnest verify . ./tests`) is
searched as it is told to, so below it no file is a test file: a file that is read through one
root is never also listed as skipped through another. A test file that can't be opened (a link
to nothing under `tests/`) is a test file, so it is skipped and listed as skipped, never
`unreadable`, and does not change the exit code: the checker can't tell whether it uses the
library, and says it left it out. A file that is no test file and can't be opened is
`unreadable`, as below. When `--model` or `--rule` names a model that only a skipped test file
holds, the error says so and names the file (`no model is named `Climb`; the model `Climb` is
in `tests/climb.ts`, a test file that uses the library and was skipped: name the file, or
search its folder, to read it`), and `verify`, `chart`, `report`, `tla` and `confirm` all print
the skipped line before they exit 2. They print the line for the Rust files that declare no
model too, and the error names each of them (`...; the file `helper.rs` uses the library and
declares no model, and was not read`), whatever `--model` or `--rule` names. A Rust `#[cfg(test)]` module inside a model file is not part of the
model, as before. `--base` applies the same convention to the files of the base commit.

A model file may reach the library only by `import { m } from "@earnest/model"`. Reaching it
any other way, such as `await import("@ear" + "nest/model")`, a `require`, or an import of a
local module that passes the library on (`import { m } from "./kit.ts"`), is refused with a
sentence, and so is the file that passes it on: any export of a binding that is the library
or came from it (`export { m }`, `export { m as kit }`, `export { m as default }`, `export
default m`, `export const kit = m`, a namespace import or any other imported name listed in
`export { ... }`, `export * as lib from "@earnest/model"`, a part of it taken apart with
`const { model } = m`) makes its own file unreadable, so a file that builds a model from it
never runs on a library the checker has not seen. Imports are hoisted in JavaScript, so
the export is refused wherever the import is. A file that is not named `.model.ts`, not named on the command line and that hides
its import from its text, such as a joined specifier, is not found: name it or call it
`.model.ts`. A file nested too deep to parse is a model file by its import only when it
spells `earnest`. A file that mentions
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
without read permission or a link to nothing (a test file excepted, above): the checker can't
tell that it holds no model. So
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
    end    gate = "open", paid = false
  stuck  no action can happen here, and the model does not mark it as an end
    start  gate = "locked", paid = false
    1. pay  paid: false -> true
    end    gate = "locked", paid = true
unfinished  Ledger  1000 states so far  3 ms  (state limit of 1000)
unreadable  src/clock.model.ts
  src/clock.model.ts:12:17  `Date.now`  the system clock is outside the library
4 models: 1 pass, 1 fail, 1 unfinished, 1 unreadable
```

- The status word is `pass`, `fail`, `unfinished` or `unreadable`.
- A model line has the status, the model name, `<n> states` (`<n> states so far` when unfinished)
  and `<n> ms`. An unchanged model served from the cache ends with `(cached)`. A cached result
  is served only by a build of the checker made from the same sources as the build that stored
  it. A result is stored unless `--time-limit` stopped it, so one that stopped at `--max-states`
  is stored too. A run that checks only some models, with `--no-cache` or not, leaves the stored
  results of the others as they were.
- `broken` names the rule's id and its sentence, and is followed by the shortest path to the first
  state that breaks it: a `start` line with every field, then one numbered line per action with
  the action's id, its input as `name = value` when it has one, and each field that changed as
  `field: before -> after`, then an `end` line with every field of the state the path ends in, so
  a reader need not add the steps up. A path with no step is its `start` line alone. `earnest
  report` prints the same lines.
- `stuck`, `out of range` and `fault` findings have the same path form. A model is out of range
  when an action gives a field a value outside what the field declares. That is a fault of the
  model, reported with the path to the state. The state out of range is counted, no rule is
  judged in it, and nothing is explored from it, so no rule is `holds` in a model with such a
  state, and a rule that was not seen broken is `unknown`. `explore` in the TypeScript library
  does the same.
- Anything the model asks that can't be evaluated in a state is a `fault` in that state: a rule,
  the `when` or the `then` of an action, or the `when` of an end, for example because it reads a
  field of `null`. A state is `stuck` only when the `when` of every action is false and every
  end is false. A state where a `when` or an end can't be evaluated is a fault and is not stuck.
  An end is asked only in a state where no action can happen. `explore` in the TypeScript
  library follows the same rule.
- A model that stops at `--max-states` or `--time-limit` with nothing broken found is
  `unfinished` with `<n> states so far`. One that already has a broken rule, a stuck state, a
  state out of range or a fault is `fail`, and its line says it stopped early with the limit
  in parentheses, as `fail  Turnstile  3 states so far  (state limit of 3)`; the exit code is 1
  for both. Each of those states has had its rules judged, also the states the search found last
  and did not go on from, so a rule a counted state breaks is `broken` with its path. No action
  is tried from the states it did not go on from, so they add no stuck state and no fault of an
  action. When `--time-limit` stops the search inside the evaluation of one state, the states
  whose rules were not judged by then are not counted: an unfinished model can be `0 states so
  far`.
- The combinations of an action's inputs are tried one at a time under both limits, and the
  states one state leads to are held only once, so an action with a very large input space (a
  billion combinations) ends as `unfinished` at the limit with bounded memory, never a kill.
- An `unreadable` file lists each problem as `file:line:column`, the feature in backticks and a
  sentence. No model in that file is checked.
- A line `skipped  <n> test files that use the library` comes before the summary line when
  `<n>` is above 0 (`1 test file that uses the library` for one). It is not in the summary
  counts and does not change the exit code. `--json` lists the paths.
- The summary line counts each model and each unreadable file once in its total, then lists
  `pass`, `fail` and `unfinished`, and adds `<n> unreadable` only when there are unreadable
  files. In JSON, `summary.models` counts models only and `summary.unreadable` counts files.
- A model's name, a file's name and every other name that stands on a line of its own print on
  that one line: a line break, a tab or any other control character, or a line or paragraph
  separator, is one space, with the white space around it dropped, as `earnest report` does. A
  name can't start a line that reads as another result.
- Values print as `"text"`, `12`, `true`, `null`, `{a, b}` for a set, `[a, b]` for a sequence or
  queue, `{k -> v}` for a map and `{name: value}` for a record. Inside the quotes of a text, `"`
  prints as `\"`, `\` as `\\`, a line feed as `\n`, a carriage return as `\r`, a tab as `\t` and
  any other control character or line separator as `\u{...}` with its code in hexadecimal, so a
  value is always on one line and reads only one way.

### Exit codes

| Code | Meaning |
|---|---|
| 0 | Every model passed |
| 1 | A rule broke, a model was stuck, out of range or faulted, a model was unfinished, or a file was unreadable. A `--model` or `--rule` that names what an unreadable file declares is this, not 2: the error says the file is unreadable and its problems are listed |
| 2 | Bad flags, or no model matched `--model` or `--rule`. The error names a skipped test file that holds the model or rule, and the skipped line comes before it. When a file was unreadable and none of them declares the name, the error names the unreadable files, which may hold it |

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
  "skipped": ["src/cart.test.ts"],
  "ruleChanges": [],
  "summary": { "models": 1, "pass": 0, "fail": 1, "unfinished": 0, "unreadable": 1 }
}
```

`skipped` lists, in path order, the test files that use the library and were not read (see
"Which files are models"); it is `[]` when there are none. `noModel` lists, in path order, the
Rust files that use the library and declare no model and were left out, and is `[]` too. `status` is `pass`, `fail` or
`unfinished`. `verdict` is `holds`, `broken` or `unknown`.
A rule is `broken` when a counted state breaks it. It is `holds` only when it was judged in
every counted state: the search ended at the last state, the rule never faulted, and no state out
of range and no fault in an action or an end cut the search short. Otherwise, when it was not
seen broken, it is `unknown`: the model was unfinished, the rule faulted in a state, or a state
out of range or a fault in an action or an end left a state unjudged or unreached. `kind` is `stuck`, `out-of-range` or `fault`.
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
but no reachable state tells the two apart, the old condition can't be read against the new
model, or the old or the new condition has no value in a state (a fault there, such as a read of
`null`) and no other state loosens or tightens it, and `reworded` when only its sentence differs. A rule whose sentence and condition are
unchanged is not listed. Rule changes compare meaning, not text: a comment or white space
in a condition is no change.

Every change carries the path of its file, as the last field of its line in the text and the
report and as `file` in each entry of `ruleChanges` in the JSON, so two models of one name in
two files are told apart.

Every rule the two commits share is compared, not only those whose text was edited. The old
condition is read with the constants and helper functions of the file at the base commit, so a
rule is `loosened` when only a constant or a helper it uses changed. A condition differs when its
text does, or when it means something else through such a constant or helper. The text of a
condition is compared and printed with each run of white space as one space, in TypeScript and
in Rust, so moving white space is not a change.

### Statechart and report

`earnest chart` prints one fenced `mermaid` block holding a `stateDiagram-v2` chart. States and
arrows carry the labels from the model's view, which a TypeScript model writes with `Model.view`
and a Rust model with a `#[view]` block (see "Writing a model in Rust"). A Rust model and its
TypeScript twin with the same view give the same chart, text for text. A state that breaks a
rule has the class `broken`.
A chart never has more than 20 arrows, or the `--max-arrows <n>` of `chart` and `report` (at
least 1): when the model has more, the chart keeps the paths to the
broken states first and says how many states and arrows it left out. The states are counted
for what they are: a box of the chart stands for every state with its label, so the note counts
the states of the boxes it left out, and an arrow is one box to another however many actions it
carries. When the shortest path to a broken state is longer than what is left of the arrows allowed,
the chart keeps the first arrows of that path, does not draw the broken state, and the note
says so: `Not shown: 5 of 26 states and 5 of 25 arrows. A broken state lies beyond what is
shown.` When a broken state is drawn and others are not, the note counts only the others and
says so: `Another broken state lies beyond what is shown.` or `10 more broken states lie beyond
what is shown.`

`earnest report` prints Markdown for a PR body: one section per model with its result line, its
chart, the path to each broken rule, a section `Unreadable files` that names each file the
checker could not read and why, a section `Skipped files` that lists the test files that use the
library and were not read, and the rule changes when `--base` is given. A file is never left out
of the report without a word: a report of a run with an unreadable or a skipped file says so.

### TLA+ export

`earnest tla` writes `<Model>.tla` and `<Model>.cfg` for each model. TLC finds the same number of
distinct states and the same verdict for each rule. `earnest confirm --tlc <jar>` exports every
model, runs TLC on each with Java, and exits 1 when a state count or a verdict differs.
`earnest confirm --tla-rs <program>` does the same with [tla-rs](https://github.com/bbcvc/tla-rs)
(crate `tla-tools`, installed with `cargo install tla-tools`), a TLA+ checker written in Rust
that takes TLC's options and prints TLC's messages, so it needs no Java. Give both flags to
check each model with both: each model then has one line per program, and the line of tla-rs
ends with `(tla-rs)`. tla-rs is run on each module with `--config` and `--workers`; it stops at
its first broken invariant, so a rule's invariants are each a run of their own, and its
differences read `tla-rs found`, `tla-rs says`. The 32-bit and text limits below are TLC's: a
model that tla-rs can't confirm is `differs`, never `cannot`. `--jobs` is the number of workers
given to each program.

A quantifier stops at the first item that decides it, in the checker and in Node (`every` and
`some`) and in Rust (`forall` and `exists` are iterators that stop), so a fault in a later item
never happens. The items come in the order of the declaration for `m.forall(T, f)`,
`m.exists(T, f)`, `forall::<T>(f)` and `exists::<T>(f)` (`m.oneOf([3, 1, 2])` tries 3, then 1,
then 2, in all three), in the order of the items for a sequence or a queue, and in the order of the
set for a set: numbers ascending, `false` before `true`. The module judges a quantifier in the
same order: an item is judged when no item before it decides, so a fault in an item that comes
after the one that decides makes no difference, and TLC's verdict is the checker's. A quantifier
over a set of other items than numbers and booleans whose test can fault is still judged for every
item: the order of such a set is not written in the module, and TLC can say `unknown` where the
checker has a verdict.

A rule `id` is the invariant `Rule_id`, and the configuration names one invariant for each rule.
A rule that can't be evaluated in a state is true there in `Rule_id`, as an action that can't be
evaluated takes no step, so TLC counts every state and never stops on such a rule. The module
also defines `Judged_id` for a rule that can fault: a state where the rule has no value breaks
it. `earnest confirm` checks both for each rule and compares TLC's verdict, `broken` when a state
breaks `Rule_id`, else `unknown` when a state breaks `Judged_id`, else `holds`, with the
checker's. TLC can't see a state out of range or a fault in an action or an end that cut the
search short, so a rule that the checker leaves `unknown` only for that reason, and whose
`Judged_id` no state breaks, is `holds` in TLC and still agrees with the checker.

TLC has two limits of its own, and the export can't remove them. TLC holds integers to 32
bits, from -2147483648 to 2147483647, and compares some texts that are not ASCII as equal when
it counts states, so `Z` and `Ś` are one state. When a model declares a number outside the 32
bits, has a text that is not ASCII, or (in `earnest confirm`, which runs the search) computes a
number outside the 32 bits while the search runs, TLC can't confirm it. `earnest tla` warns on
stderr for a declared number or text, and says that a number computed while the model runs is
seen only by `earnest confirm`. `earnest confirm` prints `cannot` in place of `differs`, with a
line `TLC can't confirm this model:` and the reason, before the differences it found. A model
that TLC confirms anyway is `same`. `cannot` is not `same`, so `earnest confirm` exits 1 for it.
The tests that need Java and the jar are built only with the cargo feature `tlc`: `cargo test
--locked -p earnest_break_it --features tlc --test tlc`. The test that needs tla-rs is built only
with the cargo feature `tla_rs`: `cargo test --locked -p earnest_break_it --features tla_rs --test
tla_rs`.

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
  tries every value of every input, with the first input changing fastest: for inputs `a` and
  `b` it tries (a1, b1), (a2, b1), (a1, b2), and so on. `explore` in the TypeScript library and
  `steps()` of a Rust model try them in the same order, so the path the checker shows to a state
  is the one found by walking `steps()` breadth first. `then` gives back only the fields that change.
- The id of an action, rule or end is the name of its exported constant. A rule, action or end
  bound to a constant that is not exported, by `export const` or by an `export { name }`, makes
  the file unreadable. So does an export that JavaScript refuses or that erases the name: the
  same name exported twice (`export { low }; export { low };`, `export { low, low }`,
  `export const low` and `export { low }`, or the same name reached by `as`), and a `type`
  modifier on an exported part (`export { type low }`, `export { low, type top }`), which
  erases the export from the JavaScript as `export type { low }` does. Write `export type {
  Name }` for a type.
- `end` marks states where stopping is correct. Any other state with no action is stuck.
- `view` is optional. Without one, a state's label in the chart is every field, as `n = 0, q = 0,
  by = 1`, and a view's `state` label is how to shorten it. `step` maps an action id to its arrow
  label; the default is the action's sentence. `colors` accepts `broken`, `start` and `end`, each with `#` and 3, 4, 6 or 8
  hexadecimal digits or a color name of letters; any other part, or any other color, makes the
  file unreadable, because the chart would leave it out without a word. A `step` entry whose id is not an
  action of the model makes the file unreadable for the same reason.

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
  `&&`, `||`, `!`, `+`, `-` (also as a sign: `-1`), `*`, `m.rem(a, b)`, `m.div(a, b)`, the `? :` choice, the `!` after a
  `m.some(value)`, the value inside an optional (`m.some(s.chosen).qty`), object literals with spread, calls to the
  primitives below and to helper functions, `as const`, and template strings in `view` labels. A function
  body is one expression, or a block of `const` declarations and `if (...) return ...;`
  statements that ends in a `return`. Nothing follows that `return`.
- A name is declared once in its scope, as JavaScript requires: a second top-level `const` of
  one name, a second `const` of one name in a block, a `const` in the body of a function that
  has a parameter of that name, and two parameters of one name (`(n, n) => ...`) are refused,
  where the language refuses the file. A nested block or a function's own parameter may shadow
  a name of an outer scope.
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
it as `earnest verify` does. The operator `/` and `/=` are refused too: JavaScript's `/` gives a
fraction, where Rust's drops it, so write `m.div(a, b)`, the whole number of times `b` goes into
`a`, which is a fault for a negative `a` or a `b` below 1 in the same words as `m.rem`, and which
`explore` and `replay` report as `earnest verify` does. Rust's `%` and `/` keep their meaning, a
fault on a negative number and, for `/`, a divisor of zero, where Rust panics. A type assertion hides a `null` from the type checker as `!` does
and changes nothing in JavaScript, so `as`, `satisfies` and `<Type>value` are refused in a term
except `as const`: `(s.o as number) < 3` is `null < 3` in the start state, and write
`m.some(s.o) < 3` instead. A postfix `!` after a value is
refused: write `m.some(value)`, which is the value of an optional and is a fault where it is
`null`, in `earnest verify` and in the library's `explore` and `replay` alike, because
JavaScript would go on with `null` as the number 0. The nesting of a model file is counted on
its code: brackets and runs of operators inside a string, a template literal (outside its
`${}`), a comment, a Rust raw string or a character literal do not count, and a file that is
not a model file is never reported as unreadable for them.

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
  `Timer`, `Request` and `Retry<MAX>`. `#[initial(value)]` works as in TypeScript. A model needs
  no `pub` and no documentation: `#[derive(Model)]` uses the type where `rustc` looks for dead
  code, so a model that only `earnest verify` reads draws no warning, and the file passes
  `clippy` under the lints of this repository.
- A choice among listed values is `#[one_of(a, b)]`, the Rust form of `m.oneOf([a, b])`, on a
  field, on an action's parameter or on a field of a record that derives `Domain`:

  ```rust
  #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
  struct Pick {
      #[one_of(1, 5, 9)]
      size: u8,
      #[one_of("Kim", "Lee")]
      #[initial(None)]
      buyer: Option<&'static str>,
  }

  #[model]
  impl Pick {
      #[action("a buyer takes the pick")]
      fn sell(&self, #[one_of("Kim", "Lee")] who: &'static str) -> Option<Self> {
          require!(self.buyer.is_none());
          Some(Self { buyer: Some(who), ..self.clone() })
      }
  }
  ```

  A number is any integer type with `#[one_of(...)]` in place of `#[range(...)]`, and every listed
  number must lie inside the type; it keeps that type, so `self.size + 1` is a fault above its
  largest value, as for a range. A text is a `&'static str` field or parameter, also inside an
  `Option`, and it must have `#[one_of(...)]`: a string literal is a text value wherever the
  model writes one (`self.buyer == Some("Kim")`, `buyer: Some("Lee")`), and `Debug` prints it
  with its quotes, as the text report does. A field starts at every value listed, in the
  order listed, unless it has `#[initial(value)]`; an input is tried with each, as `m.oneOf`
  inputs are. A list holds at least one value, each once and all of one kind, and a number and
  a text can't share a list; `#[range]` and `#[one_of]` on one field are refused, and so is
  `#[one_of]` on a type that is not an integer or a `&'static str`. The TLA+ export is the one of
  `m.oneOf`: `size \in {1, 5, 9}` and `who \in {"Kim", "Lee"}`, and TLC finds the same states.
- An action is a method in the `#[model]` block that takes `&self` and its inputs and returns
  `Option<Self>`, or `Self` when nothing can stop it: an action with no `require!` that returns
  `Some(..)` on every path draws `clippy::unnecessary_wraps`, so it may give back the state
  itself (`Self { changed fields, ..self.clone() }`) and has no `require!`, no `?`, no early
  return of `None` and no `Some(..)`: `require!` returns `None`, which a method that returns
  `Self` can't, and `rustc` refuses each of them (E0308, E0277), so the file is unreadable.
  The same holds the other way: an action that returns `Option<Self>` gives back
  `Some(Self { .. })` on every path, and a bare `Self { .. }`, a `Some(Some(..))` or a value
  that is not wrapped is refused (E0308). Each `require!(condition);` is part of its `when`:
  it is read among the statements of the body and of an early `if` block, and it is refused
  inside a `match` arm or any other expression. An early `if condition { return Some(Self { .. });
  }` gives that result when `condition` is true and the rest of the body does not run, so a
  `require!` after it applies only where `condition` is false.
  The last expression is `Some(Self { changed fields, ..self.clone() })`. Before it, `let`
  bindings and `if condition { return Some(Self { .. }); }` are allowed.
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
  except `as _`. A model file imports the library directly: a `pub use earnest::...` (also
  `pub(crate)`, also `r#earnest`) is refused, because a file that re-exports the library would
  hide it from the checker in the files that use it. A `use` inside a body, a `macro_rules!` and a `type` alias are refused too, as a
  model file holds no such item. The three attributes are written plainly, never with a path. No part of a model is under `#[cfg]` or
  `#[cfg_attr]`: the checker reads one text for every build. Only an item that is not part of
  a model (a test module, a function, a plain `impl` block) may be `#[cfg(test)]`, and it is
  skipped.
- The statechart labels are a second `impl` block with `#[view(...)]`, the Rust form of
  `Model.view({ ... })`, with the same parts, the same labels and the same refusals:

  ```rust
  #[view(
      title = "Add to cart focus",
      step(press = "press Add"),
      colors(broken = "#c62828")
  )]
  impl Cart {
      #[state]
      fn state(&self) -> String {
          format!("{:?}, focus on {:?}", self.cta, self.focus)
      }

      #[group]
      fn group(&self) -> String {
          self.surface.to_owned()
      }
  }
  ```

  Every part is optional, and a model has one view. Without a view, a state's label is every
  field, `n = 0, q = 0`, and `#[state]` is how to shorten it. A label may call the helpers of
  the `#[model]` block, as `self.name()`: the block is a second `impl` of the same type. `title`
  is the chart's title. `step(action =
  "label")` gives an action's arrow its label, where the default is the action's sentence, as
  `step` does in TypeScript; an action that the model does not have makes the file unreadable.
  `colors(part = "color")` colors `broken`, `start` or `end` with `#` and 3, 4, 6 or 8
  hexadecimal digits or a color name of letters; any other part or any other color makes the
  file unreadable, because the chart would leave it out without a word. The method marked
  `#[state]` labels a state's box, and states with the same label and group are drawn as one;
  the method marked `#[group]` names the box a state is drawn in. Each takes `&self` and gives a
  `String`, and its body is one expression: `format!("text {} text {:?}", value, value)`, or one
  value followed by `.to_string()` or `.to_owned()`. A value is read as in a rule, so it may be
  a field, a helper call or an `if`; it must be a boolean, a number, an enum variant or a text,
  because `Debug` prints anything else (an `Option`, a record, a collection) differently from
  the chart. A hole is `{}` or `{:?}`, and the form must be one that `rustc` accepts for the
  type of the value and that prints what the chart draws, judged by the type of the whole
  value (a field, an `if`, a literal, a sum), not by how it is written:

  | Value | `{}` | `{:?}` | `.to_string()` | `.to_owned()` |
  |---|---|---|---|---|
  | boolean | read | read | read | refused: gives a `bool`, not a `String` (E0308) |
  | number, any integer type | read | read | read | refused: gives the number (E0308) |
  | enum variant | refused: no `Display` (E0277) | read: prints the variant name | refused: no `Display` (E0599) | refused: gives the enum (E0308) |
  | text, a `&'static str` | read | refused: `Debug` prints it in quotes and with escapes, the chart without | read | read |
  | `Option`, record, collection | refused | refused: `Debug` prints `Some(..)` and `Name { .. }` | refused | refused |

  A width, a name, a position or a flag in a hole is refused, `{{` and `}}` are the braces, and
  a `{` that is never closed, a lone `}`, and more or fewer values than holes are refused, as
  `rustc` refuses them. The block holds nothing but labels, and `format!` is read in them only.
  The macro implements `earnest::model::View`, so `state.state_label()` and
  `state.group_label()` give the labels the chart shows, and `Cart::title()`,
  `Cart::step_labels()` and `Cart::colors()` the rest. The twins `tasks/conformance/twins/cart.rs`
  and `dial.rs` give the same chart, text for text, as `tasks/conformance/models/cart.model.ts`
  and `dial.model.ts`.
- A model is ordinary Rust: a test calls `state.pay()` to take one step. A path from the report is
  replayed with `cargo test` by `earnest::model::replay`: pick the start state from
  `Turnstile::starts()`, give each step of the path as a `Move`, and compare the states it gives
  back, the start state first, with the `state` of each step of `earnest verify --json`:

  ```rust
  let start = Mailbox::starts().remove(0);
  let path = [
      Move::new("deliver").with("letter", "Bill"),
      Move::new("deliver").with("letter", "Bill"),
  ];
  let states = replay(&start, &path).expect("the path is one the model can take");
  assert!(!states[2].bills());
  ```

  A step is found among `state.steps()`, so an action runs only where its `require!` is true.
  An input is written as the report prints it, in the JSON of `earnest verify --json` or in its
  text output, so a path copied from either replays: `Bill` or `"Bill"` for an enum variant,
  `3`, `true`, `Kim` or `"Kim"` for a text (with the escapes of the text output inside the
  quotes), `null` for a `None` (a `Some` is the value inside it) and `{row: 1, lit: true}` for
  a record, whose fields may come in any order and whose own enums and texts are written bare
  or quoted. Every input the action takes must be given. The text a step holds (`Step::input`)
  is the text output's form, made by the trait `earnest::model::Report` that `#[derive(Domain)]`
  implements for the enum or the struct, and that the library has for `bool`, every integer
  type, `&str` and `Option`: `Report::report(&value)`. A state is not compared through `Report`:
  its fields are collections as well as values, and a test compares states with `assert_eq!` or
  through the rule methods. A step that can't happen gives a `Mismatch` with the step's number
  and the steps that can happen there. `Turnstile::starts()` lists every start state, and a rule
  or an end is called the same way: `state.stays_locked()`. `n.is_multiple_of(d)` on an unsigned
  integer reads as `n % d == 0`, and `n.is_multiple_of(0)` as `n == 0`, as in Rust, so a model
  can follow `clippy::manual_is_multiple_of`. It is a method of the unsigned integer types
  only: `rustc` refuses it on an `i8`, an `i32`, an `isize` or any other signed type (E0599), on a
  number written out (E0689) and on an `Option`, so the file is unreadable, and `n % d == 0` is
  what to write where `n` can't be negative.

A Rust model and its TypeScript twin print different ids, because an id follows each
language's names: the method `no_edit_after_approval` is the rule `no_edit_after_approval`, and
the constant `noEditAfterApproval` is the rule `noEditAfterApproval`. A twin keeps every field,
action and rule the same apart from that spelling; `--rule` takes the id as the file spells it.

A raw identifier is the same name as its plain spelling everywhere in Rust: `fn r#fn` is the
rule `fn`, `fn r#loop` the action `loop` and the field `r#type` the field `type`, in the
output, in `--rule`, in the JSON and in the TLA+ names. Rust refuses two items of one name in a
namespace (two constants or functions, two types), two methods of one name for a type, two
fields, two variants and two parameters of one name, so the file is unreadable with the line of
the second one; a `let` may shadow, as in Rust.

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

### Reading the value inside an `Option`

TypeScript writes `m.some(value)`. Rust reads these forms, and each does on `None` what Rust
does:

```rust
fn bonus(&self) -> u8 {
    if let Some(x) = self.last { x * 2 } else { 1 }
}

fn weight(&self) -> u8 {
    self.last.map_or(0, |x| x * 2)
}

fn count(&self) -> u8 {
    self.last.unwrap_or(7)
}

#[action("the last number is added to the total")]
fn add(&self) -> Option<Self> {
    let x = self.last?;
    require!(self.total + x <= 9);
    Some(Self { total: self.total + x, last: None, ..self.clone() })
}
```

- `if let Some(x) = value { a } else { b }` is `a` with `x` the value inside when `value` is
  present and `b` when it is `None`. The `else` is required, and the pattern is `Some(name)`:
  `Some(_)` (write `is_some()`), `mut`, `ref`, a pattern inside the `Some`, and a condition
  joined to the `let` with `&&` are refused.
- `value.map_or(default, |x| ..)` and `value.unwrap_or(default)` give `default` for `None`. The
  default is evaluated first, as Rust evaluates an argument, so one that can't be computed
  (`self.n - 1` at 0) is a fault whichever way the value goes.
- `let x = value?;` in an action that returns `Option<Self>` makes the action not happen where
  `value` is `None`, as a `require!(value.is_some())` does, and binds the value inside for the
  rest of the body. `let Some(x) = value else { return None; };` is the same, and clippy's
  `question_mark` asks for the first in its place under this repository's lints. Any other
  `else`, and `?` in a rule, an end, a helper or an action that returns `Self`, is refused.
- `value == Some(x)` and `value != Some(x)` compare as before.

A name in a pattern is resolved first, as Rust resolves it, against what is in scope: the
file's constants, and `None`, `Some`, `Ok` and `Err`. A constant in a pattern is a value to
compare with, never a new name. Where the pattern can fail it is the test with Rust's meaning:
`if let Some(TOP) = value { a } else { b }` is `a` only when `value` is `Some(TOP)`, and
otherwise `b`; `let Some(TOP) = value else { return None; };` in an action happens only for
`Some(TOP)`; an arm of a `match` or of `matches!` may be a constant that is one variant of the
enum matched (`const WARM: Stage = Stage::Warm;`, also through another constant), which counts
for the missing variants as it does in Rust. A constant where the pattern must always match is
refused (E0005): `let TOP = value?;`, `let TOP: u8 = ..`, a parameter of an action, a helper or
a function named `TOP`, and a parameter of a function passed to `map_or`, `every`, `some`,
`forall` and the like. A value named `Some`, `Ok` or `Err`, or a `None` pattern, is refused too
(E0530, E0005). A name that is not one of these is a new value, and shadows what the file has
under that name where Rust lets it: a value named like a field (`self.n` is still the field),
like a helper (a call of that name is refused, as Rust refuses it, E0618), like an enum or a
struct (`Phase::Idle` and `Line { .. }` are still the type), and an earlier value.

Rust's rules for moving a value are applied the same way everywhere a value is read, not only
for an `Option`. A value whose type is not `Copy` is moved when it is a place (a local, a
parameter, a field of one) in a `let`, an argument of a helper, a function or a primitive
(`Set::add`, `Seq::push`, `has`, `Some(..)`, `Set::of([..])`), the value a body gives back, an
item of a record, a `?`, the value `unwrap_or`, `map_or` and `is_some_and` read, the scrutinee of
an `if let` or `let else` whose pattern binds a part that is not `Copy`, a function that
`map_or` takes and uses a captured value in, and the fields a `..base` of a struct literal does
not give. Moving it out of `&self`, out of a field of a reference or out of a function that is
called once for each item is refused (E0507), and so is using it after it was moved, on any
path that may have moved it, including a closure that captured it and a value read twice by `?`
or `let else` (E0382). The types that are `Copy` are the integers, `bool`, `&'static str`, a
reference, `Timer`, `Request`, `Retry`, a `Lock` of a `Copy` owner, an enum or a record that
derives `Copy`, and an `Option` or a tuple of those. `Set`, `Seq`, `Queue`, `Map`, and an enum
or a record without `Copy` are not. Every refusal names the fix: derive `Copy` on the type, write
`.clone()` where it is read, or compare by reference. Reading a place without moving it (a
method that takes `&self`, `==`, `<`, a `match` on an enum with no binding, `&x`, `x.clone()`) is
not a move.

Every enum and every record is a type of its own: two enums with the same variant names are two
types, so a variant of one never compares with, is assigned to, is matched against or is passed
for a value of the other, and a `match` on `self.beta` takes the variants of `Beta` only. The
reader knows the type of each enum and each record it can type a value as, and the lowering
keeps the type of each enum on every variant and every enum field, so an enum is checked where
the reader can't tell the type of a value as well; the lowering reads two records of the same
fields as one type, so two records are told apart by the reader alone. The same check is made on the type a value has where Rust
fixes it, by what the reader can tell of each expression: the two sides of `==`, `!=`, `<`,
`<=`, `>`, `>=` and of arithmetic, the value of a field in `Self { .. }` or a record, a
`#[initial(..)]` and a constant, a `let` with a type, an argument of a helper, a function or a
primitive (`add`, `has`, `push`, `unwrap_or`..), the value a body gives back, the branches of an
`if`, the items of `Set::of`, and the type written for a function's parameter against the item
it is passed (`map_or(0, |weight: u16| ..)` on an `Option<u8>` is E0631, also a reference where the
item is a value). Two integer types are two types, whatever number they hold. A type written
that the checker can't compare with the type it is passed is refused, never ignored. The
refusals say `E0308`, `E0631` or `E0277`, as `rustc` does. `apps/earnest/tests/rust_agreement`
compares the construct the first versions added with `rustc`, and `crates/earnest_rs/tests/rustc`
compares these rules with it, case by case.

The value is moved out of its place when it is read this way, so the checker reads it only
where Rust does (the rules for a move above): a field of the state or of a record that is `Copy` (an integer, a `bool`, an
enum that derives `Copy`, an `Option` of those), a parameter taken by value, or an owned result
such as `self.items.first()` or `self.line.clone()`. A reference (`&self.last`, a `&Option<u8>`
parameter), `.as_ref()`, and a field that is not `Copy` (Rust says E0507) are refused with the
fix. `unwrap()` and `expect("..")` are refused although Rust reads them, because they panic on
`None`: a walk of the model in a test with `steps()` would stop there instead of reporting a
state, so earnest has no panics there; write one of the forms above.

### `match`

```rust
fn width(&self) -> u8 {
    match self.stage {
        Stage::Cold => 1,
        Stage::Warm | Stage::Hot => 2,
        _ => 3,
    }
}

fn speed(&self) -> u8 {
    match (self.stage, self.mode) {
        (Stage::Cold, _) => 0,
        (Stage::Warm | Stage::Hot, Mode::Fast) => 2,
        (_, Mode::Fast) => 1,
        (_, Mode::Slow) => 3,
    }
}

#[action("the stage moves on")]
fn advance(&self) -> Self {
    match self.stage {
        Stage::Cold => Self { stage: Stage::Warm, ..self.clone() },
        Stage::Warm | Stage::Hot => Self { stage: Stage::Done, ..self.clone() },
        Stage::Done => self.clone(),
    }
}
```

`match` is read in a rule, an end, a helper, a view label, a `let` and as the last expression of
an action, on a value of an enum that derives `Domain` (a field, an input, a helper's parameter
or result, or a `let` with its type stated) or on a pair `(a, b)` of two. An arm is `Enum::A`,
`Enum::A | Enum::B`, `_`, or in a pair `(Enum::A | Enum::B, _)`; the first arm that matches
wins, as in Rust. `matches!(value, Enum::A | Enum::B)` is read the same way. The arms must cover
every variant or end in `_`, and a `match` that leaves one out is refused with the missing
variant, as `rustc` refuses it (E0004). A guard (`if`), a name that binds the value, a pattern
inside a pattern, a reference pattern, an arm of another enum (E0308), a `match` on a number, an
`Option` or three values, and a `require!` inside an arm are refused with a sentence. Clippy
asks for `A | B` where two arms are the same (`match_same_arms`), for the variant in place of
a `_` that stands for one (`match_wildcard_for_single_variants`) and for `(A | B, _)` in place
of `(A, _) | (B, _)` (`unnested_or_patterns`).

### Helpers

```rust
fn with_stage(&self, stage: Stage) -> Self {
    Self { stage, ..self.clone() }
}

fn name(&self) -> &'static str {
    if self.flagged { "flagged" } else { "plain" }
}

fn grown(tag: &Tag) -> Tag {
    Tag { size: tag.size + 1, ..tag.clone() }
}

fn weight(&self) -> u8 {
    u8::from(self.flagged) + u8::from(self.tag.kind == Kind::Large)
}

#[action("the tag grows and the count goes up")]
fn grow(&self) -> Option<Self> {
    require!(self.tag.size < 3 && self.count < 3);
    Some(Self { tag: grown(&self.tag), ..self.with_count(self.count + 1) })
}
```

- A helper that returns `Self` (or the model's own name) is built with struct update. An action
  gives it back, `Some(self.with_stage(..))`, or spreads it into its own result,
  `..self.with_count(..)`, where the fields the action writes win, as in Rust. It is not a value:
  a rule, a condition or another helper can't read it, and the sentence says so.
- A helper may return a `&'static str`, a `Domain` record built with struct update, an enum, a
  number, a `bool` or an `Option`. A text is compared with `==` and printed by `{}` in a label.
- `u8::from(flag)` is 0 or 1, and `u16::from(small)` is the number itself, for every pair of
  integer types for which Rust has `From` (a `u8` to any wider type, a `u16` to a `u32`, a
  `usize`, an `i32` and wider, an `i8` to any wider signed type, and so on); a conversion Rust
  does not have, and a bare number whose type is unknown, are refused with the reason. Clippy's
  `bool_to_int_with_if` and `cast_lossless` ask for these in place of `if flag { 1 } else { 0 }`
  and `as`, which the checker does not read.
- A `#[view]` block may call the helpers of the `#[model]` block, as `self.name()`, because both
  are `impl` blocks of one type. It can't call an action, a rule or an end.

### Starting at some values

`#[initial(value)]` starts a field at one value. `#[initial(A, B)]` starts it at each value
listed, in order, one start state for each, so an enum or a record starts at some of its values
only; list each value once. A field of an enum already starts at every variant, and `#[one_of]`
lists the values of a number or a `&'static str`, never of an enum.

```rust
#[initial(Stage::Cold, Stage::Warm)]
stage: Stage,
```

### What a model costs

An action's inputs are tried in every combination in every state, so the work of a run is the
number of states times the number of input combinations of all the actions, and each
combination evaluates the guard and, where it holds, the update. An action with `n: u8` from 0
to 9 and `mode: Mode` of three variants is tried 30 times in each state; with 10,000 states that
is 300,000 tries, and three such actions are 900,000. A billion combinations in one action end
as `unfinished` at the limit, never a kill, but they never finish. To keep the space small:

- Give a number the few values that matter: `#[one_of(0, 1, 5)]` in place of `#[range(0, 9)]`.
- Split an action with several inputs into actions with one each, when the inputs are not
  read together.
- Bound every counter with a range as narrow as the rules need, and give a field that can start
  anywhere an `#[initial]`: an unbounded `Seq`, a wide `Set` or a `Map` of a wide key is the
  states of its whole domain.
- Pass the inputs of one thing as one `Domain` record only when they belong together: the
  tries are still the product of its fields.
- Stop early while writing the model with `--max-states` and `--time-limit`.

### Lints

The reader accepts the forms that clippy's pedantic lints ask for, so a model file passes
`cargo clippy -D warnings`. Where this repository's own lint table (`max-fn-params-bools = 0`
and the lints it turns on) asks for something else than a project's default, the model follows
it:

- A `bool` input is refused by `fn_params_excessive_bools` here, so take a two-variant enum
  (`enum Toggle { On, Off }`) or a field of a `Domain` record. A record with more than three
  `bool` fields draws `struct_excessive_bools`: use an enum.
- A method with many inputs draws `too_many_arguments`: pass them as one `#[derive(Domain)]`
  record, `fn order(&self, line: Line)`, whose fields have their own `#[range]` or `#[one_of]`.
- An action that nothing can stop returns `Self` (`unnecessary_wraps`); one with `require!`
  returns `Option<Self>`.
- `if flag { 1 } else { 0 }` becomes `u8::from(flag)` and `x as u16` becomes `u16::from(x)`.
- `let Some(x) = value else { return None; };` becomes `let x = value?;` (`question_mark`).
- A `match` writes `A | B` for arms with the same body and names the variant a `_` would stand
  for alone.
- `n % d == 0` on an unsigned number becomes `n.is_multiple_of(d)`.
- A parameter that is a record is taken by reference (`needless_pass_by_value`), and a small
  `Copy` type by value (`trivially_copy_pass_by_ref`); an `&Option<T>` parameter draws
  `ref_option`, so pass the `Option`.
- `x >= 1 && x <= 3` draws `manual_range_contains`, and the checker reads no `contains`: where
  a `#[range]` already bounds one side, write the other side only.
- `unwrap()` draws `unwrap_used` here and is refused by the checker as well.

## The primitives

Each primitive has the same meaning in TypeScript, in Rust and in the TLA+ export. TypeScript
names are camelCase and Rust names are snake_case: `isEmpty` is `is_empty()`. In TypeScript a
name without parentheses below is a property; in Rust everything is a method call. Every
operation gives back a new value and changes nothing in place.

| Group | Declared with | Operations |
|---|---|---|
| Boolean | `m.boolean()` / `bool` | `&&`, `\|\|`, `!`, `===` |
| Number | `m.int(lo, hi)` / an integer type with `#[range(lo, hi)]` or `#[one_of(...)]` | `+`, `-`, `*`, `m.rem(a, b)` / `%`, `m.div(a, b)` / `/`, `is_multiple_of(d)`, `u8::from(flag)` and `u16::from(small)` (Rust), `<`, `<=`, `>`, `>=`, `===` |
| Enum | `m.enum([...])`, `m.oneOf([...])` / `enum`, `#[one_of(...)]` | `===`, `!==` / `==`, `!=`, `match`, `matches!` |
| Optional | `.optional()` / `Option<T>` | `=== null`, `!== null`, `m.some(x)` / `is_some()`, `is_none()`, `is_some_and(f)`, `== Some(x)`, `if let Some(x)`, `map_or(d, f)`, `unwrap_or(d)`, `let x = v?;` in an action |
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
from an optional record the same way: `{ ...m.some(s.chosen), qty: 1 }`. A field may have the name of
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
| `npm/cli` | `@earnest/cli`, which ships the binary: a Node launcher and one package for each platform that holds the release build |
| `tasks/conformance` | Model files with their expected state counts, verdicts and paths, shared by every test |
| `tasks/benchmark` | The speed measurement: fixed models in TypeScript and Rust with hand-derived state counts, and a binary that times the release `earnest verify` on them |

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

`npm/cli` has the same commands, plus `node scripts/assemble.ts` to build the package of this
machine's platform; its `pnpm test` builds the release binary, packs both packages, installs
them into a fresh project with no network and runs `earnest verify` there. `@earnest/model` is
published with its `dist/` built by `pnpm pack`, and its tests include the same check for the
installed library.

The speed targets of the spec (the first app's 12 models in under 5 s in all, the slowest under
10 s alone) are measured on stand-in models, since the app's are not in this repository:

```sh
cargo build --release --locked -p earnest-cli
cargo run --release --locked -p earnest_benchmark -- --runs 5
```

It is not part of `just check`; `tasks/benchmark/README.md` holds the models, their state
counts derived by hand and the last measurement.

To run the checks before each commit, enable the hook once per clone:

```sh
git config core.hooksPath .githooks
```
