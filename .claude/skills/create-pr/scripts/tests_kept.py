#!/usr/bin/env python3
"""Fails when a diff weakens the tests, whatever tool made the change.

Usage: python3 -I tests_kept.py [--base <ref>]

The base defaults to the merge-base of HEAD with the remote default branch: origin/HEAD, else
origin/$GITHUB_BASE_REF, origin/main or origin/master. The other side is the working tree:
tracked and untracked files that git does not ignore. So a Bash edit, a commit and an unstaged
change all count.

exit 0  one line: tests kept: N tests, M assertions (base: N0, M0)
exit 1  one line per finding: path:line: <cause>
exit 2  could not check: one line saying why

Findings:
- a non-blank line that a base test file has is changed, moved or removed, or the file is gone
  (the line rule of guard_tests.py, here against the base instead of HEAD)
- a test file has fewer tests or assertions than at the base
- an added line that rigs the run: a skip, only or xfail marker; an early exit in a test file or
  conftest.py; a pytest report or collection hook; an __eq__ override in a test file; `|| true`
  after a test command or continue-on-error in CI config or scripts; a hook bypass; a runner
  config that ignores tests or preloads code

A test that is truly wrong still fails here. Stop and tell the owner which test and why; the owner
overrides by merging. There is no allow file, because the agent could write it.

Counts are regex matches per line, so they miss tests made at run time (parametrize, test.each
rows, table rows in Go), assertions inside helpers, tests in non-test files (Rust #[test] in src)
and shell checks inside a helper function. Strings are blanked before matching in Python, JS/TS,
Go and Rust, so a marker inside a string literal is not a finding.
"""
import argparse
import collections
import difflib
import os
import re
import subprocess
import sys

# A test file. guard_tests.py (the edit hook, before an edit) imports this, so the hook and this
# check (after it lands) agree; a project adds its own folders and names to the hook's config.
TEST_NAME = [re.compile(p) for p in (r"\.(test|spec)\.[^/]+$", r"(^|/)test_[^/]*\.(py|sh)$", r"_test\.(py|go)$",
                                     r"(^|/)conftest\.py$", r"\.(snap|golden)$")]
TEST_DIR = {"test", "tests", "__tests__", "e2e", "__snapshots__", "testdata"}


def is_test_name(path, extra_names=()):
    """A test file by its name alone. extra_names: regexes a project adds."""
    return any(p.search(path) for p in TEST_NAME) or any(re.search(p, path) for p in extra_names)


def in_test_dir(path, extra_dirs=()):
    return any(s in TEST_DIR or s in extra_dirs for s in path.split("/")[:-1])


def is_test(path, extra_dirs=(), extra_names=()):
    return is_test_name(path, extra_names) or in_test_dir(path, extra_dirs)


LANG = {".py": "py", ".go": "go", ".rs": "rs", ".sh": "sh", ".bash": "sh", ".bats": "sh"}
LANG.update(dict.fromkeys([".js", ".jsx", ".ts", ".tsx", ".mjs", ".cjs", ".mts", ".cts"], "js"))


def lang(path):
    return LANG.get(os.path.splitext(path)[1], "")


C = re.compile
TESTS = {
    "py": [C(r"^\s*(?:async\s+)?def\s+test\w*\s*\("), C(r"^\s*case\(")],
    "js": [C(r"^\s*(?:it|test)(?:\.\w+)*\s*\(")],
    "go": [C(r"^func\s+(?:Test|Fuzz|Example)\w*\s*\("), C(r"\bt\.Run\(")],
    "rs": [C(r"#\[(?:\w+::)*test\]")],
    "sh": [C(r"\b(?:echo|printf)\s+[\"']?ok\b"), C(r"\bpass=\$\(\(\s*pass\s*\+\s*1\s*\)\)")],
}
ASSERTS = {
    "py": [C(r"^\s*assert\b"), C(r"\.assert\w*\("), C(r"\bpytest\.raises\(")],
    "js": [C(r"\bexpect\("), C(r"\bassert(?:\.\w+)?\(")],
    "go": [C(r"\bt\.(?:Error|Errorf|Fatal|Fatalf|Fail|FailNow)\("), C(r"\b(?:assert|require)\.\w+\(")],
    "rs": [C(r"\bassert(?:_eq|_ne)?!\(")],
    "sh": [C(r"\[\[\s"), C(r"^\s*\[\s"), C(r"\bcase\b")],
}
STRING = C(r"\"(?:\\.|[^\"\\])*\"|'(?:\\.|[^'\\])*'|`(?:\\.|[^`\\])*`")

CI_FILE = C(r"(^|/)(\.github/workflows/[^/]+\.ya?ml|\.gitlab-ci\.yml|\.circleci/[^/]+|azure-pipelines\.yml|"
            r"\.travis\.yml|bitbucket-pipelines\.yml|Jenkinsfile|\.buildkite/[^/]+)$")
