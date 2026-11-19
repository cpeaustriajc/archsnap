# PR title, body and metadata

Title in commit style. Body: the repo's template, else its recent merged PRs' form, else:

```
<1-3 lines: what changed and why, for a reviewer who hasn't seen the issue>

Evidence: <before and after of one state, and the re-run command>
Merge danger: <one-way or two-way door; blast radius>
Test changes: <the tests_kept.py line as printed, and each allowed finding with the owner's reason>
```

- Write the body to a file and run `python3 -I scripts/check_pr_body.py <file>`; add `--template`
  when the repo's template or its recent PRs' form sets the shape. Pass the same file to the code
  host. The check rejects a closing keyword, which closes the issue on merge before the verifier
  passes it, the harness's default headings and test-plan checklist, and a summary over 3 lines.
- Write the title and body through `references/ai-tells.md`, rules 26 to 35. Name
  code by its identifier: `loadConfig`, not "the config loader".
- Visual only when it replaces prose: the smallest one, after the line it supports, in this
  order of preference: a `diff`, a tree, pseudocode, Mermaid for a changed flow. A typo or
  one-line fix gets none.
- Evidence: a visual change, flow or timing follows `references/evidence.md`; else the test red before, green
  after. No E2E run: no line.
- Merge danger: `one-way door, adds premium_prices table; blast radius: search and checkout totals.`
- Metadata goes in the code host's fields, not the body: reviewers, labels, assignee, milestone and
  project, taken from the repo's rules or its recent PRs. Put the issue's key (from the branch
  name, the commits or the session) in the branch name or title, so the tracker links the PR.
