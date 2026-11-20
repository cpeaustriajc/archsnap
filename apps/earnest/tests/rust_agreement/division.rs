use std::fs;
use std::process::Command;

use serde_json::{Value, json};

use crate::harness::{Crate, Sandbox, repository, source};

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

const [modelFile] = process.argv.slice(2);
const exports = await import(pathToFileURL(modelFile).href);
const results = [];
for (const value of Object.values(exports)) {
	if (!(value instanceof Model)) continue;
	const found = explore(value, { exports });
	results.push({
		name: value.name,
		states: found.states,
		findings: found.findings.map((finding) => ({
			kind: finding.kind,
			message: finding.message,
			steps: finding.path.steps.map((step) => step.action),
		})),
	});
}
console.log(JSON.stringify(results));
"#;

fn node(file: &str) -> Vec<Value> {
    let sandbox = Sandbox::new();
    sandbox.write("hooks.mjs", HOOKS);
    sandbox.write("explore.mjs", SCRIPT);
    sandbox.write("model.model.ts", &source(file));
    let library = repository().join("packages/model/src/index.ts");
    let output = Command::new("node")
        .args(["--import", "./hooks.mjs", "explore.mjs", "model.model.ts"])
        .env("EARNEST_MODEL_LIBRARY", &library)
        .current_dir(sandbox.path())
        .output()
        .expect("`node` is on the `PATH`");
    assert!(
        output.status.success(),
        "node could not explore {file}:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let printed: Value = serde_json::from_slice(&output.stdout).expect("node prints JSON");
    printed
        .as_array()
        .expect("the script prints a list")
        .clone()
}

fn comparable(models: &[Value]) -> Vec<Value> {
    let mut sorted: Vec<&Value> = models.iter().collect();
    sorted.sort_by_key(|model| model["name"].as_str().unwrap_or_default().to_owned());
    sorted
        .into_iter()
        .map(|model| {
            let findings: Vec<Value> = model["findings"]
                .as_array()
                .expect("a model has findings")
                .iter()
                .map(|finding| {
                    let steps: Vec<Value> = match finding["path"]["steps"].as_array() {
                        Some(listed) => listed.iter().map(|step| step["action"].clone()).collect(),
                        None => finding["steps"].as_array().cloned().unwrap_or_default(),
                    };
                    json!({
                        "kind": finding["kind"],
                        "message": finding["message"],
                        "steps": steps,
                    })
                })
                .collect();
            json!({ "name": model["name"], "states": model["states"], "findings": findings })
        })
        .collect()
}

#[test]
fn node_and_the_checker_report_a_division_fault_with_the_same_sentence_and_path() {
    let sandbox = Sandbox::new();
    sandbox.write("division.model.ts", &source("agreement/division.model.ts"));
    let checked = sandbox.verified("division.model.ts");
    let theirs = node("agreement/division.model.ts");
    assert_eq!(
        comparable(&checked),
        comparable(&theirs),
        "the states, the fault and its path in TypeScript, from two implementations"
    );
    let message = checked[0]["findings"][0]["message"]
        .as_str()
        .expect("a fault has a message");
    assert!(
        message.ends_with(
            "takes a division with a negative number or a divisor below 1 in this state"
        ),
        "{message}"
    );
}

#[test]
fn the_rust_file_has_the_same_faults_as_its_typescript_twin_and_rustc_agrees_where_it_panics() {
    let rust = source("agreement/division_fault.rs");
    let sandbox = Sandbox::new();
    sandbox.write("division_fault.rs", &rust);
    let ours = sandbox.verified("division_fault.rs");
    sandbox.write("division.model.ts", &source("agreement/division.model.ts"));
    let twin = sandbox.verified("division.model.ts");
    assert_eq!(
        comparable(&ours),
        comparable(&twin),
        "a Rust model and its TypeScript twin fault in the same places"
    );
    let walked = format!(
        "{rust}\n#[cfg(test)]\nmod walked {{\n    use super::*;\n    use earnest::model::{{Behavior, Model}};\n\n    \
         #[test]\n    #[should_panic(expected = \"attempt to divide by zero\")]\n    \
         fn rust_panics_on_a_divisor_of_zero_where_the_checker_reports_the_fault() {{\n        \
         let start = Split::starts().remove(0);\n        let _ = start.steps();\n    }}\n\n    \
         #[test]\n    fn rust_truncates_a_negative_number_where_the_checker_declines_to_guess() {{\n        \
         let start = Below::starts().remove(0);\n        let steps = start.steps();\n        \
         assert_eq!(steps.len(), 1, \"rust takes the step\");\n        \
         assert_eq!(steps[0].state.q, -1, \"-1 / 1\");\n        \
         assert_eq!(-3_i8 / 2, -1, \"rust truncates toward zero\");\n    }}\n}}\n"
    );
    let compiled = Crate::make("division", &[("division_fault".to_owned(), walked)]);
    let output = compiled
        .tested()
        .unwrap_or_else(|output| panic!("what rustc does with the division\n{output}"));
    assert!(
        output.contains("rust_panics_on_a_divisor_of_zero_where_the_checker_reports_the_fault"),
        "{output}"
    );
    fs::write(sandbox.path().join("unused"), "").expect("the sandbox is writable");
}

#[test]
fn a_raw_slash_in_a_typescript_model_is_refused_and_names_m_div() {
    let sandbox = Sandbox::new();
    let model = source("agreement/division.model.ts").replace("m.div(s.n, by)", "s.n / by");
    sandbox.write("division.model.ts", &model);
    let run = sandbox.earnest(&["verify", "division.model.ts"]);
    let text = run.text();
    assert_eq!(run.code, Some(1), "{text}");
    assert!(text.contains("unreadable"), "{text}");
    assert!(text.contains("`/`"), "the feature is named\n{text}");
    assert!(
        text.contains("m.div(a, b)"),
        "the sentence names the call\n{text}"
    );
}
