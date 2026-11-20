use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;

use crate::support::{Sandbox, is_broken_after, rule};

const RUST_FILE: &str = "case.rs";
const TYPESCRIPT_FILE: &str = "case.model.ts";
const CLIMB: &str = include_str!("../../models/fifth_a/climb.ts");

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
}

fn run_in_folders(files: &[(&str, &str)], arguments: &[&str]) -> Ran {
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
    }
}

fn names(listed: &Value) -> Vec<String> {
    listed
        .as_array()
        .map(|all| {
            all.iter()
                .filter_map(Value::as_str)
                .map(|path| path.trim_start_matches("./").to_owned())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_model_skipped_as_a_test_file_is_named_when_model_asks_for_it() {
    let ran = run_in_folders(
        &[("tests/climb.ts", CLIMB)],
        &["verify", "--no-cache", "--model", "Climb"],
    );
    let said = format!("{}{}", ran.stdout, ran.stderr);
    assert!(
        said.contains("tests/climb.ts") || said.contains("skipped"),
        "the only model `Climb` is in `tests/climb.ts`, a test file the run does not read; a file \
         is never left out without a word, so the run names it or says a test file was skipped, \
         not only that no model is named `Climb`\n{said}"
    );
}

#[test]
fn a_test_file_read_through_its_own_folder_is_not_listed_as_skipped() {
    let ran = run_in_folders(
        &[("tests/climb.ts", CLIMB)],
        &["verify", "--json", "--no-cache", ".", "./tests"],
    );
    let json = ran.json();
    let read = json["models"].as_array().is_some_and(|models| {
        models.iter().any(|model| {
            model["file"]
                .as_str()
                .is_some_and(|file| file.ends_with("tests/climb.ts"))
        })
    });
    let skipped = names(&json["skipped"]).contains(&"tests/climb.ts".to_owned());
    assert!(
        !(read && skipped),
        "`./tests` names the folder, so `tests/climb.ts` is read there and its model checked; \
         `skipped` lists the test files that were not read, so it can't also list this one\n{json:#}"
    );
}

#[test]
fn a_model_found_through_two_spellings_of_its_folder_is_checked_once() {
    let ran = run_in_folders(
        &[("src/climb.ts", CLIMB)],
        &["verify", "--json", "--no-cache", ".", "./src"],
    );
    let json = ran.json();
    assert_eq!(
        json["summary"]["models"], 1,
        "`.` and `./src` both reach the one file `src/climb.ts`, and the same file under two \
         spellings of its path is checked and listed once\n{json:#}"
    );
}

fn printed_inside(value: &Value) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::String(text) => format!("{text:?}"),
        Value::Object(fields) => {
            let shown: Vec<String> = fields
                .iter()
                .map(|(name, field)| format!("{name}: {}", printed_inside(field)))
                .collect();
            format!("{{{}}}", shown.join(", "))
        }
        other => other.to_string(),
    }
}

fn as_the_report_prints(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| printed_inside(value), str::to_owned)
}

mod inputs {
    include!("../../models/fifth_a/inputs.rs");

    #[test]
    fn replay_takes_an_optional_and_a_record_input_as_the_report_prints_them() {
        use super::{RUST_FILE, Sandbox, as_the_report_prints, is_broken_after, rule};
        use earnest::model::{Move, replay};

        let report = Sandbox::with(&[(RUST_FILE, include_str!("../../models/fifth_a/inputs.rs"))])
            .verify(&[]);
        let kept = rule(report.model("Post"), "kept");
        assert!(
            is_broken_after(kept, 2),
            "nothing comes, then the post goes to the lit spot of row 1: two steps\n{report}"
        );
        let steps = kept["path"]["steps"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let path: Vec<Move> = steps
            .iter()
            .map(|step| {
                let action = step["action"].as_str().unwrap_or_default();
                let given = step["input"].as_array().cloned().unwrap_or_default();
                given.iter().fold(Move::new(action), |taken, input| {
                    let name = input["name"].as_str().unwrap_or_default();
                    taken.with(name, &as_the_report_prints(&input["value"]))
                })
            })
            .collect();
        let start = Post::starts().remove(0);
        let replayed = replay(&start, &path);
        assert!(
            replayed.is_ok(),
            "an input is written as the report prints it, so the path `deliver` with `letter = \
             null`, then `place` with `spot = {{row: 1, lit: true}}`, replays in Rust: {}\n{report}",
            replayed.map_or_else(|refused| refused.to_string(), |_| String::new())
        );
    }
}

mod quoted {
    include!("../../models/fifth_a/quoted.rs");

