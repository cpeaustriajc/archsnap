//! Runs `earnest verify` on each model of this crate and checks it passes with the state count
//! derived in `README.md`, so a change to a model or to the checker that moves either shows up
//! in `cargo test`.

use serde_json::Value;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root is two folders above this crate")
}

fn profile_folder() -> PathBuf {
    let test = env::current_exe().expect("a test knows where its own binary is");
    test.parent()
        .and_then(Path::parent)
        .expect("a test binary sits in `deps` under the folder of its build profile")
        .to_path_buf()
}

fn checker() -> &'static Path {
    static BINARY: OnceLock<PathBuf> = OnceLock::new();
    BINARY.get_or_init(|| {
        let folder = profile_folder();
        let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let mut build = Command::new(cargo);
        build
            .args(["build", "--locked", "--package", "earnest-cli"])
            .args(["--bin", "earnest"])
            .current_dir(repository());
        if folder.file_name().is_some_and(|name| name == "release") {
            build.arg("--release");
        }
        let compiled = build.output().expect("cargo starts");
        assert!(
            compiled.status.success(),
            "the checker does not build:\n{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let binary = folder.join(format!("earnest{}", env::consts::EXE_SUFFIX));
        assert!(
            binary.is_file(),
            "the checker is missing at {}",
            binary.display()
        );
        binary
    })
}

fn verified(file: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(file);
    let folder = tempfile::tempdir().expect("a folder for the checker's cache");
    let output = Command::new(checker())
        .args(["verify", "--json", "--no-cache"])
        .arg(&path)
        .current_dir(folder.path())
        .output()
        .expect("the checker starts");
    assert!(
        output.status.success(),
        "{file} does not pass:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("the checker prints JSON")
}

fn assert_passes(file: &str, name: &str, states: u64, rules: &[&str]) {
    let json = verified(file);
    let models = json["models"].as_array().expect("a list of models");
    assert_eq!(models.len(), 1, "{file} holds one model");
    let model = &models[0];
    assert_eq!(model["name"], name, "the model of {file}");
    assert_eq!(model["status"], "pass", "{name} passes");
    assert_eq!(model["states"], states, "{name} has the hand-derived count");
    assert_eq!(
        model["findings"],
        Value::Array(Vec::new()),
        "{name} has no finding"
    );
    let verdicts: Vec<(&str, &str)> = model["rules"]
        .as_array()
        .expect("a list of rules")
        .iter()
        .map(|rule| {
            (
                rule["id"].as_str().expect("an id"),
                rule["verdict"].as_str().expect("a verdict"),
            )
        })
        .collect();
    let expected: Vec<(&str, &str)> = rules.iter().map(|rule| (*rule, "holds")).collect();
    assert_eq!(verdicts, expected, "every rule of {name} holds");
    assert_eq!(
        json["unreadable"],
        Value::Array(Vec::new()),
        "{file} is read"
    );
}

#[test]
fn the_thread_pool_passes_with_35612_states() {
    assert_passes(
        "pool.rs",
        "Pool",
        35612,
        &[
            "lent_once",
            "none_lost",
            "own_thread",
            "none_idle_while_waiting",
            "runs_what_it_borrowed",
            "borrows_what_it_can_share",
            "none_idle_beside_a_level",
            "finished_holds_nothing",
        ],
    );
}

#[test]
fn the_cache_passes_with_394_states() {
    assert_passes(
        "cache.rs",
        "Cache",
        394,
        &[
            "served_for_the_same_input",
            "no_cache_checks_again",
            "timed_out_never_stored",
            "reused_when_nothing_changed",
        ],
    );
}

#[test]
fn the_run_outcome_passes_with_7222_states() {
    assert_passes(
        "outcome.rs",
        "Outcome",
        7222,
        &[
            "limit_with_broken_is_fail",
            "limit_without_broken_is_unfinished",
            "pass_only_when_whole",
            "holds_only_when_judged",
            "broken_exactly_when_seen",
            "exit_zero_only_when_all_passed",
            "exit_one_when_something_is_wrong",
            "exit_two_when_nothing_matched",
            "skipped_never_changes_the_exit",
            "named_skipped_file_is_reported",
            "named_unreadable_file_is_reported",
            "unreadable_files_are_named_with_no_such_model",
        ],
    );
}
