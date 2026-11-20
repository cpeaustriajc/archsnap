# Limits of the current build

Written on 2026-11-20, for version 1 on the branch `claude/optimistic-knuth-3l0iq8`. This page says
what earnest does not do, what has been checked and how far that evidence goes. The contract is in
the [README](../README.md); the open items are in
[issue 10](https://github.com/cpeaustriajc/earnest/issues/10).

## What has been checked

| Check | Result | What it does not show |
|---|---|---|
| `just check` in CI | Passes: 1,287 Rust tests, 146 vitest tests, Verus, and `earnest verify` on earnest's own three models | That the tests cover every model a user can write |
| TLC 1.8.0 in CI (`just tlc`) | `same` state counts and verdicts for every conformance, twin and agreement model, and for earnest's own models | Agreement on models outside that set |
| Six adversarial passes | 61 faults found and fixed, each now a test in `tasks/break_it` | That a seventh pass would find nothing |
| A spec audit, requirement by requirement | R1 to R21 are built; the gaps it found are closed or listed below | The speed target, which needs models this repository does not hold |
| Earnest checked with earnest (`tasks/models`) | Six shortfalls found in the thread pool, the cache and the run outcomes; all six fixed | That the code matches the models: nothing ties a model to the code it describes |
| A first-time user test | A small coding agent, given only the README, wrote a TypeScript model, a fixed copy and a Rust twin, each right on the first run | How a first-time user fares on a large model |

### The adversarial passes

Each pass derives the right answer by hand, runs the real binary, and compares it with Node, with
the model compiled as real Rust, and with TLC.

| Pass | Faults | Reported a broken model as passing | Where most faults were |
|---|---|---|---|
| 1 | 8 | 2 | Rules dropped without a word; a constant replaced by an input |
| 2 | 7 | 3 | A file left out; shadowed names |
| 3 | 11 | at least 4 | Renamed imports; numbers past 2^53; files hidden from discovery |
| 4 | 14 | 2 | A declaration made twice; imports through another file |
| 5 | 12 | 1 | Rust views, `Self` actions and the test-file rule, all new that round |
| 6 | 9 | 1 | Rust patterns, moves and types, all new that round |

No pass has come back clean. Two things have held since the second pass. The search itself has
given no wrong answer: every fault since then is in how a file is found or read. And the output is
the same on every run and for every `--jobs` value, tested on up to 203 models at once.

## Limits by design

- **Safety only.** Earnest checks rules that must hold in every reachable state, stuck states and
  faults. It has no liveness or fairness checks ("this eventually happens"). A model with no cycle
  and no stuck state reaches its end on every run; that is the only way to state "it finishes".
- **Finite, declared data.** Every field and input has a declared finite range. Earnest visits
  every reachable state and holds them in memory: `Grid1m`, with 1,000,000 states, takes about
  330 MiB.
- **TLA+ goes one way.** Earnest writes TLA+ and does not read it.
- **TLC cannot confirm every model.** TLC holds numbers to 32 bits and counts some texts above
  ASCII as equal. For such a model `earnest confirm` prints `cannot` and the reason.
- **A model is not tied to the code.** Nothing checks that the app does what its model says. The
  spec puts model-to-code tests and replay tests after version 1.

## Where the reader can still be wrong

A reader must give a file the meaning its language gives it, or refuse it. The search can only be
as right as that reading.

- **The Rust reader imitates part of `rustc`.** It has to know which names a pattern binds, which
  values a line moves, and which types two values have. Each pass since Rust models gained
  `match`, `if let`, `?`, conversions and views found Rust that `rustc` rejects and the reader
  accepted, and in pass 6 one case where it read valid Rust with another meaning. The fixes now
  state general rules for patterns, moves and types. If those still leak, the choices are to
  narrow what a Rust model may contain, or to compile the file with the project's own toolchain
  before reading it. The second bends the requirement that earnest needs no setup.
- **Two records with the same fields are one type after reading.** The Rust reader tells them
  apart; the step after it does not. A mix-up the reader cannot type is not caught.
- **A set of texts is walked in three orders.** The checker, Node and Rust each take the items of
  a set of texts in a different order. A `forall` or `exists` over such a set, whose test can
  fault on a later item, can be `unknown` in the checker and `holds` in Node.
- **The TypeScript reader accepts less, and has been quiet for two passes.** A model is calls to
  the library, and anything else is refused. The price is that ordinary TypeScript is refused:
  `%`, `/`, postfix `!`, `as`, and a rule that is not an exported `const`.
- **Node cannot see one fault.** A number computed past 2^53 - 1 inside a rule, an end or a `when`
  is a fault in `earnest verify`. Node's `explore` and `replay` compute with the rounded value.
  `earnest verify` is the authority.
- **A TypeScript file that uses the library and declares no model** is still treated as a model
  file outside a test folder, and reported unreadable. Only Rust files have the stricter rule.
- **A file that takes the library from another local file** is not found on its own by a folder
  run. The run still fails, because the file that passes the library on is refused.

## Speed

- A simple model runs at roughly 300,000 to 1,000,000 states a second on four cores of a shared
  container, where timings move by up to two times between runs.
- **Inputs multiply.** Every action's inputs are tried in every combination in every state. The
  `Cache` model in `tasks/models` has 394 states and 1,440 input combinations, and takes about
  8 seconds. The evaluator interprets the model and is some 30 to 50 times slower than the same
  model compiled as Rust.
- **Parallel speed-up is partial.** One model of 1,000,000 states is about twice as fast on four
  cores as on one. A run of several models now takes about as long as its largest model, measured
  on `tasks/models`; with two large models in one run it takes longer.
- **The spec's speed target is not measured.** It is set on the first app's 12 models, which are
  not in this repository. `tasks/benchmark` is a stand-in: its 12 models take about 2.5 seconds.

## Packaging and platforms

- **Nothing is published.** `@earnest/cli` and `@earnest/model` install from a packed tarball in
  the tests, not from a registry. From source: `cargo install --path apps/earnest`.
- **Only linux-x64 was built and run.** The macOS, Windows and linux-arm64 packages of
  `@earnest/cli` are manifests with no binary. Windows paths are untested.
- **The docs site does not exist.** This page and the READMEs are the documentation.

## Not verified

- **Mermaid on GitHub.** The charts are valid by the tests' reading of the text. No test renders
  one, and whether GitHub draws a large chart readably was never looked at.
- **tla-rs.** `earnest confirm --tla-rs` was built against the crate `tla-tools` and reported to
  agree on the conformance models. That run was not repeated, and several projects carry the name,
  so it is not certain this is the one the spec means.
- **Earlier local TLC runs** used a nightly build of TLC. The CI runs use the 1.8.0 release.

## Friction for the agent writing a model

- **A linter may warn on every action.** oxlint's default rule `unicorn/no-thenable` flags the
  `then` key. A project that fails on warnings rejects every TypeScript model until the rule is
  turned off for model files or the key is renamed, which touches about 246 uses.
- **This repository's own lint table is stricter than most.** It forbids a `bool` parameter, so a
  Rust model here uses a two-variant enum where the API would take a `bool`. The README's "Lints"
  section lists the forms that satisfy both clippy and the reader.
- **Rust has no `div_ceil`,** no arrays indexed by a number, and no start value that depends on
  another field.
- **Five older Rust fixtures are not rustfmt-clean.** A test pins the list.
- **A wide state prints a wide path.** Each step shows the fields that changed and the path ends
  with the full state; a model with large records gives lines of a thousand characters.

## Decisions that changed the spec

- The engine is six crates, not one (Decision 001 in the dossier).
- The readers stay on oxc and `syn`; tree-sitter was measured and not used (Decision 002).
- TypeScript models write `m.rem`, `m.div` and `m.some` where JavaScript has `%`, `/` and `!`,
  so that Node and the checker report the same faults.
- A rule that was not judged in every counted state is `unknown`, not `holds`.
- A file named as a test, or under a tests folder, is not a model file. The run says how many it
  skipped.
