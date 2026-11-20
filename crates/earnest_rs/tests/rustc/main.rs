//! Compares what `rustc` says about a model file with what the reader and the lowering say. Each
//! case is a method in a model file that is compiled as real Rust, in a temporary crate that
//! depends on `earnest`, and read as a model file: a case `rustc` accepts must be read, and a
//! case `rustc` refuses must be refused with a sentence, and the code `rustc` gives for it is
//! the one the case lists. The cases of one group are compiled together, because `rustc` stops
//! at the first phase that fails for the whole crate: a name, a type, a pattern or a move.

mod cases;

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use earnest_rs::read;
use earnest_semantic::lower;

const BASE: &str = include_str!("base.txt");

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Group {
    Borrow,
    Types,
    Patterns,
    Names,
}

impl Group {
    fn folder(self) -> &'static str {
        match self {
            Self::Borrow => "borrow",
            Self::Types => "types",
            Self::Patterns => "patterns",
            Self::Names => "names",
        }
    }
}

pub(crate) struct Case {
    pub(crate) name: &'static str,
    pub(crate) method: &'static str,
    pub(crate) call: &'static str,
    pub(crate) rustc: &'static str,
    pub(crate) group: Group,
}

impl Case {
    fn source(&self) -> String {
        BASE.replace("METHOD", self.method)
            .replace("CALL", self.call)
    }

    fn module(index: usize) -> String {
        format!("case_{index}")
    }
}

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root is two folders above this crate")
}

fn codes_by_module(group: Group, members: &[(usize, &Case)]) -> BTreeMap<String, BTreeSet<String>> {
    let base = Path::new(env!("CARGO_TARGET_TMPDIR")).join("rustc-cases");
    let root = base.join(group.folder());
    let src = root.join("src");
    let _ = fs::remove_dir_all(&src);
    fs::create_dir_all(&src).expect("the crate's source folder can be made");
    let manifest = format!(
        "[package]\nname = \"cases_{}\"\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n\n\
         [dependencies]\nearnest = {{ path = \"{}\" }}\n\n[workspace]\n",
        group.folder(),
        repository().join("crates/earnest").display()
    );
    fs::write(root.join("Cargo.toml"), manifest).expect("the manifest can be written");
    fs::copy(repository().join("Cargo.lock"), root.join("Cargo.lock"))
        .expect("the lock file can be copied");
    let mut library = String::new();
    for (index, case) in members {
        let module = Case::module(*index);
        fs::write(src.join(format!("{module}.rs")), case.source()).expect("a case can be written");
        let _ = writeln!(library, "mod {module};");
    }
    fs::write(src.join("lib.rs"), library).expect("the library file can be written");
    let program = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(program)
        .current_dir(&root)
        .env("CARGO_TARGET_DIR", base.join("target"))
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("RUSTFLAGS")
        .args(["build", "--offline", "--message-format=short"])
        .output()
        .expect("cargo starts");
    let said = String::from_utf8_lossy(&output.stderr).into_owned();
    let mut found: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for line in said.lines() {
        let Some(rest) = line.strip_prefix("src/") else {
            continue;
        };
        let Some((module, after)) = rest.split_once(".rs:") else {
            continue;
        };
        let Some((_, code)) = after.split_once("error[") else {
            continue;
        };
        let Some((code, _)) = code.split_once(']') else {
            continue;
        };
        found
            .entry(module.to_owned())
            .or_default()
            .insert(code.to_owned());
    }
    if !output.status.success() {
        assert!(
            !found.is_empty(),
            "the crate fails to build for a reason that is no case's:\n{said}"
        );
    }
    found
}

fn refusal(source: &str) -> Option<String> {
    let reading = read("src/case.rs", source);
    if let Some(first) = reading.problems.first() {
        return Some(first.message.clone());
    }
    let [draft] = reading.drafts.as_slice() else {
        return Some("the file holds no single model".to_owned());
    };
    lower(draft)
        .err()
        .and_then(|problems| problems.first().map(|problem| problem.message.clone()))
}

fn check(group: Group) {
    let members: Vec<(usize, &Case)> = cases::CASES
        .iter()
        .enumerate()
        .filter(|(_, case)| case.group == group)
        .collect();
    let compiled = codes_by_module(group, &members);
    for (index, case) in members {
        let codes = compiled
            .get(&Case::module(index))
            .cloned()
            .unwrap_or_default();
        let said = refusal(&case.source());
        if case.rustc.is_empty() {
            assert!(
                codes.is_empty(),
                "rustc accepts `{}` but gave {codes:?}",
                case.name
            );
            assert_eq!(
                said, None,
                "rustc accepts `{}` and the checker must read it",
                case.name
            );
        } else {
            assert!(
                codes.contains(case.rustc),
                "rustc refuses `{}` with {}, and gave {codes:?}",
                case.name,
                case.rustc
            );
            assert!(
                said.is_some(),
                "rustc refuses `{}` ({}) and the checker must refuse it with a sentence",
                case.name,
                case.rustc
            );
        }
    }
}

#[test]
fn rustc_and_the_reader_agree_on_moves() {
    check(Group::Borrow);
}

#[test]
fn rustc_and_the_reader_agree_on_types() {
    check(Group::Types);
}

#[test]
fn rustc_and_the_reader_agree_on_patterns() {
    check(Group::Patterns);
}

#[test]
fn rustc_and_the_reader_agree_on_names_in_patterns() {
    check(Group::Names);
}
