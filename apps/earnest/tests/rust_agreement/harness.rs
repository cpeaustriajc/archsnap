use std::env;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

use assert_cmd::Command as Earnest;
use serde_json::Value;
use tempfile::TempDir;

const HANG_GUARD: Duration = Duration::from_mins(5);

pub(crate) fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root is two folders above the CLI crate")
}

pub(crate) fn conformance(relative: &str) -> PathBuf {
    repository().join("tasks/conformance").join(relative)
}

pub(crate) fn source(relative: &str) -> String {
    let path = conformance(relative);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} can't be read: {error}", path.display()))
}

pub(crate) struct Run {
    pub(crate) code: Option<i32>,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

impl Run {
    pub(crate) fn json(&self) -> Value {
        serde_json::from_str(&self.stdout).unwrap_or_else(|error| {
            panic!(
                "stdout is not JSON ({error})\nexit code: {:?}\nstdout:\n{}\nstderr:\n{}",
                self.code, self.stdout, self.stderr
            )
        })
    }

    pub(crate) fn text(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
}

pub(crate) struct Sandbox {
    folder: TempDir,
}

impl Sandbox {
    pub(crate) fn new() -> Self {
        let folder = tempfile::Builder::new()
            .prefix("earnest-agreement-")
            .tempdir()
            .expect("a temporary folder can be made");
        Self { folder }
    }

    pub(crate) fn path(&self) -> &Path {
        self.folder.path()
    }

    pub(crate) fn write(&self, name: &str, text: &str) -> &Self {
        fs::write(self.path().join(name), text).expect("a file in the sandbox can be written");
        self
    }

    pub(crate) fn earnest(&self, arguments: &[&str]) -> Run {
        let output = Earnest::new(env!("CARGO_BIN_EXE_earnest"))
            .current_dir(self.path())
            .timeout(HANG_GUARD)
            .args(arguments)
            .output()
            .expect("the earnest binary starts");
        Run {
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    pub(crate) fn verified(&self, file: &str) -> Vec<Value> {
        let report = self
            .earnest(&["verify", file, "--json", "--no-cache"])
            .json();
        report["models"]
            .as_array()
            .unwrap_or_else(|| panic!("no `models` array in {report}"))
            .clone()
    }
}

pub(crate) struct Crate {
    root: PathBuf,
    target: PathBuf,
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

impl Crate {
    pub(crate) fn make(name: &str, modules: &[(String, String)]) -> Self {
        let base = Path::new(env!("CARGO_TARGET_TMPDIR")).join("rust-agreement");
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
            root.join("Cargo.lock"),
            &fs::read_to_string(repository().join("Cargo.lock"))
                .expect("the lock file can be read"),
        );
        let mut library = String::from("//! Model files compared with the checker.\n\n");
        for (module, text) in modules {
            write(src.join(format!("{module}.rs")), text);
            let _ = writeln!(library, "mod {module};");
        }
        write(src.join("lib.rs"), &library);
        Self {
            root,
            target: base.join("target"),
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

    pub(crate) fn tested(&self) -> Result<String, String> {
        let output = self.cargo(&["test", "--offline"]);
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            Err(format!(
                "stdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    pub(crate) fn built(&self) -> Result<(), String> {
        let output = self.cargo(&["build", "--offline", "--quiet"]);
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
}
