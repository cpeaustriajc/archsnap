#!/usr/bin/env python3
"""A command hook that points Claude back at the user's newest steer after a compaction.

Wire it in the project's .claude/settings.json (compact_reinject.settings.json has the block)
for SessionStart with matcher "compact". It adds no facts: in the losses that led to it, the
compaction summary already held the fact and Claude still did not act on it.

A trial: the last hex digit of the session id picks the arm, even gets the text, odd is the
control. Each run appends "time, session id, arm, project dir" to
$COMPACT_TRIAL_LOG, else ~/.claude/compact-trial.log. A cloud session's log stays on its own
machine. End the trial by setting TRIAL = False; every session then gets the text.
"""
import datetime
import json
import os
import sys

TRIAL = True
KEEP = ("CHECKPOINT.md", "PROGRESS.md", "progress.md")
SKIP = {".git", "node_modules", "worktrees"}
TEXT = (
    "A compaction just ran. Before your next action:\n"
    "1. Find the user's newest request and steer in the summary, in their words, and act on "
    "that, not on an older plan.\n"
    "2. Re-read the open items, open PRs and what was checked from where the summary says "
    "they are kept."
)


def arm(session_id):
    if not TRIAL:
        return "reinject"
    return "reinject" if session_id[-1:].lower() in "02468ace" else "control"


def kept_files(root, depth=4, limit=3):
    """Checkpoint files under root, at most `depth` folders down, in sorted walk order. Skips
    other worktrees, whose files belong to other sessions."""
    found = []
    base = root.rstrip(os.sep).count(os.sep)
    for path, dirs, files in os.walk(root):
        dirs[:] = sorted(d for d in dirs if d not in SKIP)
        if path.count(os.sep) - base >= depth:
            dirs[:] = []
        found += [os.path.join(path, f) for f in sorted(files) if f in KEEP]
        if len(found) >= limit:
            break
    return found[:limit]


def log(session_id, which, root):
    path = os.environ.get("COMPACT_TRIAL_LOG") or os.path.expanduser("~/.claude/compact-trial.log")
    when = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    try:
        with open(path, "a", encoding="utf-8") as f:
            f.write(f"{when}\t{session_id}\t{which}\t{root}\n")
    except OSError:
        pass


def run(event):
    """The hook output for one event, or None for no output."""
    session_id = event.get("session_id") or ""
    if not session_id:
        return None
    root = os.environ.get("CLAUDE_PROJECT_DIR") or event.get("cwd") or ""
    which = arm(session_id)
    log(session_id, which, root)
    if which != "reinject":
        return None
    text = TEXT
    files = kept_files(root) if root and os.path.isdir(root) else []
    if files:
        text += "\n3. Re-read: " + " ".join(files)
    return {"hookSpecificOutput": {"hookEventName": "SessionStart", "additionalContext": text}}


def main():
    try:
        event = json.load(sys.stdin)
    except ValueError:
        return 0
    out = run(event if isinstance(event, dict) else {})
    if out:
        print(json.dumps(out))
    return 0


if __name__ == "__main__":
    sys.exit(main())