SCRIPT_FILE = C(r"(^|/)(package\.json|[Mm]akefile|justfile|Taskfile\.ya?ml|tox\.ini|[^/]+\.(sh|bash))$")
RUNNER_CONFIG = C(r"(^|/)(package\.json|pytest\.ini|pyproject\.toml|setup\.cfg|tox\.ini|bunfig\.toml|"
                  r"(jest|vitest|playwright)\.config\.[^/]+)$")
TEST_WORD = C(r"\b(?:test\w*|pytest|jest|vitest|mocha|cargo|tox|nox|check\w*|smoke|verify|spec)\b")
BYPASS = "--no-" "verify"  # split so this file is not a finding of its own

# (cause, regex, which files, blank strings first)
RIGS = [
    ("adds a skip, only or xfail marker",
     C(r"@pytest\.mark\.(?:skip|skipif|xfail)\b|\bpytest\.(?:skip|xfail|importorskip)\(|@unittest\.(?:skip\w*|expectedFailure)\b|"
       r"\.skipTest\(|\bSkipTest\b|\b(?:it|test|describe|suite|context)\.(?:skip|only|todo|failing|fails)\b|"
       r"^\s*(?:xit|xtest|xdescribe|fit|fdescribe)\s*\(|\bt\.Skip(?:Now|f)?\(|#\[ignore\]"), "test", True),
    ("adds an early exit to a test file",
     C(r"\bos\._exit\(|\bsys\.exit\(\s*(?:0|None|True|False)?\s*\)|^\s*(?:exit|quit)\(\s*0?\s*\)|"
       r"^\s*raise\s+SystemExit\b(?:\(\s*(?:0|None)?\s*\))?\s*(?:#.*)?$|"
       r"\bprocess\.exit\(\s*0?\s*\)|\bprocess\.exitCode\s*=\s*0\b|\bos\.Exit\(0\)"), "test", True),
    ("adds an early exit to a shell test", C(r"(?:^\s*|[;&|{]\s*|\b(?:then|do|else)\s+)exit\s+0\b"), "shtest", False),
    ("adds a pytest report or collection hook",
     C(r"\bpytest_runtest_makereport\b|\bpytest_collection_modifyitems\b|\bpytest_ignore_collect\b|\bTestReport\b|"
       r"\.force_(?:result|exception)\(|\.outcome\s*=(?!=)"), "py", True),
    ("adds an __eq__ override to a test file", C(r"\bdef\s+__(?:eq|ne)__\s*\(|\basymmetricMatch\b"), "test", True),
    ("adds continue-on-error to CI config", C(r"\bcontinue-on-error\s*:\s*true\b"), "ci", False),
    ("ignores a test command's failure (|| true)",
     C(r"\|\|\s*(?:true|:)(?=$|[\s;)\"'&|])|\|\|\s*exit\s+0\b"), "ciscript", False),
    ("adds a hook bypass (" + BYPASS + ")",
     C(re.escape(BYPASS) + r"|\bHUSKY=0\b|\bgit\s+commit\b[^|;&]*\s-n\b"), "nondoc", False),
    ("adds runner config that ignores tests or preloads code",
     C(r"passWithNoTests|testPathIgnorePatterns|--deselect\b|^\s*addopts\b|^\s*preload\s*="), "config", False),
]


def scope_hit(scope, path):
    if scope == "test":
        return is_test(path) or os.path.basename(path) == "conftest.py"
    if scope == "shtest":
        return is_test(path) and lang(path) == "sh"
    if scope == "py":
        return lang(path) == "py"
    if scope == "ci":
        return bool(CI_FILE.search(path))
    if scope == "ciscript":
        return bool(CI_FILE.search(path) or SCRIPT_FILE.search(path))
    if scope == "config":
        return bool(RUNNER_CONFIG.search(path))
    return not path.endswith(".md")  # nondoc


def code(line, path):
    """The line with string literals blanked, in languages where a marker inside a string is data."""
    return STRING.sub('""', line) if lang(path) in ("py", "js", "go", "rs") else line


def count(text, path):
    """(tests, assertions) in one test file."""
    lg = lang(path)
    tests = asserts = 0
    for line in text.splitlines():
        line = code(line, path)
        tests += sum(len(p.findall(line)) for p in TESTS.get(lg, []))
        asserts += sum(len(p.findall(line)) for p in ASSERTS.get(lg, []))
    return tests, asserts


COMMENTED = {".py": "py"}
COMMENTED.update(dict.fromkeys([".js", ".jsx", ".ts", ".tsx", ".mjs", ".cjs", ".mts", ".cts", ".go", ".java", ".kt", ".kts",
                                ".scala", ".swift", ".rs", ".c", ".cc", ".cpp", ".h", ".hpp", ".cs", ".dart"], "c"))
