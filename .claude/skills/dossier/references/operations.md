# Operations that change the record

Step 4 of SKILL.md uses these steps.

**Record a decision.** When I ask, record it. Offer one unasked, for example while moving
rationale out of a repo, only when all three hold: it is a one-way door, it surprises a reader
without its context, and it came from a real trade-off between alternatives.
1. Read the index and take the next number (skip this where the backend has no numbers).
2. Write the page from the decision template.
3. Add its row to the top of the index, where there is one.
4. Add or update the Home row, if the decision creates a page.
5. Link the research pages it rests on.

**Supersede a decision** (immutable ADR with a Status chain, `references/shape.md`): the old page stays
immutable except its Status.
1. Record the new decision as a new page, and have its Context say what changed since the old one.
2. On the old page, change the Status line to `Superseded by [[Decision NNN …]]`.
3. In the index, update the old row's Status and add the new row at the top.

**When only part of it is replaced**, the rest still stands, so the old decision stays in force.
Write its Status as one line: `Partly superseded by [[Decision NNN …]]: <the part replaced>`, for
example `Partly superseded by [[Decision 010 Web Checkout]]: the single-store-only rule`. Put the
same words in its index row. The new page's Context names the part it replaces and says the rest
of the old decision stands. When several decisions each replace a different part, list them in one
Status line, separated by `;`. Once no part is left, the Status becomes plain `Superseded by`.

**File research.** Write the page or add to one, date it, and add the Home row. If it replaces
earlier findings, move them under `## History`.

**Move rationale out of a repo.**
1. Take text that explains why, gives evidence, measures something, records a rejected
   alternative, or tells history.
2. Put it on the matching research page or decision page.
3. Rewrite anything addressed to an agent so it reads as project history, for people.
4. Leave the instruction itself in the repo, where agents load it.
