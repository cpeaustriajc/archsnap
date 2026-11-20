//! `earnest confirm --tla-rs <program>` runs the program on each exported module and compares what
//! it prints with the checker, as `--tlc` does. The program here is a small script that prints
//! what tla-rs prints, so the tests need neither Java nor the real program.

#![cfg(unix)]

use assert_cmd::Command;
use std::fs;
use std::os::unix::fs::PermissionsExt;
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
const HANG_GUARD: Duration = Duration::from_mins(5);

fn program(folder: &Path, states: usize, violates: &str) -> std::path::PathBuf {
    let path = folder.join("fake-tla-rs");
    let text = format!(
        "#!/bin/sh\n\
         echo \"$@\" >> \"$PWD/arguments.log\"\n\
         config=\"$3\"\n\
         if [ -n \"{violates}\" ] && grep -q \"INVARIANT {violates}\" \"$config\"; then\n\
         echo \"Error: Invariant {violates} is violated.\"\n\
         fi\n\
         echo \"{states} states generated, {states} distinct states found, 0 states left on queue.\"\n"
    );
    fs::write(&path, text).expect("the script can be written");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("the script can run");
    path
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

fn with_counter() -> tempfile::TempDir {
    let folder = tempfile::tempdir().expect("a temporary folder can be made");
    fs::write(folder.path().join("counter.model.ts"), COUNTER).expect("a file can be written");
    folder
}

#[test]
fn a_program_that_agrees_with_the_checker_confirms_the_model() {
    let folder = with_counter();
    let fake = program(folder.path(), 4, "");
    let (code, stdout, stderr) = earnest(
        folder.path(),
        &["confirm", "--tla-rs", &fake.to_string_lossy()],
    );
    assert_eq!(code, Some(0), "the exit code: {stdout}{stderr}");
    assert_eq!(
        stdout.trim_end(),
        "same     Counter  4 states  pass  (tla-rs)",
        "one line, marked with the program that agreed"
    );
}

#[test]
fn the_program_gets_the_module_the_configuration_and_the_workers_in_its_own_folder() {
    let folder = with_counter();
    let fake = program(folder.path(), 4, "");
    earnest(
        folder.path(),
        &[
            "confirm",
            "--tla-rs",
            &fake.to_string_lossy(),
            "--jobs",
            "3",
        ],
    );
    let work = folder.path().join(".earnest/confirm/tla-rs/Counter");
    assert!(work.join("Counter.tla").is_file(), "the module is written");
    assert!(
        work.join("count.cfg").is_file(),
        "the counting configuration"
    );
    let logged = fs::read_to_string(work.join("arguments.log")).expect("the program ran there");
    assert_eq!(
        logged.lines().next(),
        Some("Counter.tla --config count.cfg --workers 3"),
        "the program is run on the module with its configuration and the workers"
    );
    assert_eq!(
        logged.lines().nth(1),
        Some("Counter.tla --config rule0-0.cfg --workers 3"),
        "each invariant of a rule is a run of its own, because the program stops at the first one"
    );
}

#[test]
fn a_different_state_count_and_a_different_verdict_differ() {
    let folder = with_counter();
    let fake = program(folder.path(), 9, "Rule_bounded");
    let (code, stdout, _) = earnest(
        folder.path(),
        &["confirm", "--tla-rs", &fake.to_string_lossy()],
    );
    assert_eq!(code, Some(1), "a difference is exit 1: {stdout}");
    assert_eq!(
        stdout.trim_end(),
        "differs  Counter  4 states  pass  (tla-rs)\n  \
         states: the checker found 4, tla-rs found 9\n  \
         rule bounded: the checker says holds, tla-rs says broken",
        "the lines name the program and say what differs"
    );
}

#[test]
fn a_program_that_is_not_there_is_an_error() {
    let folder = with_counter();
    let (code, _, stderr) = earnest(folder.path(), &["confirm", "--tla-rs", "nowhere"]);
    assert_eq!(code, Some(2), "the exit code: {stderr}");
    assert!(
        stderr.contains("the tla-rs program nowhere can't be read"),
        "the error names the program: {stderr}"
    );
}

#[test]
fn a_program_that_does_not_start_is_a_difference_not_a_pass() {
    let folder = with_counter();
    let plain = folder.path().join("not-a-program");
    fs::write(&plain, "text").expect("a file can be written");
    let (code, stdout, _) = earnest(
        folder.path(),
        &["confirm", "--tla-rs", &plain.to_string_lossy()],
    );
    assert_eq!(code, Some(1), "the exit code: {stdout}");
    assert!(
        stdout.starts_with("differs  Counter") && stdout.contains("did not start"),
        "the line says the program did not start: {stdout}"
    );
}

#[test]
fn confirm_needs_a_program_to_confirm_with() {
    let folder = with_counter();
    let (code, _, stderr) = earnest(folder.path(), &["confirm"]);
    assert_eq!(code, Some(2), "the exit code: {stderr}");
    assert!(
        stderr.contains("--tlc") && stderr.contains("--tla-rs"),
        "the error names both flags: {stderr}"
    );
}
