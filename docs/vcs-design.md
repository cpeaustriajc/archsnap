# Archsnap as a VCS for parallel agents — design

Status: decisions agreed, nothing built yet.

## The two goals

1. **Small storage.** Disk use grows with *what agents change*, not with *how many agents run*. That
   covers both workspaces (source, `node_modules`, build output) and history.
2. **Fearless parallelism.** An agent can do anything inside its workspace and nothing it does can
   hurt another agent, main, or your work. Every state is recoverable; conflicts never stop an agent.

Target scale: **100+ agents at once on one machine**, for many users, not just us.

## Decisions

| Question | Decision |
|---|---|
| Who owns history | **Archsnap.** Its own store is the source of truth. |
| Git compatibility | **Two-way sync of landed history only.** Git (and GitHub/CI) sees landed changes as normal commits; agent snapshots, the operation log and conflicts stay in Archsnap. Changes made in git are pulled back in. |
| Where it runs | **Local first** (one machine). The store format is designed so a shared/remote store can come later. |
| Platforms | **Linux and macOS.** |
| How snapshots are taken | **On agent tool calls**, via a hook after each edit or command. No file watchers (they hit OS limits at 100+ workspaces). `archsnap snap` for manual use. |
| Structural languages | **TypeScript/JavaScript** (oxc, already used) **and Rust** (`syn`). Everything else: line diff and line merge. |
| Land queue at scale | **Affected tests + batching.** The import graph picks which tests a change can affect; changes that don't overlap land together. |
| Who resolves conflicts | **The owning agent first** (the one whose change came later); escalate to a human only if it fails. |
| Human review | **An Archsnap report page per landed change**: structural + architecture diff. Humans review by exception. |

## Where Archsnap starts from

Archsnap already reads TS/JS as **areas, imports, symbols and a system map**, reads git through
plumbing, and caches per-commit scans. The weekly HTML report stays, as `archsnap report` (`run`
kept as an alias), and grows a per-change view for review.

## Storage

### The store (`<repo>/.archsnap/store`, later shareable)

- Files are content-addressed (BLAKE3) and kept once, whether source, `node_modules` or build output.
- Large files are split with content-defined chunking, so a small edit to a big file stores only
  the changed chunks.
- Objects are packed with zstd and deltas against similar objects.
- History objects: **snapshot** (a tree of file hashes), **change** (stable id surviving rebases,
  pointing at its latest snapshot), **operation** (log entry: what ran, state before and after).

### Workspaces

| Platform | Mechanism | Cost per workspace |
|---|---|---|
| Linux | overlayfs: read-only lower layer materialised from the store, per-agent upper layer | only written files |
| macOS | `clonefile()` of a warm base directory (APFS) | only written blocks |
| Later | virtual filesystem (FUSE / FSKit) serving straight from the store; remote store | only written files, instant |

The warm base (source + installed dependencies + build output) is rebuilt once when main moves,
so new workspaces start ready.

### Keeping history small

- Landed history is kept in full. Agent snapshots **thin out with age**: all for an hour, one per
  minute for a day, then only those landed, named or still referenced. `archsnap gc` enforces this.
- Git only ever receives landed changes, so the git mirror stays the size of real history.
- `archsnap du`: space per workspace, shared, history.

## Fearless parallelism

| Guarantee | Mechanism |
|---|---|
| Agents can't touch each other's files | One layer per workspace; the store is read-only |
| Agents can't break main | Main only moves through the land queue, after checks pass |
| No work lost | Snapshot after every agent tool call |
| Any mistake undoable | Operation log; `archsnap undo`, `archsnap restore <time>` |
| No runtime collisions | Per-workspace port block and env overlay |
| Conflicts don't block | Per-symbol merge; leftovers become conflict objects the agent resolves later |
| Humans can see what happened | Report page per landed change |

### Merge per symbol

