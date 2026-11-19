//! The conformance models, read from TypeScript, lowered and searched: each must give the
//! state count, verdicts and findings that `tasks/conformance/README.md` derives by hand.

mod expected;
mod limits;
mod models;
mod unreadable;

use earnest_ast::Draft;
use earnest_check::{Limits, Outcome, Path, explore};
use earnest_ir::Model;
use earnest_semantic::lower;
use earnest_ts::read;
use std::fs;
use std::path::PathBuf;

fn source(fixture: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tasks/conformance/models")
        .join(fixture);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn drafts(fixture: &str) -> Vec<Draft> {
    let reading = read(&format!("models/{fixture}"), &source(fixture));
    assert_eq!(reading.problems, [], "{fixture} is readable");
    reading.drafts
}

fn model(fixture: &str, name: &str) -> Model {
    let found = drafts(fixture);
    let draft = found
        .iter()
        .find(|draft| draft.name == name)
        .unwrap_or_else(|| panic!("{fixture} declares no model named {name}"));
    lower(draft).unwrap_or_else(|problems| panic!("{fixture} does not lower: {problems:#?}"))
}

fn search(fixture: &str, name: &str, limits: &Limits) -> Outcome {
    explore(&model(fixture, name), limits)
}

fn steps(path: &Path) -> Vec<String> {
    path.steps
        .iter()
        .map(|step| {
            if step.input.is_empty() {
                return step.action.clone();
            }
            let inputs: Vec<String> = step
                .input
                .iter()
                .map(|input| format!("{} = {}", input.name, input.value.text))
                .collect();
            format!("{}[{}]", step.action, inputs.join(", "))
        })
        .collect()
}
