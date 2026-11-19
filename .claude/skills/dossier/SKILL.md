---
# knowledge 1.2.1
name: dossier
description: >
  This skill should be used when the user says "note that down as a decision", "what did we decide
  about X", "supersede that decision", "record this research" or "set up this project". It keeps
  earnest's dossier: decision and research pages in a Claude Doc. Agent rules: the instruction
  file.
---

Decision log and research pages live where the team writes, not in the code repo (an existing
`docs/adr/` is followed: `references/shape.md`), and are written for people; agent instructions
go in the project's instruction file. Paths are relative to this skill's folder.

1. Find the log in the order of `references/locate.md`.
2. Read the Backend section, then `references/shape.md`. Before the first write, learn the
   log's own conventions; they win over the Backend section, and it over `shape.md`.
3. "What did we decide about X": read every match in full and follow each "Superseded by" and
   "Partly superseded by" chain to its end. Report the current decision (or the part that
   stands), then what it replaced, with dates and page names. The chain is the authority, not
   the latest commit. No match: say so; code and memory are not the record. Before a code
   change, end with Preserve, Change, Avoid, Risk.
4. Record, supersede, file research or move rationale: `references/operations.md`. A decision
   page is never rewritten; only its Status line changes. A missing reason reads "not
   recorded" and you ask me: an invented one props up every later decision.
5. Publish per `references/publish.md`: I see the first write to a shared space, and every page
   passes its checks and `references/plain-prose.md`. Save after every change. Where a save
   reaches others at once (a synced folder, a hosted workspace, a wiki), saving is publishing.
   A backend with its own editor can conflict: show me both versions, and never force-push.

## Backend

The dossier is one private Claude Doc named "earnest dossier", served by the Claude Docs tools
(`read`, `update`, `batch`, `create`, `guide`). This repository is public, so it holds no link to
the doc: the link comes from the user map (`references/locate.md`, step 2), keyed by
`git remote get-url origin`. Load the docs skill, or `guide( items = ["topic.index"] )`, before
the first call.

- **Layout**: three top-level tabs, in order Home (`a0`), Decisions (`a1`) and Research (`a2`).
  Home has the `Page | What it answers` table and a mention of the spec doc, "A formal
  verification suite for agents". Each decision is a sub-tab of Decisions named
  `Decision NNN <Topic>`, ordered `b001`, `b002` and so on. Each research topic is a sub-tab of
  Research, ordered `b01`, `b02` and so on.
- **Find**: `read` the doc (`ref` = the project) to list its tabs by name, then `read` each likely
  tab with `payload = {"kind":"search","text":"<words>"}`. A search covers one tab only.
- **Read**: `read` the tab's body node with `{"projection":"outline"}`, then a `view` of the parts
  you need.
- **Record**: one `batch` that creates the decision's tab and body from the decision template,
  sets its body, and places it under Decisions with `subtabOf` and the next order. Then `update`
  the Decisions table to add its row at the top. Follow `guide( items = ["topic.tabs"] )`.
- **Supersede**: `update` the old tab's Status line with a `find` target only. The rest of that tab
  stays as written. Update its row in the Decisions table.
- **Link**: a mention chip, `{"type":"mention","ref":"file/<tab id>"}`, in place of `[[Page]]`. A
  Status line reads `Superseded by <mention of the new tab>`.
- **Save**: every write is live in the doc, so saving is publishing. `scripts/check_pages.py`
  checks file folders only: check names, Status, Date, sections, links and Home by hand, as for
  Notion.
