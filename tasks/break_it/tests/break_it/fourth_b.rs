use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use tempfile::TempDir;

use crate::support::{Sandbox, is_broken_after, rule};

const RUST_FILE: &str = "case.rs";
const TYPESCRIPT_FILE: &str = "case.model.ts";
const TIME_LIMIT_SECONDS: &str = "1";
const DEADLINE: Duration = Duration::from_secs(30);
const POLL: Duration = Duration::from_millis(50);

fn binary() -> PathBuf {
    Sandbox::with(&[]).verify(&[]);
    let test = env::current_exe().expect("a test knows where its own binary is");
    let folder = test
        .parent()
        .and_then(Path::parent)
        .expect("a test binary sits in `deps` under the folder of its build profile");
    folder.join(format!("earnest{}", env::consts::EXE_SUFFIX))
}

fn run_before_the_deadline(files: &[(&str, &str)], arguments: &[&str]) -> Option<String> {
    run_for(files, arguments, DEADLINE)
}

fn run_for(files: &[(&str, &str)], arguments: &[&str], deadline: Duration) -> Option<String> {
    let binary = binary();
    let folder = TempDir::new().expect("a temporary folder can be made");
    for (name, text) in files {
        std::fs::write(folder.path().join(name), text).expect("a file can be written");
    }
    let mut child = Command::new(binary)
        .args(arguments)
        .current_dir(folder.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("earnest starts");
    let started = Instant::now();
    while started.elapsed() < deadline {
        if child.try_wait().expect("the child can be polled").is_some() {
            let output = child.wait_with_output().expect("the output can be read");
            return Some(String::from_utf8_lossy(&output.stdout).into_owned());
        }
        thread::sleep(POLL);
    }
    child.kill().expect("the child can be stopped");
    child.wait().expect("the child can be reaped");
    None
}

fn typescript_with_rule(condition: &str) -> String {
    format!(
        r#"import {{ m }} from "@earnest/model";

export const Bad = m.model("Bad", {{ n: m.int(0, 1).initial(0) }});

export const top = m.end(Bad, "Always", () => true);

export const clock = m.rule(Bad, "The clock is read", () => {condition});
"#
    )
}

fn has_a_fault(explored: &serde_json::Value) -> bool {
    explored["findings"]
        .as_array()
        .is_some_and(|findings| findings.iter().any(|finding| finding["kind"] == "fault"))
}

#[test]
fn explore_in_node_gives_unknown_for_a_number_read_from_null_by_order_sum_or_remainder() {
    let source = include_str!("../../models/fourth_b/null_compare.model.ts");
    let sandbox = Sandbox::with(&[(TYPESCRIPT_FILE, source)]);
    let checked = sandbox.verify(&[]);
    let explored = sandbox.explore(TYPESCRIPT_FILE, "Opt");
    for id in ["below", "sum", "remainder"] {
        assert_eq!(
            rule(checked.model("Opt"), id)["verdict"],
            "unknown",
            "`o` is null in the start state, and a number compared or added with an optional \
             number is a fault there, so `{id}` is never seen broken and can't be judged\n{checked}"
        );
        assert_eq!(
            rule(&explored.json, id)["verdict"],
            "unknown",
            "`explore` runs `s.o! < 3` as JavaScript, where `null < 3` is true and `null + 1` is \
             1, so it reports `{id}` as holding in a state where the contract makes it a fault\n\
             {explored}"
        );
    }
    assert!(
        has_a_fault(&explored.json),
        "the start state faults, so `explore` reports a fault finding\n{explored}"
    );
}

#[test]
fn explore_in_node_does_not_take_a_step_whose_then_reads_null() {
    let source = include_str!("../../models/fourth_b/null_then.model.ts");
    let sandbox = Sandbox::with(&[(TYPESCRIPT_FILE, source)]);
    let checked = sandbox.verify(&[]);
    let explored = sandbox.explore(TYPESCRIPT_FILE, "Bump");
    assert_eq!(
        checked.model("Bump")["states"],
        1,
        "`o` is null in the only start state, so the `then` of `bump` reads a value that is \
         null: a fault, and no state follows\n{checked}"
    );
    assert_eq!(
        explored.json["states"], 1,
        "`explore` runs `s.o! + 1` as JavaScript, gets 1 from `null + 1` and goes on to a \
         second state that the contract does not allow\n{explored}"
    );
    assert!(
        has_a_fault(&explored.json),
        "the `then` of `bump` faults in the start state\n{explored}"
    );
}

#[test]
fn a_rust_model_that_names_the_crate_as_a_raw_identifier_is_not_left_out() {
    let source = include_str!("../../models/fourth_b/raw_crate.rs");
    let report = Sandbox::with(&[(RUST_FILE, source)]).verify(&[]);
    let low = rule(report.model("Climb"), "low");
    assert!(
        report.is_unreadable() || is_broken_after(low, 2),
        "`use r#earnest::model::{{..}}` compiles and `r#earnest` is `earnest`, so the file uses \
         the library and `low` is broken after two steps; it is checked or unreadable, never \
         left out\n{report}"
    );
}

#[test]
fn a_raw_identifier_gives_the_plain_name_as_the_id_of_a_rule_an_action_and_a_field() {
    let source = include_str!("../../models/fourth_b/raw_names.rs");
    let report = Sandbox::with(&[(RUST_FILE, source)]).verify(&[]);
    let model = report.model("Raw");
    let broken = rule(model, "fn");
    assert!(
        is_broken_after(broken, 2),
        "`fn r#fn` is the method `fn`: the library's `Claim::id` is `fn` and the contract says \
         ids are the method names, so the rule is `fn`, broken after two steps\n{report}"
    );
    assert_eq!(
        broken["path"]["steps"][0]["action"], "loop",
        "`fn r#loop` is the action `loop`\n{report}"
    );
    assert_eq!(
        broken["path"]["start"][0]["name"], "type",
        "the field `r#type` is the field `type`, as `Debug` prints it\n{report}"
    );
}

#[test]
fn the_text_output_prints_a_model_name_with_a_line_break_on_its_own_line() {
    let source = include_str!("../../models/fourth_b/newline_name.model.ts");
    let printed = run_before_the_deadline(
        &[(TYPESCRIPT_FILE, source)],
        &["verify", "--no-cache", TYPESCRIPT_FILE],
    )
    .expect("a model of three states is checked at once");
    let model_line = printed
        .lines()
        .find(|line| line.contains("First line"))
        .unwrap_or_default();
    assert!(
        model_line.contains("Second line") && model_line.contains("3 states"),
        "the text output has one line per model, with its status, name, states and time; a line \
         break in the name is one space, as in a rule's sentence\n{printed}"
    );
}

#[test]
fn the_text_output_prints_an_unreadable_file_with_a_line_break_in_its_name_on_one_line() {
    let source = typescript_with_rule("Date.now() > 0");
    let name = "first\nsecond.model.ts";
    let printed = run_before_the_deadline(&[(name, &source)], &["verify", "--no-cache", "."])
        .expect("a small file is read at once");
    let problem_line = printed
        .lines()
        .find(|line| line.contains("`Date.now`"))
        .unwrap_or_default();
    assert!(
        problem_line.contains("first") && problem_line.contains("second.model.ts:"),
        "an unreadable file's problem is one line of `file:line:column`, the feature and a \
         sentence; the file's name is on it whole\n{printed}"
    );
}

#[test]
fn a_time_limit_stops_a_model_whose_rule_takes_a_billion_steps_in_one_state() {
    let source = include_str!("../../models/fourth_b/slow_rule.model.ts");
    let printed = run_before_the_deadline(
        &[(TYPESCRIPT_FILE, source)],
        &[
            "verify",
            "--json",
            "--no-cache",
            "--time-limit",
            TIME_LIMIT_SECONDS,
            TYPESCRIPT_FILE,
        ],
    );
    assert!(
        printed.is_some(),
        "`--time-limit {TIME_LIMIT_SECONDS}` stops a model after that many seconds, but the rule \
         `sums` takes 10^9 steps in each state (about three minutes) and the run was still \
         going after {} seconds",
        DEADLINE.as_secs()
    );
}
