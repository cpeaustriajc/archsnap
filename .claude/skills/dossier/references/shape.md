# Shape of the knowledge

Step 2 of SKILL.md reads this. It is the default for a new log.

## An existing log

The project's own conventions win, then the Backend section's shape, then this page. Before the first
write, read the log's index or home page and its two newest decision pages. Match their template,
numbering, title pattern, status words, link syntax and language. A repo whose
`docs/adr/` holds MADR files (https://adr.github.io/madr/) gets a MADR file, not a `Decision NNN` page. Where the log has
no convention for something, use this page.

## Pages

- **Home** (or the workspace's root page). One paragraph says the README is the product spec,
  meaning what we build, and the decisions record why. Below it, a `Page | What it answers` table
  lists every page.
- **Decisions index.** A table, newest first by date, with columns `# | Decision | Status | Date |
  Issue`. A new row goes at the top. Sort by date, not by number: numbers follow the order
  decisions were recorded, and they can be out of date order.
- **`Decision NNN <Topic>`.** One page per decision, numbered with zero padding (`001`). Immutable ADR (Michael Nygard,
  "Documenting Architecture Decisions", 2011, https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions):
  facts and wording stay as decided.
- **Backends with search but no sorted views** (Linear) drop the number and the index: each
  decision is titled `Decision: <Topic>` and found by search. The Backend section gives the shape that
  replaces the two items above.
- **Research pages.** One topic each. Date the research, and mark anything not confirmed on a
  primary source as **UNVERIFIED**. A research page is a living document: when newer facts replace what it describes, rewrite it
  for the current state, and move the old description under `## History` at the foot, saying what
  replaced it. Deleting it would erase the evidence behind older decisions.

## Decision template

```markdown
# Decision NNN: <Topic>

**Status:** Proposed | Accepted | Superseded by [[Decision NNN …]] | Partly superseded by [[Decision NNN …]]: <the part>
**Date:** YYYY-MM-DD · **Issue:** [#N](link) or "none recorded"

## Context
The problem, and links to the research pages.

## Decision
What we chose.

## Consequences
What it rules out and what it costs.
```

Use the backend's own link syntax: `[[Page]]` on a wiki or in Obsidian, a page mention in Notion.

## Dates

A decision's date comes from something a reader can check: the commit or session that acted on
it (or the log of the files it changed), an issue, or my word. If none exists, write the month
and say that the day is not recorded. A decision and its build are different events, so a page
may carry both: `**Built:** YYYY-MM-DD (<sha>)`.
