#!/usr/bin/env python3
"""A command hook that stops Claude from passing a failing test by changing the test or the runner.

Wire it in the project's .claude/settings.json (guard_tests.settings.json has the block) for
PreToolUse and PostToolUse on Edit|Write|MultiEdit, UserPromptSubmit and Stop. One script,
dispatched on hook_event_name. It imports tests_kept.py, which sits beside it, for the test-file
patterns and the line rule.

Project facts come from guard_tests.json beside this script (a missing or unreadable file is mode off):
  mode              "rigs" (the content rules), "full" (rigs, the HEAD-line rule and the turn check) or "off"
  extra_test_dirs   folder names that mark a test file, beside tests_kept.TEST_DIR
  extra_test_names  regular expressions on the path from the repository root, beside tests_kept.TEST_NAME
  shared_files      file names or root paths that hold other settings too: only an added pass-anyway flag counts
  picked            {"at": date, "by": who}: when and by whom the mode was chosen

Events:
  PreToolUse        a finding is an "ask" with the file and the line; the reason also tells the model what to do
                    if the ask is refused. Where no prompt can be shown, Claude Code refuses an ask itself.
  PostToolUse       an edit that was asked about and ran was approved: its file's HEAD lines are open until the
                    turn ends, and the lines it lost do not count at the turn's end.
  UserPromptSubmit  notes HEAD and the lines already lost (full rules), and answers `/test-guard` and
                    `/test-guard allow <path>` from the raw prompt. This event fires only for a prompt a
                    person submitted, never for the model, a subagent or a tool, so it stands in for the
                    old check that a command came from the user.
  Stop              blocks the stop once when a HEAD line of a tracked test or config file is gone and no
                    approved edit lost it (full rules), whatever tool did it.

An edit to this script, tests_kept.py, guard_tests.json or the project's .claude/settings.json asks in
every mode except off. Session state (allowed files, turn start) goes to
$TMPDIR/claude-hooks/<session_id>/guard-tests.json, never in the project.
"""
import collections
import json
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)  # python3 -I leaves the script's folder off sys.path
sys.dont_write_bytecode = True  # no __pycache__ in the project's hooks folder
try:
    import tests_kept as tk  # noqa: E402
except ImportError as missing:  # fail closed on an edit: Setup copies tests_kept.py beside this file
    if '"PreToolUse"' in sys.stdin.read():
        print(json.dumps({"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "deny",
              "permissionDecisionReason": f"test-guard: could not check this edit ({missing}). Run review-loop setup again to copy tests_kept.py beside guard_tests.py."}}))
    sys.exit(0)

try:
    import fcntl
except ImportError:  # Windows: no lock, the state may lose a parallel write
    fcntl = None

MOD = "test-guard"
TOOLS = ("Edit", "MultiEdit", "Write")
MODES = ("rigs", "full", "off")
NAMED = {"rigs": "rig rules only", "full": "full rules", "off": "off"}
OWN_NAMES = ("guard_tests.py", "tests_kept.py", "replay.py", "guard_tests.json")
USAGE = ("Use /test-guard allow <path> to let Claude change the lines that path has at HEAD, "
         "or /test-guard to list allowed paths.")
NO_BASELINE = re.compile(r"not a git repository|invalid object name '?HEAD'?|does not exist in '?HEAD'?|"
                         r"exists on disk, but not in '?HEAD'?|unknown revision|ambiguous argument 'HEAD'|bad revision 'HEAD'")

# `test`: the HEAD-line rule and the content rules. `config`: runner and CI config, the HEAD-line rule and
# CONFIG_ADD. `shared`: files that also hold other settings, CONFIG_ADD only.
# A removed exclude line or a dropped test step makes the suite skip tests and stay green.
CONFIG_NAME = re.compile(r"(^|/)((jest|vitest|playwright)\.config\.[^/]+|vitest\.workspace\.[^/]+|pytest\.ini|"
                         r"\.pre-commit-config\.ya?ml)$")
CONFIG_DIR = re.compile(r"^(\.github/workflows|\.husky)/")
SHARED_NAME = re.compile(r"(^|/)(package\.json|pyproject\.toml|setup\.cfg|tox\.ini)$")
ANY_CONFIG_DIR = re.compile(r"/(\.github/workflows|\.husky)/")


