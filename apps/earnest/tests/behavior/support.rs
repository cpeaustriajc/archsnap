use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use assert_cmd::Command;
use serde_json::{Map, Value, json};
use tempfile::TempDir;

const HANG_GUARD: Duration = Duration::from_mins(5);

const MODEL_STATUSES: [&str; 3] = ["pass", "fail", "unfinished"];

pub(crate) fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root is two folders above the CLI crate")
}

pub(crate) struct Sandbox {
    folder: TempDir,
}

impl Sandbox {
    pub(crate) fn empty() -> Self {
        let folder = tempfile::Builder::new()
            .prefix("earnest-conformance-")
            .tempdir()
            .expect("a temporary folder can be made");
        Self { folder }
    }

    pub(crate) fn with(fixtures: &[&str]) -> Self {
        let sandbox = Self::empty();
        for fixture in fixtures {
            sandbox.copy(fixture, &format!("models/{fixture}"));
        }
        sandbox
    }

    pub(crate) fn path(&self) -> &Path {
        self.folder.path()
    }

    pub(crate) fn copy(&self, fixture: &str, relative: &str) {
        let source = repository().join("tasks/conformance/models").join(fixture);
        let text = fs::read_to_string(&source)
            .unwrap_or_else(|error| panic!("fixture {} can't be read: {error}", source.display()));
        self.write(relative, &text);
    }

    pub(crate) fn write(&self, relative: &str, text: &str) {
        let target = self.path().join(relative);
        let parent = target.parent().expect("a file in the sandbox has a folder");
        fs::create_dir_all(parent).expect("a folder in the sandbox can be made");
        fs::write(&target, text).expect("a file in the sandbox can be written");
    }

    pub(crate) fn read(&self, relative: &str) -> String {
        let target = self.path().join(relative);
        fs::read_to_string(&target)
            .unwrap_or_else(|error| panic!("{} can't be read: {error}", target.display()))
    }

    pub(crate) fn entries(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(self.path())
            .expect("the sandbox can be listed")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    pub(crate) fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_earnest"));
        command.current_dir(self.path()).timeout(HANG_GUARD);
        command
    }

