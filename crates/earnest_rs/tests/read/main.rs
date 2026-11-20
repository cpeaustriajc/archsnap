//! Reading Rust model files: the conformance twins, the forms the README allows, and files
//! that are unreadable. Expected results come from `tasks/conformance/README.md` and from
//! working the models out by hand. The models in `models/` are also compiled here as ordinary
//! Rust, so what the reader accepts is what `rustc` accepts, and their own tests replay a path.

mod forms;
mod limits;
mod names;
#[path = "models/party.rs"]
mod party;
#[path = "models/shelf.rs"]
mod shelf;
#[path = "models/ticket.rs"]
mod ticket;
mod twins;
mod typed;
mod unreadable;

use earnest_ast::Draft;
use earnest_check::{Limits, Outcome, Path, explore};
use earnest_rs::read;
use earnest_semantic::lower;

fn drafts(file: &str, source: &str) -> Vec<Draft> {
    let reading = read(file, source);
    assert_eq!(reading.problems, [], "{file} reads without a problem");
    reading.drafts
}

fn only_draft(file: &str, source: &str) -> Draft {
    let mut found = drafts(file, source);
    assert_eq!(found.len(), 1, "{file} holds one model");
    found.remove(0)
}

fn search(draft: &Draft) -> Outcome {
    let model = match lower(draft) {
        Ok(model) => model,
        Err(problems) => panic!("{} does not lower: {problems:?}", draft.name),
    };
    explore(&model, &Limits::default())
}

fn actions(path: &Path) -> Vec<&str> {
    path.steps.iter().map(|step| step.action.as_str()).collect()
}

fn shown(path: &Path) -> Vec<String> {
    let line = |named: &[earnest_check::Named]| -> String {
        let fields: Vec<String> = named
            .iter()
            .map(|field| format!("{} = {}", field.name, field.value.text))
            .collect();
        fields.join(", ")
    };
    let mut lines = vec![line(&path.start)];
    for step in &path.steps {
        let input = line(&step.input);
        lines.push(format!("{}[{input}] -> {}", step.action, line(&step.state)));
    }
    lines
}
