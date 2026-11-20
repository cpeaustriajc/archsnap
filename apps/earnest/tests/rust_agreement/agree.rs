use std::fmt::Write as _;

use serde_json::Value;

use crate::harness::{Crate, Sandbox, source};

struct Fixture {
    folder: &'static str,
    module: &'static str,
    model: &'static str,
}

const FIXTURES: [Fixture; 5] = [
    Fixture {
        folder: "twins",
        module: "quotient",
        model: "Quotient",
    },
    Fixture {
        folder: "twins",
        module: "basket",
        model: "Basket",
    },
    Fixture {
        folder: "twins",
        module: "phases",
        model: "Phases",
    },
    Fixture {
        folder: "twins",
        module: "builder",
        model: "Builder",
    },
    Fixture {
        folder: "agreement",
        module: "tried",
        model: "Tried",
    },
];

struct Walked {
    states: u64,
    broken: Vec<(String, usize)>,
    stuck: bool,
}

fn checker(fixture: &Fixture, text: &str) -> Walked {
    let sandbox = Sandbox::new();
    let file = format!("{}.rs", fixture.module);
    sandbox.write(&file, text);
    let models = sandbox.verified(&file);
    assert_eq!(models.len(), 1, "{} holds one model", fixture.module);
    let model = &models[0];
    assert_eq!(model["name"], fixture.model, "the model of {file}");
    let mut broken: Vec<(String, usize)> = model["rules"]
        .as_array()
        .expect("a model has rules")
        .iter()
        .filter(|rule| rule["verdict"] == "broken")
        .map(|rule| {
            let steps = rule["path"]["steps"]
                .as_array()
                .expect("a broken rule has a path");
            (
                rule["id"].as_str().expect("a rule has an id").to_owned(),
                steps.len(),
            )
        })
        .collect();
    broken.sort();
    let kinds: Vec<&Value> = model["findings"]
        .as_array()
        .expect("a model has findings")
        .iter()
        .map(|finding| &finding["kind"])
        .collect();
    assert!(
        kinds.iter().all(|kind| *kind == "stuck"),
        "{} has no fault and no state out of range: {kinds:?}",
        fixture.module
    );
    Walked {
        states: model["states"].as_u64().expect("a model counts its states"),
        broken,
        stuck: !kinds.is_empty(),
    }
}

fn with_walk(fixture: &Fixture, text: &str, wanted: &Walked) -> String {
    let mut listed = String::new();
    for (id, depth) in &wanted.broken {
        let _ = write!(listed, "(\"{id}\".to_owned(), {depth}), ");
    }
    format!(
        "{text}\n#[cfg(test)]\nmod walked {{\n    use super::*;\n    use earnest::model::{{Behavior, Model}};\n    \
         use std::collections::{{HashSet, VecDeque}};\n\n    #[test]\n    fn rustc_reaches_what_the_checker_counts() {{\n        \
         let mut seen: HashSet<{model}> = HashSet::new();\n        let mut queue = VecDeque::new();\n        \
         for start in {model}::starts() {{\n            if seen.insert(start.clone()) {{\n                \
         queue.push_back((start, 0_usize));\n            }}\n        }}\n        \
         let mut broken: Vec<(String, usize)> = Vec::new();\n        let mut stuck = false;\n        \
         while let Some((state, depth)) = queue.pop_front() {{\n            \
         for rule in state.rules() {{\n                \
         if !rule.holds && !broken.iter().any(|(id, _)| id == rule.id) {{\n                    \
         broken.push((rule.id.to_owned(), depth));\n                }}\n            }}\n            \
         let steps = state.steps();\n            \
         if steps.is_empty() && !state.ends().iter().any(|end| end.holds) {{\n                \
         stuck = true;\n            }}\n            \
         for step in steps {{\n                \
         if seen.insert(step.state.clone()) {{\n                    \
         queue.push_back((step.state, depth + 1));\n                }}\n            }}\n        }}\n        \
         broken.sort();\n        \
         assert_eq!(seen.len(), {states}, \"the distinct states rustc reaches\");\n        \
         assert_eq!(broken, vec![{listed}], \"the rules that break, with the shortest path\");\n        \
         assert_eq!(stuck, {stuck}, \"whether a state is stuck\");\n    }}\n}}\n",
        model = fixture.model,
        states = wanted.states,
        stuck = wanted.stuck,
    )
}

