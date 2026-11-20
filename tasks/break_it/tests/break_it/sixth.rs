use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;

use crate::support::{Sandbox, is_broken_after, rule};

const RUST_FILE: &str = "case.rs";

fn binary() -> PathBuf {
    Sandbox::with(&[]).verify(&[]);
    let test = env::current_exe().expect("a test knows where its own binary is");
    let folder = test
        .parent()
        .and_then(Path::parent)
        .expect("a test binary sits in `deps` under the folder of its build profile");
    folder.join(format!("earnest{}", env::consts::EXE_SUFFIX))
}

struct Ran {
    stdout: String,
    stderr: String,
    code: Option<i32>,
}

impl Ran {
    fn json(&self) -> Value {
        serde_json::from_str(&self.stdout).unwrap_or_else(|error| {
            panic!(
                "earnest did not print JSON ({error}):\n{}\n{}",
                self.stdout, self.stderr
            )
        })
    }

    fn said(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
}

fn run_in_folder(files: &[(&str, &str)], arguments: &[&str]) -> Ran {
    let binary = binary();
    let folder = TempDir::new().expect("a temporary folder can be made");
    for (name, text) in files {
        let path = folder.path().join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("a folder in the sandbox can be made");
        }
        fs::write(path, text).expect("a file in the sandbox can be written");
    }
    let output = Command::new(binary)
        .args(arguments)
        .current_dir(folder.path())
        .output()
        .expect("earnest starts");
    Ran {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        code: output.status.code(),
    }
}

fn assert_unreadable(source: &str, why: &str) {
    let report = Sandbox::with(&[(RUST_FILE, source)]).verify(&[]);
    assert!(report.is_unreadable(), "{why}\n{report}");
}

#[test]
fn a_constant_in_an_if_let_pattern_is_compared_not_bound() {
    let report = Sandbox::with(&[(
        RUST_FILE,
        include_str!("../../models/sixth/constant_pattern.rs"),
    )])
    .verify(&[]);
    let only_top = rule(report.model("Pick"), "only_top");
    assert!(
        report.is_unreadable() || is_broken_after(only_top, 1),
        "`TOP` is a constant, so in `if let Some(TOP) = self.chosen` it is a constant pattern \
         that matches only `Some(3)`, not a new name: after `choose` with `number = 0` the \
         `else` branch runs and `self.chosen.is_none()` is false, so `only_top` is broken after \
         one step, as `state.rules()` says when the file runs; the checker reads `TOP` as a \
         binding that matches every `Some`\n{report}"
    );
}

#[test]
fn a_binding_named_after_a_constant_is_unreadable() {
    assert_unreadable(
        include_str!("../../models/sixth/constant_binding.rs"),
        "`let TOP = self.chosen?;` with a constant `TOP` in scope is a refutable constant \
         pattern, not a binding, and `rustc` refuses the file (E0005); the same holds for `let \
         TOP: u8 = ..`, a closure parameter and a helper parameter named after a constant",
    );
}

#[test]
fn an_option_moved_by_question_mark_and_used_again_is_unreadable() {
    assert_unreadable(
        include_str!("../../models/sixth/moved_twice.rs"),
        "`let first = held?;` moves `held`, an `Option<Tag>` that is not `Copy`, so `let second \
         = held?;` uses a moved value and `rustc` refuses the file (E0382); the README says the \
         checker reads the value inside an `Option` only where Rust does",
    );
}

#[test]
fn a_field_that_is_not_copy_moved_out_of_the_state_is_unreadable() {
    assert_unreadable(
        include_str!("../../models/sixth/moved_out.rs"),
        "`let stage = self.stage;` moves a `Stage` that does not derive `Copy` out of `&self`, and \
         `rustc` refuses the file (E0507), as it refuses `if let Some(x) = self.field` on such a \
         field, which the checker already refuses",
    );
}

#[test]
fn a_variant_of_another_enum_with_the_same_names_is_unreadable() {
    assert_unreadable(
        include_str!("../../models/sixth/twin_enums.rs"),
        "`match self.beta { Alpha::On => .., Alpha::Off => .. }` matches a `Beta` against the \
         variants of `Alpha`: `rustc` refuses the arms (E0308), and the README says an arm of \
         another enum is refused",
    );
    assert_unreadable(
        include_str!("../../models/sixth/twin_compare.rs"),
        "`self.beta == Alpha::On` compares a `Beta` with an `Alpha`, which have no `PartialEq` \
         between them: `rustc` refuses the file (E0308)",
    );
}

#[test]
fn a_closure_whose_parameter_type_differs_from_the_option_is_unreadable() {
    assert_unreadable(
        include_str!("../../models/sixth/closure_type.rs"),
        "`self.chosen.map_or(0, |weight: u16| ..)` on an `Option<u8>` gives the closure a `u16` \
         where `map_or` passes a `u8`, and `rustc` refuses the file (E0631)",
    );
}

#[test]
fn a_rust_model_whose_library_names_are_imported_under_aliases_is_not_left_out() {
    let ran = run_in_folder(
        &[("aliased.rs", include_str!("../../models/sixth/aliased.rs"))],
        &["verify", "--json", "--no-cache"],
    );
    let json = ran.json();
    let low = json["models"]
        .as_array()
        .and_then(|models| models.iter().find(|model| model["name"] == "Climb"))
        .map_or(&Value::Null, |model| rule(model, "low"));
    let unreadable = json["unreadable"]
        .as_array()
        .is_some_and(|files| !files.is_empty());
    assert!(
        unreadable || is_broken_after(low, 2),
        "`aliased.rs` implements the library's `Model` and `Behavior` for `Climb` under the names \
         `Starts` and `Walks`; walked as real Rust, `low` breaks after two steps. The README says \
         nothing hides a model from the test and that an item of the library imported under \
         another name is refused, so the file is `unreadable`, as it is when named on the command \
         line, and the run exits 1; today it is listed in `noModel` and the run exits {:?}\n\
         {json:#}",
        ran.code
    );
}

#[test]
fn a_run_stopped_by_an_unknown_model_name_says_which_files_declare_no_model() {
    let ran = run_in_folder(
        &[
            ("helper.rs", include_str!("../../models/sixth/helper.rs")),
            (
                "tests/climb.ts",
                include_str!("../../models/fifth_a/climb.ts"),
            ),
        ],
        &["verify", "--no-cache", "--model", "Nope"],
    );
    let said = ran.said();
    assert!(
        said.contains("skipped     1 test file that uses the library")
            && said.contains("declares no model"),
        "the run leaves out `tests/climb.ts` as a test file and `helper.rs` as a file that \
         declares no model, and the README says the run says so for each; before the exit 2 of \
         `--model` it prints the line of the test file and not the line of `helper.rs`\n{said}"
    );
}
