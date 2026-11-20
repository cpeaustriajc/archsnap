//! Compares the order in which the checker, the TypeScript library in Node and the Rust library
//! walk the items of a set and the keys of a map, for every kind of item the libraries allow.
//! The fixtures in `tasks/conformance/agreement` put each kind in a set or a map in an order
//! that is not the order it is declared or written in, and ask a rule per kind that faults on
//! every item but one: a rule holds only where the walk reaches the deciding item first. The
//! checker and Node must give the same verdicts and print the same start state, a Rust model
//! and its TypeScript twin must give the same output, and the Rust model built as real Rust
//! must give the verdicts its `Report` text and its rules give in a breadth-first walk.

#[path = "../rust_agreement/harness.rs"]
mod harness;

use std::process::Command;

use serde_json::{Value, json};

use harness::{Crate, Sandbox, repository, source};

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
		rules: found.rules.map((rule) => ({
			id: rule.id,
			verdict: rule.verdict,
			start: rule.path === null ? null : rule.path.start,
		})),
		findings: found.findings.map((finding) => ({
			kind: finding.kind,
			start: finding.path.start,
		})),
	});
}
console.log(JSON.stringify(results));
"#;

struct Twin {
    typescript: &'static str,
    rust: &'static str,
    model: &'static str,
}

const TWINS: [Twin; 2] = [
    Twin {
        typescript: "agreement/walk_text.model.ts",
        rust: "agreement/walk_text.rs",
        model: "Shelf",
    },
    Twin {
        typescript: "agreement/walk_shapes.model.ts",
        rust: "agreement/walk_shapes.rs",
        model: "Shapes",
    },
];

fn comparable(model: &Value) -> Value {
    let rules: Vec<Value> = model["rules"]
        .as_array()
        .expect("a model has rules")
        .iter()
        .map(|rule| {
            json!({
                "verdict": rule["verdict"],
                "start": rule["path"]["start"],
            })
        })
        .collect();
    let findings: Vec<Value> = model["findings"]
        .as_array()
        .expect("a model has findings")
        .iter()
        .map(|finding| json!({ "kind": finding["kind"], "start": finding["path"]["start"] }))
        .collect();
    json!({ "states": model["states"], "rules": rules, "findings": findings })
}

fn from_node(model: &Value) -> Value {
    let rules: Vec<Value> = model["rules"]
        .as_array()
        .expect("node gives rules")
        .iter()
        .map(|rule| json!({ "verdict": rule["verdict"], "start": rule["start"] }))
        .collect();
    json!({ "states": model["states"], "rules": rules, "findings": model["findings"] })
}

fn node(file: &str) -> Value {
    let sandbox = Sandbox::new();
    sandbox.write("hooks.mjs", HOOKS);
    sandbox.write("explore.mjs", SCRIPT);
    sandbox.write("model.model.ts", &source(file));
    let library = repository().join("npm/model/src/index.ts");
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
        .and_then(|models| models.first())
        .expect("node finds the model")
        .clone()
}

fn checked(file: &str, name: &str) -> Value {
    let sandbox = Sandbox::new();
    let local = file.rsplit('/').next().expect("a path has a last part");
    sandbox.write(local, &source(file));
    let models = sandbox.verified(local);
    assert_eq!(models.len(), 1, "{file} holds one model");
    assert_eq!(models[0]["name"], name, "the model of {file}");
    models[0].clone()
}

fn start_line(file: &str) -> String {
    let sandbox = Sandbox::new();
    let local = file.rsplit('/').next().expect("a path has a last part");
    sandbox.write(local, &source(file));
    let text = sandbox.earnest(&["verify", local, "--no-cache"]).text();
    text.lines()
        .find_map(|line| line.trim_start().strip_prefix("start  "))
        .unwrap_or_else(|| panic!("a start state is printed for {file}:\n{text}"))
        .to_owned()
}

fn verdicts(model: &Value) -> Vec<(String, String)> {
    model["rules"]
        .as_array()
        .expect("a model has rules")
        .iter()
        .map(|rule| {
            (
                rule["id"].as_str().expect("a rule has an id").to_owned(),
                rule["verdict"]
                    .as_str()
                    .expect("a rule has a verdict")
                    .to_owned(),
            )
        })
        .collect()
}

#[test]
fn the_checker_and_node_walk_the_items_of_a_set_in_one_order() {
    for twin in &TWINS {
        let ours = checked(twin.typescript, twin.model);
        let theirs = node(twin.typescript);
        assert_eq!(
            comparable(&ours),
            from_node(&theirs),
            "{}: verdicts, state and findings from two implementations",
            twin.typescript
        );
    }
}

#[test]
fn the_probes_hold_only_for_the_item_the_order_puts_first() {
    let wanted: [(&str, &[&str]); 2] = [
        (
            "agreement/walk_text.model.ts",
            &[
                "shown: broken",
                "appleFirst: holds",
                "pearFirst: unknown",
                "upperFirst: holds",
                "lowerFirst: unknown",
                "nullFirst: holds",
                "aFirstOfSome: unknown",
            ],
        ),
        (
            "agreement/walk_shapes.model.ts",
            &[
                "shown: broken",
                "numsAscend: holds",
                "numsDoNotDescend: unknown",
                "falseFirst: holds",
                "spotsByNames: holds",
                "spotsNotInDeclaredOrder: unknown",
                "emptyGroupFirst: holds",
                "prefixFirst: holds",
                "shortLineFirst: holds",
                "lowLineBeforeMid: unknown",
                "brightMapFirst: holds",
            ],
        ),
    ];
    for (file, expected) in wanted {
        let twin = TWINS
            .iter()
            .find(|twin| twin.typescript == file)
            .expect("the fixture is one of the twins");
        let found: Vec<String> = verdicts(&checked(file, twin.model))
            .into_iter()
            .map(|(id, verdict)| format!("{id}: {verdict}"))
            .collect();
        assert_eq!(found, expected, "the verdict of each probe in {file}");
    }
}