class Config:
    def __init__(self, mode="off", extra_dirs=(), extra_names=(), shared=(), picked=None):
        self.mode = mode if mode in MODES else "off"
        self.extra_dirs = set(extra_dirs)
        self.extra_names = list(extra_names)
        self.shared = [s.replace("\\", "/") for s in shared]
        self.picked = picked

    def is_shared(self, p):
        return bool(SHARED_NAME.search(p)) or any(p == s or p.endswith("/" + s) or ("/" not in s and p.split("/")[-1] == s)
                                                  for s in self.shared)

    def name_kind(self, path):
        """The kind a name alone gives, with no git: a test name or a runner config name."""
        p = path.replace("\\", "/")
        if tk.is_test_name(p, self.extra_names):
            return "test"
        return "config" if CONFIG_NAME.search(p) else None

    def kind_of(self, rel):
        """The kind of a path from the repository root."""
        p = rel.replace("\\", "/")
        k = self.name_kind(p)
        if k:
            return k
        if tk.in_test_dir(p, self.extra_dirs):
            return "test"
        if CONFIG_DIR.search(p):
            return "config"
        return "shared" if self.is_shared(p) else None

    def maybe_guarded(self, path):
        """A quick test on the absolute path: False means no kind, so no git runs."""
        p = path.replace("\\", "/")
        return bool(self.extra_names or self.name_kind(p) or tk.in_test_dir(p, self.extra_dirs)
                    or ANY_CONFIG_DIR.search(p) or self.is_shared(p))


DEFAULT = Config()


def load_config(path):
    try:
        with open(path, encoding="utf-8") as f:
            raw = json.load(f)
        strs = lambda k: [x for x in raw.get(k) or [] if isinstance(x, str)]  # noqa: E731
        for rx in strs("extra_test_names"):
            re.compile(rx)
        return Config(raw.get("mode"), strs("extra_test_dirs"), strs("extra_test_names"), strs("shared_files"),
                      raw.get("picked") if isinstance(raw.get("picked"), dict) else None)
    except FileNotFoundError:
        return Config()
    except (OSError, ValueError, AttributeError, TypeError, re.error) as e:
        log(f"could not read {os.path.basename(path)} ({e}); the guard is off for this call.")
        return Config()


def log(text):
    print(f"{MOD}: {text}", file=sys.stderr)


# ---- the rules ----

Finding = collections.namedtuple("Finding", "what line lost", defaults=(False,))

SKIP_DECORATOR = re.compile(r"^\s*@(pytest\.mark\.(skip|skipif|xfail)|unittest\.(skip|skipIf|skipUnless|expectedFailure))\b")
SKIP_CALL = re.compile(r"\b(pytest\.(skip|xfail)|self\.skipTest|t\.Skip(Now|f)?|ctx\.skip)\s*\(|^\s*return;?\s*$")
DEF = re.compile(r"^\s*((async\s+)?def|class)\s")
OPENER = re.compile(r"^\s*((async\s+)?def|class|func)\s|\bfunction\b|=>\s*\{\s*$")
HEADER = re.compile(r"^\s*((async\s+)?def\s+test|class\s+Test|func\s+Test|(it|test)\s*\()")
# Each skips or focuses the tests of the whole file or block, or turns type checks off.
FILE_SKIP = re.compile(r"\b(describe|context|suite|it|test)\.only\b|\b(fit|fdescribe|ftest|xdescribe)\s*\(|"
                       r"\b(describe|context|suite)\.skip\b|^\s*pytestmark\s*=.*\b(skip|skipif|xfail)\b|@ts-nocheck|"
                       r"^\s*if\s*\(\s*(false|0)\s*\)|^\s*if\s+(False|0)\s*:")
EXIT = re.compile(r"\b(sys\.exit|os\._exit|process\.exit|os\.Exit)\s*\(\s*0?\s*\)|\bpytest\.exit\s*\(")
# The pytest report patch (MacDiarmid et al.): set `outcome` to passed and clear `longrepr`.
REPORT = re.compile(r"\.outcome\s*=\s*['\"](passed|skipped)['\"]|\b(rep|report)\.(outcome|longrepr)\s*=(?!=)|"
                    r"\blongrepr\s*=\s*None\b|\bTestReport\.\w+\s*=(?!=)|\bsetattr\s*\(\s*TestReport\b")