    #[test]
    fn a_debug_hole_on_a_text_gives_the_chart_the_label_rust_prints() {
        use super::{RUST_FILE, Sandbox, run_in_folders};
        use earnest::model::View;

        let source = include_str!("../../models/fifth_a/quoted.rs");
        let start = Lamp::starts().remove(0);
        assert_eq!(
            start.state_label().as_deref(),
            Some("Off by \"Kim\""),
            "`{{:?}}` of a `&'static str` prints it in quotes"
        );
        let report = Sandbox::with(&[(RUST_FILE, source)]).verify(&[]);
        let chart = run_in_folders(&[(RUST_FILE, source)], &["chart", RUST_FILE]);
        assert!(
            report.is_unreadable() || !chart.stdout.contains("Off by Kim"),
            "`state.state_label()` is `Off by \"Kim\"` when the file runs, and the chart shows \
             the labels it gives, so no box reads `Off by Kim`; a `{{:?}}` of a text is refused \
             or printed as Rust prints it\n{}\n{report}",
            chart.stdout
        );
    }
}

fn assert_unreadable(file: &str, source: &str, why: &str) {
    let report = Sandbox::with(&[(file, source)]).verify(&[]);
    assert!(report.is_unreadable(), "{why}\n{report}");
}

#[test]
fn a_display_hole_on_an_enum_that_rust_refuses_is_unreadable() {
    assert_unreadable(
        RUST_FILE,
        include_str!("../../models/fifth_a/display_enum.rs"),
        "`format!(\"the lamp is {}\", self.mode)` and `self.mode.to_string()` need `Display`, \
         which a `Domain` enum does not have: `rustc` refuses the file (E0277, E0599), so the \
         checker refuses it too and draws no chart",
    );
}

#[test]
fn an_action_returning_self_with_require_is_unreadable() {
    assert_unreadable(
        RUST_FILE,
        include_str!("../../models/fifth_a/self_require.rs"),
        "`require!` returns `None`, which a method that returns `Self` can't (E0308): an action \
         that returns `Self` has no `require!`, and `rustc` refuses the file, so the checker \
         does not pass it",
    );
}

#[test]
fn is_multiple_of_on_a_signed_number_is_unreadable() {
    assert_unreadable(
        RUST_FILE,
        include_str!("../../models/fifth_a/signed_multiple.rs"),
        "`is_multiple_of` is a method of the unsigned integers only, so `self.n.is_multiple_of(2)` \
         on an `i8` does not compile (E0599); the checker refuses the file rather than pass it",
    );
}

#[test]
fn explore_and_the_checker_agree_on_a_null_number_read_through_as() {
    let source = include_str!("../../models/fifth_a/cast.model.ts");
    let sandbox = Sandbox::with(&[(TYPESCRIPT_FILE, source)]);
    let checked = sandbox.verify(&[]);
    if checked.is_unreadable() {
        return;
    }
    let explored = sandbox.explore(TYPESCRIPT_FILE, "Opt");
    assert_eq!(
        rule(checked.model("Opt"), "below")["verdict"],
        rule(&explored.json, "below")["verdict"],
        "`(s.o as number) < 3` is `null < 3` in the start state: the checker makes it a fault and \
         `explore` runs it as `true`, so the file is refused as `s.o! < 3` is, or the two \
         agree\n{checked}\n{explored}"
    );
}

#[test]
fn a_model_that_takes_the_library_from_a_model_file_that_exports_m_is_not_left_out() {
    let report = Sandbox::with(&[
        (
            "passes_on.model.ts",
            include_str!("../../models/fifth_a/passes_on.model.ts"),
        ),
        (
            "takes_from_model.ts",
            include_str!("../../models/fifth_a/takes_from_model.ts"),
        ),
    ])
    .verify(&[]);
    let low = rule(report.model("Climb"), "low");
    assert!(
        report.is_unreadable() || is_broken_after(low, 2),
        "`export {{ m }}` passes the library on, which a model file can't do, and \
         `takes_from_model.ts` builds `Climb` from it, whose `low` breaks after two steps in \
         Node; the run refuses the file that passes it on or checks `Climb`, never passes\n\
         {report}"
    );
}

#[test]
fn a_rule_exported_only_as_a_type_is_unreadable() {
    assert_unreadable(
        TYPESCRIPT_FILE,
        include_str!("../../models/fifth_a/inline_type.model.ts"),
        "`export { type low }` is erased from the JavaScript, so `low` is a rule bound to a \
         constant that is not exported, as with `export type { low }`, which is refused",
    );
}

#[test]
fn a_rule_exported_twice_makes_the_file_unreadable() {
    assert_unreadable(
        TYPESCRIPT_FILE,
        include_str!("../../models/fifth_a/exported_twice.model.ts"),
        "`export { low };` twice is a SyntaxError in JavaScript (\"Duplicate export of 'low'\") \
         and TS2300 in `tsc`, so the file does not run and is refused",
    );
}
