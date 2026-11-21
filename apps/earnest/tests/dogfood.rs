//! Shortfalls that `earnest` found in itself with the models of `tasks/models`, and what a
//! first-time user hit in its output.

use assert_cmd::Command;
use std::fs;
use std::path::Path;
use std::time::Duration;

const HANG_GUARD: Duration = Duration::from_mins(5);

const TWO: &str = "\
import { m } from \"@earnest/model\";

export const Oven = m.model(\"Oven\", {
\theat: m.int(0, 5).initial(0),
});

export const warm = m.action(Oven, \"the oven warms up\", {
\twhen: (s) => s.heat < 5,
\tchanges: (s) => ({ heat: s.heat + 1 }),
});

export const withinRange = m.rule(Oven, \"The heat stays within range\", (s) => s.heat <= 5);

export const hot = m.end(Oven, \"The oven is hot\", (s) => s.heat === 5);

export const Stove = m.model(\"Stove\", {
\tring: m.int(0, 3).initial(0),
});

export const glow = m.action(Stove, \"the ring glows\", {
\twhen: (s) => s.ring < 3,
\tchanges: (s) => ({ ring: s.ring + 1 }),
});

export const ringWithinRange = m.rule(Stove, \"The ring stays within range\", (s) => s.ring <= 3);

export const lit = m.end(Stove, \"The ring is lit\", (s) => s.ring === 3);
";

const LAMP: &str = "\
use earnest::model::{Model, model};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Lamp {
\t#[initial(false)]
\tlit: bool,
}

#[model]
impl Lamp {
\t#[action(\"someone flips the switch\")]
\tfn flip(&self) -> Option<Self> {
\t\tloop {}
\t}

\t#[rule(\"The lamp is never lit\")]
\tfn dark(&self) -> bool {
\t\t!self.lit
\t}
}
";

const CLOCK: &str = "\
import { m } from \"@earnest/model\";

export const Clock = m.model(\"Clock\", { time: m.int(0, 3).initial(0) });

export const early = m.rule(Clock, \"The time is early\", (s) => s.time <= Date.now());
";

const PANEL: &str = "\
import { m } from \"@earnest/model\";

export const Panel = m.model(\"Panel\", {
\tlit: m.boolean().initial(false),
\tpresses: m.int(0, 3).initial(0),
\tmode: m.oneOf([\"Idle\", \"Busy\"]).initial(\"Idle\"),
});

export const press = m.action(Panel, \"someone presses the button\", {
\twhen: (s) => s.presses < 3,
\tchanges: (s) => ({ lit: true, presses: s.presses + 1 }),
});

export const dark = m.rule(Panel, \"The panel is never lit\", (s) => !s.lit);

export const done = m.end(Panel, \"The button was pressed three times\", (s) => s.presses === 3);
";

const CLIMB: &str = "\
import { m } from \"@earnest/model\";

export const Climb = m.model(\"Climb\", { n: m.int(0, 30).initial(0) });

export const up = m.action(Climb, \"climbs\", {
\twhen: (s) => s.n < 30,
\tchanges: (s) => ({ n: s.n + 1 }),
});

export const low = m.rule(Climb, \"stays low\", (s) => s.n < 3);

export const top = m.end(Climb, \"top\", (s) => s.n === 30);
";

struct Run {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn folder_with(files: &[(&str, &str)]) -> tempfile::TempDir {
    let folder = tempfile::tempdir().expect("a temporary folder can be made");
    for (relative, text) in files {
        let target = folder.path().join(relative);
        fs::create_dir_all(target.parent().expect("a file has a folder"))
            .expect("a folder can be made");
        fs::write(target, text).expect("a file can be written");
    }
    folder
}

fn earnest(folder: &Path, arguments: &[&str]) -> Run {
    let output = Command::new(env!("CARGO_BIN_EXE_earnest"))
        .current_dir(folder)
        .timeout(HANG_GUARD)
        .args(arguments)
        .output()
        .expect("the binary runs");
    Run {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn is_cached(run: &Run, name: &str) -> bool {
    run.stdout
        .lines()
        .find(|line| line.split_whitespace().nth(1) == Some(name))
        .unwrap_or_else(|| panic!("{name} is on a line of\n{}", run.stdout))
        .ends_with("(cached)")
}

#[test]
fn a_run_with_no_cache_for_one_model_leaves_the_stored_results_of_the_others() {
    let folder = folder_with(&[("two.model.ts", TWO)]);
    earnest(folder.path(), &["verify", "two.model.ts"]);
    let second = earnest(folder.path(), &["verify", "two.model.ts"]);
    assert!(
        is_cached(&second, "Oven") && is_cached(&second, "Stove"),
        "{}",
        second.stdout
    );

    let forced = earnest(
        folder.path(),
        &["verify", "--no-cache", "--model", "Stove", "two.model.ts"],
    );
    assert_eq!(forced.code, Some(0), "{}{}", forced.stdout, forced.stderr);
    assert!(!is_cached(&forced, "Stove"), "{}", forced.stdout);

    let after = earnest(folder.path(), &["verify", "two.model.ts"]);
    assert!(
        is_cached(&after, "Oven"),
        "Oven was not checked by the run with --no-cache and is still stored\n{}",
        after.stdout
    );
    assert!(
        is_cached(&after, "Stove"),
        "Stove was checked again and its new result is stored\n{}",
        after.stdout
    );
}

#[test]
fn a_run_with_no_cache_still_checks_every_model_it_names_again() {
    let folder = folder_with(&[("two.model.ts", TWO)]);
    earnest(folder.path(), &["verify", "two.model.ts"]);
    let forced = earnest(folder.path(), &["verify", "--no-cache", "two.model.ts"]);
    assert!(!forced.stdout.contains("(cached)"), "{}", forced.stdout);
}

#[test]
fn a_result_that_the_state_limit_stopped_is_stored_although_a_time_limit_is_set() {
    let folder = folder_with(&[("two.model.ts", TWO)]);
    let arguments = [
        "verify",
        "--model",
        "Oven",
        "--max-states",
        "2",
        "--time-limit",
        "100",
        "two.model.ts",
    ];
    let first = earnest(folder.path(), &arguments);
    assert!(
        first.stdout.contains("state limit of 2"),
        "{}",
        first.stdout
    );
    let second = earnest(folder.path(), &arguments);
    assert!(is_cached(&second, "Oven"), "{}", second.stdout);
}

#[test]
fn a_result_that_the_time_limit_stopped_is_never_stored() {
    let folder = folder_with(&[("two.model.ts", TWO)]);
    let arguments = [
        "verify",
        "--model",
        "Oven",
        "--time-limit",
        "0",
        "two.model.ts",
    ];
    let first = earnest(folder.path(), &arguments);
    assert!(
        first.stdout.contains("time limit of 0 s"),
        "{}",
        first.stdout
    );
    let second = earnest(folder.path(), &arguments);
    assert!(!is_cached(&second, "Oven"), "{}", second.stdout);
}

fn assert_unreadable_file_is_named(arguments: &[&str], file: &str, name: &str) {
    let folder = folder_with(&[("lamp.rs", LAMP), ("clock.model.ts", CLOCK)]);
    let run = earnest(folder.path(), arguments);
    assert_eq!(
        run.code,
        Some(1),
        "an unreadable file is exit 1, not 2: {arguments:?}\n{}{}",
        run.stdout,
        run.stderr
    );
    let said = format!("{}{}", run.stdout, run.stderr);
    assert!(
        run.stderr
            .contains(&format!("`{name}` is declared in `{file}`"))
            && run.stderr.contains("unreadable"),
        "the error says the model is in the unreadable file: {arguments:?}\n{said}"
    );
    assert!(
        !run.stderr.contains("no model is named"),
        "it does not say that no such model exists: {arguments:?}\n{said}"
    );
    assert!(
        said.contains(file),
        "the file's problems are listed\n{said}"
    );
}

#[test]
fn a_model_named_in_an_unreadable_file_is_said_to_be_there_by_every_command() {
    for command in ["verify", "chart", "report"] {
        assert_unreadable_file_is_named(&[command, "--model", "Lamp"], "lamp.rs", "Lamp");
        assert_unreadable_file_is_named(&[command, "--model", "Clock"], "clock.model.ts", "Clock");
    }
    assert_unreadable_file_is_named(
        &["tla", "--out", "out", "--model", "Lamp"],
        "lamp.rs",
        "Lamp",
    );
    assert_unreadable_file_is_named(
        &["confirm", "--tlc", "lamp.rs", "--model", "Lamp"],
        "lamp.rs",
        "Lamp",
    );
}

#[test]
fn a_rule_in_an_unreadable_file_is_said_to_be_there() {
    let folder = folder_with(&[("lamp.rs", LAMP), ("clock.model.ts", CLOCK)]);
    for (rule, file) in [("dark", "lamp.rs"), ("early", "clock.model.ts")] {
        let run = earnest(folder.path(), &["verify", "--rule", rule]);
        assert_eq!(run.code, Some(1), "{}{}", run.stdout, run.stderr);
        assert!(
            run.stderr.contains(&format!("is declared in `{file}`")),
            "{}",
            run.stderr
        );
    }
}

#[test]
fn a_name_no_unreadable_file_declares_says_which_files_could_not_be_read() {
    let folder = folder_with(&[("lamp.rs", LAMP), ("clock.model.ts", CLOCK)]);
    let run = earnest(folder.path(), &["verify", "--model", "Nothing"]);
    assert_eq!(run.code, Some(2), "{}{}", run.stdout, run.stderr);
    assert!(
        run.stderr.contains("no model is named `Nothing`")
            && run.stderr.contains("`lamp.rs`")
            && run.stderr.contains("`clock.model.ts`")
            && run.stderr.contains("may declare it"),
        "{}",
        run.stderr
    );
    let unparsed = folder_with(&[("broken.rs", "use earnest::model::Model;\nfn {{{ not rust\n")]);
    let run = earnest(unparsed.path(), &["verify", "--model", "Lamp"]);
    assert_eq!(run.code, Some(2), "{}{}", run.stdout, run.stderr);
    assert!(
        run.stderr
            .contains("the unreadable file `broken.rs` may declare it"),
        "{}",
        run.stderr
    );
}

#[test]
fn a_path_ends_with_one_line_holding_the_whole_state_it_ends_in() {
    let folder = folder_with(&[("panel.model.ts", PANEL)]);
    let run = earnest(folder.path(), &["verify", "--no-cache", "panel.model.ts"]);
    assert_eq!(run.code, Some(1), "{}{}", run.stdout, run.stderr);
    let lines: Vec<&str> = run.stdout.lines().map(str::trim).collect();
    let start = lines
        .iter()
        .position(|line| line.starts_with("start"))
        .expect("a path starts");
    assert_eq!(
        lines[start..start + 3],
        [
            "start  lit = false, presses = 0, mode = \"Idle\"",
            "1. press  lit: false -> true, presses: 0 -> 1",
            "end    lit = true, presses = 1, mode = \"Idle\"",
        ],
        "the field the action did not change is in the last line too\n{}",
        run.stdout
    );
    let report = earnest(folder.path(), &["report", "panel.model.ts"]);
    assert!(
        report
            .stdout
            .contains("  end    lit = true, presses = 1, mode = \"Idle\""),
        "{}",
        report.stdout
    );
}

#[test]
fn a_path_of_no_step_has_no_end_line() {
    let at_start = PANEL.replace(
        "\"The panel is never lit\", (s) => !s.lit",
        "\"The panel is never lit\", (s) => s.lit",
    );
    let folder = folder_with(&[("panel.model.ts", &at_start)]);
    let run = earnest(folder.path(), &["verify", "--no-cache", "panel.model.ts"]);
    assert!(run.stdout.contains("start  lit = false"), "{}", run.stdout);
    assert!(!run.stdout.contains("end    "), "{}", run.stdout);
}

fn arrows_drawn(text: &str) -> usize {
    text.lines()
        .filter(|line| line.contains(" --> s") && !line.contains("[*]"))
        .count()
}

#[test]
fn max_arrows_raises_or_lowers_the_limit_of_a_chart_and_of_a_report() {
    let folder = folder_with(&[("climb.model.ts", CLIMB)]);
    let default = earnest(folder.path(), &["chart", "climb.model.ts"]);
    assert_eq!(arrows_drawn(&default.stdout), 20, "{}", default.stdout);
    for command in ["chart", "report"] {
        let more = earnest(
            folder.path(),
            &[command, "--max-arrows", "30", "climb.model.ts"],
        );
        assert_eq!(arrows_drawn(&more.stdout), 30, "{command}\n{}", more.stdout);
        assert!(!more.stdout.contains("Not shown"), "{}", more.stdout);
        let fewer = earnest(
            folder.path(),
            &[command, "--max-arrows", "7", "climb.model.ts"],
        );
        assert_eq!(
            arrows_drawn(&fewer.stdout),
            7,
            "{command}\n{}",
            fewer.stdout
        );
    }
}

#[test]
fn max_arrows_must_be_at_least_one() {
    let folder = folder_with(&[("climb.model.ts", CLIMB)]);
    for command in ["chart", "report"] {
        let run = earnest(
            folder.path(),
            &[command, "--max-arrows", "0", "climb.model.ts"],
        );
        assert_eq!(run.code, Some(2), "{command}: {}{}", run.stdout, run.stderr);
    }
}

#[test]
fn the_note_does_not_call_a_drawn_broken_state_beyond_what_is_shown() {
    let folder = folder_with(&[("climb.model.ts", CLIMB)]);
    let drawn = earnest(folder.path(), &["chart", "climb.model.ts"]);
    assert!(
        drawn.stdout.contains("class s3,s4,"),
        "the broken states are drawn\n{}",
        drawn.stdout
    );
    assert!(
        drawn
            .stdout
            .contains("10 more broken states lie beyond what is shown."),
        "{}",
        drawn.stdout
    );
    let not_drawn = earnest(
        folder.path(),
        &["chart", "--max-arrows", "2", "climb.model.ts"],
    );
    assert!(
        not_drawn
            .stdout
            .contains("28 broken states lie beyond what is shown."),
        "no broken state is drawn, so none is another\n{}",
        not_drawn.stdout
    );
}
