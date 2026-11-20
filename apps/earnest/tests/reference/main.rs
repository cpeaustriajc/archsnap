//! Compares the checker with the TypeScript library's own search in Node on every readable
//! TypeScript conformance model: the same distinct state count, the same verdict for each rule
//! and the same finding kinds from two implementations that share no code.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use assert_cmd::Command as Earnest;
use serde_json::{Value, json};
use tempfile::TempDir;

const HANG_GUARD: Duration = Duration::from_mins(5);
const UNREADABLE_FOLDER: &str = "unreadable";
const SLOW_FOLDER: &str = "limits";
const STATES_OF_A_SLOW_MODEL: &str = "20";

const HOOKS: &str = r#"import { registerHooks } from "node:module";
import { pathToFileURL } from "node:url";

const library = pathToFileURL(process.env.EARNEST_MODEL_LIBRARY).href;

registerHooks({
	resolve(specifier, context, nextResolve) {
		if (specifier === "@earnest/model") return { url: library, shortCircuit: true };
		return nextResolve(specifier, context);
	},
});
"#;

const SCRIPT: &str = r#"import { pathToFileURL } from "node:url";
import { Model, explore } from "@earnest/model";

const [modelFile, maxStates] = process.argv.slice(2);
const exports = await import(pathToFileURL(modelFile).href);
const limit = maxStates === undefined ? {} : { maxStates: Number(maxStates) };
const results = [];
for (const value of Object.values(exports)) {
	if (!(value instanceof Model)) continue;
	const found = explore(value, { exports, ...limit });
	results.push({
		name: value.name,
		status: found.status,
		states: found.states,
		rules: found.rules.map((rule) => ({ id: rule.id, verdict: rule.verdict })),
		findings: found.findings.map((finding) => finding.kind),
	});
}
console.log(JSON.stringify(results));
"#;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root is two folders above the CLI crate")
}

fn typescript_models(folder: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let entries = fs::read_dir(folder)
        .unwrap_or_else(|error| panic!("{} can't be listed: {error}", folder.display()));
    for path in entries.filter_map(Result::ok).map(|entry| entry.path()) {
        if path.is_dir() {
            if path
                .file_name()
                .is_none_or(|name| name != UNREADABLE_FOLDER)
            {
                files.extend(typescript_models(&path));
            }
        } else if path.extension().is_some_and(|extension| extension == "ts") {
            files.push(path);
        }
    }
    files.sort();
    files
}

fn state_limit(file: &Path) -> Option<&'static str> {
    file.parent()
        .and_then(Path::file_name)
        .is_some_and(|folder| folder == SLOW_FOLDER)
        .then_some(STATES_OF_A_SLOW_MODEL)
}

fn node_results(scratch: &Path, file: &Path) -> Vec<Value> {
    let library = repository().join("packages/model/src/index.ts");
    let output = Command::new("node")
        .args(["--import", "./hooks.mjs", "explore.mjs"])
        .arg(file)
        .args(state_limit(file))
        .env("EARNEST_MODEL_LIBRARY", &library)
        .current_dir(scratch)
        .output()
        .expect("`node` is on the `PATH`");
    assert!(
        output.status.success(),
        "node could not explore {}:\n{}{}",
        file.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let printed: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "node did not print JSON for {} ({error}): {}",
            file.display(),
            String::from_utf8_lossy(&output.stdout)
        )
    });
    printed
        .as_array()
        .expect("the script prints a list of models")
        .clone()
}

fn checker_results(scratch: &Path, file: &Path) -> Vec<Value> {
    let mut command = Earnest::new(env!("CARGO_BIN_EXE_earnest"));
    command
        .current_dir(scratch)
        .timeout(HANG_GUARD)
        .args(["verify", "--json", "--no-cache"])
        .arg(file);
    if let Some(limit) = state_limit(file) {
        command.args(["--max-states", limit]);
    }
    let output = command.output().expect("the earnest binary starts");
    let report: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "earnest did not print JSON for {} ({error}): {}{}",
            file.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    assert_eq!(
        report["unreadable"],
        json!([]),
        "{} is readable",
        file.display()
    );
    report["models"]
        .as_array()
        .expect("the report has a `models` array")
        .clone()
}

