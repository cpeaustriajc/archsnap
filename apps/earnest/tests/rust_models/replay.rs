use std::fmt::Write as _;

use serde_json::Value;

use crate::crates::{Crate, fixtures};
use crate::support::{Sandbox, rule, source};

struct Case {
    module: &'static str,
    model: &'static str,
    rule: &'static str,
}

const CASES: [Case; 4] = [
    Case {
        module: "turnstile",
        model: "Turnstile",
        rule: "locked",
    },
    Case {
        module: "mailbox",
        model: "Mailbox",
        rule: "bills",
    },
    Case {
        module: "oven",
        model: "Oven",
        rule: "safe",
    },
    Case {
        module: "oven",
        model: "Oven",
        rule: "closed",
    },
];

fn value_text(value: &Value) -> String {
    match value {
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(text) => text.clone(),
        Value::Array(items) => {
            let listed: Vec<String> = items.iter().map(value_text).collect();
            format!("[{}]", listed.join(", "))
        }
        other => panic!("a value this test can't write as `Debug` does: {other}"),
    }
}

fn debug_of(model: &str, state: &Value) -> String {
    let fields: Vec<String> = state
        .as_array()
        .unwrap_or_else(|| panic!("a state is a list of fields: {state}"))
        .iter()
        .map(|field| {
            format!(
                "{}: {}",
                field["name"].as_str().unwrap_or_default(),
                value_text(&field["value"])
            )
        })
        .collect();
    format!("{model} {{ {} }}", fields.join(", "))
}

fn moves_of(path: &Value) -> String {
    let moves: Vec<String> = path["steps"]
        .as_array()
        .unwrap_or_else(|| panic!("a path has steps: {path}"))
        .iter()
        .map(|step| {
            let mut text = format!(
                "Move::new({:?})",
                step["action"].as_str().unwrap_or_default()
            );
            for input in step["input"].as_array().into_iter().flatten() {
                let _ = write!(
                    text,
                    ".with({:?}, {:?})",
                    input["name"].as_str().unwrap_or_default(),
                    value_text(&input["value"])
                );
            }
            text
        })
        .collect();
    moves.join(", ")
}

fn states_of(model: &str, path: &Value) -> Vec<String> {
    let mut states = vec![debug_of(model, &path["start"])];
    for step in path["steps"].as_array().into_iter().flatten() {
        states.push(debug_of(model, &step["state"]));
    }
    states
}

fn test_for(case: &Case, path: &Value) -> String {
    let wanted = states_of(case.model, path);
    let listed: Vec<String> = wanted.iter().map(|state| format!("{state:?}")).collect();
    format!(
        "\n#[test]\nfn {module}_{rule}() {{\n    let wanted = [{wanted}];\n    \
         let start = {model}::starts()\n        .into_iter()\n        \
         .find(|state| format!(\"{{state:?}}\") == wanted[0])\n        \
         .expect(\"a start state the checker gave\");\n    \
         let path = [{moves}];\n    \
         let states = replay(&start, &path).expect(\"the path can be taken\");\n    \
         let shown: Vec<String> = states.iter().map(|state| format!(\"{{state:?}}\")).collect();\n    \
         assert_eq!(shown, wanted);\n    \
         assert!(!states.last().expect(\"a start state\").rules().iter().all(|claim| claim.holds));\n}}\n",
        module = case.module,
        rule = case.rule,
        wanted = listed.join(", "),
        model = case.model,
        moves = moves_of(path),
    )
}

const NEGATIVE: &str = "
#[test]
#[should_panic(expected = \"assertion `left == right` failed\")]
fn a_state_that_differs_from_the_checkers_is_caught() {
    let start = Turnstile::starts().remove(0);
    let states = replay(&start, &[Move::new(\"push\")]).expect(\"the path can be taken\");
    assert_eq!(format!(\"{:?}\", states[1]), \"Turnstile { gate: Open, paid: true }\");
}

#[test]
fn a_step_the_state_does_not_allow_is_refused_with_what_could_happen() {
    let start = Turnstile::starts().remove(0);
    let paid = [Move::new(\"pay\"), Move::new(\"pay\")];
    let refused = replay(&start, &paid).expect_err(\"a visitor pays once\");
    assert_eq!(refused.step, 2);
    assert!(refused.reason.contains(\"`push`\"), \"{refused}\");
    let unknown = replay(&start, &[Move::new(\"jump\")]).expect_err(\"no such action\");
    assert_eq!(unknown.step, 1);
}
";

fn replay_source(paths: &[(&Case, Value)]) -> String {
    let mut text = String::from(
        "use crate::mailbox::Mailbox;\nuse crate::oven::Oven;\nuse crate::turnstile::Turnstile;\n\
         use earnest::model::{Behavior, Model, Move, replay};\n",
    );
    for (case, path) in paths {
        text.push_str(&test_for(case, path));
    }
    text.push_str(NEGATIVE);
    text
}

fn checker_path(case: &Case) -> Value {
    let sandbox = Sandbox::new();
    sandbox.write("model.rs", &source(&format!("models/{}.rs", case.module)));
    let found = sandbox.verified("model.rs", case.model);
    let broken = rule(&found, case.rule);
    assert_eq!(broken["verdict"], "broken", "{} is broken", case.rule);
    broken["path"].clone()
}

#[test]
fn r21_a_path_from_the_report_replays_through_the_rust_library_to_the_checkers_states() {
    let paths: Vec<(&Case, Value)> = CASES
        .iter()
        .map(|case| (case, checker_path(case)))
        .collect();
    let modules: Vec<(String, String)> = fixtures("models")
        .into_iter()
        .filter(|(module, _)| ["turnstile", "mailbox", "oven"].contains(&module.as_str()))
        .collect();
    let replaying = Crate::make("replay", &modules, Some(&replay_source(&paths)));
    let output = replaying
        .test_output()
        .unwrap_or_else(|output| panic!("`cargo test` of the replay failed\n{output}"));
    assert!(
        output.contains("a_state_that_differs_from_the_checkers_is_caught"),
        "the negative test is not run\n{output}"
    );
}
