use crate::support::{Run, path_under};

const README_EXAMPLE: &str = "\
pass        Counter  4 states  1 ms
fail        Turnstile  3 states  1 ms
  broken  staysLocked  \"The gate stays locked until the visitor pays\"
    start  gate = \"locked\", paid = false
    1. push  gate: \"locked\" -> \"open\"
  stuck  no action can happen here, and the model does not mark it as an end
    start  gate = \"locked\", paid = false
    1. pay  paid: false -> true
unfinished  Ledger  1000 states so far  3 ms  (state limit of 1000)
unreadable  src/clock.model.ts
  src/clock.model.ts:12:17  `Date.now`  the system clock is outside the library
4 models: 1 pass, 1 fail, 1 unfinished, 1 unreadable
";

fn readme_example() -> Run {
    Run {
        code: Some(1),
        stdout: README_EXAMPLE.to_owned(),
        stderr: String::new(),
    }
}

#[test]
fn the_harness_reads_the_result_lines_of_the_readme_example() {
    let run = readme_example();
    assert_eq!(
        run.model_lines(),
        [
            vec!["pass", "Counter", "4 states", "1 ms"],
            vec!["fail", "Turnstile", "3 states", "1 ms"],
            vec![
                "unfinished",
                "Ledger",
                "1000 states so far",
                "3 ms",
                "(state limit of 1000)"
            ],
        ],
        "the fields of each result line"
    );
    assert_eq!(
        run.summary_line(),
        "4 models: 1 pass, 1 fail, 1 unfinished, 1 unreadable",
        "the summary line"
    );
    assert_eq!(run.summary_total(), 4, "the model count");
    for status in ["pass", "fail", "unfinished", "unreadable"] {
        assert_eq!(run.summary_count(status), 1, "the count of `{status}`");
    }
    assert_eq!(
        run.summary_count("skipped"),
        0,
        "a status the line leaves out"
    );
}

#[test]
fn the_harness_reads_the_paths_of_the_readme_example() {
    let run = readme_example();
    let detail = run.detail("Turnstile");
    assert_eq!(detail.len(), 6, "the detail lines of the failing model");
    assert!(
        run.detail("Counter").is_empty(),
        "a passing model has no detail"
    );

    let broken = path_under(&detail, &["broken", "staysLocked"]);
    assert_eq!(
        broken.start, "gate = \"locked\", paid = false",
        "the start line"
    );
    assert_eq!(broken.actions(), ["push"], "the actions to the broken rule");
    assert_eq!(
        broken.steps,
        ["push  gate: \"locked\" -> \"open\""],
        "the step without its number"
    );

    let stuck = path_under(&detail, &["stuck"]);
    assert_eq!(stuck.actions(), ["pay"], "the actions to the stuck state");
}