fn comparable(model: &Value) -> Value {
    let rules: Vec<Value> = model["rules"]
        .as_array()
        .expect("a model has a `rules` array")
        .iter()
        .map(|rule| json!({ "id": rule["id"], "verdict": rule["verdict"] }))
        .collect();
    let kinds: Vec<&Value> = model["findings"]
        .as_array()
        .expect("a model has a `findings` array")
        .iter()
        .map(|finding| &finding["kind"])
        .collect();
    json!({ "states": model["states"], "rules": rules, "findings": kinds })
}

fn from_node(model: &Value) -> Value {
    json!({
        "states": model["states"],
        "rules": model["rules"],
        "findings": model["findings"],
    })
}

fn scratch_folder() -> TempDir {
    let scratch = tempfile::Builder::new()
        .prefix("earnest-reference-")
        .tempdir()
        .expect("a temporary folder can be made");
    fs::write(scratch.path().join("hooks.mjs"), HOOKS).expect("the hooks can be written");
    fs::write(scratch.path().join("explore.mjs"), SCRIPT).expect("the script can be written");
    scratch
}

#[test]
fn node_and_the_checker_agree_on_every_readable_typescript_conformance_model() {
    let scratch = scratch_folder();
    let files = typescript_models(&repository().join("tasks/conformance/models"));
    assert!(
        files.len() >= 20,
        "the conformance models are found: {files:?}"
    );
    let mut compared = Vec::new();
    let mut disagreements = Vec::new();
    for file in &files {
        let ours = checker_results(scratch.path(), file);
        let theirs = node_results(scratch.path(), file);
        assert_eq!(
            ours.len(),
            theirs.len(),
            "both find the same number of models in {}",
            file.display()
        );
        for checker in &ours {
            let name = &checker["name"];
            let node = theirs
                .iter()
                .find(|node| node["name"] == *name)
                .unwrap_or_else(|| panic!("node found no model {name} in {}", file.display()));
            let (checker_found, node_found) = (comparable(checker), from_node(node));
            compared.push(format!("{name}: {checker_found}"));
            if checker_found != node_found {
                disagreements.push(format!(
                    "{name} in {}\n  checker: {checker_found}\n  node:    {node_found}",
                    file.display()
                ));
            }
        }
    }
    assert!(
        disagreements.is_empty(),
        "the checker and Node disagree on {} of {} models:\n{}",
        disagreements.len(),
        compared.len(),
        disagreements.join("\n")
    );
    assert!(
        compared.len() >= 23,
        "every model was compared:\n{}",
        compared.join("\n")
    );
}

#[test]
fn no_finding_of_the_checker_is_left_out_of_the_comparison() {
    let with_remainder = json!({
        "states": 3,
        "rules": [{ "id": "floor", "says": "x", "verdict": "holds", "path": null }],
        "findings": [
            { "kind": "stuck", "message": "no action can happen here", "path": {} },
            { "kind": "fault", "message": "the `when` of the action `refill` takes a remainder with a negative number or a divisor below 1 in this state", "path": {} },
        ],
    });
    assert_eq!(
        comparable(&with_remainder),
        json!({
            "states": 3,
            "rules": [{ "id": "floor", "verdict": "holds" }],
            "findings": ["stuck", "fault"],
        }),
        "the remainder fault is compared like any other finding"
    );
    let with_null_read = json!({
        "states": 4,
        "rules": [],
        "findings": [
            { "kind": "fault", "message": "the `then` of the action `price` reads a value that is null in this state", "path": {} },
        ],
    });
    assert_eq!(
        comparable(&with_null_read)["findings"],
        json!(["fault"]),
        "any other fault must be reported by both"
    );
}