#[test]
fn rustc_and_the_checker_agree_on_every_construct_added_to_the_reader() {
    let mut modules = Vec::new();
    let mut states = Vec::new();
    for fixture in &FIXTURES {
        let text = source(&format!("{}/{}.rs", fixture.folder, fixture.module));
        let wanted = checker(fixture, &text);
        assert!(
            wanted.states > 1,
            "{} has states to compare",
            fixture.module
        );
        states.push((fixture.module, wanted.states, wanted.broken.clone()));
        modules.push((
            fixture.module.to_owned(),
            with_walk(fixture, &text, &wanted),
        ));
    }
    let walking = Crate::make("agreement", &modules);
    let output = walking
        .tested()
        .unwrap_or_else(|output| panic!("what rustc computes differs from the checker\n{output}"));
    for fixture in &FIXTURES {
        let name = format!(
            "{}::walked::rustc_reaches_what_the_checker_counts",
            fixture.module
        );
        assert!(
            output.contains(&name),
            "{} was walked\n{output}",
            fixture.module
        );
    }
    for (module, count, broken) in states {
        assert!(
            !broken.is_empty(),
            "{module} breaks a rule, so the paths are compared ({count} states)"
        );
    }
}

#[test]
fn a_helper_that_calls_a_helper_is_labelled_in_the_chart_as_rust_labels_it() {
    let text = source("twins/builder.rs");
    let sandbox = Sandbox::new();
    sandbox.write("builder.rs", &text);
    let run = sandbox.earnest(&["chart", "builder.rs", "--model", "Builder"]);
    let chart = run.text();
    for label in [
        "plain, weight 0",
        "flagged, weight 1",
        "flagged, weight 2",
        "plain, weight 1",
    ] {
        assert!(chart.contains(label), "{label} is drawn\n{chart}");
    }
    let labels = format!(
        "{text}\n#[cfg(test)]\nmod labels {{\n    use super::*;\n    use earnest::model::{{Behavior, Model, View}};\n    \
         use std::collections::{{BTreeSet, HashSet, VecDeque}};\n\n    #[test]\n    fn the_labels_rustc_gives() {{\n        \
         let mut seen: HashSet<Builder> = HashSet::new();\n        let mut queue = VecDeque::new();\n        \
         for start in Builder::starts() {{\n            seen.insert(start.clone());\n            queue.push_back(start);\n        }}\n        \
         let mut shown = BTreeSet::new();\n        \
         while let Some(state) = queue.pop_front() {{\n            shown.extend(state.state_label());\n            \
         for step in state.steps() {{\n                if seen.insert(step.state.clone()) {{\n                    \
         queue.push_back(step.state);\n                }}\n            }}\n        }}\n        \
         let wanted: Vec<String> = [\"plain, weight 0\", \"flagged, weight 1\", \"flagged, weight 2\", \"plain, weight 1\"]\n            \
         .iter()\n            .map(|label| (*label).to_owned())\n            .collect();\n        \
         let mut got: Vec<String> = shown.into_iter().collect();\n        got.sort();\n        \
         let mut expected = wanted;\n        expected.sort();\n        assert_eq!(got, expected);\n    }}\n}}\n"
    );
    let walking = Crate::make("labels", &[("builder".to_owned(), labels)]);
    walking
        .tested()
        .unwrap_or_else(|output| panic!("the labels rustc gives differ from the chart\n{output}"));
}