WORKFLOW = re.compile(r"(^|/)\.github/workflows/")
USES = re.compile(r"^(\s*(?:-\s+)?uses:\s*)([^@\s#]+)@\S+(?:\s+#.*)?$")


def hidden(lines, kind):
    """For each line: does it start inside a block comment or a string that spans lines? kind: "c", "py" or "".
    Quotes that end at the line end are skipped, so `'**/*.ts'` opens no comment. It reads quotes and
    comments, not regular expressions."""
    if not kind:
        return [False] * len(lines)
    out, open_ = [], ""
    for line in lines:
        out.append(open_ != "")
        i, n = 0, len(line)
        while i < n:
            if open_:
                if line[i] == "\\" and open_ != "*/":
                    i += 1
                elif line.startswith(open_, i):
                    i += len(open_) - 1
                    open_ = ""
                i += 1
                continue
            two, three = line[i:i + 2], line[i:i + 3]
            if (kind == "c" and two == "//") or (kind == "py" and line[i] == "#"):
                break
            if kind == "c" and two == "/*":
                open_ = "*/"
                i += 1
            elif kind == "c" and line[i] == "`":
                open_ = "`"
            elif kind == "py" and three in ('"""', "'''"):
                open_ = three
                i += 2
            elif line[i] in "\"'":
                q = line[i]
                i += 1
                while i < n and line[i] != q:
                    if line[i] == "\\":
                        i += 1
                    i += 1
            i += 1
    return out


def same_step(a, b):
    """Two workflow lines `uses: <name>@<ref>` with the same name: a new ref or comment keeps the line."""
    x, y = USES.match(a), USES.match(b)
    return bool(x and y and x.group(1) == y.group(1) and x.group(2) == y.group(2))


Aligned = collections.namedtuple("Aligned", "gone commented added lines")


def align(base, now, path=""):
    """Matches each non-blank base line, in order, to a line of `now` with the same text. With a path, a
    line must also keep its comment state (a line moved into a block comment is lost), and in a workflow
    file a `uses:` step keeps its line under a new ref. gone: (base line number, text) of each base line
    with no match; commented: the first one is still there, inside a comment; added: per line of `now`,
    whether no base line took it."""
    lines, kind = now.splitlines(), COMMENTED.get(os.path.splitext(path)[1], "")
    nh = hidden(lines, kind)
    base_lines = base.splitlines()
    bh = hidden(base_lines, kind)
    steps = bool(WORKFLOW.search(path.replace("\\", "/")))
    added = [True] * len(lines)
    gone, commented, at = [], False, 0
    for i, line in enumerate(base_lines):
        if not line.strip():
            continue
        j = at
        while j < len(lines) and not ((lines[j] == line or (steps and same_step(line, lines[j]))) and nh[j] == bh[i]):
            j += 1
        if j < len(lines):
            added[j] = False
            at = j + 1
            continue
        if not gone:
            commented = not bh[i] and any(l == line and nh[k] for k, l in enumerate(lines))
        gone.append((i + 1, line))
    return Aligned(gone, commented, added, lines)


def lost_lines(base, now, path=""):
    """(base line number, text) of each non-blank base line not kept in order: the line rule of guard_tests.py."""
    return align(base, now, path).gone


def added_lines(base, now):
    """(line number in now, text) of each line the diff adds."""
    a, b = base.splitlines(), now.splitlines()
    out = []
    for tag, _i1, _i2, j1, j2 in difflib.SequenceMatcher(None, a, b, autojunk=False).get_opcodes():
        if tag in ("insert", "replace"):
            out += [(j + 1, b[j]) for j in range(j1, j2)]
    return out


def rigs(path, added):
    found = []
    for n, line in added:
        for cause, regex, scope, blank in RIGS:
            if not scope_hit(scope, path):
                continue
            text = code(line, path) if blank else line
            if scope == "ciscript" and not (CI_FILE.search(path) or TEST_WORD.search(text)):
                continue
            if regex.search(text):
                found.append((path, n, f"{cause}: `{line.strip()}`"))
    return found


