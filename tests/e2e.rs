use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::Command as Bin;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

const WEEK_37: &str = "2026-09-08T12:00:00+00:00";
const WEEK_38: &str = "2026-09-15T12:00:00+00:00";

struct Repo {
    dir: TempDir,
}

impl Repo {
    fn new() -> Self {
        let repo = Repo { dir: TempDir::new().unwrap() };
        repo.git(&["init", "-q", "-b", "main"], WEEK_37);
        repo
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn write(&self, rel: &str, contents: &str) -> &Self {
        let p = self.path().join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, contents).unwrap();
        self
    }

    fn commit(&self, msg: &str, date: &str) -> &Self {
        self.git(&["add", "-A"], date);
        self.git(&["commit", "-q", "-m", msg], date);
        self
    }

    fn git(&self, args: &[&str], date: &str) {
        let status = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
            .args(["-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null"])
            .args(args)
            .current_dir(self.path())
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_DATE", date)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    }

    fn out(&self) -> PathBuf {
        self.path().join(".archsnap")
    }

    fn snapshot(&self, week: &str) -> Value {
        let p = self.out().join("snapshots").join(format!("{week}.json"));
        serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap()
    }
}

fn archsnap(dir: &Path) -> Bin {
    let mut cmd = Bin::cargo_bin("archsnap").unwrap();
    cmd.current_dir(dir).env("NO_COLOR", "1").arg("run");
    cmd
}

/// Two weeks of history: week 37 has src/a -> src/b, week 38 adds src/c.
fn two_week_repo() -> Repo {
    let repo = Repo::new();
    repo.write("src/a/index.ts", "import { b } from '../b/util.js';\nexport const a = b;\n")
        .write("src/b/util.ts", "export const b = 1;\n")
        .write("node_modules/pkg/index.js", "module.exports = 1;\n")
        .write("src/types.d.ts", "declare const x: number;\n")
        .commit("week 37", WEEK_37);
    repo.write(
        "src/c/main.tsx",
        "import React from 'react';\nimport { a } from '../a';\nexport const c = () => a;\n",
    )
    .commit("week 38", WEEK_38);
    repo
}

#[test]
fn builds_weekly_snapshots_and_report() {
    let repo = two_week_repo();

    archsnap(repo.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("[1/2]"))
        .stderr(predicate::str::contains("[2/2]"));

    let w37 = repo.snapshot("2026-W37");
    let w38 = repo.snapshot("2026-W38");
    assert!(w37["modules"].get("src/a").is_some());
    assert!(w37["modules"].get("src/c").is_none());
    assert!(w38["modules"].get("src/c").is_some());
    assert!(w38["modules"].get("node_modules/pkg").is_none(), "node_modules must be ignored");
    assert_eq!(w38["stats"]["files"], 3, ".d.ts and node_modules are not counted");

    let edges = w38["edges"].as_array().unwrap();
    let has = |from: &str, to: &str| edges.iter().any(|e| e["from"] == from && e["to"] == to);
    assert!(has("src/a", "src/b"), ".js specifier resolves to the .ts file");
    assert!(has("src/c", "src/a"), "directory import resolves to index.ts");
    assert_eq!(w38["externals"]["react"], 1);

    let html = fs::read_to_string(repo.out().join("index.html")).unwrap();
    assert!(html.contains("2026-W37") && html.contains("2026-W38"));
    assert!(html.contains("src/c"));
}

#[test]
fn second_run_reuses_cached_snapshots() {
    let repo = two_week_repo();
    archsnap(repo.path()).assert().success();
    archsnap(repo.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("cached"));
}

#[test]
fn not_a_git_repo_fails_with_exit_2() {
    let dir = TempDir::new().unwrap();
    archsnap(dir.path())
        .assert()
        .code(2)
        .stderr(predicate::str::contains("archsnap(not_git_repo)"))
        .stderr(predicate::str::contains("--repo"));
}

#[test]
fn repo_without_commits_fails_with_exit_2() {
    let repo = Repo::new();
    archsnap(repo.path())
        .assert()
        .code(2)
        .stderr(predicate::str::contains("archsnap(no_commits)"));
}

#[test]
fn unwritable_output_fails_with_exit_3_before_scanning() {
    let repo = two_week_repo();
    let blocker = repo.path().join("not-a-dir");
    fs::write(&blocker, "file in the way").unwrap();
    archsnap(repo.path())
        .arg("--out")
        .arg(&blocker)
        .assert()
        .code(3)
        .stderr(predicate::str::contains("archsnap(output_not_writable)"))
        .stderr(predicate::str::contains("[1/").not());
}

#[test]
fn syntax_error_skips_file_and_points_at_it() {
    let repo = two_week_repo();
    repo.write("src/b/broken.ts", "export function f( {\n  return 1\n")
        .commit("broken", WEEK_38);

    archsnap(repo.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("archsnap(parse_skipped)"))
        .stderr(predicate::str::contains("src/b/broken.ts:"));

    let w38 = repo.snapshot("2026-W38");
    assert_eq!(w38["stats"]["skipped"], 1);
    assert_eq!(w38["stats"]["files"], 3, "the broken file is not counted");
}

#[test]
fn strict_turns_warnings_into_exit_1_but_still_writes_report() {
    let repo = two_week_repo();
    repo.write("src/b/broken.ts", "export function f( {\n").commit("broken", WEEK_38);

    archsnap(repo.path()).arg("--strict").assert().code(1);
    assert!(repo.out().join("index.html").exists());
}

#[test]
fn unresolved_relative_import_warns_with_specifier() {
    let repo = two_week_repo();
    repo.write("src/c/extra.ts", "import { x } from './missing';\nexport const y = x;\n")
        .commit("unresolved", WEEK_38);

    archsnap(repo.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("archsnap(unresolved_import)"))
        .stderr(predicate::str::contains("./missing"));

    assert_eq!(repo.snapshot("2026-W38")["stats"]["unresolved"], 1);
}

#[test]
fn shallow_clone_degrades_to_current_snapshot_only() {
    let origin = two_week_repo();
    let parent = TempDir::new().unwrap();
    let url = format!("file://{}", origin.path().display());
    let status = Command::new("git")
        .args(["clone", "-q", "--depth", "1", &url, "clone"])
        .current_dir(parent.path())
        .status()
        .unwrap();
    assert!(status.success());
    let clone = parent.path().join("clone");

    archsnap(&clone)
        .assert()
        .success()
        .stderr(predicate::str::contains("archsnap(shallow_clone)"))
        .stderr(predicate::str::contains("fetch-depth: 0"));

    let snaps: Vec<_> = fs::read_dir(clone.join(".archsnap/snapshots")).unwrap().collect();
    assert_eq!(snaps.len(), 1);
}

#[test]
fn corrupt_cached_snapshot_is_rebuilt() {
    let repo = two_week_repo();
    archsnap(repo.path()).assert().success();
    let p = repo.out().join("snapshots/2026-W37.json");
    fs::write(&p, "{ not json").unwrap();

    archsnap(repo.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("archsnap(snapshot_corrupt)"));
    assert!(repo.snapshot("2026-W37")["modules"].get("src/a").is_some());
}

#[test]
fn jsx_in_plain_js_files_parses() {
    let repo = two_week_repo();
    repo.write("src/ui/button.js", "import { a } from '../a';\nexport const B = () => <div>{a}</div>;\n")
        .commit("jsx in js", WEEK_38);

    archsnap(repo.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("parse_skipped").not());
    assert!(repo.snapshot("2026-W38")["modules"].get("src/ui").is_some());
}

#[test]
fn imports_of_committed_but_unscanned_files_are_not_unresolved() {
    let repo = two_week_repo();
    let big = format!("export const x = 1;\n{}", "// padding\n".repeat(60_000));
    repo.write("src/big/huge.ts", &big)
        .write("src/c/types.d.ts", "export type T = number;\n")
        .write("src/c/Widget.vue", "<template><div/></template>\n")
        .write(
            "src/c/uses.ts",
            "import type { T } from './types';\nimport { x } from '../big/huge';\nimport W from './Widget.vue';\nexport const u: T = x;\n",
        )
        .commit("unscanned targets", WEEK_38);

    archsnap(repo.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("unresolved_import").not());
    assert_eq!(repo.snapshot("2026-W38")["stats"]["unresolved"], 0);
}

#[test]
fn symlinks_are_not_parsed_as_source() {
    let repo = two_week_repo();
    std::os::unix::fs::symlink("../a/index.ts", repo.path().join("src/b/link.ts")).unwrap();
    repo.commit("symlink", WEEK_38);

    archsnap(repo.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("parse_skipped").not());
}

#[test]
fn missing_git_object_fails_with_exit_3() {
    let repo = two_week_repo();
    let out = Command::new("git")
        .args(["rev-parse", "HEAD:src/b/util.ts"])
        .current_dir(repo.path())
        .output()
        .unwrap();
    let oid = String::from_utf8(out.stdout).unwrap().trim().to_string();
    fs::remove_file(repo.path().join(".git/objects").join(&oid[..2]).join(&oid[2..])).unwrap();

    archsnap(repo.path())
        .assert()
        .code(3)
        .stderr(predicate::str::contains("archsnap(git_failed)"));
}

#[test]
fn zero_depth_and_zero_weeks_are_rejected() {
    let repo = two_week_repo();
    archsnap(repo.path()).args(["--depth", "0"]).assert().code(2);
    archsnap(repo.path()).args(["--weeks", "0"]).assert().code(2);
}

#[test]
fn first_week_does_not_mark_everything_new() {
    let repo = two_week_repo();
    archsnap(repo.path()).assert().success();
    let html = fs::read_to_string(repo.out().join("index.html")).unwrap();
    let data = html.split("type=\"application/json\">").nth(1).unwrap().split("</script>").next().unwrap();
    let report: Value = serde_json::from_str(data).unwrap();
    let oldest = report["weeks"].as_array().unwrap().last().unwrap();
    assert_eq!(oldest["diff"]["added_modules"].as_array().unwrap().len(), 0);
    assert_eq!(oldest["diff"]["added_edges"].as_array().unwrap().len(), 0);
}
