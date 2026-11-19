# Stacked PRs

Read this when the work's issue has a blocker whose PR is still open.

Build items of one project ship as stacked diffs in blocked-by order, landed bottom-up. Branch off the blocker's
branch, and open the PR against it; an item with no open blocker branches off the trunk. A stack
of parallel branches off the trunk costs a restack later: every PR rebases and loses its verify
verdict. The PR base is the record every tool shares, so a fresh session finds the base there.
A builder rebases only its own branch; the coordinator restacks
after a merge, bottom up.

Use the tool the project already uses (`gh stack`, Graphite `gt`, git-town); with none, plain git
and the PR base (`git rebase --onto <trunk> <old base> <branch>` after the bottom merges, then
`gh pr edit --base`). Check each command with `--help` before its first use: these tools change
often.

Sources: `github/gh-stack` README; graphite.com/docs/cheatsheet; git-town.com/stacked-changes; the
autopilot-stack playbook in `cursor/plugins` (pstack), which keeps one linear stack and records
the parent only as the PR base.
