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
- the largest model on one worker (`--jobs 1`) and on all cores, with the speed-up;
- the R12 comparison: the 12 mixed models and the largest model (`grid_1m`) in one
  `earnest verify`, against the longest of the 13 alone (`grid_1m` or `wide`, whichever takes
  longer). R12 asks for about 1x.

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

Last run: the release build of this commit, `--runs 7`.

Machine: a shared container with 4 cores visible to the process (`nproc` 4), 16 GB of memory and
Linux on x86-64, so other work on the same host adds noise to every number below. Rerun on the
machine whose numbers matter.

Cores: 4. Runs per model: 7 after one unmeasured run. Wall time includes starting the process and reading the files; peak memory is the largest resident set of any run.

### Scaled models

| Model | Language | States | Median wall time (s) | States per second | Peak memory (MiB) |
|---|---|---:|---:|---:|---:|
| Grid1k | TypeScript | 1,000 | 0.003 | 288,433 | 5.5 |
| Grid1k | Rust | 1,000 | 0.005 | 216,262 | 5.9 |
| Ledger | TypeScript | 36,400 | 0.089 | 409,310 | 17.4 |
| Ledger | Rust | 36,400 | 0.088 | 411,750 | 18.0 |
| Wide | TypeScript | 10,000 | 1.248 | 8,013 | 34.2 |
| Wide | Rust | 10,000 | 1.253 | 7,982 | 35.0 |
| Grid100k | TypeScript | 100,000 | 0.093 | 1,077,307 | 27.5 |
| Grid100k | Rust | 100,000 | 0.086 | 1,166,044 | 27.9 |
| Grid1m | TypeScript | 1,000,000 | 1.064 | 939,515 | 334.7 |
| Grid1m | Rust | 1,000,000 | 1.077 | 928,466 | 334.9 |

### The 12 mixed models, each alone

| Model | Language | States | Median wall time (s) | States per second | Peak memory (MiB) |
|---|---|---:|---:|---:|---:|
| counter100 | TypeScript | 100 | 0.002 | 43,047 | 4.6 |
| traffic | TypeScript | 30 | 0.002 | 12,998 | 4.5 |
| plane | TypeScript | 400 | 0.002 | 173,837 | 4.6 |
| basket | TypeScript | 16 | 0.002 | 6,968 | 4.4 |
| mailbox4 | TypeScript | 31 | 0.002 | 13,489 | 4.4 |
| trail3 | TypeScript | 40 | 0.002 | 17,316 | 4.5 |
| shelf | TypeScript | 64 | 0.002 | 27,741 | 4.5 |
| seats | TypeScript | 18 | 0.002 | 7,812 | 4.4 |
| votes | TypeScript | 21 | 0.002 | 9,126 | 4.5 |
| pairs | TypeScript | 120 | 0.002 | 51,791 | 4.5 |
| ledger | TypeScript | 36,400 | 0.089 | 409,310 | 17.4 |
| wide | TypeScript | 10,000 | 1.248 | 8,013 | 34.2 |

### Spec targets

- 12 models together: 1.333 s, target under 5 s (a stand-in: these benchmark models, not the first app's 12)
- slowest model alone: 1.248 s, target under 10 s (a stand-in: these benchmark models, not the first app's)
- the 12 models in that one run: 47,240 states, peak memory 47.7 MiB

### One large model on one worker and on all cores

- grid_1m: 1 worker 2.619 s, 4 workers (all cores) 1.064 s, speed-up 2.46x

### R12: many models and one large model in one run

- the 13 models in one run: 2.226 s; the largest model (grid_1m) alone: 1.064 s; the slowest of the 12 alone (wide): 1.248 s
- R12 asks for about as long as the largest model alone: the one run took 1.78x the longest of them alone (wide)
- peak memory 363.9 MiB

### Before and after the thread-sharing change (R12)

Four whole runs of the benchmark on the same container, two of the build before the change and
two of this one, taken one after the other (`--runs 7` each; the medians of each run):

| | Before (2 runs) | After (2 runs) |
|---|---|---|
| grid_1m, 1 worker | 4.1 s and 4.6 s | 2.6 s and 2.6 s |
| grid_1m, all 4 cores | 2.4 s and 2.9 s | 1.1 s and 1.6 s |
| the 12 models in one run | 4.5 s and 4.5 s | 1.3 s and 2.0 s |
| the 12 and grid_1m in one run | 4.6 s and 4.5 s | 2.2 s and 2.3 s |
| that run over the longest of the 13 alone | 1.6x and 1.9x | 1.8x and 1.3x |

Two earlier before-runs, taken while other work ran on the host, gave 4.9 s to 5.6 s for the 13
models in one run, up to 2.6x. The spread between runs on this host is up to 2x, so the ratios
above can only be read as a range. What does not move with the noise: the 13 models hold about
7 CPU seconds of work (`wide` about 4.5 and `grid_1m` about 2.5), so four cores can't finish them
in less than about 1.8 s, which is 1.3x to 1.7x the longest of them alone. R12's "about as long
as the largest model alone" holds for a set in which the others are small next to the largest;
in this set `wide` is a second large model.
