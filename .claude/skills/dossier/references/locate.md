# Find the log

Step 1 of SKILL.md. Stop at the first hit.

1. **A pointer in the project's instruction file (`AGENTS.md`, `CLAUDE.md` or the harness's own)**: a line naming the decision log by URL or path, for
   example `Decisions: https://github.com/o/r/wiki/Decisions`, a `notion.so/…` link, or
   `obsidian: ~/Vaults/Work/Projects/acme`. Infer the backend from the URL or path.
2. **The user map** `${XDG_CONFIG_HOME:-~/.config}/dossier/dossier.json`. It maps a git remote, or an absolute repo
   path, to a location string in the same format.
3. **Detection.** If the repo has a GitHub remote, run `scripts/ghwiki.sh detect`. It reports
   whether the repo's wiki exists.
4. **Ask me once.** Offer the backends that my connected tools make possible, then save the
   answer to the user map so no one is asked again. Add a pointer line to that instruction file only if I
   want the repo to carry it.
