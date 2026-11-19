---
# engineering 3.4.2
name: review-loop
description: >
  This skill should be used after a build or a fix, before its PR is marked ready, and when the
  user says "run the review loop" or "is this ready for review". It runs verify, code review and
  simplify in rounds until a round changes nothing.
---

The thread that built the change runs this loop on its own diff, because it holds the purpose;
an orchestrator does not run it for a thread. Paths are relative to this skill's folder.

## Project commands

archsnap is one Rust crate. Run these from the repository root.

- **Verify**: `cargo fmt --check`, then `cargo clippy --all-targets -- -D warnings`, then
  `cargo test`. The change's issue may name a further check, such as a model compared with TLC;
  run it too.
- **Code review**: the `code-review` skill at `high` effort on the diff against the base branch.
- **Simplify**: the `simplify` skill on the same diff.

## Loop

One round, in order:

1. Run the project's verify check for the change's issue; with none, the step is BLOCKED.
   Tests: `references/tests.md`.
2. Run `python3 -I scripts/tests_kept.py --base <base>`. Each line it prints is a finding a user
   would hit; exit 2 is BLOCKED.
3. Run the project's code review of the diff, with the issue's acceptance lines and the
   project's users as the purpose.
4. Fix each finding a user would hit. Handle the rest as the project's review rule says, else
   list them in the report.
5. Run the project's simplify pass on the diff and apply its fixes.

Repeat until a round, run after the last change, has verify and `tests_kept.py` passing, no
review finding a user would hit, and no step changing the code.

Report the last round: the verify and `tests_kept.py` lines as printed, the review result, and
each finding not fixed.
