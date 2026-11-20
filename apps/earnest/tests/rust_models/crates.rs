use std::collections::BTreeSet;
use std::env;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

use crate::support::{conformance, repository};

const FIXTURE_FOLDERS: [(&str, &str); 2] = [("twins", "twins"), ("models", "conformance")];

const NOT_FORCED_BY_THE_LIBRARY: [&str; 3] = [
    "unreachable_pub",
    "clippy::needless_update",
    "clippy::manual_is_multiple_of",
];

const NOT_RUSTFMT_CLEAN: [&str; 5] = [
    "counter.rs",
    "download.rs",
    "mailbox.rs",
    "oven.rs",
    "turnstile.rs",
];

pub(crate) struct Crate {
    root: PathBuf,
    target: PathBuf,
    modules: Vec<String>,
}

pub(crate) struct Diagnostic {
    pub(crate) code: String,
    pub(crate) file: String,
    pub(crate) message: String,
}

fn lint_tables() -> String {
    let manifest = fs::read_to_string(repository().join("Cargo.toml"))
        .expect("the workspace manifest can be read");
    let start = manifest
        .find("[workspace.lints.rust]")
        .expect("the workspace has a lint table for rustc");
    let end = manifest
        .find("[profile.release]")
        .expect("the lint tables end where the profiles start");
    manifest[start..end].replace("[workspace.lints.", "[lints.")
}

fn rust_files(folder: &str) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = fs::read_dir(conformance(folder))
        .unwrap_or_else(|error| panic!("{folder} can't be listed: {error}"))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .collect();
    found.sort();
    found
}

pub(crate) fn fixtures(folder: &str) -> Vec<(String, String)> {
    rust_files(folder)
        .into_iter()
        .map(|path| {
            let module = path
                .file_stem()
                .expect("a fixture has a name")
                .to_string_lossy()
                .into_owned();
            let text = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{} can't be read: {error}", path.display()));
            (module, text)
        })
        .collect()
}

impl Crate {
    pub(crate) fn make(name: &str, modules: &[(String, String)], tests: Option<&str>) -> Self {
        let base = Path::new(env!("CARGO_TARGET_TMPDIR")).join("rust-models");
        let root = base.join(name);
        let src = root.join("src");
        let _ = fs::remove_dir_all(&src);
        fs::create_dir_all(&src).expect("the crate's source folder can be made");
        let write = |path: PathBuf, text: &str| {
            fs::write(&path, text)
                .unwrap_or_else(|error| panic!("{} can't be written: {error}", path.display()));
        };
        let earnest = repository().join("crates/earnest");
        write(
            root.join("Cargo.toml"),
            &format!(
                "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\
                 publish = false\n\n[dependencies]\nearnest = {{ path = \"{}\" }}\n\n\
                 [workspace]\n\n{}",
                earnest.display(),
                lint_tables()
            ),
        );
        write(
            root.join("clippy.toml"),
            &fs::read_to_string(repository().join("clippy.toml"))
                .expect("the clippy configuration can be read"),
        );
        write(
            root.join("Cargo.lock"),
            &fs::read_to_string(repository().join("Cargo.lock"))
                .expect("the lock file can be read"),
        );
        let mut library = String::from("//! Model files in a crate that depends on `earnest`.\n\n");
        for (module, text) in modules {
            write(src.join(format!("{module}.rs")), text);
            let _ = writeln!(library, "mod {module};");
        }
        if let Some(text) = tests {
            write(src.join("replay.rs"), text);
            library.push_str("\n#[cfg(test)]\nmod replay;\n");
        }
        write(src.join("lib.rs"), &library);
        Self {
            root,
            target: base.join("target"),
            modules: modules.iter().map(|(module, _)| module.clone()).collect(),
        }
    }

    fn cargo(&self, arguments: &[&str]) -> Output {
        let program: OsString = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        Command::new(program)
            .current_dir(&self.root)
            .env("CARGO_TARGET_DIR", &self.target)
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            .env_remove("RUSTFLAGS")
            .args(arguments)
            .output()
            .expect("cargo starts")
    }

    pub(crate) fn builds(&self) -> Result<(), String> {
        let output = self.cargo(&["build", "--offline", "--quiet"]);
        report(&output)
    }

    pub(crate) fn test_output(&self) -> Result<String, String> {
        let output = self.cargo(&["test", "--offline"]);
        report(&output).map(|()| String::from_utf8_lossy(&output.stdout).into_owned())
    }

