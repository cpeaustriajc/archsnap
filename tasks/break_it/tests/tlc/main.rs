//! The tests that need Java and the TLC jar, built only with the cargo feature `tlc`:
//! `cargo test --locked -p earnest_break_it --features tlc --test tlc`. The jar is the one
//! `just tlc` puts at `target/tools/tla2tools.jar`, or the path in `EARNEST_TLC_JAR`.

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use tempfile::TempDir;

const TLC_DEADLINE: Duration = Duration::from_mins(10);
const POLL: Duration = Duration::from_millis(50);
const TLC_JAR_VARIABLE: &str = "EARNEST_TLC_JAR";
const TLC_JAR_FROM_THE_ROOT: &str = "../../target/tools/tla2tools.jar";

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

struct Finished {
    stdout: String,
    success: bool,
}

fn tlc_jar() -> PathBuf {
    env::var_os(TLC_JAR_VARIABLE).map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join(TLC_JAR_FROM_THE_ROOT),
        PathBuf::from,
    )
}

fn run_for(files: &[(&str, &str)], arguments: &[&str], deadline: Duration) -> Option<Finished> {
    let binary = binary();
    let folder = TempDir::new().expect("a temporary folder can be made");
    for (name, text) in files {
        std::fs::write(folder.path().join(name), text).expect("a file can be written");
    }
    let mut child = Command::new(binary)
        .args(arguments)
        .current_dir(folder.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("earnest starts");
    let started = Instant::now();
    while started.elapsed() < deadline {
        if child.try_wait().expect("the child can be polled").is_some() {
            let output = child.wait_with_output().expect("the output can be read");
            return Some(Finished {
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                success: output.status.success(),
            });
        }
        thread::sleep(POLL);
    }
    child.kill().expect("the child can be stopped");
    child.wait().expect("the child can be reaped");
    None
}

fn assert_tlc_confirms(name: &str, source: &str, why: &str) {
    let jar = tlc_jar();
    assert!(
        jar.is_file(),
        "TLC is missing at {}: `just tlc` downloads it to target/tools, and {TLC_JAR_VARIABLE} \
         names another copy",
        jar.display()
    );
    let jar = jar.to_string_lossy().into_owned();
    let confirmed = run_for(
        &[(name, source)],
        &["confirm", "--tlc", &jar, name],
        TLC_DEADLINE,
    )
    .expect("TLC checks a model of a few states in minutes");
    assert!(
        confirmed.success && confirmed.stdout.starts_with("same"),
        "the contract says TLC finds the same number of distinct states and the same verdict \
         for each rule, and `{name}` is a model {why}\n{}",
        confirmed.stdout
    );
}

#[test]
fn tlc_finds_the_same_verdicts_for_a_number_beyond_32_bits() {
    assert_tlc_confirms(
        "big.model.ts",
        include_str!("../../models/fourth_b/big_number.model.ts"),
        "whose numbers the search holds exactly (the largest is 2^31 + 2) and TLC's 32-bit \
         integers can't",
    );
}

#[test]
fn tlc_finds_the_same_verdicts_for_a_product_past_32_bits_of_numbers_within_them() {
    assert_tlc_confirms(
        "square.model.ts",
        include_str!("../../models/fourth_b/square.model.ts"),
        "whose numbers are all below 2^31 and whose largest square, 50 003 times itself, is \
         2 500 300 009",
    );
}

#[test]
fn tlc_counts_two_texts_that_differ_only_above_their_first_byte_as_two_states() {
    assert_tlc_confirms(
        "text.model.ts",
        include_str!("../../models/fourth_b/text_collision.model.ts"),
        "with two states, the letters `Z` (U+005A) and `\u{15a}` (U+015A), which TLC takes for \
         one when it counts states",
    );
}
