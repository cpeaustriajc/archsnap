# Benchmark

A repeatable speed measurement of the release `earnest verify`. The spec sets two targets: the
first app's 12 models in under 5 s in all, and the slowest of them under 10 s alone. Those 12
models are not in this repository, so this task holds fixed models that stand in for them, and
the numbers below say how the checker does on those, not on the app's.

```sh
cargo build --release --locked -p earnest-cli
cargo run --release --locked -p earnest_benchmark -- --runs 5
```

`--runs <n>` is the number of measured runs of each case (default 5, after one run that is not
counted) and `--binary <path>` the `earnest` to measure (default `target/release/earnest`). The
binary runs `earnest verify --no-cache --json` in an empty temporary folder, so nothing is served
from a cache and nothing is written to the repository, and prints Markdown:

- the scaled models in TypeScript and in Rust, with their median wall time, states per second
  and peak memory;
- the 12 mixed models, each alone;
- the two spec lines, marked as a stand-in, with the 12 models checked together in one
  `earnest verify`;
- the largest model on one worker (`--jobs 1`) and on all cores, with the speed-up.

Wall time is the time from starting the process to its exit, so it includes reading the files.
Peak memory is the largest resident set (`VmHWM` in `/proc`) any run reached; it is `n/a` where
`/proc` has no such file. The binary fails, with the model and both numbers, when the checker's
state count for a model differs from the one derived below, when a model is not `pass`, or when
a file is unreadable. It is not part of `just check`; that runs only its unit tests, which also
check that every derived count below is in this file.

## Models and their state counts

The files are in `models/`. Each scaled model is also in Rust (`.rs` beside the `.model.ts`),
with the same names, fields and actions, and the same count. Every count is derived by hand from
the model and the contract in the root `README.md`, never from the checker.

| Model | File | States | Why that many states |
|---|---|---|---|
| `Grid1k` | `grid_1k` | 1,000 | Three wheels `a`, `b`, `c`, each 0 to 9, each turned up one at a time by its own action and never down. Every combination is reachable from (0, 0, 0): 10 × 10 × 10 = 1,000. Only (9, 9, 9) has no action and it is a marked end |
| `Grid100k` | `grid_100k` | 100,000 | The same with five wheels: 10^5 |
| `Grid1m` | `grid_1m` | 1,000,000 | The same with six wheels: 10^6. This is the largest model |
| `Ledger` | `ledger` | 36,400 | A queue of capacity 5 over 3 letters and a record `{a, b}` of two numbers 0 to 9. A queue holds 0 to 5 letters, 3^k contents of k letters: 1 + 3 + 9 + 27 + 81 + 243 = 364. The record takes 10 × 10 = 100 values. Deliveries and reads change only the queue and the two bumps only the record, so every pair is reachable: 364 × 100 = 36,400. An empty queue can take a letter and any other can be read, so nothing is stuck |
| `Wide` | `wide` | 10,000 | `x` and `y` each 0 to 99. One action takes three inputs `a`, `b`, `c` of 0 to 9, 1,000 combinations in every state, and moves `x` to `(x + a) mod 100` and `y` to `(y + b + c) mod 100`. Choosing `a = 1` and `b = 1, c = 0` steps each by one, so each reaches every value, and `a` and `b + c` are chosen independently: 100 × 100 = 10,000. The action is always allowed, so nothing is stuck. The checker evaluates it 10,000 × 1,000 = 10^7 times |

The 12 mixed models that stand in for the app's 12 are the next ten (in `models/mixed/`) and
`Ledger` and `Wide` above. All are TypeScript.

| Model | File | States | Why that many states |
|---|---|---|---|
| `Counter` | `mixed/counter100` | 100 | `n` goes 0 to 99 one at a time, and 99 is a marked end. 100 |
| `Traffic` | `mixed/traffic` | 30 | `light` cycles through 3 values with an always allowed action, and `cars` goes 0 to 9 on its own. 3 × 10 = 30 |
| `Plane` | `mixed/plane` | 400 | `x` and `y` each go 0 to 19 on their own. 20 × 20 = 400. (19, 19) is a marked end |
| `Basket` | `mixed/basket` | 16 | Subsets of 4 items, each added when absent and removed when present: 2^4 = 16 |
| `Mailbox` | `mixed/mailbox4` | 31 | A queue of capacity 4 over 2 letters: 1 + 2 + 4 + 8 + 16 = 31 |
| `Trail` | `mixed/trail3` | 40 | A sequence of at most 3 over 3 stops, pushed to below 3 items and popped above 0: 1 + 3 + 9 + 27 = 40 |
| `Shelf` | `mixed/shelf` | 64 | A map from 3 items to a number 0 to 2. Each key is absent or maps to one of 3 numbers, so 4 values, and the action that sets and the one that deletes reach every combination: 4^3 = 64 |
| `Seats` | `mixed/seats` | 18 | `seat` has no initial value, so it starts at every record `{row 1 to 3, aisle}`: 3 × 2 = 6. `owner` has none either and starts at `null`, `Kim` or `Lee`: 3. Every combination is a start state and no action leaves them. 6 × 3 = 18 |
| `Votes` | `mixed/votes` | 21 | `Idle` (0, 0): 1. `Voting` with every `(yes, no)` of sum at most 3: 1 + 2 + 3 + 4 = 10. `Done`, closed from each of them: 10. 1 + 10 + 10 = 21 |
| `Pairs` | `mixed/pairs` | 120 | `(a, b)` of 0 to 14 with `a <= b`, reached by raising `b` and then `a` up to it: 15 + 14 + ... + 1 = 120 |

