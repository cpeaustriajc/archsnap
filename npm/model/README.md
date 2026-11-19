# @earnest/model

The TypeScript library that models for the `earnest` model checker are written with. A model file
imports `m` from it and declares fields, actions, rules and ends. `earnest verify` reads the file
without running it; this library makes the same file type-check under strict `tsc`, with every
type inferred from the declarations, and run as ordinary code in Node.

The repository's `README.md` is the contract: what a model file may contain, every primitive and
its operations, and the JSON shapes used below.

## Writing a model

```ts
import { m } from "@earnest/model";

export const Turnstile = m.model("Turnstile", {
  gate: m.enum(["locked", "open"]).initial("locked"),
  paid: m.boolean().initial(false),
  visitors: m.int(0, 3).initial(0),
});

export const pay = Turnstile.action("the visitor pays", {
  when: (s) => !s.paid,
  then: () => ({ paid: true }),
});

export const push = Turnstile.action("the visitor pushes", {
  when: (s) => s.paid && s.visitors < 3,
  then: (s) => ({ gate: "open", visitors: s.visitors + 1 }),
});

export const staysLocked = Turnstile.rule({
  says: "The gate stays locked until the visitor pays",
  holds: (s) => s.gate === "locked" || s.paid,
});

export const full = Turnstile.end({
  says: "Three visitors went through",
  when: (s) => s.visitors === 3,
});

export type State = m.Infer<typeof Turnstile>;
```

- Nothing is annotated. `s` is the state type the fields declare, an action's second parameter is
  typed from its `input`, and `then` gives back only the fields that change. A wrong literal
  (`gate: "ajar"`), a field the model does not have, or a value of the wrong kind in `then` is a
  compile error. In an action, write `input` before `when` and `then`.
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

`m.setOf(a, b)`, `m.seqOf(a, b)` and `m.mapOf([k, v])` write collection values; a queue's
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
  `when` or the `then` of an action, or the `when` of an end. A state is `stuck` only when the
  `when` of every action is false and every end is false, so a state where a `when` or an end
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

- `%` with a negative left side, or with a negative right side, is a fault in the checker. In
  Node the operator can't be intercepted, so it gives JavaScript's result. Only `%` by zero is
  seen, because it gives `NaN`, and is reported as the same fault.
- `===` on two records compares the objects in Node. Compare records field by field. Sets, maps,
  sequences, queues, locks, timers, requests and retries are shared by content, so `===` on them
  compares content.
- Set members and map keys are printed in one fixed order: `null`, booleans, numbers, text in
  code-unit order, records, collections. The checker's order may differ; compare sets as sets.
  `replay` does.
- A fault is a `TypeError` thrown by the model: reading a field of `null` reports "reads a value
  that is null", any other reports "uses a value of the wrong kind".
- The library does not check that a file keeps to what a model file may contain. `earnest verify`
  does.

## Development

From this folder, after `pnpm install --frozen-lockfile`:

```sh
pnpm run typecheck   # tsrs --noEmit, over src/ and test/
pnpm run format      # oxfmt --check
pnpm run format:fix  # oxfmt, rewriting the files in place
pnpm test            # vitest run
```

- The package has no runtime dependencies and no build step. Node 22.18 or newer runs the `.ts`
  files by stripping types, so the source uses only erasable syntax.
- The devDependencies are pinned to exact versions: `tsrs` (the Rust port of the TypeScript
  checker), `oxfmt` (the formatter) and `vitest`, with `@types/node` for the tests. The lockfile
  is `pnpm-lock.yaml`.
- `typecheck` is one project: `tsconfig.json` covers `src/` and `test/` and excludes
  `test/rejected/`. It sets `types` to `node` so the tests can import `node:` modules.
- `test/types.ts` only has to compile: it holds the type-level proofs, such as what `m.Infer`
  gives and which values a `then` rejects.
- `test/rejected/` holds one program for each mistake the compiler must catch, and `accepted.ts`,
  which it must not complain about. `test/rejected.test.ts` runs `tsrs` on the folder and checks
  that each bad program gets exactly one error, on the expected line. The folder is excluded from
  the package's own `tsconfig.json`.