# Python only, at statement position, so a string or a call such as `raise SystemExit(main())` is not one.
PY_EXIT = re.compile(r"(?:^|;)\s*(?:raise\s+SystemExit(?:\s*\(\s*(?:0|None)?\s*\))?|(?:exit|quit)\s*\(\s*0?\s*\))\s*(?:;|#.*)?$")
EQ_LINE = re.compile(r"__eq__.*\breturn\s+True\b|__eq__\s*=\s*lambda[^:]*:\s*True\b|__ne__.*\breturn\s+False\b|"
                     r"asymmetricMatch.*\breturn\s+true\b|asymmetricMatch\s*[:=]\s*\([^)]*\)\s*=>\s*true\b")
EQ_DEF = re.compile(r"\b(def\s+__eq__|asymmetricMatch)\s*\(")
EQ_RETURN = re.compile(r"^\s*return\s+(True|true);?\s*$")
# Each counts as added only when the file holds more of it than at HEAD, so a line that keeps one can change.
CONFIG_ADD = [re.compile(p) for p in (r"passWithNoTests", r"--deselect\b", r"continue-on-error:\s*true\b")]
CONFIG_ONLY_ADD = [re.compile(r"\|\|\s*(true|exit\s+0)\b")]


def count(rx, text):
    return len(rx.findall(text))


def minus(lines, take=()):
    """`lines` with `take` taken out, each line as many times as `take` holds it."""
    left = dict(take) if isinstance(take, dict) else collections.Counter(take)
    out = []
    for line in lines:
        if left.get(line, 0) > 0:
            left[line] -= 1
        else:
            out.append(line)
    return out


def apply(text, change):
    old, new = change.get("old_string"), change.get("new_string")
    if not isinstance(old, str) or not isinstance(new, str):
        raise ValueError("edit without old_string or new_string")
    return text.replace(old, new) if change.get("replace_all") else text.replace(old, new, 1)


def head_test(lines, added, i):
    """The test an added line sits in: the nearest function opener above it, if that opener is a test header HEAD has."""
    for j in range(i - 1, -1, -1):
        if OPENER.search(lines[j]):
            return lines[j] if HEADER.search(lines[j]) and not added[j] else None
    return None


def decorated(lines, added, i):
    """The definition a decorator at i applies to, if HEAD has it and it is a test."""
    for j in range(i + 1, len(lines)):
        if DEF.search(lines[j]):
            return lines[j] if HEADER.search(lines[j]) and not added[j] else None
    return None


def findings(kind, path, head, nxt, opened=False, had=(), cfg=DEFAULT, rel=None):
    """What an edit that turns `head` (None for a new file) into `nxt` may not do to a file of `kind`, or None.
    opened: a person approved changing this file's HEAD lines for the turn, so only the content rules run.
    had: the HEAD lines the file lacked before this edit; the HEAD-line rule judges only the lines this edit loses.
    rel: the path from the repository root when `path` is absolute, for the project's own name patterns."""
    if head is None:
        gone, commented, lines, added = [], False, nxt.splitlines(), None
    else:
        a = tk.align(head, nxt, path)
        gone, commented, lines, added = [t for _, t in a.gone], a.commented, a.lines, a.added
    lost = (minus(gone, had) or [None])[0]
    if lost is not None and kind != "shared" and not opened:
        verb = "comments out" if commented and lost == gone[0] else "changes or removes"
        return Finding(f"{verb} a line that {path} has at HEAD", lost, True)
    if added is None:
        added = [True] * len(lines)
    for i, l in enumerate(lines):
        if not added[i]:
            continue
        if kind != "test":
            rigs = CONFIG_ADD + CONFIG_ONLY_ADD if kind == "config" else CONFIG_ADD
            if head is not None and any(count(rx, l) and count(rx, nxt) > count(rx, head) for rx in rigs):
                return Finding(f"stops tests from running or from failing, in {path}", l)
            continue
        if head is not None:
            t = decorated(lines, added, i) if SKIP_DECORATOR.search(l) else head_test(lines, added, i) if SKIP_CALL.search(l) else None
            if t:
                return Finding(f"skips the HEAD test `{t.strip()}` in {path}", l)
            if FILE_SKIP.search(l):
                return Finding(f"skips or focuses tests, or turns off checks, in {path}, which has tests at HEAD", l)
        # A helper script in a test folder may exit; a test file or conftest.py may not.
        if (EXIT.search(l) or (path.endswith(".py") and PY_EXIT.search(l))) and cfg.name_kind(rel or path) == "test":
            return Finding(f"exits the test process with success in {path}", l)
        if REPORT.search(l):
            return Finding(f"rewrites the test report in {path}", l)
        prev = next((x for x in reversed(lines[:i]) if x.strip()), "")
        if EQ_LINE.search(l) or (EQ_RETURN.search(l) and EQ_DEF.search(prev)):
            return Finding(f"adds an equality that is always true in {path}", l)
    return None


