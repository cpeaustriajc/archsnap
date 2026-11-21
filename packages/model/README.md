# @earnest/model

The TypeScript library that models for the `earnest` model checker are written with. A model file
imports `m` from it and declares fields, actions, rules and ends. `earnest verify` reads the file
without running it; this library makes the same file type-check under strict `tsc`, with every
type inferred from the declarations, and run as ordinary code in Node.

The repository's `README.md` is the contract: what a model file may contain, every primitive and
its operations, and the JSON shapes used below.

## Installing

```sh
npm install --save-dev @earnest/model
```

The published package holds compiled JavaScript and declarations in `dist/`, so it runs in Node
and type-checks under a strict `tsc` in a project that installs it. In this repository `exports`
points at `src/index.ts`, which Node runs by stripping types, and `publishConfig.exports` points
at `dist/` for the tarball that `bun pm pack` makes; `bun run build` emits it, and `prepack` runs
the build and swaps `publishConfig.exports` into `exports` for the tarball (`postpack` restores
`package.json`), since bun does not apply `publishConfig` itself. `test/installed.test.ts` packs the library, installs the tarball into a fresh project
and runs `explore`, `replay` and a strict type check there.

## Writing a model

```ts
import { m } from "@earnest/model";

export const Turnstile = m.model("Turnstile", {
  gate: m.enum(["locked", "open"]).initial("locked"),
  paid: m.boolean().initial(false),
  visitors: m.int(0, 3).initial(0),
});

export const pay = m.action(Turnstile, "the visitor pays", {
  when: (s) => !s.paid,
  changes: () => ({ paid: true }),
});

export const push = m.action(Turnstile, "the visitor pushes", {
  when: (s) => s.paid && s.visitors < 3,
  changes: (s) => ({ gate: "open", visitors: s.visitors + 1 }),
});

export const staysLocked = m.rule(
  Turnstile,
  "The gate stays locked until the visitor pays",
  (s) => s.gate === "locked" || s.paid,
);

export const full = m.end(Turnstile, "Three visitors went through", (s) => s.visitors === 3);

export type State = m.Infer<typeof Turnstile>;
```

- Nothing is annotated. `s` is the state type the fields declare, an action's second parameter is
  typed from its `input`, and `changes` gives back only the fields that change. A wrong literal
  (`gate: "ajar"`), a field the model does not have, or a value of the wrong kind in `changes` is
  a compile error. In an action, `input`, `when` and `changes` may be written in any order.
- `m.action`, `m.rule`, `m.end` and `m.view` take the model as their first argument. A rule or an
  end then has its sentence and a condition. An action has its sentence and an object with
  `input`, `when` and `changes`. A view has an object.
- Declarations are plain values. `.optional()` and `.initial(value)` give back a new declaration,
  so one `const Press = m.enum([...])` can be a field in two models and an input of an action.
- State is read-only, and every operation on a set, map, sequence, queue, lock, timer, request or
  retry gives back a new value.
- `m.Infer<typeof Model>` is the state type, and `m.Infer<typeof Declaration>` the type of a
  declaration's values. `m` is a module namespace (`export * as m`), so `m.Infer` is an ordinary
  exported type and nothing in the library needs more than type stripping to run.

## The builders

| Builder                     | Values                                           | Starts at, without `.initial` |
| --------------------------- | ------------------------------------------------ | ----------------------------- |
| `m.boolean()`               | `false`, `true`                                  | every value                   |
| `m.int(lo, hi)`             | whole numbers from `lo` to `hi`                  | every value                   |
| `m.enum([...])`             | the listed texts                                 | every value                   |
| `m.oneOf([...])`            | the listed texts, numbers or booleans            | every value                   |
| `.optional()`               | `null`, then the values of the declaration       | every value                   |
| `m.record({...})`           | an object with exactly those fields              | every value                   |
| `m.set(T)`                  | `SetValue<T>`                                    | the empty set                 |
| `m.map(K, V)`               | `MapValue<K, V>`                                 | the empty map                 |
| `m.seq(T, { max })`         | `SeqValue<T>` of at most `max` items             | the empty sequence            |
| `m.queue(T, { capacity })`  | `QueueValue<T>` of at most `capacity` items      | the empty queue               |
| `m.lock()`, `m.lock(Owner)` | `LockValue`, `OwnedLockValue<Owner>`             | free                          |
| `m.timer()`                 | `TimerValue`: idle, running, fired               | idle                          |
| `m.request()`               | `RequestValue`: idle, pending, succeeded, failed | idle                          |
| `m.retry({ max })`          | `RetryValue` from zero to `max` attempts         | zero                          |

A declaration that makes no sense throws when it is written: bounds the wrong way round, a
value listed twice in `m.enum` or `m.oneOf`, an optional of an optional, and a lock with an
owner as a map value or a sequence or queue item, whose free state `null` could not be told from
a missing entry. `earnest verify` refuses the same.

`m.rem(a, b)` is the remainder of whole numbers: `%` is not allowed in a model file, and `m.rem` of a
negative `a` or a `b` below 1 is a fault that `explore` and `replay` report as `earnest verify`
does. `m.div(a, b)` is the whole number of times `b` goes into `a`, as Rust's `/` on integers
(`m.div(7, 2)` is 3): `/` is not allowed in a model file, since JavaScript's gives a fraction, and
`m.div` of a negative `a` or a `b` below 1 is the same kind of fault, "takes a division with a
negative number or a divisor below 1". `m.some(value)` is the value inside an optional: `%`'s sibling for `!`, and for `as`, `satisfies` and `<Type>value`, which a model
file may not write (only `as const` is read). `m.some(null)` is a fault that `explore` and `replay` report as `earnest
verify` does ("reads a value that is null"), where JavaScript would go on with `null` as 0, so
a number read from an optional goes through it. `m.setOf(a, b)`, `m.seqOf(a, b)` and `m.mapOf([k, v])` write collection values; a queue's
`.initial` takes `m.seqOf(...)`. `m.forall(T, f)` and `m.exists(T, f)` range over a declaration
or a collection. Every declaration has `values()`, which lists what it declares in the order the
checker tries them, `has(value)`, `starts()`, `count` and `fromJson(json)`.

