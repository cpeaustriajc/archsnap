//! Checks on the repository's own layout: every workspace member takes the workspace lints, and
//! every crate under `crates/` has a test and a `README.md`.

use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root is two folders above the CLI crate")
}

fn member_manifests() -> Vec<PathBuf> {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(repository())
        .output()
        .expect("cargo metadata runs");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: Value =
        serde_json::from_slice(&output.stdout).expect("cargo metadata prints JSON");
    let members: Vec<&str> = metadata["workspace_members"]
        .as_array()
        .expect("cargo metadata lists the workspace members")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    metadata["packages"]
        .as_array()
        .expect("cargo metadata lists the packages")
        .iter()
        .filter(|package| {
            package["id"]
                .as_str()
                .is_some_and(|id| members.contains(&id))
        })
        .filter_map(|package| package["manifest_path"].as_str())
        .map(PathBuf::from)
        .collect()
}

fn crate_folders() -> Vec<PathBuf> {
    let crates = repository().join("crates");
    let Ok(entries) = fs::read_dir(&crates) else {
        return Vec::new();
    };
    let mut folders: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|folder| folder.join("Cargo.toml").is_file())
        .collect();
    folders.sort();
    folders
}

fn package_folders() -> Vec<PathBuf> {
    let packages = repository().join("packages");
    let Ok(entries) = fs::read_dir(&packages) else {
        return Vec::new();
    };
    let mut folders: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|folder| folder.join("package.json").is_file())
        .collect();
    folders.sort();
    folders
}

fn takes_workspace_lints(manifest: &str) -> bool {
    let compact: Vec<String> = manifest
        .lines()
        .map(|line| line.split_whitespace().collect::<String>())
        .collect();
    let in_table = compact
        .iter()
        .skip_while(|line| line.as_str() != "[lints]")
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .any(|line| line == "workspace=true");
    let inline = compact
        .iter()
        .any(|line| line == "lints.workspace=true" || line == "lints={workspace=true}");
    in_table || inline
}

fn rust_files(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for path in entries.filter_map(Result::ok).map(|entry| entry.path()) {
        if path.is_dir() {
            files.extend(rust_files(&path));
        } else if path.extension() == Some(OsStr::new("rs")) {
            files.push(path);
        }
    }
    files
}

fn has_a_test(folder: &Path) -> bool {
    let has_test_file = !rust_files(&folder.join("tests")).is_empty();
    let has_unit_test = rust_files(&folder.join("src"))
        .iter()
        .any(|file| fs::read_to_string(file).is_ok_and(|source| source.contains("#[test]")));
    has_test_file || has_unit_test
}

fn names(folders: &[PathBuf]) -> String {
    folders
        .iter()
        .map(|folder| folder.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

#[test]
fn every_workspace_member_takes_the_workspace_lints() {
    let manifests = member_manifests();
    assert!(!manifests.is_empty(), "the workspace has no members");
    let without: Vec<PathBuf> = manifests
        .into_iter()
        .filter(|manifest| {
            let text = fs::read_to_string(manifest).expect("a member's Cargo.toml can be read");
            !takes_workspace_lints(&text)
        })
        .collect();
    assert!(
        without.is_empty(),
        "these manifests lack `[lints] workspace = true`: {}",
        names(&without)
    );
}

#[test]
fn every_crate_has_a_test() {
    let without: Vec<PathBuf> = crate_folders()
        .into_iter()
        .filter(|folder| !has_a_test(folder))
        .collect();
    assert!(
        without.is_empty(),
        "these crates have neither a `tests/` folder with a test nor a `#[test]` in `src/`: {}",
        names(&without)
    );
}

#[test]
fn every_crate_has_a_readme() {
    let without: Vec<PathBuf> = crate_folders()
        .into_iter()
        .filter(|folder| !folder.join("README.md").is_file())
        .collect();
    assert!(
        without.is_empty(),
        "these crates have no `README.md`: {}",
        names(&without)
    );
}

#[test]
fn every_package_has_a_readme() {
    let folders = package_folders();
    assert!(
        !folders.is_empty(),
        "`packages/` holds the packages published to npm"
    );
    let without: Vec<PathBuf> = folders
        .into_iter()
        .filter(|folder| !folder.join("README.md").is_file())
        .collect();
    assert!(
        without.is_empty(),
        "these packages have no `README.md`: {}",
        names(&without)
    );
}

#[test]
fn the_lint_check_reads_each_way_of_writing_the_table() {
    assert!(
        takes_workspace_lints("[package]\nname = \"a\"\n\n[lints]\nworkspace = true\n"),
        "a `[lints]` table with `workspace = true` takes the workspace lints"
    );
    assert!(
        takes_workspace_lints("[package]\nname = \"a\"\nlints.workspace = true\n"),
        "a dotted `lints.workspace = true` takes the workspace lints"
    );
    assert!(
        !takes_workspace_lints("[package]\nname = \"a\"\n\n[dependencies]\nworkspace = true\n"),
        "`workspace = true` under another table does not take the workspace lints"
    );
    assert!(
        !takes_workspace_lints("[lints]\n\n[lints.clippy]\nall = \"warn\"\n"),
        "a `[lints]` table of its own lints does not take the workspace lints"
    );
}