def check(base_files, now_files):
    """Pure core. Each argument maps path -> text. Returns (findings, (N0, M0), (N, M))."""
    findings = []
    n0 = m0 = n1 = m1 = 0
    for path in sorted(p for p in base_files if is_test(p)):
        t0, a0 = count(base_files[path], path)
        n0, m0 = n0 + t0, m0 + a0
        if path not in now_files:
            findings.append((path, 1, f"test file deleted or renamed (base: {t0} tests, {a0} assertions)"))
            continue
        for n, line in lost_lines(base_files[path], now_files[path]):
            findings.append((path, n, f"changes or removes base line {n}: `{line.strip()}`"))
        t1, a1 = count(now_files[path], path)
        if t1 < t0:
            findings.append((path, 1, f"tests dropped from {t0} to {t1}"))
        if a1 < a0:
            findings.append((path, 1, f"assertions dropped from {a0} to {a1}"))
    for path in sorted(now_files):
        if is_test(path):
            t1, a1 = count(now_files[path], path)
            n1, m1 = n1 + t1, m1 + a1
        if now_files[path] != base_files.get(path):
            findings += rigs(path, added_lines(base_files.get(path, ""), now_files[path]))
    if n1 < n0:
        findings.append(("(all test files)", 0, f"tests dropped from {n0} to {n1}"))
    if m1 < m0:
        findings.append(("(all test files)", 0, f"assertions dropped from {m0} to {m1}"))
    return findings, (n0, m0), (n1, m1)


class CannotCheck(Exception):
    pass


def git(root, *args, data=None):
    r = subprocess.run(["git", *args], cwd=root, input=data, capture_output=True)
    if r.returncode:
        raise CannotCheck(f"git {' '.join(args)}: {r.stderr.decode(errors='replace').strip() or 'failed'}")
    return r.stdout


def default_base(root):
    names = []
    head = subprocess.run(["git", "symbolic-ref", "-q", "--short", "refs/remotes/origin/HEAD"], cwd=root,
                          capture_output=True, text=True).stdout.strip()
    if head:
        names.append(head)
    if os.environ.get("GITHUB_BASE_REF"):
        names.append("origin/" + os.environ["GITHUB_BASE_REF"])
    names += ["origin/main", "origin/master"]
    for name in names:
        r = subprocess.run(["git", "merge-base", "HEAD", name], cwd=root, capture_output=True, text=True)
        if r.returncode == 0:
            return r.stdout.strip()
    raise CannotCheck("no base: no origin/HEAD, origin/main or origin/master to compare with; pass --base <ref>")


def read_base(root, base, paths):
    """path -> text at base, from one git cat-file --batch."""
    if not paths:
        return {}
    out = git(root, "cat-file", "--batch", data="".join(f"{base}:{p}\n" for p in paths).encode())
    texts, at = {}, 0
    for p in paths:
        nl = out.index(b"\n", at)
        header = out[at:nl].split()
        at = nl + 1
        if len(header) == 3 and header[1] == b"blob":
            size = int(header[2])
            texts[p] = out[at:at + size].decode(errors="replace")
            at += size + 1
    return texts


def read_now(root, path):
    full = os.path.join(root, path)
    if os.path.islink(full) or not os.path.isfile(full):
        return None
    with open(full, "rb") as f:
        return f.read().decode(errors="replace")


def gather(root, base):
    z = lambda b: [p for p in b.decode(errors="replace").split("\0") if p]  # noqa: E731
    base_tests = [p for p in z(git(root, "ls-tree", "-r", "-z", "--name-only", base)) if is_test(p)]
    tracked_now = z(git(root, "ls-files", "-z", "--cached", "--others", "--exclude-standard"))
    changed = set(z(git(root, "diff", "--name-only", "--no-renames", "-z", base)))
    changed |= set(z(git(root, "ls-files", "-z", "--others", "--exclude-standard")))
    want_now = sorted({p for p in tracked_now if is_test(p)} | changed)
    now = {}
    for p in want_now:
        text = read_now(root, p)
        if text is not None:
            now[p] = text
    # An unchanged file needs no base text: it is a test file (read here) or not in now.
    return read_base(root, base, sorted(set(base_tests) | (changed & set(now)))), now


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--base", help="ref to compare with (default: merge-base with the remote default branch)")
    args = ap.parse_args(argv)
    try:
        top = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True)
        if top.returncode:
            raise CannotCheck("not a git repository")
        root = top.stdout.strip()
        if args.base:
            r = subprocess.run(["git", "rev-parse", "--verify", "-q", args.base + "^{commit}"], cwd=root,
                               capture_output=True, text=True)
            if r.returncode:
                raise CannotCheck(f"no base: {args.base} is not a commit")
            base = r.stdout.strip()
        else:
            base = default_base(root)
        base_files, now = gather(root, base)
    except CannotCheck as e:
        print(f"tests kept: could not check: {e}")
        return 2
    findings, (n0, m0), (n1, m1) = check(base_files, now)
    if findings:
        for path, n, cause in findings:
            print(f"{path}:{n}: {cause}")
        print("A test that is truly wrong: stop and tell the owner which test and why; the owner overrides by merging.",
              file=sys.stderr)
        return 1
    print(f"tests kept: {n1} tests, {m1} assertions (base: {n0}, {m0})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
