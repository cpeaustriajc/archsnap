# archsnap

Weekly architecture snapshots of a TypeScript/JavaScript repo, for everyone on the team, PMs included.

`archsnap run` walks git history, takes one snapshot per ISO week, and writes a self-contained
`.archsnap/index.html` with:

- **The week in plain words**: new or removed areas, areas that grew or shrank, new dependencies
  between areas, packages started or stopped.
- **How the codebase got here**: lines of code per week, split by the five largest areas. Select a
  week (or use ← →) to read what changed in it. Weeks without commits show as gaps.
- **System map**: what runs, what stores data and what it talks to, in five lanes (who and what starts
  it, apps and services, shared code, data and queues, outside services). Read from `package.json`
  files, Cloudflare `wrangler` config (Workers, D1, KV, R2, Durable Objects, queues, crons, served
  assets, routes), well-known SDKs (Stripe, Postgres, Redis, …), Xcode and XcodeGen projects, and the
  URLs the code actually calls. The week's summary says what changed, e.g. "Added queue emails."
- **How the code fits together**: an import matrix of every area (rows import from columns), grouped
  by workspace package. Select a cell, or pick two areas, to see how they talk: which names cross the
  boundary, which file defines them and which files use them.
- An **areas table** with the same numbers, readable without colour.

It reads only committed code (never your working tree), makes no network or LLM calls, and a
12-week run on a ~200-file repo takes about a second.

## Usage

```sh
cargo install --path .
archsnap run                      # in any git repo
archsnap run --repo ../app --out /tmp/app-report
archsnap run --depth 4            # finer areas, e.g. apps/web/src/components
archsnap run --weeks 26 --strict
```

| Flag | Default | Meaning |
|---|---|---|
| `--repo <path>` | current dir | Repository to scan |
| `--out <dir>` | `<repo>/.archsnap` | Where snapshots and `index.html` go |
| `--weeks <n>` | 12 | How many weeks that had commits to snapshot (at least 1) |
| `--depth <n>` | 1 | Folder levels per area below each workspace package, after `src/` (`apps/web/src/billing/api/x.ts` → `apps/web/src/billing` at 1) |
| `--strict` | off | Exit 1 when there are warnings (the report is still written) |
| `--verbose` | off | Print every git call |

Snapshots are cached in `<out>/snapshots/<week>.json` and reused on the next run. The newest week
is always rescanned.

## Weekly in GitHub Actions

```yaml
on:
  schedule: [{ cron: "0 6 * * 1" }]   # Mondays
  workflow_dispatch:
jobs:
  archsnap:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with: { fetch-depth: 0 }       # full history; a shallow clone only gets this week
      - run: cargo install --git https://github.com/<you>/archsnap
      - run: archsnap run
      - uses: actions/upload-artifact@v4
        with: { name: archsnap-report, path: .archsnap/index.html }
```

## Errors and warnings

Problems that only affect part of the snapshot are **warnings**: archsnap points at the exact spot,
keeps going, and lists them in the report.

```
  ⚠ archsnap(unresolved_import): can't find `./missing` imported from src/cart/cart.ts
   ╭─[src/cart/cart.ts:2:22]
 1 │ import { pay } from '../pay/index.js';
 2 │ import { gone } from './missing';
   ·                      ─────┬─────
   ·                           ╰── imported here
   ╰────
  help: no committed file matches this path; it is shown as unresolved in the report
```

| Code | What happened | What archsnap does |
|---|---|---|
| `parse_skipped` | A file has a syntax error | Leaves it out of the snapshot |
| `unresolved_import` | A relative or `@/` import matches no committed file | Counts it as unresolved |
| `shallow_clone` | The clone has no history | Snapshots the current week only |
| `snapshot_corrupt` | A cached snapshot can't be read | Rebuilds it from git |

Printed warnings come from the newest week only; older weeks just record counts.

| Exit code | Meaning |
|---|---|
| 0 | Report written (warnings allowed) |
| 1 | `--strict` and there were warnings |
| 2 | Bad flags, not a git repo, no commits, or git missing |
| 3 | Can't write the output folder, a git command failed, or a git object is missing |
| 70 | Bug in archsnap |

## What counts

- Files: `.ts .tsx .mts .cts .js .jsx .mjs .cjs` (JSX allowed in `.js`), except `.d.ts`, `*.min.*`,
  files over 512 KB, symlinks, and anything under `node_modules`, `dist`, `build`, `out`, `coverage`,
  `vendor`, `.next`. Imports of those skipped files still count as resolved.
- Imports: static `import`/`export … from`, and `import("…")` with a plain string. Query suffixes
  like `?raw` are dropped.
  `require()` is not read yet.
- `@/x` and `~/x` resolve to `src/x` or `x` under the importing file's folder or any parent, so
  monorepo packages like `apps/web/src` work. Other tsconfig `paths` aliases show up as packages.
- Imports of stylesheets, images, JSON and other assets are ignored.
- Workspace package imports (`@shop/core`) resolve to the package's files, so they link areas instead
  of showing up as packages.
- An outside URL counts as a service the code calls when it is passed to `fetch`, `axios`, `ky`,
  `new URL` and similar, stored in an endpoint-named constant (`API_BASE`, `baseURL`), or sits in a file
  that makes network calls. URLs in comments, JSX attributes and link fields (`href`, `image`, …) do
  not count, and placeholder domains (`example.com`, `acme.com`, …) are ignored. Hosts group by
  domain (`api.stripe.com` → Stripe); the map shows the 12 most-connected and folds the rest.
- File-by-file import detail is kept for the newest week only, to keep the report small on big repos.

## Tests

End-to-end tests drive the real binary against throwaway git repos:

```sh
cargo test
```
