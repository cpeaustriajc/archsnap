use std::process::Command;

use serde_json::{Map, Value};

use crate::support::{Sandbox, model, repository, rule, steps};

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

const SCRIPT: &str = r#"import { readFileSync } from "node:fs";
import { replay } from "@earnest/model";

const [modelFile, modelName, pathFile] = process.argv.slice(2);
const models = await import(modelFile);
const path = JSON.parse(readFileSync(pathFile, "utf8"));
const states = replay(models[modelName], path);
console.log(JSON.stringify(states));
"#;

fn as_record(state: &Value) -> Value {
    match state.as_array() {
        Some(entries) => Value::Object(
            entries
                .iter()
                .map(|entry| {
                    let name = entry["name"].as_str().unwrap_or_default().to_owned();
                    (name, entry["value"].clone())
                })
                .collect::<Map<String, Value>>(),
        ),
        None => state.clone(),
    }
}

fn replay_in_node(sandbox: &Sandbox, file: &str, name: &str, path: &Value) -> Vec<Value> {
    let library = repository().join("packages/model/src/index.ts");
    assert!(
        library.is_file(),
        "the TypeScript model library is missing at {}",
        library.display()
    );
    sandbox.write("hooks.mjs", HOOKS);
    sandbox.write("replay.mjs", SCRIPT);
    sandbox.write("path.json", &path.to_string());
    let output = Command::new("node")
        .args(["--import", "./hooks.mjs", "replay.mjs"])
        .args([&format!("./{file}"), name, "path.json"])
        .env("EARNEST_MODEL_LIBRARY", &library)
        .current_dir(sandbox.path())
        .output()
        .expect("`node` is on the `PATH`");
    assert!(
        output.status.success(),
        "node could not replay the path of {name}:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let printed: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "node did not print the states as JSON ({error}): {}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    printed
        .as_array()
        .unwrap_or_else(|| panic!("`replay` gives back a list of states, not {printed}"))
        .iter()
        .map(as_record)
        .collect()
}

fn assert_node_sees_the_states_of(fixture: &str, name: &str, rule_id: &str, step_count: usize) {
    let sandbox = Sandbox::with(&[fixture]);
    let file = format!("models/{fixture}");
    let run = sandbox.earnest(&["verify", &file, "--json"]);
    run.exits(1);
    let report = run.json();
    let path = &rule(model(&report, name), rule_id)["path"];
    assert_eq!(
        steps(path).len(),
        step_count,
        "the steps of the path to {name}.{rule_id}"
    );

    let after_each_step: Vec<Value> = steps(path)
        .iter()
        .map(|step| as_record(&step["state"]))
        .collect();
    let mut from_the_start = vec![as_record(&path["start"])];
    from_the_start.extend(after_each_step.iter().cloned());

    let replayed = replay_in_node(&sandbox, &file, name, path);
    assert!(
        replayed == from_the_start || replayed == after_each_step,
        "node passes through the states of the checker, one for each action\n\
         node: {replayed:?}\nchecker: {from_the_start:?}"
    );
}

#[test]
fn r21_a_bug_path_from_the_report_replays_in_node_with_the_same_states() {
    assert_node_sees_the_states_of("turnstile.model.ts", "Turnstile", "locked", 1);
}

#[test]
fn r21_a_path_of_several_steps_from_a_chosen_start_replays_in_node() {
    assert_node_sees_the_states_of("lamp.model.ts", "Lamp", "dim", 3);
}

#[test]
fn r21_a_step_that_changes_two_fields_replays_in_node() {
    assert_node_sees_the_states_of("cart.model.ts", "Cart", "focusLands", 2);
}
