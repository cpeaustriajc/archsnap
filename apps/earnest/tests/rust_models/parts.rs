use crate::support::{DIFFERS_BY_LANGUAGE, Sandbox, actions, rule, source, without};

const UPLOAD_TWINS: (&str, &str) = ("twins/upload.rs", "models/upload.model.ts");

const TIGHTER_TYPESCRIPT: (&str, &str) = ("s.tries.attempts <= 2", "s.tries.attempts <= 1");
const TIGHTER_RUST: (&str, &str) = ("self.tries.attempts() <= 2", "self.tries.attempts() <= 1");

fn both(typescript: &str, rust: &str) -> (serde_json::Value, serde_json::Value) {
    let sandbox = Sandbox::new();
    sandbox.write("model.rs", rust);
    sandbox.write("model.model.ts", typescript);
    (
        sandbox.verified("model.rs", "Upload"),
        sandbox.verified("model.model.ts", "Upload"),
    )
}

#[test]
fn a_rust_model_with_a_timer_a_request_and_a_retry_gives_the_states_and_verdicts_of_its_twin() {
    let (twin, original) = both(&source(UPLOAD_TWINS.1), &source(UPLOAD_TWINS.0));
    assert_eq!(twin["states"], 16, "4 + 6 + 6 states by tries used");
    assert_eq!(twin["status"], "pass", "every rule holds");
    for id in ["quiet", "waiting", "counted"] {
        assert_eq!(rule(&twin, id)["verdict"], "holds", "{id}");
    }
    assert_eq!(
        without(&twin, &DIFFERS_BY_LANGUAGE),
        without(&original, &DIFFERS_BY_LANGUAGE),
        "the same result in both languages"
    );
}

#[test]
fn a_rule_that_breaks_through_a_timer_a_request_and_a_retry_has_the_same_path_in_both() {
    let typescript = source(UPLOAD_TWINS.1).replacen(TIGHTER_TYPESCRIPT.0, TIGHTER_TYPESCRIPT.1, 1);
    let rust = source(UPLOAD_TWINS.0).replacen(TIGHTER_RUST.0, TIGHTER_RUST.1, 1);
    let (twin, original) = both(&typescript, &rust);
    assert_eq!(
        twin["status"], "fail",
        "a second wait is one retry too many"
    );
    assert_eq!(
        actions(&rule(&twin, "counted")["path"]),
        [
            "send", "fail", "wait", "wake", "rearm", "send", "fail", "wait"
        ],
        "send, fail and wait, wait for the timer and clear it, then the same again"
    );
    assert_eq!(
        without(&twin, &DIFFERS_BY_LANGUAGE),
        without(&original, &DIFFERS_BY_LANGUAGE),
        "the same states, verdicts and paths in both languages"
    );
}

#[test]
fn the_tla_export_of_the_rust_parts_is_the_one_of_the_typescript_parts() {
    let sandbox = Sandbox::new();
    sandbox.write("model.rs", &source(UPLOAD_TWINS.0));
    sandbox.write("model.model.ts", &source(UPLOAD_TWINS.1));
    assert_eq!(
        sandbox.tla("model.rs", "Upload", "rust"),
        sandbox.tla("model.model.ts", "Upload", "typescript"),
        "the module and the configuration"
    );
}