#[test]
fn a_rust_model_and_its_typescript_twin_give_the_same_output() {
    for twin in &TWINS {
        let rust = checked(twin.rust, twin.model);
        let typescript = checked(twin.typescript, twin.model);
        assert_eq!(
            comparable(&rust),
            comparable(&typescript),
            "{} and {}",
            twin.rust,
            twin.typescript
        );
        let rust_text = start_line(twin.rust);
        let typescript_text = start_line(twin.typescript);
        assert_eq!(
            rust_text, typescript_text,
            "the printed start state, which lists every set in its order"
        );
    }
}

fn walking(twin: &Twin, model: &Value, shown: &str) -> String {
    let text = source(twin.rust);
    let fields: Vec<String> = model["rules"][0]["path"]["start"]
        .as_array()
        .expect("the first rule breaks at the start")
        .iter()
        .map(|field| {
            let name = field["name"].as_str().expect("a field has a name");
            format!("format!(\"{name} = {{}}\", Report::report(&state.{name}))")
        })
        .collect();
    let rules: Vec<String> = verdicts(model)
        .into_iter()
        .map(|(id, _)| format!("(\"{id}\", catch_unwind(AssertUnwindSafe(|| state.{id}())).ok())"))
        .collect();
    format!(
        "{text}\n#[cfg(test)]\nmod walked {{\n    use super::*;\n    \
         use earnest::model::{{Behavior, Model, Report}};\n    \
         use std::collections::{{HashSet, VecDeque}};\n    \
         use std::panic::{{AssertUnwindSafe, catch_unwind}};\n\n    \
         #[test]\n    fn rust_walks_the_items_as_the_checker_does() {{\n        \
         std::panic::set_hook(Box::new(|_| {{}}));\n        \
         let start = {model}::starts().remove(0);\n        \
         let shown: Vec<String> = {{ let state = &start; vec![{fields}] }};\n        \
         assert_eq!(shown.join(\", \"), {shown:?}, \"the start state as Report prints it\");\n        \
         let mut seen: HashSet<{model}> = HashSet::new();\n        \
         let mut queue = VecDeque::new();\n        \
         for first in {model}::starts() {{\n            \
         if seen.insert(first.clone()) {{\n                queue.push_back(first);\n            }}\n        }}\n        \
         let mut broken: Vec<&str> = Vec::new();\n        \
         let mut faulted: Vec<&str> = Vec::new();\n        \
         while let Some(state) = queue.pop_front() {{\n            \
         for (id, outcome) in [{rules}] {{\n                \
         match outcome {{\n                    Some(false) => broken.push(id),\n                    \
         None => faulted.push(id),\n                    Some(true) => {{}}\n                }}\n            }}\n            \
         for step in state.steps() {{\n                \
         if seen.insert(step.state.clone()) {{\n                    queue.push_back(step.state);\n                }}\n            }}\n        }}\n        \
         let mut verdicts: Vec<String> = Vec::new();\n        \
         for id in [{ids}] {{\n            \
         let verdict = if broken.contains(&id) {{ \"broken\" }} else if faulted.contains(&id) {{ \"unknown\" }} else {{ \"holds\" }};\n            \
         verdicts.push(format!(\"{{id}}: {{verdict}}\"));\n        }}\n        \
         assert_eq!(seen.len(), {states}, \"the distinct states\");\n        \
         assert_eq!(verdicts, {wanted:?}, \"the verdict of every rule\");\n    }}\n}}\n",
        model = twin.model,
        fields = fields.join(", "),
        rules = rules.join(", "),
        ids = verdicts(model)
            .iter()
            .map(|(id, _)| format!("\"{id}\""))
            .collect::<Vec<String>>()
            .join(", "),
        states = model["states"],
        wanted = verdicts(model)
            .iter()
            .map(|(id, verdict)| format!("{id}: {verdict}"))
            .collect::<Vec<String>>(),
    )
}

#[test]
fn rust_built_with_rustc_walks_every_kind_in_the_order_of_the_checker() {
    let mut modules = Vec::new();
    for twin in &TWINS {
        let model = checked(twin.rust, twin.model);
        let shown = start_line(twin.rust);
        let module = twin
            .rust
            .rsplit('/')
            .next()
            .and_then(|name| name.strip_suffix(".rs"))
            .expect("a Rust fixture ends in .rs");
        modules.push((module.to_owned(), walking(twin, &model, &shown)));
    }
    let compiled = Crate::make("walk_order", &modules);
    let output = compiled
        .tested()
        .unwrap_or_else(|output| panic!("what rustc computes differs from the checker\n{output}"));
    for (module, _) in &modules {
        let name = format!("{module}::walked::rust_walks_the_items_as_the_checker_does");
        assert!(output.contains(&name), "{module} was walked\n{output}");
    }
}

#[test]
fn the_fixtures_build_as_real_rust() {
    let modules: Vec<(String, String)> = TWINS
        .iter()
        .filter_map(|twin| {
            let module = twin.rust.rsplit('/').next()?.strip_suffix(".rs")?;
            Some((module.to_owned(), source(twin.rust)))
        })
        .collect();
    let built = Crate::make("walk_order_built", &modules);
    built
        .built()
        .unwrap_or_else(|output| panic!("a fixture does not compile\n{output}"));
}