    pub(crate) fn clippy(&self) -> Vec<Diagnostic> {
        let output = self.cargo(&[
            "clippy",
            "--offline",
            "--message-format=json",
            "--",
            "-D",
            "warnings",
        ]);
        let text = String::from_utf8_lossy(&output.stdout);
        let found: Vec<Diagnostic> = text
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter(|line| line["reason"] == "compiler-message")
            .filter_map(|line| {
                let message = &line["message"];
                let level = message["level"].as_str()?;
                if level != "error" && level != "warning" {
                    return None;
                }
                let code = message["code"]["code"].as_str()?.to_owned();
                let file = message["spans"]
                    .as_array()?
                    .iter()
                    .find(|span| span["is_primary"] == true)?["file_name"]
                    .as_str()?
                    .to_owned();
                let shown = message["message"].as_str()?.to_owned();
                Some(Diagnostic {
                    code,
                    file,
                    message: shown,
                })
            })
            .collect();
        assert!(
            found.is_empty() || !output.status.success(),
            "clippy reports a diagnostic and still succeeds"
        );
        found
    }

    pub(crate) fn unformatted(&self) -> Vec<(String, usize)> {
        let mut found = Vec::new();
        for module in &self.modules {
            let file = self.root.join("src").join(format!("{module}.rs"));
            let output = Command::new("rustfmt")
                .args(["--check", "--edition", "2024", "--color", "never"])
                .arg(&file)
                .output()
                .expect("rustfmt starts");
            let text = String::from_utf8_lossy(&output.stdout);
            let changed = text
                .lines()
                .filter(|line| {
                    (line.starts_with('+') && !line.starts_with("+++"))
                        || (line.starts_with('-') && !line.starts_with("---"))
                })
                .count();
            assert!(
                output.status.success() || changed > 0,
                "rustfmt could not read {module}.rs\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            if changed > 0 {
                found.push((format!("{module}.rs"), changed));
            }
        }
        found
    }
}

fn report(output: &Output) -> Result<(), String> {
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn shown(found: &[Diagnostic]) -> String {
    found
        .iter()
        .map(|diagnostic| {
            format!(
                "{} {} {}",
                diagnostic.file, diagnostic.code, diagnostic.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn crate_of(folder: &str, name: &str) -> Crate {
    let (_, label) = FIXTURE_FOLDERS
        .iter()
        .find(|(known, _)| *known == folder)
        .expect("a known fixture folder");
    Crate::make(&format!("{label}-{name}"), &fixtures(folder), None)
}

#[test]
fn r02_the_new_rust_fixtures_build_pass_clippy_under_the_repository_lints_and_are_rustfmt_clean() {
    let twins = crate_of("twins", "check");
    if let Err(output) = twins.builds() {
        panic!("the new fixtures do not build\n{output}");
    }
    let found = twins.clippy();
    assert!(found.is_empty(), "clippy -D warnings:\n{}", shown(&found));
    assert_eq!(twins.unformatted(), [], "rustfmt --check");
}

#[test]
fn r02_the_conformance_models_build_and_clippy_finds_only_what_their_own_text_can_fix() {
    let models = crate_of("models", "check");
    if let Err(output) = models.builds() {
        panic!("the conformance models do not build\n{output}");
    }
    let found = models.clippy();
    let forced: Vec<&Diagnostic> = found
        .iter()
        .filter(|diagnostic| !NOT_FORCED_BY_THE_LIBRARY.contains(&diagnostic.code.as_str()))
        .collect();
    assert!(
        forced.is_empty(),
        "a lint that the library or a macro forces on a model file:\n{}",
        forced
            .iter()
            .map(|diagnostic| format!("{} {}", diagnostic.code, diagnostic.message))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn r02_the_conformance_models_that_rustfmt_would_change_are_the_ones_known_not_to_be_clean() {
    let models = crate_of("models", "format");
    let changed: BTreeSet<String> = models
        .unformatted()
        .into_iter()
        .map(|(file, _)| file)
        .collect();
    let known: BTreeSet<String> = NOT_RUSTFMT_CLEAN
        .iter()
        .map(|file| (*file).to_owned())
        .collect();
    assert_eq!(
        changed, known,
        "the protected conformance models that `rustfmt --check` changes: update the list, or \
         reformat the files"
    );
}
