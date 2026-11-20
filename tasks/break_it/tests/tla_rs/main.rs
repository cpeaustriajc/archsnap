//! The tests that need the tla-rs program, built only with the cargo feature `tla_rs`:
//! `cargo test --locked -p earnest_break_it --features tla_rs --test tla_rs`. The program is the
//! one `just tla-rs` puts at `target/tools/bin/tla-rs`, or the path in `EARNEST_TLA_RS`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const TLA_RS_VARIABLE: &str = "EARNEST_TLA_RS";
const TLA_RS_FROM_THE_ROOT: &str = "../../target/tools/bin/tla-rs";
const CONFORMANCE_MODELS: &str = "../conformance/models";

fn binary() -> PathBuf {
    let test = env::current_exe().expect("a test knows where its own binary is");
    let folder = test
        .parent()
        .and_then(Path::parent)
        .expect("a test binary sits in `deps` under the folder of its build profile");
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut build = Command::new(cargo);
    build
        .args(["build", "--locked", "--package", "earnest-cli"])
        .args(["--bin", "earnest"])
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."));
    if folder.file_name().is_some_and(|name| name == "release") {
        build.arg("--release");
    }
    let compiled = build.output().expect("cargo starts");
    assert!(
        compiled.status.success(),
        "the earnest binary does not build:\n{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    folder.join(format!("earnest{}", env::consts::EXE_SUFFIX))
}

fn tla_rs() -> PathBuf {
    let program = env::var_os(TLA_RS_VARIABLE).map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join(TLA_RS_FROM_THE_ROOT),
        PathBuf::from,
    );
    assert!(
        program.is_file(),
        "tla-rs is missing at {}: `just tla-rs` installs it to target/tools, and \
         {TLA_RS_VARIABLE} names another copy",
        program.display()
    );
    program
}

fn conformance_models() -> Vec<PathBuf> {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join(CONFORMANCE_MODELS);
    let mut models: Vec<PathBuf> = fs::read_dir(&folder)
        .expect("the conformance models are there")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|extension| extension == "rs")
                || path.to_string_lossy().ends_with(".model.ts")
        })
        .collect();
    models.sort();
    models
}

#[test]
fn tla_rs_finds_the_same_states_and_verdicts_as_the_checker_on_every_conformance_model() {
    let binary = binary();
    let program = tla_rs();
    let work = tempfile::tempdir().expect("a temporary folder can be made");
    let models = conformance_models();
    assert!(!models.is_empty(), "there are conformance models");
    for model in models {
        let confirmed = Command::new(&binary)
            .args(["confirm", "--tla-rs"])
            .arg(&program)
            .arg(&model)
            .current_dir(work.path())
            .output()
            .expect("earnest starts");
        let text = String::from_utf8_lossy(&confirmed.stdout);
        assert!(
            confirmed.status.success() && text.lines().all(|line| line.starts_with("same")),
            "tla-rs and the checker differ on {}:\n{text}",
            model.display()
        );
    }
}