    pub(crate) fn earnest(&self, arguments: &[&str]) -> Run {
        Run::of(self.command().args(arguments))
    }
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
    pub(crate) fn of(command: &mut Command) -> Self {
        let output = command.output().expect("the earnest binary starts");
        Self {
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    pub(crate) fn exits(&self, code: i32) -> &Self {
        assert_eq!(self.code, Some(code), "expected exit code {code}\n{self}");
        self
    }

    pub(crate) fn json(&self) -> Value {
        serde_json::from_str(&self.stdout)
            .unwrap_or_else(|error| panic!("stdout is not JSON ({error})\n{self}"))
    }

    pub(crate) fn model_lines(&self) -> Vec<Vec<String>> {
        self.stdout
            .lines()
            .filter(|line| !line.starts_with(char::is_whitespace))
            .map(fields)
            .filter(|line| {
                line.first()
                    .is_some_and(|status| MODEL_STATUSES.contains(&status.as_str()))
            })
            .collect()
    }

    pub(crate) fn find_model_line(&self, name: &str) -> Option<Vec<String>> {
        self.model_lines()
            .into_iter()
            .find(|line| line.get(1).is_some_and(|model| model == name))
    }

    pub(crate) fn model_line(&self, name: &str) -> Vec<String> {
        self.find_model_line(name)
            .unwrap_or_else(|| panic!("no result line for model {name}\n{self}"))
    }

    pub(crate) fn detail(&self, name: &str) -> Vec<String> {
        self.stdout
            .lines()
            .skip_while(|line| {
                line.starts_with(char::is_whitespace)
                    || fields(line).get(1).is_none_or(|model| model != name)
            })
            .skip(1)
            .take_while(|line| line.starts_with(char::is_whitespace))
            .map(|line| line.trim().to_owned())
            .collect()
    }

    pub(crate) fn summary_line(&self) -> String {
        self.stdout
            .lines()
            .rev()
            .find(|line| {
                let mut words = line.split_whitespace();
                let counted = words.next().is_some_and(|word| word.parse::<u64>().is_ok());
                let named = words
                    .next()
                    .is_some_and(|word| word == "models:" || word == "model:");
                counted && named
            })
            .unwrap_or_else(|| panic!("no summary line\n{self}"))
            .to_owned()
    }

    pub(crate) fn summary_count(&self, status: &str) -> u64 {
        let line = self.summary_line();
        let (_, counts) = line
            .split_once(':')
            .expect("the summary line has a colon after the model count");
        counts
            .split(',')
            .filter_map(|count| count.trim().split_once(' '))
            .find(|(_, word)| *word == status)
            .map_or(0, |(number, _)| {
                number
                    .parse()
                    .unwrap_or_else(|error| panic!("bad count in summary ({error})\n{self}"))
            })
    }

    pub(crate) fn summary_total(&self) -> u64 {
        self.summary_line()
            .split_whitespace()
            .next()
            .and_then(|word| word.parse().ok())
            .expect("the summary line starts with the model count")
    }
}

pub(crate) fn fields(line: &str) -> Vec<String> {
    line.trim()
        .split("  ")
        .map(str::trim)
        .filter(|field| !field.is_empty())
        .map(str::to_owned)
        .collect()
}

pub(crate) struct TextPath {
    pub(crate) start: String,
    pub(crate) steps: Vec<String>,
}

impl TextPath {
    pub(crate) fn actions(&self) -> Vec<String> {
        self.steps
            .iter()
            .map(|step| fields(step).first().cloned().unwrap_or_default())
            .collect()
    }
}

fn is_head(line: &str, head: &[&str]) -> bool {
    let found = fields(line);
    found.len() >= head.len()
        && found
            .iter()
            .zip(head)
            .enumerate()
            .all(|(position, (have, want))| {
                if position == 0 {
                    have.starts_with(want)
                } else {
                    have == want
                }
            })
}

fn step_body(line: &str, number: usize) -> Option<String> {
    line.strip_prefix(&format!("{number}."))
        .map(|rest| rest.trim().to_owned())
}

pub(crate) fn find_path_under(detail: &[String], head: &[&str]) -> Option<TextPath> {
    let position = detail.iter().position(|line| is_head(line, head))?;
    let mut rest = detail.iter().skip(position.saturating_add(1));
    let start_fields = fields(rest.next()?);
    if start_fields.first().is_none_or(|word| word != "start") {
        return None;
    }
    let start = start_fields
        .iter()
        .skip(1)
        .cloned()
        .collect::<Vec<_>>()
        .join("  ");
    let mut steps = Vec::new();
    for line in rest {
        match step_body(line, steps.len().saturating_add(1)) {
            Some(body) => steps.push(body),
            None => break,
        }
    }
    Some(TextPath { start, steps })
}

pub(crate) fn path_under(detail: &[String], head: &[&str]) -> TextPath {
    find_path_under(detail, head).unwrap_or_else(|| {
        panic!(
            "no `{}` finding followed by a `start` line in:\n{}",
            head.join("  "),
            detail.join("\n")
        )
    })
}

pub(crate) fn models(report: &Value) -> &Vec<Value> {
    report["models"]
        .as_array()
        .unwrap_or_else(|| panic!("the report has no `models` array: {report}"))
}

pub(crate) fn find_model<'a>(report: &'a Value, name: &str) -> Option<&'a Value> {
    models(report).iter().find(|model| model["name"] == name)
}

pub(crate) fn model<'a>(report: &'a Value, name: &str) -> &'a Value {
    find_model(report, name).unwrap_or_else(|| panic!("no model {name} in {report}"))
}

pub(crate) fn find_rule<'a>(model: &'a Value, id: &str) -> Option<&'a Value> {
    model["rules"]
        .as_array()
        .unwrap_or_else(|| panic!("the model has no `rules` array: {model}"))
        .iter()
        .find(|rule| rule["id"] == id)
}

pub(crate) fn rule<'a>(model: &'a Value, id: &str) -> &'a Value {
    find_rule(model, id).unwrap_or_else(|| panic!("no rule {id} in {model}"))
}

pub(crate) fn findings<'a>(model: &'a Value, kind: &str) -> Vec<&'a Value> {
    model["findings"]
        .as_array()
        .unwrap_or_else(|| panic!("the model has no `findings` array: {model}"))
        .iter()
        .filter(|finding| finding["kind"] == kind)
        .collect()
}

pub(crate) fn finding<'a>(model: &'a Value, kind: &str) -> &'a Value {
    let found = findings(model, kind);
    assert_eq!(found.len(), 1, "expected one {kind} finding in {model}");
    found.first().expect("one finding was counted")
}

pub(crate) fn steps(path: &Value) -> &Vec<Value> {
    path["steps"]
        .as_array()
        .unwrap_or_else(|| panic!("the path has no `steps` array: {path}"))
}

pub(crate) fn actions(path: &Value) -> Vec<String> {
    steps(path)
        .iter()
        .map(|step| step["action"].as_str().unwrap_or_default().to_owned())
        .collect()
}

pub(crate) fn named(pairs: &[(&str, Value)]) -> Value {
    Value::Array(
        pairs
            .iter()
            .map(|(name, value)| json!({ "name": name, "value": value }))
            .collect(),
    )
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

pub(crate) fn without_timings(report: &Value) -> Value {
    without(report, &["milliseconds", "cached"])
}
