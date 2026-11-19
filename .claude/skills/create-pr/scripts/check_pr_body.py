#!/usr/bin/env python3
"""Check a PR body file before it goes to the code host (create-pr, references/pr-body.md).

Usage: check_pr_body.py <body-file> [--template]
  --template  the repo's template or its recent PRs' form sets the shape: skip the shape checks.
Exit 0 prints "pr body ok". Exit 1 prints "<file>:<line>: <cause>" per finding. Exit 2: unreadable.
Fenced code blocks and inline code are not checked. Without --template, the summary above the
first Evidence, Merge danger or Test changes line has at most 3 lines.
"""
import re
import sys

CLOSING = re.compile(
    r"\b(close[sd]?|fix(e[sd])?|resolve[sd]?)\b:?\s+"
    r"(#\d+|[\w.-]+/[\w.-]+#\d+|https?://\S+/(issues|issue)/\S+|[A-Z][A-Z0-9]+-\d+\b)",
    re.IGNORECASE,
)
HEADING = re.compile(r"^#{1,6}\s")
TASK = re.compile(r"^\s*[-*+]\s+\[[ xX]\]")
ARROW = re.compile(r"→|⇒| -> | => ")
REQUIRED = ("Merge danger:", "Test changes:")
LINE_FIELDS = ("Evidence:",) + REQUIRED
SUMMARY_MAX = 3


def check(text, template=False):
    """Return the findings as (line number, cause) pairs."""
    found, fence, seen, summary, in_summary = [], False, set(), 0, True
    for n, raw in enumerate(text.splitlines(), 1):
        if raw.lstrip().startswith(("```", "~~~")):
            fence = not fence
            continue
        if fence:
            continue
        line = re.sub(r"`[^`]*`", "", raw)
        if m := CLOSING.search(line):
            found.append((n, f"closing keyword '{m.group(0)}': link the issue by its key instead"))
        if "—" in line:
            found.append((n, "em dash: end the sentence or use a comma"))
        if ARROW.search(line):
            found.append((n, "arrow: write the sentence"))
        for req in REQUIRED:
            if line.startswith(req):
                seen.add(req)
        if line.startswith(LINE_FIELDS):
            in_summary = False
        if template:
            continue
        if in_summary and line.strip() and not line.lstrip().startswith("!["):
            summary += 1
            if summary == SUMMARY_MAX + 1:
                found.append((n, f"summary over {SUMMARY_MAX} lines: keep it short"))
        if HEADING.match(line):
            found.append((n, "heading: the default body has none"))
        if TASK.match(line):
            found.append((n, "checklist item: the default body has none"))
    if not template:
        found += [(0, f"missing '{req}' line") for req in REQUIRED if req not in seen]
    return found


def main(argv):
    args = [a for a in argv if a != "--template"]
    if len(args) != 1:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    try:
        text = open(args[0], encoding="utf-8").read()
    except OSError as e:
        print(f"could not read {args[0]}: {e}")
        return 2
    found = check(text, "--template" in argv)
    for n, cause in found:
        print(f"{args[0]}:{n}: {cause}")
    if not found:
        print("pr body ok")
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