# ---- git and paths ----

class CouldNotCheck(Exception):
    pass


def git(args, cwd):
    r = subprocess.run(["git", *args], cwd=cwd, capture_output=True)
    return r.returncode, r.stdout.decode("utf-8", "replace"), r.stderr.decode("utf-8", "replace")


def canon(path):
    """The absolute path with the folder's symlinks resolved, so every key of the state agrees."""
    return os.path.join(os.path.realpath(os.path.dirname(path)), os.path.basename(path))


def absolute(path, cwd):
    return os.path.normpath(path if os.path.isabs(path) else os.path.join(cwd, path))


def nearest_dir(path):
    d = os.path.dirname(path) or "/"
    while d != "/" and not os.path.isdir(d):
        d = os.path.dirname(d) or "/"
    return d


def locate(path):
    """(repository root, path from it), or None outside a repository."""
    rc, out, err = git(["rev-parse", "--show-toplevel"], nearest_dir(path))
    if rc:
        if NO_BASELINE.search(err):
            return None
        raise CouldNotCheck(f"git rev-parse exited {rc}: {err.strip()}")
    top = os.path.realpath(out.strip())
    rel = os.path.relpath(path, top).replace(os.sep, "/")
    return None if rel.startswith("../") else (top, rel)


def head_text(top, rel):
    rc, out, err = git(["show", f"HEAD:{rel}"], top)
    if rc == 0:
        return out
    if NO_BASELINE.search(err):
        return None
    raise CouldNotCheck(f"git show exited {rc}: {err.strip()}")


def read(path):
    with open(path, "rb") as f:
        return f.read().decode("utf-8", "replace")


# ---- session state ----

def state_path(session_id):
    safe = re.sub(r"[^A-Za-z0-9_.-]", "_", str(session_id or "none"))
    return os.path.join(os.environ.get("TMPDIR") or "/tmp", "claude-hooks", safe, "guard-tests.json")


def new_state():
    return {"allowed": [], "turn": None, "noted": [], "calls": {}, "approved": {}, "opened": []}


def load_state(sp):
    st = new_state()
    try:
        with open(sp, encoding="utf-8") as f:
            st.update({k: v for k, v in json.load(f).items() if k in st})
    except FileNotFoundError:
        pass
    except (OSError, ValueError, AttributeError) as e:
        log(f"could not read the session state ({e}); starting it again.")
    return st


def update_state(sp, fn):
    os.makedirs(os.path.dirname(sp), mode=0o700, exist_ok=True)
    with open(sp + ".lock", "a") as lock:
        if fcntl:
            fcntl.flock(lock, fcntl.LOCK_EX)
        st = load_state(sp)
        fn(st)
        tmp = f"{sp}.{os.getpid()}.tmp"
        with open(tmp, "w", encoding="utf-8") as f:
            json.dump(st, f)
        os.replace(tmp, sp)
    return st


# ---- the lines a turn lost ----

def losses(root, sha, st, cfg):
    """The first lost line of each guarded tracked file that differs from `sha`, deleted files included."""
    rc, out, err = git(["diff", "--name-only", "--no-renames", "-z", sha, "--"], root)
    if rc:
        raise CouldNotCheck(f"git diff exited {rc}")
    found = {}
    for rel in filter(None, out.split("\0")):
        if cfg.kind_of(rel) not in ("test", "config"):
            continue
        path = os.path.join(root, rel)
        if path in st["allowed"] or path in st["opened"]:
            continue
        rc, head, _ = git(["show", f"{sha}:{rel}"], root)
        if rc:
            continue
        now = read(path) if os.path.isfile(path) else ""
        lost = (minus([t for _, t in tk.align(head, now, rel).gone], st["approved"].get(path) or {}) or [None])[0]
        if lost is not None:
            found[path] = lost
    return found


