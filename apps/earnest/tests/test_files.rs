//! Test files that use the library are not model files: a run skips them, says so, and exits 0.

use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::time::Duration;

const COUNTER: &str = "\
import { m } from \"@earnest/model\";

export const Counter = m.model(\"Counter\", {
\tcount: m.int(0, 3).initial(0),
});

export const increment = Counter.action(\"the counter goes up by one\", {
\twhen: (s) => s.count < 3,
\tthen: (s) => ({ count: s.count + 1 }),
});

export const bounded = Counter.rule({
\tsays: \"The count is never above three\",
\tholds: (s) => s.count <= 3,
});

export const full = Counter.end({
\tsays: \"The counter is full\",
\twhen: (s) => s.count === 3,
});
";
const VITEST: &str = "\
import { m } from \"@earnest/model\";
import { expect, test } from \"vitest\";

test(\"replays a path\", () => {
\texpect(m).toBeDefined();
});
";
const RUST_TEST: &str = "use earnest::model::Model;\n\n#[test]\nfn replays() {}\n";
const HANG_GUARD: Duration = Duration::from_mins(5);

fn app() -> tempfile::TempDir {
    let folder = tempfile::tempdir().expect("a temporary folder can be made");
    for (relative, text) in [
        ("src/counter.model.ts", COUNTER),
        ("src/counter.test.ts", VITEST),
        ("src/counter.spec.mts", VITEST),
        ("scripts/tests/replay.ts", VITEST),
        ("crates/app/tests/replay.rs", RUST_TEST),
        ("crates/app/build.rs", RUST_TEST),
        ("src/plain.test.ts", "export const one = 1;\n"),
    ] {
        let target = folder.path().join(relative);
        fs::create_dir_all(target.parent().expect("a file has a folder"))
            .expect("a folder can be made");
        fs::write(target, text).expect("a file can be written");
    }
    folder
}

fn earnest(folder: &Path, arguments: &[&str]) -> (Option<i32>, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_earnest"))
        .current_dir(folder)
        .timeout(HANG_GUARD)
        .args(arguments)
        .output()
        .expect("the binary runs");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn a_bare_run_skips_the_test_files_that_use_the_library_and_says_so() {
    let folder = app();
    let (code, stdout, stderr) = earnest(folder.path(), &["verify", "--no-cache"]);
    assert_eq!(code, Some(0), "the exit code: {stdout}{stderr}");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.contains("unreadable"))
            .count(),
        0,
        "no file is unreadable: {stdout}"
    );
    let skipped = lines
        .iter()
        .position(|line| {
            line.starts_with("skipped") && line.contains("5 test files that use the library")
        })
        .unwrap_or_else(|| panic!("a skipped line is in the output: {stdout}"));
    assert_eq!(
        lines
            .get(skipped + 1)
            .map(|line| line.starts_with("1 model:")),
        Some(true),
        "the skipped line comes right before the summary: {stdout}"
    );
}

#[test]
fn the_json_lists_the_skipped_paths_and_the_summary_ignores_them() {
    let folder = app();
    let (code, stdout, _) = earnest(folder.path(), &["verify", "--no-cache", "--json"]);
    assert_eq!(code, Some(0), "the exit code");
    let document: Value = serde_json::from_str(&stdout).expect("the output is JSON");
    let mut skipped: Vec<&str> = document["skipped"]
        .as_array()
        .expect("`skipped` is a list")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    skipped.sort_unstable();
    assert_eq!(
        skipped,
        [
            "crates/app/build.rs",
            "crates/app/tests/replay.rs",
            "scripts/tests/replay.ts",
            "src/counter.spec.mts",
            "src/counter.test.ts",
        ],
        "the test files that use the library, and not the one that does not"
    );
    assert_eq!(
        document["unreadable"],
        serde_json::json!([]),
        "nothing is unreadable"
    );
    assert_eq!(document["summary"]["models"], 1, "one model");
}

#[test]
fn a_run_with_no_test_file_has_an_empty_skipped_list_and_no_line() {
    let folder = tempfile::tempdir().expect("a temporary folder can be made");
    fs::write(folder.path().join("counter.model.ts"), COUNTER).expect("a file can be written");
    let (_, stdout, _) = earnest(folder.path(), &["verify", "--no-cache"]);
    assert!(!stdout.contains("skipped"), "no line: {stdout}");
    let (_, json, _) = earnest(folder.path(), &["verify", "--no-cache", "--json"]);
    let document: Value = serde_json::from_str(&json).expect("the output is JSON");
    assert_eq!(document["skipped"], serde_json::json!([]), "an empty list");
}

#[test]
fn a_file_named_on_the_command_line_is_read_whatever_its_name() {
    let folder = app();
    let (code, stdout, _) = earnest(
        folder.path(),
        &["verify", "--no-cache", "src/counter.test.ts"],
    );
    assert_eq!(code, Some(1), "the file is read and refused: {stdout}");
    assert!(
        stdout.contains("unreadable  src/counter.test.ts"),
        "it is unreadable, not skipped: {stdout}"
    );
    assert!(!stdout.contains("skipped"), "nothing is skipped: {stdout}");
}

#[test]
fn the_report_names_the_skipped_files() {
    let folder = app();
    let (code, stdout, _) = earnest(folder.path(), &["report"]);
    assert_eq!(code, Some(0), "the exit code: {stdout}");
    assert!(
        stdout.contains("## Skipped files\n\n5 test files that use the library were not read:"),
        "a section names them: {stdout}"
    );
    assert!(
        stdout.contains("scripts/tests/replay.ts"),
        "and lists the paths: {stdout}"
    );
}