- Different symbols changed in one file → clean merge.
- Imports (`import` / `use`) → set union, in the file's order.
- Same symbol changed on both sides → line merge inside it; failing that, a conflict object scoped
  to that symbol. The rest of the file stays valid code.
- One side moved or renamed a symbol, the other edited it → the edit follows the symbol.
- After merging: rescan and check for new import cycles, broken imports and `archsnap.toml` rules
  (e.g. `ui` may not import `db`).

### The land queue

1. Rebase the change onto latest main (per-symbol merge).
2. Find affected tests from the import graph (TS/JS and Rust modules); fall back to the full suite
   when the graph can't decide (config changes, unknown languages).
3. Batch changes whose affected areas don't overlap; run their checks once together.
4. Pass → main moves forward and the batch is synced to git. Fail → bisect the batch, send each
   failing change back to its agent with the output attached.

### Conflicts

The agent whose change came later gets the conflict object (both sides, base, and the structural
diff of each). If it can't resolve it within its retry budget, the change is parked and flagged in
`archsnap ws list` and the report for a human.

## Diff

Three layers from one scan:

1. **Architecture** (exists): areas, dependencies, packages, services, endpoints, queues.
2. **Symbols** (new): functions, classes, types, structs, impls, traits — added, removed, changed
   (signature vs body), moved, renamed — by hash of the normalised syntax tree.
3. **Lines** (new): only inside changed symbols; whole-file for other languages.

## Git sync

- `archsnap import <git-repo>` brings in a repo's history once.
- `archsnap git push` exports newly landed changes as git commits (also automatic after landing).
- `archsnap git pull` brings commits made directly in git (humans, other tools) in as changes.
- Mapping between git commit ids and change ids is kept in the store.

## CLI sketch

```sh
archsnap import ../app               # bring in a git repo
archsnap ws new fix-rates            # thin workspace from main: path, ports, env
archsnap ws list                     # workspaces, size, changes, conflicts, queue state
archsnap snap                        # record state (agents call this via a hook)
archsnap diff [a] [b]                # structural diff; --json for agents
archsnap land fix-rates              # queue → rebase → affected checks → main → git
archsnap resolve                     # conflict objects for this workspace
archsnap undo | restore <time>       # operation log
archsnap du | gc                     # space use; thin out old snapshots
archsnap git push | pull             # sync landed history with git
archsnap report                      # weekly report + per-change review pages
```

## Milestones

| | What | Goal |
|---|---|---|
| M0 | Library split, `run` → `report`, e2e tests green | Safe base |
| M1 | Store (BLAKE3, chunking, packs) + `import` from git + `du` | Storage |
| M2 | Workspaces (Linux overlayfs, macOS clonefile) + ports/env + warm base | Storage, isolation |
| M3 | Hook snapshots + operation log + `undo`/`restore` + `gc` | Nothing lost |
| M4 | Symbol tables (TS/JS, Rust) + `archsnap diff` + per-change report | Seeing what agents did |
| M5 | Per-symbol merge + checks + conflict objects | Never blocked |
| M6 | Land queue: affected tests, batching, bisect | Main safe at 100+ agents |
| M7 | Two-way git sync of landed history | GitHub/CI keep working |
| Later | Virtual filesystem, shared/remote store | Scale past one machine |

Each milestone ships with e2e tests against throwaway repos. M1, M2 and M5 also get property tests:
the store round-trips every file byte for byte; a workspace never changes the store; merging
changes to different symbols gives the same result in either order and never drops a symbol.

M7 comes late, but until it ships `archsnap import` can be re-run, so nobody is locked out of git.

## Still open

1. Which agent runner gets the snapshot hook first (Claude Code, others)?
2. cj-stack as the first repo to dogfood on: its GitHub owner/name.
3. 100 agents on one box also need CPU/RAM for 100 dev servers and test runs. Does Archsnap
   schedule that (limit concurrent checks, queue builds), or is it the user's problem?