def base(cwd):
    """(repository root, HEAD commit) seen from the session's folder, or None outside a repo with a commit."""
    rc, out, _ = git(["rev-parse", "--show-toplevel"], cwd)
    if rc:
        return None
    rc2, sha, _ = git(["rev-parse", "--verify", "-q", "HEAD"], cwd)
    return (os.path.realpath(out.strip()), sha.strip()) if rc2 == 0 and sha.strip() else None


# ---- the events ----

def out_json(obj):
    print(json.dumps(obj))


def ask_text(f, path):
    tail = ("Approve to let Claude change existing lines in this file until the turn ends."
            if f.lost else "Approve it if you asked for this change.")
    return (f"{MOD}: this edit {f.what}: `{f.line.strip()}`. {tail} If it is refused, or nobody can approve it, "
            f"finish the task without this change, then stop and tell the user which test, the old line and why, "
            f"so they can run `/test-guard allow {path}`.")


def own_text(path):
    return (f"{MOD}: this edit changes {path}, which holds the test guard itself or its rules. Approve it if you asked "
            f"for this change. If it is refused, or nobody can approve it, finish the task without this change, then "
            f"stop and tell the user which file and why, so they can run `/test-guard allow {path}`.")


def own_files():
    dirs = [os.path.dirname(HERE)]
    if os.environ.get("CLAUDE_PROJECT_DIR"):
        dirs.append(os.path.join(os.environ["CLAUDE_PROJECT_DIR"], ".claude"))
    own = {canon(os.path.join(HERE, n)) for n in OWN_NAMES}
    own |= {canon(os.path.join(d, n)) for d in dirs for n in ("settings.json", "settings.local.json")}
    return own


def decision(kind, reason):
    out_json({"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": kind,
                                     "permissionDecisionReason": reason}})


def after_edit(tool, inp, path):
    """The text the file will hold, or None when an Edit names a file that does not exist (the tool fails on its own)."""
    if tool == "Write":
        if not isinstance(inp.get("content"), str):
            raise ValueError("Write without content")
        return inp["content"]
    if not os.path.isfile(path):
        return None
    changes = inp.get("edits") if tool == "MultiEdit" else [inp]
    text = read(path)
    for c in changes or []:
        text = apply(text, c)
    return text


def pre(ev, cfg):
    tool, inp = ev.get("tool_name"), ev.get("tool_input") or {}
    if cfg.mode == "off" or tool not in TOOLS or not isinstance(inp.get("file_path"), str):
        return
    try:
        path = canon(absolute(inp["file_path"], ev.get("cwd") or os.getcwd()))
        sp = state_path(ev.get("session_id"))
        st = load_state(sp)
        if path in st["allowed"]:
            return
        if path in own_files():
            return decision("ask", own_text(path))
        if not cfg.maybe_guarded(path):
            return
        where = locate(path)
        kind = cfg.kind_of(where[1]) if where else None
        if not kind:
            return
        head = head_text(*where)
        text = after_edit(tool, inp, path)
        if text is None:
            return
        # In rigs mode the HEAD-line rule never runs, as for an opened path.
        open_ = cfg.mode == "rigs" or path in st["opened"]
        before = read(path) if head is not None and os.path.isfile(path) else ""
        had = [t for _, t in tk.lost_lines(head, before, path)] if head is not None else []
        f = findings(kind, path, head, text, open_, had, cfg, where[1])
        lost = [t for _, t in tk.lost_lines(head, text, path)] if head is not None else []
        # An edit to an opened path runs with no ask; its lost lines are recorded as approved too.
        call = {"path": path, "lost": lost, "opens": bool(f and f.lost)} if (f or (open_ and lost)) else None
        if call and ev.get("tool_use_id"):
            update_state(sp, lambda s: s["calls"].__setitem__(ev["tool_use_id"], call))
        if f:
            decision("ask", ask_text(f, path))
    except Exception as e:  # fail closed: the model retries, or the user allows the file
        decision("deny", f"{MOD}: could not check this edit ({e}). Retry once; if it fails again, stop and tell the user, "
                         f"who can run `/test-guard allow <path>`.")


def post(ev, cfg):
    """An edit that was asked about and ran was approved."""
    tid = ev.get("tool_use_id")
    if cfg.mode == "off" or ev.get("tool_name") not in TOOLS or not tid:
        return

    def record(st):
        c = st["calls"].pop(tid, None)
        if not c:
            return
        if c["opens"] and c["path"] not in st["opened"]:
            st["opened"].append(c["path"])
        if c["lost"]:
            had = st["approved"].setdefault(c["path"], {})
            for l in set(c["lost"]):
                had[l] = max(had.get(l, 0), c["lost"].count(l))
    update_state(state_path(ev.get("session_id")), record)


