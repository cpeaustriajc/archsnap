#!/usr/bin/env python3
"""Check a folder of decision pages (GitHub wiki clone, Obsidian folder) against references/shape.md.

Usage: python3 -I check_pages.py <folder>
Prints one line per problem and exits 1 if there is any.
"""
import re
import sys
from pathlib import Path

DECISION = re.compile(r"^Decision[ -](\d{3})[ -]")
LINK = re.compile(r"\[\[([^\]|#]+)(?:[#|][^\]]*)?\]\]")
STATUS = re.compile(r"^\*\*Status:\*\*\s*(.*)$", re.M)
DATE = re.compile(r"^\*\*Date:\*\*\s*(\d{4}-\d{2}(?:-\d{2})?)", re.M)
STATUS_OK = re.compile(
    r"^(Proposed|Accepted|Superseded by \[\[[^\]]+\]\]|Partly superseded by \[\[[^\]]+\]\]:\s*\S.*)$")
SECTIONS = ("Context", "Decision", "Consequences")
MENTION = re.compile(r"\b(Claude|agents?)\b", re.I)


def norm(name):
    return name.replace("-", " ").strip().lower()


def check(folder):
    problems = []
    pages = {p.stem: p for p in sorted(Path(folder).glob("*.md")) if p.name[0] not in "_."}
    known = {norm(s) for s in pages}
    text = {s: p.read_text(encoding="utf-8") for s, p in pages.items()}
    decisions = {}

    def bad(stem, msg):
        problems.append(f"{stem}: {msg}")

    for stem, body in text.items():
        if "." in stem:
            bad(stem, "a dot in the page name breaks [[links]]")
        if MENTION.search(body):
            bad(stem, "check: mentions Claude or agents; pages are written for people, so judge whether the word means an AI agent")
        for target in LINK.findall(body):
            if norm(target) not in known:
                bad(stem, f"link [[{target}]] resolves to no page")
        m = DECISION.match(stem)
        if not m:
            continue
        number = int(m.group(1))
        if number in decisions:
            bad(stem, f"number {m.group(1)} is already used by {decisions[number]}")
        decisions[number] = stem
        if not re.search(rf"^# Decision {m.group(1)}: \S", body, re.M):
            bad(stem, f"first heading is not '# Decision {m.group(1)}: <Topic>'")
        s = STATUS.search(body)
        if not s:
            bad(stem, "no **Status:** line")
        elif not STATUS_OK.match(s.group(1).strip()):
            bad(stem, f"Status '{s.group(1).strip()}' is not Proposed, Accepted, Superseded by [[..]] "
                      "or Partly superseded by [[..]]: <the part>")
        if not DATE.search(body):
            bad(stem, "no **Date:** line with YYYY-MM-DD (or YYYY-MM)")
        for sec in SECTIONS:
            if not re.search(rf"^## {sec}\s*$", body, re.M):
                bad(stem, f"no '## {sec}' section")

    home = text.get("Home")
    index = text.get("Decisions")
    if home is None:
        bad("Home", "no Home page")
    else:
        linked = {norm(t) for t in LINK.findall(home)}
        for stem in pages:
            if stem in ("Home", "Decisions") or (DECISION.match(stem) and index is not None):
                continue
            if norm(stem) not in linked:
                bad("Home", f"does not link [[{stem}]]")
    if index is not None:
        rows = [c for c in ([x.strip() for x in r.strip().strip("|").split("|")]
                            for r in index.splitlines() if r.lstrip().startswith("|")) if re.fullmatch(r"\d{3}", c[0])]
        seen = {}
        for r in rows:
            seen[int(r[0])] = r
        for number, stem in decisions.items():
            if number not in seen:
                bad("Decisions", f"no row for [[{stem}]]")
                continue
            s = STATUS.search(text[stem])
            word = s.group(1).strip().split(" by")[0].lower() if s else ""
            if word and word not in " ".join(seen[number]).lower():
                bad("Decisions", f"row for {stem} does not show its Status '{word}'")
        dates = [d.group(0)[:7] for r in rows for d in [re.search(r"\d{4}-\d{2}(-\d{2})?", " ".join(r))] if d]
        if dates != sorted(dates, reverse=True):
            bad("Decisions", "rows are not newest first by date")
    return problems


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    found = check(sys.argv[1])
    print("\n".join(found))
    print(f"{len(found)} problem(s)")
    sys.exit(1 if found else 0)