## Results

Last run: the release build of this commit, `--runs 5`.

Machine: a shared container with 4 cores visible to the process (`nproc` 4), 16 GB of memory and
Linux on x86-64, so other work on the same host adds noise to every number below. Rerun on the
machine whose numbers matter.

Cores: 4. Runs per model: 5 after one unmeasured run. Wall time includes starting the process and reading the files; peak memory is the largest resident set of any run.

### Scaled models

| Model | Language | States | Median wall time (s) | States per second | Peak memory (MiB) |
|---|---|---:|---:|---:|---:|
| Grid1k | TypeScript | 1,000 | 0.003 | 294,117 | 5.5 |
| Grid1k | Rust | 1,000 | 0.003 | 292,568 | 5.7 |
| Ledger | TypeScript | 36,400 | 0.121 | 299,766 | 18.9 |
| Ledger | Rust | 36,400 | 0.144 | 252,254 | 19.3 |
| Wide | TypeScript | 10,000 | 2.443 | 4,092 | 39.4 |
| Wide | Rust | 10,000 | 2.417 | 4,137 | 39.7 |
| Grid100k | TypeScript | 100,000 | 0.185 | 541,747 | 31.2 |
| Grid100k | Rust | 100,000 | 0.205 | 488,011 | 32.8 |
| Grid1m | TypeScript | 1,000,000 | 3.536 | 282,779 | 281.7 |
| Grid1m | Rust | 1,000,000 | 2.989 | 334,544 | 282.0 |

### The 12 mixed models, each alone

| Model | Language | States | Median wall time (s) | States per second | Peak memory (MiB) |
|---|---|---:|---:|---:|---:|
| counter100 | TypeScript | 100 | 0.002 | 43,159 | 3.8 |
| traffic | TypeScript | 30 | 0.002 | 12,925 | 4.4 |
| plane | TypeScript | 400 | 0.002 | 172,637 | 4.2 |
| basket | TypeScript | 16 | 0.002 | 6,899 | 4.7 |
| mailbox4 | TypeScript | 31 | 0.002 | 13,356 | 4.5 |
| trail3 | TypeScript | 40 | 0.002 | 17,233 | 4.7 |
| shelf | TypeScript | 64 | 0.002 | 27,234 | 3.7 |
| seats | TypeScript | 18 | 0.002 | 7,407 | 5.3 |
| votes | TypeScript | 21 | 0.002 | 8,581 | 5.1 |
| pairs | TypeScript | 120 | 0.002 | 49,403 | 5.1 |
| ledger | TypeScript | 36,400 | 0.121 | 299,766 | 18.9 |
| wide | TypeScript | 10,000 | 2.443 | 4,092 | 39.4 |

### Spec targets

- 12 models together: 2.459 s, target under 5 s (a stand-in: these benchmark models, not the first app's 12)
- slowest model alone: 2.443 s, target under 10 s (a stand-in: these benchmark models, not the first app's)
- the 12 models in that one run: 47,240 states, peak memory 48.2 MiB

### One large model on one worker and on all cores

- grid_1m: 1 worker 4.761 s, 4 workers (all cores) 3.536 s, speed-up 1.35x

The same build measured earlier on the same container gave `Wide` 1.330 s (so the 12 models
together 2.486 s) and 2.17x for `Grid1m` on all cores against one worker, so on this host the
times move by a factor of almost two between runs, and the speed-up by more. Read the targets
with that spread: the 12 stand-in models together took 2.5 s against a target of 5 s, and the
slowest alone 1.3 s to 2.4 s against 10 s. One state of `Wide` costs 1,000 evaluations of its
action, so it is the slowest per state by far; the 12 together are dominated by it. Every other
model of the 12 takes about 2 ms, which is the cost of starting the process.
