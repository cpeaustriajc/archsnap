use serde_json::{Value, json};

use crate::expected::{self, TWINS};
use crate::support::{Sandbox, actions, finding, model, models, rule, without};

const DIFFERS_BY_LANGUAGE: [&str; 4] = ["file", "line", "milliseconds", "cached"];

fn verify_one(file: &str, name: &str) -> (Option<i32>, Value) {
    let sandbox = Sandbox::with(&[file]);
    let run = sandbox.earnest(&["verify", &format!("models/{file}"), "--json"]);
    let report = run.json();
    assert_eq!(models(&report).len(), 1, "{file} holds one model\n{run}");
    (run.code, model(&report, name).clone())
}

#[test]
fn r02_a_rust_twin_gives_the_same_states_verdicts_and_paths_as_its_typescript_original() {
    for (name, typescript, rust) in TWINS {
        let (typescript_code, original) = verify_one(typescript, name);
        let (rust_code, twin) = verify_one(rust, name);
        let wanted = expected::expected(name);
        assert_eq!(twin["states"], wanted.states, "the states of {rust}");
        assert_eq!(twin["status"], wanted.status, "the status of {rust}");
        assert_eq!(
            without(&twin, &DIFFERS_BY_LANGUAGE),
            without(&original, &DIFFERS_BY_LANGUAGE),
            "{rust} and {typescript} give the same result"
        );
        assert_eq!(
            rust_code, typescript_code,
            "{rust} and {typescript} exit alike"
        );
    }
}

#[test]
fn r02_a_rust_model_reports_its_states_and_the_path_to_its_broken_rule() {
    let sandbox = Sandbox::with(&["turnstile.rs"]);
    let run = sandbox.earnest(&["verify", "--json"]);
    run.exits(1);
    let report = run.json();
    let turnstile = model(&report, "Turnstile");
    assert_eq!(
        turnstile["file"], "models/turnstile.rs",
        "the file of the Rust model"
    );
    assert_eq!(turnstile["status"], "fail", "the status of the Rust model");
    assert_eq!(
        turnstile["states"], 4,
        "the distinct states of the Rust model"
    );
    let locked = rule(turnstile, "locked");
    assert_eq!(
        locked["says"], "The gate stays locked until the visitor pays",
        "the sentence comes from the `rule` attribute"
    );
    assert_eq!(
        locked["path"],
        json!({
            "start": [
                { "name": "gate", "value": "Locked" },
                { "name": "paid", "value": false },
            ],
            "steps": [{
                "action": "push",
                "label": "the visitor pushes",
                "input": [],
                "changes": [{ "field": "gate", "before": "Locked", "after": "Open" }],
                "state": [
                    { "name": "gate", "value": "Open" },
                    { "name": "paid", "value": false },
                ],
            }],
        }),
        "an enum value is known by its variant name"
    );
}

#[test]
fn r02_a_rust_model_takes_inputs_from_method_parameters() {
    let sandbox = Sandbox::with(&["mailbox.rs"]);
    let run = sandbox.earnest(&["verify", "--json"]);
    run.exits(1);
    let report = run.json();
    let mailbox = model(&report, "Mailbox");
    assert_eq!(
        mailbox["states"], 7,
        "the distinct states of the Rust model"
    );
    assert_eq!(
        rule(mailbox, "fronted")["verdict"],
        "holds",
        "the rule that holds"
    );
    let path = &rule(mailbox, "bills")["path"];
    assert_eq!(
        actions(path),
        ["deliver", "deliver"],
        "the path to the broken rule"
    );
    assert_eq!(
        path["steps"][0]["input"],
        json!([{ "name": "letter", "value": "Bill" }]),
        "a method parameter is the input of the step"
    );
    assert_eq!(
        path["steps"][1]["state"],
        json!([{ "name": "inbox", "value": ["Bill", "Bill"] }]),
        "the queue after two deliveries"
    );
}

#[test]
fn r02_a_rust_model_names_its_stuck_state_and_respects_its_end() {
    let sandbox = Sandbox::with(&["counter.rs", "download.rs"]);
    let run = sandbox.earnest(&["verify", "--json"]);
    run.exits(1);
    let report = run.json();
    assert_eq!(
        model(&report, "Counter")["status"],
        "pass",
        "a Rust model with an end"
    );
    assert_eq!(
        model(&report, "Counter")["states"],
        4,
        "its distinct states"
    );
    let download = model(&report, "Download");
    assert_eq!(
        download["states"], 4,
        "the distinct states of the stuck model"
    );
    assert_eq!(
        actions(&finding(download, "stuck")["path"]),
        ["start", "fail"],
        "the path to the stuck state"
    );
}
