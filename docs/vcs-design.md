# Archsnap as a VCS for parallel agents — design (draft for review)

Status: proposal, nothing built yet.

## The two goals

1. **Small storage.** Ten agents must not cost ten copies of the repo, its `node_modules` and its
   build output. Disk use should grow with *what agents change*, not with *how many agents run*.
2. **Fearless parallelism.** An agent can do anything inside its workspace (delete files, rewrite
   history, run a bad migration) and nothing it does can hurt another agent, main, or your work.
   Every state is recoverable, and conflicts never stop an agent.

Everything else (structural diff, architecture checks, the weekly report) serves these two or comes
along for free from what Archsnap already does.

## Where Archsnap starts from

Archsnap already reads a TypeScript/JavaScript codebase as **areas, imports, symbols and a system
map**, reads git through plumbing (`ls-tree`, `cat-file --batch`), and caches per-commit scans. The
weekly HTML report stays, as `archsnap report` (`run` kept as an alias).

## Goal 1: small storage

### One shared content store

All workspaces sit on top of **one content-addressed store** (`~/.archsnap/store`): every file is
kept once, keyed by its hash, whether it comes from source, `node_modules` or build output. A
workspace is a thin layer that records only the files that agent changed.

| What | git worktrees, 10 agents | Archsnap, 10 agents |
|---|---|---|
| Source | 10 copies | 1 copy + each agent's edits |
| `node_modules` | 10 installs | 1 install, shared read-only |
| Build output | 10 builds | 1 warm build + each agent's rebuilt files |
| Snapshot history | n/a | only changed files, packed and deduplicated |

### How a workspace is made, by platform

| Platform | Mechanism | Cost per workspace |
|---|---|---|
| Linux (any FS) | overlayfs: shared read-only lower layer from the store, per-agent upper layer | only written files |
| Linux btrfs/XFS | reflink copy (`FICLONE`) as a simpler alternative | only written blocks |
| macOS (APFS) | `clonefile()` of a warm base directory | only written blocks |
| Later, any OS | virtual filesystem (FUSE / macOS FSKit) serving files straight from the store | only written files, instant create |

The virtual filesystem is the end state (it's what Google's CitC and Meta's EdenFS do), but it's
the riskiest part, so it comes last and the earlier mechanisms keep working underneath it.

### Keeping history small

- Snapshots store only changed files; the store packs them with delta compression, like git.
- Automatic snapshots are frequent, so they **thin out with age**: keep every snapshot for an hour,
  then one per minute for a day, then only snapshots that were landed, named or referenced by the
  operation log. `archsnap gc` enforces this.
- `archsnap du` shows where the space goes: per workspace, shared, and history.

## Goal 2: fearless parallelism

Guarantees, each backed by a mechanism:

| Guarantee | Mechanism |
|---|---|
| An agent can't touch another agent's files | Each workspace is its own layer; the shared store is read-only |
| An agent can't break main | Main only moves through the land queue, after checks pass |
| No work is ever lost, even mid-edit | A file watcher snapshots every change automatically; agents don't have to remember to save |
| Any mistake can be undone | Every operation is logged; `archsnap undo` / `archsnap restore <time>` |
| Agents don't collide at runtime | Each workspace gets its own port block and env overlay |
| Conflicts don't block | Merges are per symbol; unresolved ones become conflict objects, the agent keeps working |
| You can see what each agent did | Every change carries a structural diff and its architecture impact |

### Merge per symbol

- Different symbols changed in one file → clean merge.
- Import lists → set union, kept in the file's order.
- Same symbol changed on both sides → line merge inside the symbol; if that fails, a conflict object
  scoped to that symbol only. The rest of the file stays valid code.
- One side moved or renamed a symbol, the other edited it → the edit follows the symbol.
- After merging, rescan and check: new import cycles, broken imports, rules in `archsnap.toml`
  (e.g. `ui` may not import `db`). Warnings by default, blockers with `--strict`.

### The land queue

`archsnap land <ws>` queues the workspace's change. In order: rebase onto latest main (per-symbol
merge), run the checks from `archsnap.toml` (e.g. `test = "pnpm test"`) in that workspace, and move
main forward if they pass. Failures go back to the workspace with the output attached.

### Better diff

From one scan, three layers:

1. **Architecture** (exists today): areas, dependencies, packages, services, endpoints, queues.
2. **Symbols** (new): functions, classes, types, consts added, removed, changed (signature vs body),
   moved and renamed, detected by a hash of the normalised AST.
3. **Lines** (new): only inside changed symbols.

```
apps/api/src/prices
  + fn convertCurrency(amount, to)          new, exported
  ~ fn getRates()                           body changed (12 lines)
  → fn formatPrice  moved from packages/ui/src/format.ts
  architecture: apps/api/src/prices now depends on packages/fx
                new endpoint GET /api/fx-rates
```

## Git compatibility

Git stays readable and writable throughout: Archsnap imports from and exports to git, and landed
changes become ordinary git commits on your branch, so GitHub, CI and editors keep working. The
store is Archsnap's own (needed for goal 1), but git is always one `archsnap export` away, and in the
first milestones git remains the source of truth.

## CLI sketch

```sh
archsnap ws new fix-rates            # thin workspace from main: path, ports, env
archsnap ws list                     # workspaces, size on disk, changes, conflicts, queue
archsnap diff [a] [b]                # structural diff; --json for agents
archsnap land fix-rates              # queue → rebase → checks → main
archsnap resolve                     # list or resolve conflict objects
archsnap undo | restore <time>       # step back through the operation log
archsnap du | gc                     # where space goes; thin out old snapshots
archsnap report                      # today's weekly HTML report
```

## Code layout (proposed)

```
src/
  lib.rs
  store/        content-addressed store, packing, gc
  ws/           overlay.rs, reflink.rs, clonefile.rs, ports.rs, env.rs, watch.rs
  oplog.rs      operation log, undo, restore
  git.rs        existing plumbing; + import/export
  scan.rs       existing; + symbol table per file
  system.rs     existing
  diff/         arch.rs (today's diff.rs), symbols.rs, lines.rs
  merge/        symbols.rs, imports.rs, conflicts.rs, checks.rs
  queue.rs      land queue
  report/       today's report.rs + report.html
  main.rs       CLI
```

## Milestones (re-ordered for the two goals)

| | What | Goal it serves |
|---|---|---|
| M0 | Library split, `run` → `report`, e2e tests green | Safe base |
| M1 | Shared store + thin workspaces (one platform first) + ports/env + `du` | Storage, isolation |
| M2 | Watcher auto-snapshots + operation log + `undo`/`restore` + `gc` | Fearless: nothing lost |
| M3 | Symbol table + `archsnap diff` | Seeing what agents did |
| M4 | Per-symbol merge + checks + land queue + conflict objects | Fearless: main safe, never blocked |
| M5 | Virtual filesystem workspaces | Storage: instant, near-zero cost |

Each milestone ships with e2e tests against throwaway repos, as today. M1 and M4 also get
property tests: a workspace never changes the store; merging changes that touch different symbols
gives the same result in either order and never drops a symbol.

## Open questions

1. Which OS and filesystem do your agents run on? It decides M1's first mechanism.
2. Which agent runner should workspaces integrate with first (Claude Code, something else)?
3. cj-stack as the first repo to dogfood on: what's its GitHub owner/name?
4. Is TS/JS-only structural merge fine for v1, with line merge for everything else?
