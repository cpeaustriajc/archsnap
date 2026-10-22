# Archsnap as a VCS for parallel agents — design (draft for review)

Status: proposal, nothing built yet.

## Idea in one paragraph

Archsnap already understands a TypeScript/JavaScript codebase as **areas, imports, symbols and a
system map**, and already reads git history through plumbing (`ls-tree`, `cat-file --batch`). The
new Archsnap keeps that and adds version control on top: many agents work in parallel in cheap,
isolated **workspaces**, every change is **snapshotted automatically**, changes are **diffed and
merged by structure** (symbols, imports, areas) instead of by lines, and every merge says what it
did to the architecture. The weekly report stays, as `archsnap report`.

## Why not just git worktrees

| Worktree pain | Archsnap answer |
|---|---|
| Each worktree needs its own `node_modules` / build | Workspaces are copy-on-write clones of a warm base, build artifacts included |
| Ports, DBs and env files collide between agents | Each workspace gets its own port range and env overlay |
| Two agents touching one file conflict on merge | Merge per symbol: different functions in one file merge cleanly; import lists merge as sets |
| Conflicts block the agent | Conflicts are stored as objects; the agent keeps working and resolves later |
| Reviewing agent output means reading line diffs | Every change comes with an architecture diff: new dependencies, new endpoints, moved code |

## Principles

1. **Git stays the source of truth (v1).** Archsnap writes commits and refs into the same git
   object store, under `refs/archsnap/*`. It never rewrites your branches. If Archsnap breaks, `git`
   still has everything. Own storage is a later, optional step (M5).
2. **Structure first, lines as fallback.** TS/JS files get symbol-level diff and merge via oxc
   (already a dependency). Every other file gets an ordinary line diff and 3-way merge.
3. **Never lose work.** Every operation is a snapshot first; there is no command that discards
   state without an undo (`archsnap undo` walks the operation log).
4. **Built for agents, readable for humans.** Every command has `--json` output and stable exit
   codes; every result also has a plain-words summary, as the report does today.

## Concepts

- **Snapshot** — an immutable state of a workspace (a git commit under `refs/archsnap/snaps/<ws>/*`)
  plus its scan (areas, edges, symbols, system), cached like today's `snapshots/*.json`.
- **Workspace** — a directory an agent works in: a CoW clone of a base snapshot, with its own
  ports/env. `archsnap snap` (called by an agent hook after each edit or tool call) records it.
- **Change** — a stable id for one unit of work that survives rebases (like jj change ids), so an
  agent's work keeps its identity while being moved onto a new main.
- **Conflict object** — when a merge can't be resolved, both sides and the base are recorded on the
  symbol (not the whole file). The file stays valid code with markers only inside that symbol.
- **Operation log** — every command appends an entry; `archsnap undo` restores the previous state.

## Better diff

Three layers, each built on the one below, all from one scan:

1. **Architecture** (exists today in `diff.rs` / `system.rs`): areas added/removed/grown, new
   dependencies between areas, packages, services, endpoints, queues.
2. **Symbols** (new): per file, exported and top-level functions, classes, types, consts — added,
   removed, changed (signature vs body), **moved** between files and **renamed**, detected by a hash
   of the normalised AST (identifiers and whitespace stripped).
3. **Lines** (new): an ordinary diff inside changed symbols only, so review noise drops sharply.

Example `archsnap diff` summary:

```
apps/api/src/prices
  + fn convertCurrency(amount, to)          new, exported
  ~ fn getRates()                           body changed (12 lines)
  → fn formatPrice  moved from packages/ui/src/format.ts
  architecture: apps/api/src/prices now depends on packages/fx
                new endpoint GET /api/fx-rates
```

## Merge

Three-way, per symbol:

- Different symbols changed in one file → clean merge.
- Import lists → set union, then sorted as the file had them.
- Same symbol changed on both sides → line merge inside the symbol; if that fails, a conflict object.
- One side renamed/moved, the other edited → the edit follows the symbol to its new place.
- After merging, rescan and **check the architecture**: new import cycles, broken imports
  (`unresolved_import` already exists), or rules in `archsnap.toml` (e.g. `ui` may not import `db`).
  Violations are warnings on the change, or blockers with `--strict`.

## Landing work: the queue

`archsnap land <ws>` puts a workspace's change in a queue. For each entry, in order: rebase onto the
latest main (per-symbol merge), run the configured checks (`archsnap.toml`: `test = "pnpm test"`) in
that workspace, and fast-forward main if they pass. Failures go back to the workspace with the output
attached, so the agent can fix and re-land. Main only ever moves forward through the queue.

## CLI sketch

```sh
archsnap ws new fix-rates            # CoW workspace from main, prints path, ports, env
archsnap ws list                     # workspaces, their changes, conflicts, queue state
archsnap snap                        # record current state (agents call this via a hook)
archsnap diff [a] [b]                # structural diff; --json for agents
archsnap land fix-rates              # queue → rebase → checks → main
archsnap resolve                     # list or resolve conflict objects
archsnap undo                        # step back through the operation log
archsnap report                      # today's weekly HTML report (`run` stays as an alias)
```

## Workspaces: how they're made

- **macOS (APFS):** `clonefile()` of a warm base directory — instant, shares disk until written.
- **Linux (btrfs/XFS):** reflink copy (`FICLONE`); on ext4, fall back to overlayfs, then hardlinks
  for read-only dependency folders plus a plain copy of source.
- The base is kept warm: after main moves, Archsnap updates the base and re-runs install/build
  once, so new workspaces start ready.
- Ports: each workspace gets a block (e.g. 41000 + 100·n) exposed as `PORT`, `ARCHSNAP_PORT_BASE`;
  env overlay via `.env.archsnap` in the workspace.

## Code layout (proposed)

Split today's binary into a library plus CLI so agents and tests can call it directly:

```
src/
  lib.rs
  git.rs        existing plumbing; + write-tree, commit-tree, update-ref
  scan.rs       existing; + symbol table per file
  system.rs     existing
  diff/         arch.rs (today's diff.rs), symbols.rs, lines.rs
  merge/        symbols.rs, imports.rs, conflicts.rs, checks.rs
  ws/           create.rs (CoW per platform), ports.rs, env.rs
  oplog.rs      operation log + undo
  queue.rs      land queue
  report/       today's report.rs + report.html
  main.rs       CLI
```

## Milestones

| | What | You can use it for |
|---|---|---|
| M0 | Library split, `run` → `report` (alias kept), all e2e tests green | Nothing new; safe base |
| M1 | Symbol table in the scan + `archsnap diff` (3 layers, `--json`) | Reviewing any branch or agent PR |
| M2 | Workspaces (CoW, ports, env) + `snap` + operation log + `undo` | Running agents in parallel today, merging with git |
| M3 | Per-symbol merge + architecture checks + `land` queue | Agents landing work without you merging by hand |
| M4 | Conflict objects + `resolve` | Agents never blocked by conflicts |
| M5 | Decide on own storage, based on measured git bottlenecks | Only if M1–M4 show git is the limit |

Each milestone ships with e2e tests against throwaway repos, as today, plus property tests for
merge (merging A then B equals merging B then A when they touch different symbols; merge never
loses a symbol that only one side changed).

## Open questions for you

1. Which agent runner should workspaces integrate with first (Claude Code hooks, something else)?
2. Which OS/filesystem do your agents run on? That decides which CoW path ships first.
3. cj-stack as the first real repo to dogfood on: what's its GitHub owner/name?
4. Is TS/JS-only structural merge fine for v1, with line merge for everything else?
