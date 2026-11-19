use std::sync::{Arc, Barrier};
use std::thread;

use crate::expected::{self, MODELS};
use crate::support::{Run, Sandbox, find_path_under, model, models, without_timings};

const CACHED: &str = "(cached)";

fn is_cached(line: &[String]) -> bool {
    line.last().is_some_and(|field| field == CACHED)
}

fn states(line: &[String]) -> &str {
    line.get(2).map_or("", String::as_str)
}

#[test]
fn r11_an_unchanged_model_comes_from_the_cache_and_a_changed_one_is_checked_again() {
    let sandbox = Sandbox::with(&["counter.model.ts", "heater.model.ts"]);

    let first = sandbox.earnest(&["verify"]);
    first.exits(0);
    assert!(
        !first.stdout.contains(CACHED),
        "nothing is cached on the first run\n{first}"
    );

    let second = sandbox.earnest(&["verify"]);
    second.exits(0);
    for name in ["Counter", "Heater"] {
        let line = second.model_line(name);
        assert!(
            is_cached(&line),
            "{name} is unchanged and comes from the cache\n{second}"
        );
        assert_eq!(
            states(&line),
            states(&first.model_line(name)),
            "a cached line of {name} has the same state count\n{second}"
        );
    }

    let smaller = sandbox
        .read("models/counter.model.ts")
        .replace("s.count < 3", "s.count < 2")
        .replace("s.count === 3", "s.count === 2");
    sandbox.write("models/counter.model.ts", &smaller);
    let third = sandbox.earnest(&["verify"]);
    third.exits(0);
    let counter = third.model_line("Counter");
    assert!(
        !is_cached(&counter),
        "the changed model is checked again\n{third}"
    );
    assert_eq!(
        states(&counter),
        "3 states",
        "the changed model has its new count\n{third}"
    );
    assert!(
        is_cached(&third.model_line("Heater")),
        "the model that did not change still comes from the cache\n{third}"
    );
}

#[test]
fn r11_no_cache_checks_every_model_again() {
    let sandbox = Sandbox::with(&["counter.model.ts", "heater.model.ts"]);
    sandbox.earnest(&["verify"]).exits(0);

    let again = sandbox.earnest(&["verify", "--no-cache"]);
    again.exits(0);
    assert!(
        !again.stdout.contains(CACHED),
        "`--no-cache` never prints `(cached)`\n{again}"
    );
    assert_eq!(
        again.model_lines().len(),
        2,
        "both models are checked\n{again}"
    );

    let as_json = sandbox.earnest(&["verify", "--no-cache", "--json"]);
    as_json.exits(0);
    let report = as_json.json();
    for found in models(&report) {
        assert_eq!(
            found["cached"], false,
            "`--no-cache` checks {} again",
            found["name"]
        );
    }
}

#[test]
fn r11_a_cached_result_is_the_result_of_the_first_run() {
    let sandbox = Sandbox::with(&["counter.model.ts", "oven.model.ts", "download.model.ts"]);
    let first = sandbox.earnest(&["verify", "--json"]);
    first.exits(1);
    let second = sandbox.earnest(&["verify", "--json"]);
    second.exits(1);
    let fresh = first.json();
    let cached = second.json();
    for name in ["Counter", "Oven", "Download"] {
        assert_eq!(
            model(&fresh, name)["cached"],
            false,
            "{name} on the first run"
        );
        assert_eq!(
            model(&cached, name)["cached"],
            true,
            "{name} on the second run"
        );
    }
    assert_eq!(
        without_timings(&cached),
        without_timings(&fresh),
        "a cached run gives the same JSON apart from `milliseconds` and `cached`"
    );

    let text = sandbox.earnest(&["verify"]);
    text.exits(1);
    let detail = text.detail("Oven");
    for rule in ["safe", "closed"] {
        assert!(
            find_path_under(&detail, &["broken", rule]).is_some(),
            "a cached model still prints the path to `{rule}`\n{text}"
        );
    }
}

fn verify_at_the_same_moment(sandbox: Sandbox, gate: &Barrier) -> (Sandbox, Run) {
    gate.wait();
    let run = sandbox.earnest(&["verify", "--json"]);
    (sandbox, run)
}

#[test]
fn r13_two_worktrees_run_at_the_same_moment_and_both_get_full_results() {
    let gate = Arc::new(Barrier::new(2));
    let runners: Vec<_> = (0..2)
        .map(|_| {
            let sandbox = Sandbox::with(&expected::files());
            let gate = Arc::clone(&gate);
            thread::spawn(move || verify_at_the_same_moment(sandbox, &gate))
        })
        .collect();
    let finished: Vec<(Sandbox, Run)> = runners
        .into_iter()
        .map(|runner| runner.join().expect("a run does not panic"))
        .collect();
    let reports: Vec<_> = finished
        .iter()
        .map(|(_, run)| {
            run.exits(1);
            run.json()
        })
        .collect();
    for report in &reports {
        assert_eq!(
            models(report).len(),
            MODELS.len(),
            "every model has a result"
        );
        for expected in MODELS {
            let found = model(report, expected.model);
            assert_eq!(
                found["status"], expected.status,
                "status of {}",
                expected.model
            );
            assert_eq!(
                found["states"], expected.states,
                "states of {}",
                expected.model
            );
            assert_eq!(
                found["cached"], false,
                "{} is not served by the other run",
                expected.model
            );
        }
    }
    let both: Vec<_> = reports.iter().map(without_timings).collect();
    assert_eq!(
        both.first(),
        both.last(),
        "both worktrees get the same results"
    );
}

#[test]
fn r13_a_run_writes_only_to_the_dot_earnest_folder_it_is_run_from() {
    let sandbox = Sandbox::with(&["counter.model.ts", "turnstile.model.ts", "turnstile.rs"]);
    let before = sandbox.read("models/turnstile.model.ts");
    assert_eq!(sandbox.entries(), ["models"], "the folder before the run");
    sandbox.earnest(&["verify"]).exits(1);
    assert_eq!(
        sandbox.entries(),
        [".earnest", "models"],
        "a run adds `.earnest` and nothing else"
    );
    let mut inside: Vec<String> = std::fs::read_dir(sandbox.path().join("models"))
        .expect("the models folder can be listed")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    inside.sort();
    assert_eq!(
        inside,
        ["counter.model.ts", "turnstile.model.ts", "turnstile.rs"],
        "a run adds nothing beside the models"
    );
    assert_eq!(
        sandbox.read("models/turnstile.model.ts"),
        before,
        "a run does not change a model file"
    );
}