## Running a model in Node

```ts
import { explore, replay } from "@earnest/model";
import * as turnstile from "./turnstile.model.ts";

const found = explore(turnstile.Turnstile, { exports: turnstile });
```

`explore` searches breadth first and gives
`{ status, states, rules: [{ id, says, verdict, path }], findings: [{ kind, message, path }], reason }`
with the words and the path shape of `earnest verify --json`: `pass`, `fail` or `unfinished`;
`holds`, `broken` or `unknown`; `stuck`, `out-of-range` or `fault`. Each broken rule has the
shortest path to a state that breaks it. A rule is `unknown` when it was not seen broken but
the search is unfinished or the rule could not be evaluated in a state. `findings` has at most
one of each kind. `reason` is `null`, or why the search is unfinished.

- The id of an action, a rule or an end is the name it is exported under, and a running program
  can't see that name, so pass the model file's exports as `exports`. A part that is not among
  them has its sentence as its id.
- `maxStates` stops the search before it holds more distinct states than that; the default is
  5,000,000. Every state that is counted has its rules judged, also the states the limit kept
  the search from going on from, so a rule one of them breaks is `broken` with its path. No
  action is tried from those states.
- The search follows the checker's order, so the two results can be compared: start states in
  field order with the first field varying slowest, actions in declaration order, inputs with
  the first input varying fastest, and states judged in the order they were found.
- Anything the model asks that can't be evaluated in a state is a `fault` there: a rule, the
  condition or the change of an action, or the `when` of an end. A state is `stuck` only when the
  condition of every action is false and every end is false, so a state where a condition or an end
  faults is not stuck. An end is asked only in a state where no action can happen.
- A state that is out of range is counted, is not checked against the rules and is not searched
  from.

`replay(Model, path, { exports })` applies a `{ start, steps }` path one action at a time and
gives every state on the way, the start state first, as objects in JSON form. It throws an error
that names the step when the path does not begin at a start state, a step names no action, an
input is missing or undeclared, the action's `when` is false, the action can't be evaluated, or
the state after the action differs from the step's `state`. Without `exports` a step is matched
by its `label`, the action's sentence.

## Where Node and the checker differ

- Arithmetic past 2^53 - 1 is a fault in the checker, "computes a number too large to hold",
  wherever a model computes it. In Node the library sees only the numbers it is given back: a
  next state that holds one is the same fault in the change of its action. A number computed
  inside a rule, an end or a `when`, or by a helper on the way to a value that is not a number,
  is the rounded value JavaScript computes, so `s.n + 9007199254740992 - 9007199254740992 ===
  s.n` is judged by what JavaScript makes of it. `m.int` throws on a bound outside the safe
  integers, and `m.oneOf` on a number outside them or one that is not whole, as the checker
  refuses the same file.
- `===` on two records compares the objects in Node. Compare records field by field. Sets, maps,
  sequences, queues, locks, timers, requests and retries are shared by content, so `===` on them
  compares content.
- Set members and map keys are walked and printed in one fixed order, the checker's and the Rust
  library's: `null`, booleans, numbers, text by code point (`compareText`, which puts a character
  past U+FFFF after one from U+E000 to U+FFFF, where `<` does not), records field by field in the
  order of the field names, collections item by item. A quantifier over a set stops at the first
  item in that order. `replay` still accepts the members of a set in any order. The README of the
  repository has the rule for each kind.
- A fault is a `TypeError` thrown by the model, `m.rem` of a negative number, or `m.some(null)`:
  reading a field of `null` or `m.some(null)` reports "reads a value that is null", any other
  `TypeError` reports "uses a value of the wrong kind". Where a model file reads a number from
  an optional it must use `m.some`, because JavaScript would give `null < 3` as true and `null +
  1` as 1; the checker refuses the postfix `!` that would hide it.
- The library does not check that a file keeps to what a model file may contain. `earnest verify`
  does.

## Development

From this folder, after `bun install --frozen-lockfile` at the repository root:

```sh
bun run typecheck   # tsrs --noEmit, over src/, scripts/ and test/
bun run format      # oxfmt --check
bun run format:fix  # oxfmt, rewriting the files in place
bun run test        # vitest run
```

- The package has no runtime dependencies and no build step. Node 22.18 or newer runs the `.ts`
  files by stripping types, so the source uses only erasable syntax.
- The devDependencies are pinned to exact versions: `tsrs` (the Rust port of the TypeScript
  checker), `oxfmt` (the formatter) and `vitest`, with `@types/node` for the tests. The lockfile
  is `bun.lock` at the repository root.
- `typecheck` is one project: `tsconfig.json` covers `src/`, `scripts/` and `test/` and excludes
  `test/rejected/`. It sets `types` to `node` so the tests can import `node:` modules.
- `test/types.ts` only has to compile: it holds the type-level proofs, such as what `m.Infer`
  gives and which values `changes` rejects.
- `test/rejected/` holds one program for each mistake the compiler must catch, and `accepted.ts`,
  which it must not complain about. `test/rejected.test.ts` runs `tsrs` on the folder and checks
  that each bad program gets exactly one error, on the expected line. The folder is excluded from
  the package's own `tsconfig.json`.
