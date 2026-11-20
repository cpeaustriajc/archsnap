use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use assert_cmd::Command;
use serde_json::{Map, Value};
use tempfile::TempDir;

const HANG_GUARD: Duration = Duration::from_mins(5);

pub(crate) const DIFFERS_BY_LANGUAGE: [&str; 4] = ["file", "line", "milliseconds", "cached"];

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

pub(crate) struct Sandbox {
    folder: TempDir,
}

pub(crate) struct Run {
    pub(crate) code: Option<i32>,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

impl fmt::Display for Run {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "exit code: {:?}\nstdout:\n{}\nstderr:\n{}",
            self.code, self.stdout, self.stderr
        )
    }
}

impl Run {
    pub(crate) fn exits(&self, code: i32) -> &Self {
        assert_eq!(self.code, Some(code), "expected exit code {code}\n{self}");
        self
    }

    pub(crate) fn json(&self) -> Value {
        serde_json::from_str(&self.stdout)
            .unwrap_or_else(|error| panic!("stdout is not JSON ({error})\n{self}"))
    }
}

impl Sandbox {
    pub(crate) fn new() -> Self {
        let folder = tempfile::Builder::new()
            .prefix("earnest-rust-models-")
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
        let output = Command::new(env!("CARGO_BIN_EXE_earnest"))
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

    pub(crate) fn verified(&self, file: &str, name: &str) -> Value {
        let run = self.earnest(&["verify", file, "--json", "--no-cache"]);
        let report = run.json();
        let found = report["models"]
            .as_array()
            .unwrap_or_else(|| panic!("no `models` array\n{run}"));
        assert_eq!(found.len(), 1, "{file} holds one model\n{run}");
        let only = &found[0];
        assert_eq!(only["name"], name, "the model of {file}\n{run}");
        only.clone()
    }

    pub(crate) fn tla(&self, file: &str, model: &str, folder: &str) -> (String, String) {
        let out = self.path().join(folder);
        let target = out.to_string_lossy().into_owned();
        self.earnest(&["tla", file, "--out", &target]).exits(0);
        let read = |extension: &str| {
            let path = out.join(format!("{model}.{extension}"));
            fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{} can't be read: {error}", path.display()))
        };
        (read("tla"), read("cfg"))
    }
}

pub(crate) fn without(value: &Value, keys: &[&str]) -> Value {
    match value {
        Value::Object(members) => Value::Object(
            members
                .iter()
                .filter(|(key, _)| !keys.contains(&key.as_str()))
                .map(|(key, member)| (key.clone(), without(member, keys)))
                .collect::<Map<String, Value>>(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(|item| without(item, keys)).collect()),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => value.clone(),
    }
}

pub(crate) fn rule<'a>(model: &'a Value, id: &str) -> &'a Value {
    model["rules"]
        .as_array()
        .unwrap_or_else(|| panic!("the model has no `rules` array: {model}"))
        .iter()
        .find(|found| found["id"] == id)
        .unwrap_or_else(|| panic!("no rule {id} in {model}"))
}

pub(crate) fn actions(path: &Value) -> Vec<String> {
    path["steps"]
        .as_array()
        .unwrap_or_else(|| panic!("the path has no `steps` array: {path}"))
        .iter()
        .map(|step| step["action"].as_str().unwrap_or_default().to_owned())
        .collect()
}