def prompt(ev, cfg):
    cwd = ev.get("cwd") or os.getcwd()
    sp = state_path(ev.get("session_id"))
    m = re.fullmatch(r"/test-guard(?:\s+(.*))?", (ev.get("prompt") or "").strip(), re.S)
    if m:
        args = (m.group(1) or "").strip()
        verb, _, rest = args.partition(" ")
        rest = rest.strip()
        if not args:
            if cfg.mode != "off":
                p = cfg.picked or {}
                when = f", set {str(p['at'])[:10]}" if p.get("at") else ""
                who = f" by {p['by']}" if p.get("by") else ""
                head = f"Rules: {NAMED[cfg.mode]} in this project{when}{who}."
            else:
                head = "Rules: off in this project (guard_tests.json is missing or says off)."
            paths = load_state(sp)["allowed"]
            text = f"{head}\n" + (f"Allowed: {', '.join(paths)}" if paths else f"No path is allowed. {USAGE}")
        elif verb == "allow" and rest:
            path = canon(absolute(rest, cwd))
            update_state(sp, lambda s: s["allowed"].append(path) if path not in s["allowed"] else None)
            text = f"Claude can now change the HEAD lines of {path} for the rest of this session."
        else:
            text = USAGE
        return out_json({"decision": "block", "reason": text,
                         "hookSpecificOutput": {"hookEventName": "UserPromptSubmit", "suppressOriginalPrompt": True}})

    # Any other submitted prompt starts a turn: note HEAD and the lines already lost. Only the turn check needs it.
    if cfg.mode != "full":
        return

    def start(st):
        st["calls"], st["opened"], st["turn"] = {}, [], None
        try:
            b = base(cwd)
            if b:
                st["turn"] = {"root": b[0], "sha": b[1], "had": sorted(losses(b[0], b[1], st, cfg).items())}
        except Exception as e:
            log(f"could not read HEAD at the turn's start ({e}); this turn's Bash changes are not checked.")
    update_state(sp, start)


def stop(ev, cfg):
    if cfg.mode != "full" or ev.get("stop_hook_active"):
        return
    sp = state_path(ev.get("session_id"))
    st = load_state(sp)
    t = st["turn"]
    if not t:
        return
    try:
        # A commit in the turn keeps the turn's HEAD as the base; a checkout or a reset takes the new one.
        sha, now = t["sha"], base(t["root"])
        if now and now[1] != sha and git(["merge-base", "--is-ancestor", sha, now[1]], t["root"])[0] != 0:
            sha = now[1]
        had = {p: l for p, l in t["had"]}
        fresh = [(p, l) for p, l in losses(t["root"], sha, st, cfg).items()
                 if had.get(p) != l and f"{p}\n{l}" not in st["noted"]]
        if not fresh:
            return
        update_state(sp, lambda s: s["noted"].extend(f"{p}\n{l}" for p, l in fresh))
        log(f"this turn changed or removed HEAD lines of {', '.join(p for p, _ in fresh)} outside Edit and Write.")
        listed = "; ".join(f"{p}: `{l.strip()}`" for p, l in fresh)
        out_json({"decision": "block", "reason":
                  f"{MOD}: this turn changed or removed a line that a test or runner config has at HEAD, outside Edit and "
                  f"Write: {listed}. If the user did not ask for it, put the line back; if the test is wrong, stop and tell "
                  f"the user which test, the old line and why."})
    except Exception as e:
        log(f"could not compare the test files with HEAD ({e}).")


EVENTS = {"PreToolUse": pre, "PostToolUse": post, "UserPromptSubmit": prompt, "Stop": stop}


def main():
    try:
        ev = json.load(sys.stdin)
        handler = EVENTS.get(ev.get("hook_event_name"))
    except (ValueError, AttributeError):
        return 0
    if handler:
        try:
            handler(ev, load_config(os.path.join(HERE, "guard_tests.json")))
        except Exception as e:  # the edit check has its own fail-closed catch; the rest fail open
            log(f"{ev.get('hook_event_name')} failed ({e}).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
