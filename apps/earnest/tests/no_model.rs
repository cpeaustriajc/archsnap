//! A Rust file that uses the library and declares no model, such as a helper that walks
//! `Model::starts()` and `steps()`, is left out of a run and said to be, and a file that could
//! hide a model never is.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use serde_json::{Value, json};
use tempfile::TempDir;

const MODEL: &str = "use earnest::model::{Model, model, require};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Lamp {
    #[initial(false)]
    on: bool,
}

#[model]
impl Lamp {
    #[action(\"the lamp is switched on\")]
    fn switch(&self) -> Option<Self> {
        require!(!self.on);
        Some(Self { on: true })
    }

    #[end(\"The lamp is on\")]
    fn lit(&self) -> bool {
        self.on
    }
}
";

const WALKER: &str = "use earnest::model::{Behavior, Model};

pub fn walk<M: Model + Behavior + Clone>() -> usize {
    M::starts().iter().map(|state| state.steps().len()).sum()
}
";

fn folder(files: &[(&str, &str)]) -> TempDir {
    let folder = tempfile::Builder::new()
        .prefix("earnest-no-model-")
        .tempdir()
        .expect("a temporary folder can be made");
    for (name, text) in files {
        let path = folder.path().join(name);
        fs::create_dir_all(path.parent().expect("a file has a folder"))
            .expect("the folder can be made");
        fs::write(path, text).expect("a file can be written");
    }
    folder
}

fn earnest(folder: &Path, arguments: &[&str]) -> (Option<i32>, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_earnest"))
        .current_dir(folder)
        .args(arguments)
        .output()
        .expect("the earnest binary starts");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn a_helper_that_names_the_library_and_declares_no_model_is_left_out_and_said_to_be() {
    let found = folder(&[("lamp.rs", MODEL), ("walk.rs", WALKER)]);
    let (code, stdout, stderr) = earnest(found.path(), &["verify", "--no-cache"]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    assert!(stdout.contains("pass        Lamp"), "{stdout}");
    assert!(
        stdout.contains("skipped     1 file that uses the library and declares no model"),
        "{stdout}"
    );
    assert!(!stdout.contains("unreadable"), "{stdout}");
    assert!(
        stdout.contains("1 model: 1 pass"),
        "the summary counts models only: {stdout}"
    );
}

#[test]
fn the_json_lists_the_left_out_files_next_to_the_skipped_test_files() {
    let found = folder(&[
        ("lamp.rs", MODEL),
        ("walk.rs", WALKER),
        ("helpers/also.rs", WALKER),
        ("tests/lamp.rs", MODEL),
    ]);
    let (code, stdout, stderr) = earnest(found.path(), &["verify", "--json", "--no-cache"]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let report: Value = serde_json::from_str(&stdout).expect("the output is JSON");
    assert_eq!(report["skipped"], json!(["tests/lamp.rs"]), "the test file");
    assert_eq!(
        report["noModel"],
        json!(["helpers/also.rs", "walk.rs"]),
        "the files that declare no model, in path order"
    );
    assert_eq!(report["unreadable"], json!([]), "nothing is unreadable");
}

#[test]
fn the_other_commands_and_the_report_say_so_too() {
    let found = folder(&[("lamp.rs", MODEL), ("walk.rs", WALKER)]);
    let (code, _, stderr) = earnest(found.path(), &["chart", "--model", "Lamp"]);
    assert_eq!(code, Some(0), "{stderr}");
    assert!(
        stderr.contains("skipped     1 file that uses the library and declares no model"),
        "{stderr}"
    );
    let (code, stdout, stderr) = earnest(found.path(), &["report"]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    assert!(
        stdout.contains("## Files that declare no model\n\n1 file that uses the library and declares no model was not read:\n\n```\nwalk.rs\n```"),
        "{stdout}"
    );
}

#[test]
fn a_file_named_on_the_command_line_is_read_or_refused_whatever_it_declares() {
    let found = folder(&[("walk.rs", WALKER)]);
    let (code, stdout, stderr) = earnest(found.path(), &["verify", "walk.rs", "--no-cache"]);
    assert_eq!(code, Some(1), "{stdout}{stderr}");
    assert!(
        stdout.contains("unreadable  walk.rs"),
        "it was named, so it was read and refused: {stdout}"
    );
    assert!(!stdout.contains("declares no model"), "{stdout}");
}

#[test]
fn a_model_hidden_by_spelling_or_place_is_never_left_out() {
    let hidden = [
        (
            "cfg.rs",
            "use earnest::model::Model;\n#[cfg(test)]\n#[derive(Clone, r#Model)]\nstruct Lamp { on: bool }\n",
        ),
        (
            "module.rs",
            "use earnest::model::Model;\nmod inner {\n    #[derive(Clone, earnest::model::Model)]\n    pub struct Lamp { on: bool }\n}\n",
        ),
        (
            "attribute.rs",
            "use earnest::model;\n#[earnest::model::model]\nimpl Lamp {}\n",
        ),
        (
            "unparsable.rs",
            "use earnest::model::Model;\nfn broken( {\n",
        ),
    ];
    for (name, text) in hidden {
        let found = folder(&[(name, text)]);
        let (code, stdout, stderr) = earnest(found.path(), &["verify", "--no-cache"]);
        assert_eq!(
            code,
            Some(1),
            "{name} is read and refused: {stdout}{stderr}"
        );
        assert!(stdout.contains("unreadable"), "{name}: {stdout}");
        assert!(!stdout.contains("declares no model"), "{name}: {stdout}");
    }
}

#[test]
fn a_test_file_stays_a_test_file_and_a_file_that_does_not_name_the_library_is_not_listed() {
    let found = folder(&[
        ("lamp.rs", MODEL),
        ("tests/walk.rs", WALKER),
        ("plain.rs", "pub fn one() -> u8 { 1 }\n"),
    ]);
    let (code, stdout, stderr) = earnest(found.path(), &["verify", "--json", "--no-cache"]);
    assert_eq!(code, Some(0), "{stdout}{stderr}");
    let report: Value = serde_json::from_str(&stdout).expect("the output is JSON");
    assert_eq!(
        report["skipped"],
        json!(["tests/walk.rs"]),
        "a test file that uses the library"
    );
    assert_eq!(report["noModel"], json!([]), "nothing else is left out");
}
