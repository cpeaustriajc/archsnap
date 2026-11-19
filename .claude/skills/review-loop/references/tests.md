# Tests that prove a change

Read this before you write, change or run a test.

## Tier

Prove each behaviour at the lowest tier that fails without it:

1. Property or fuzz test (Hypothesis, fast-check). With mutation testing (mutmut, Stryker), it is
   also the check against output hardcoded for the test inputs.
2. Unit test on a pure function.
3. Integration test against the real dependency (testcontainers), with no mocks.
4. Golden or snapshot test with a structured diff. Only the owner updates a golden file.
5. End-to-end test through the accessibility tree (Playwright, agent-browser) that asserts on text
   and roles.
6. Screenshot review (`ui-polish`), only for how it looks.
7. Exploratory run (`break-it`). Each finding becomes a test in tiers 1 to 5.

In every tier, a new test fails before the fix for the stated reason, checks behaviour and not
implementation, and its failure output names the cause in a few lines. Take each expected value
from the spec, never from the code. Fix or quarantine a flaky test before you trust a result. Run
the tests that cover the change, not the whole slow suite, unless the project says otherwise.

## A test that contradicts the spec

Stop. Name the test and the line, and explain why it is wrong. Do not change the test, and do not
special-case the code to pass it (ImpossibleBench, arXiv 2510.20270: a stop exit cuts cheating
but does not end it, so `tests_kept.py`, the script that compares the tests before and after, checks the diff as well).

A check that fails on code you did not touch: ask the owner before you bypass it.

## Proof

Proof is the exit code and log that a deterministic check saved: the project's verify skill and
`tests_kept.py`. A pass that you typed, or a review round that has not run since the
last fix, is not proof. The code review is a backstop: it misses cheats spread over several
files.
