use crate::run::Checked;
use earnest_tla::{Files, Invariants};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const INVARIANT_ERROR: &str = "Error: Invariant ";
const VIOLATED: &str = " is violated";
const ERROR: &str = "Error: ";
const TRACE_HEADERS: [&str; 2] = [
    "Error: The behavior up to this point is:",
    "Error: The following behavior constitutes a counter-example:",
];
const GENERATED: &str = " states generated";
const DISTINCT: &str = " distinct states found";
const LEFT: &str = " states left on queue.";
const BROKEN: &str = "broken";
const HOLDS: &str = "holds";
const UNKNOWN: &str = "unknown";
const OUT_OF_RANGE: &str = "out-of-range";
const FAULT: &str = "fault";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TlcRun {
    pub(crate) distinct_states: Option<u64>,
    pub(crate) violated: Vec<String>,
    pub(crate) errors: Vec<String>,
}

fn count(text: &str, suffix: &str) -> Option<u64> {
    text.trim()
        .strip_suffix(suffix)?
        .replace(',', "")
        .parse()
        .ok()
}

fn final_distinct_states(line: &str) -> Option<u64> {
    let mut parts = line.split(", ");
    let (generated, distinct, left) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() {
        return None;
    }
    count(generated, GENERATED)?;
    if count(left, LEFT)? != 0 {
        return None;
    }
    count(distinct, DISTINCT)
}

fn violated_invariant(line: &str) -> Option<&str> {
    let rest = line.strip_prefix(INVARIANT_ERROR)?;
    let (name, _) = rest.split_once(VIOLATED)?;
    Some(name)
}

pub(crate) fn read_tlc(output: &str) -> TlcRun {
    let mut run = TlcRun::default();
    for line in output.lines().map(str::trim) {
        if let Some(name) = violated_invariant(line) {
            if !run.violated.iter().any(|known| known == name) {
                run.violated.push(name.to_owned());
            }
        } else if let Some(error) = line.strip_prefix(ERROR) {
            if !TRACE_HEADERS.contains(&line) {
                run.errors.push(error.to_owned());
            }
        } else if let Some(states) = final_distinct_states(line) {
            run.distinct_states = Some(states);
        }
    }
    run
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RuleRun {
    pub(crate) id: String,
    pub(crate) invariant: String,
    pub(crate) judged: Option<String>,
    pub(crate) run: TlcRun,
}

fn tlc_verdict(rule: &RuleRun) -> &'static str {
    let violated = |invariant: &String| rule.run.violated.contains(invariant);
    let finished = rule.run.errors.is_empty() && rule.run.distinct_states.is_some();
    if violated(&rule.invariant) {
        BROKEN
    } else if finished && !rule.judged.as_ref().is_some_and(violated) {
        HOLDS
    } else {
        UNKNOWN
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tool {
    Tlc,
    TlaRs,
}

impl Tool {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Tlc => "TLC",
            Self::TlaRs => "tla-rs",
        }
    }
}

fn cut_short(checked: &Checked) -> bool {
    checked
        .result
        .findings
        .iter()
        .any(|finding| finding.kind == OUT_OF_RANGE || finding.kind == FAULT)
}

pub(crate) fn differences(
    checked: &Checked,
    counting: &TlcRun,
    rules: &[RuleRun],
    tool: Tool,
) -> Vec<String> {
    let name = tool.name();
    let result = &checked.result;
    let unseen_by_tlc = cut_short(checked);
    let mut found = Vec::new();
    if let Some(reason) = &result.reason {
        found.push(format!(
            "the checker stopped early ({reason}), so there is no full result to compare"
        ));
    }
    match counting.distinct_states {
        Some(states) if u64::try_from(result.states).is_ok_and(|ours| ours == states) => {}
        Some(states) => found.push(format!(
            "states: the checker found {}, {name} found {states}",
            result.states
        )),
        None => found.push(format!(
            "states: the checker found {}, {name} did not finish its count{}",
            result.states,
            counting
                .errors
                .first()
                .map(|error| format!(" ({error})"))
                .unwrap_or_default()
        )),
    }
    for ours in &result.rules {
        let Some(theirs) = rules.iter().find(|rule| rule.id == ours.id) else {
            found.push(format!("rule {}: {name} was not run on it", ours.id));
            continue;
        };
        let verdict = tlc_verdict(theirs);
        let cut_short_where_tlc_sees_nothing =
            unseen_by_tlc && ours.verdict == UNKNOWN && verdict == HOLDS;
        if verdict != ours.verdict && !cut_short_where_tlc_sees_nothing {
            found.push(format!(
                "rule {}: the checker says {}, {name} says {verdict}{}",
                ours.id,
                ours.verdict,
                theirs
                    .run
                    .errors
                    .first()
                    .map(|error| format!(" ({error})"))
                    .unwrap_or_default()
            ));
        }
    }
    found
}

pub(crate) fn confirm_lines(
    checked: &Checked,
    differences: &[String],
    limits: &[String],
    tool: Tool,
) -> Vec<String> {
    let result = &checked.result;
    let word = if differences.is_empty() {
        "same"
    } else if limits.is_empty() {
        "differs"
    } else {
        "cannot"
    };
    let by = match tool {
        Tool::Tlc => String::new(),
        Tool::TlaRs => format!("  ({})", tool.name()),
    };
    let mut lines = vec![format!(
        "{word:<7}  {}  {} states  {}{by}",
        result.name,
        result.states,
        result.status.word()
    )];
    if !differences.is_empty() {
        lines.extend(
            limits
                .iter()
                .map(|why| format!("  {} can't confirm this model: {why}", tool.name())),
        );
    }
    lines.extend(differences.iter().map(|each| format!("  {each}")));
    lines
}

#[derive(Debug)]
pub(crate) struct Checker<'a> {
    pub(crate) tool: Tool,
    pub(crate) program: &'a Path,
    pub(crate) folder: PathBuf,
    pub(crate) workers: usize,
}

const INVARIANT: &str = "INVARIANT ";

fn merged(runs: Vec<TlcRun>) -> TlcRun {
    let mut merged = TlcRun {
        distinct_states: runs.first().and_then(|run| run.distinct_states),
        ..TlcRun::default()
    };
    for run in runs {
        if run.distinct_states != merged.distinct_states {
            merged.distinct_states = None;
        }
        for name in run.violated {
            if !merged.violated.contains(&name) {
                merged.violated.push(name);
            }
        }
        merged.errors.extend(run.errors);
    }
    merged
}

impl Checker<'_> {
    fn command(&self, files: &Files, config_name: &str) -> Command {
        let module = format!("{}.tla", files.name);
        let workers = self.workers.max(1).to_string();
        match self.tool {
            Tool::Tlc => {
                let mut command = Command::new("java");
                command
                    .arg("-cp")
                    .arg(self.program)
                    .args(["tlc2.TLC", "-continue", "-workers"])
                    .arg(workers)
                    .args(["-metadir", "states", "-config", config_name])
                    .arg(module);
                command
            }
            Tool::TlaRs => {
                let mut command = Command::new(self.program);
                command
                    .arg(module)
                    .args(["--config", config_name, "--workers"])
                    .arg(workers);
                command
            }
        }
    }

    fn run(&self, files: &Files, config_name: &str, config: &str) -> Result<TlcRun, String> {
        let name = self.tool.name();
        let failed = |what: &str, error: std::io::Error| {
            format!("{what} in {}: {error}", self.folder.to_string_lossy())
        };
        fs::write(self.folder.join(config_name), config)
            .map_err(|error| failed(&format!("a {name} configuration can't be written"), error))?;
        let output = self
            .command(files, config_name)
            .current_dir(&self.folder)
            .stdin(Stdio::null())
            .output()
            .map_err(|error| failed(&format!("`{}` did not start", self.program_word()), error))?;
        let mut run = read_tlc(&String::from_utf8_lossy(&output.stdout));
        if run.distinct_states.is_none() && run.errors.is_empty() {
            let said = String::from_utf8_lossy(&output.stderr);
            run.errors.push(
                said.lines()
                    .find(|line| !line.trim().is_empty())
                    .map_or_else(
                        || format!("{name} printed no state count"),
                        |line| line.trim().to_owned(),
                    ),
            );
        }
        Ok(run)
    }

    fn program_word(&self) -> String {
        match self.tool {
            Tool::Tlc => "java".to_owned(),
            Tool::TlaRs => self.program.to_string_lossy().into_owned(),
        }
    }

    fn judge(&self, files: &Files, number: usize, rule: &Invariants) -> Result<TlcRun, String> {
        match self.tool {
            Tool::Tlc => self.run(
                files,
                &format!("rule{number}.cfg"),
                &files.config_judging(rule),
            ),
            Tool::TlaRs => {
                let mut runs = Vec::new();
                for (part, invariant) in rule.names().enumerate() {
                    let mut config = files.config_checking(&[]);
                    config.push_str(INVARIANT);
                    config.push_str(invariant);
                    config.push('\n');
                    runs.push(self.run(files, &format!("rule{number}-{part}.cfg"), &config)?);
                }
                Ok(merged(runs))
            }
        }
    }

    pub(crate) fn check(&self, files: &Files) -> Result<(TlcRun, Vec<RuleRun>), String> {
        fs::create_dir_all(&self.folder).map_err(|error| {
            format!(
                "the folder {} can't be made: {error}",
                self.folder.to_string_lossy()
            )
        })?;
        fs::write(
            self.folder.join(format!("{}.tla", files.name)),
            &files.module,
        )
        .map_err(|error| {
            format!(
                "the module can't be written in {}: {error}",
                self.folder.to_string_lossy()
            )
        })?;
        let counting = self.run(files, "count.cfg", &files.config_checking(&[]))?;
        let mut rules = Vec::new();
        for (number, exported) in files.rules.iter().enumerate() {
            let run = self.judge(files, number, exported)?;
            rules.push(RuleRun {
                id: exported.rule.clone(),
                invariant: exported.holds.clone(),
                judged: exported.judged.clone(),
                run,
            });
        }
        Ok((counting, rules))
    }
}

#[cfg(test)]
mod tests {
    use super::{RuleRun, TlcRun, Tool, confirm_lines, differences, merged, read_tlc};
    use crate::run::{Checked, FindingResult, ModelResult, PathTaken, RuleResult, Status};

    const PASSING: &str = "\
TLC2 Version 2.19 of 08 August 2024 (rev: 5a47802)
Running breadth-first search Model-Checking with fp 78 and seed -2152603330336572939 with 1 worker on 4 cores with 1024MB heap and 64MB offheap memory [pid: 4242] (Linux 6.1 amd64, Eclipse Adoptium 21.0.2 x86_64, MSBDiskFPSet, DiskStateQueue).
Parsing file /work/.earnest/confirm/Counter/Counter.tla
Semantic processing of module Naturals
Semantic processing of module Counter
Starting... (2026-11-19 10:00:00)
Computing initial states...
Finished computing initial states: 1 distinct state generated at 2026-11-19 10:00:00.
Model checking completed. No error has been found.
  Estimates of the probability that TLC did not check all reachable states
  because two distinct states had the same fingerprint:
  calculated (optimistic):  val = 1.1E-18
5 states generated, 4 distinct states found, 0 states left on queue.
The depth of the complete state graph search is 4.
Finished in 00s at (2026-11-19 10:00:00)
";

    const VIOLATING: &str = "\
Starting... (2026-11-19 10:00:00)
Computing initial states...
Finished computing initial states: 1 distinct state generated at 2026-11-19 10:00:00.
Error: Invariant Rule_safe is violated.
Error: The behavior up to this point is:
State 1: <Initial predicate>
/\\ door = \"Closed\"
/\\ heat = \"Off\"

State 2: <Action_start line 14, col 5 to line 16, col 14 of module Oven>
/\\ door = \"Closed\"
/\\ heat = \"On\"

State 3: <Action_open line 18, col 5 to line 20, col 14 of module Oven>
/\\ door = \"Open\"
/\\ heat = \"On\"

Error: Invariant Rule_safe is violated.
Error: The behavior up to this point is:
State 1: <Initial predicate>
/\\ door = \"Closed\"
/\\ heat = \"Off\"

Progress(3) at 2026-11-19 10:00:01: 1,234 states generated (74,040 s/min), 1,004 distinct states found (60,240 ds/min), 12 states left on queue.
9 states generated, 4 distinct states found, 0 states left on queue.
The depth of the complete state graph search is 3.
Finished in 00s at (2026-11-19 10:00:01)
";

    const BROKEN_AT_START: &str = "\
Computing initial states...
Error: Invariant Rule_floor is violated by the initial state:
/\\ level = 0

Finished computing initial states: 1 distinct state generated at 2026-11-19 10:00:00.
7 states generated, 4 distinct states found, 0 states left on queue.
";

    const FAILING: &str = "\
Computing initial states...
Finished computing initial states: 1 distinct state generated at 2026-11-19 10:00:00.
Error: Evaluating invariant Rule_capped failed.
Attempted to apply the tuple
<<>>
to the integer 1 which is outside the domain of the tuple.
Error: The behavior up to this point is:
State 1: <Initial predicate>
/\\ chosen = <<>>

2 states generated, 2 distinct states found, 1 states left on queue.
";

    #[test]
    fn a_run_without_errors_gives_its_distinct_state_count() {
        assert_eq!(
            read_tlc(PASSING),
            TlcRun {
                distinct_states: Some(4),
                violated: Vec::new(),
                errors: Vec::new(),
            }
        );
    }

    #[test]
    fn a_violated_invariant_is_named_once_and_the_final_count_is_the_one_read() {
        assert_eq!(
            read_tlc(VIOLATING),
            TlcRun {
                distinct_states: Some(4),
                violated: vec!["Rule_safe".to_owned()],
                errors: Vec::new(),
            },
            "the progress line with other numbers is not the count"
        );
    }

    #[test]
    fn an_invariant_violated_by_the_initial_state_is_named() {
        let run = read_tlc(BROKEN_AT_START);
        assert_eq!(run.violated, ["Rule_floor"], "the invariant");
        assert_eq!(run.distinct_states, Some(4), "the count");
        assert!(
            run.errors.is_empty(),
            "a violation is not an error of the run"
        );
    }

    #[test]
    fn a_run_that_stops_on_an_error_has_the_error_and_no_count() {
        let run = read_tlc(FAILING);
        assert_eq!(
            run.errors,
            ["Evaluating invariant Rule_capped failed."],
            "the error, without the trace header"
        );
        assert_eq!(
            run.distinct_states, None,
            "states were left on the queue, so the count is not the whole model's"
        );
        assert!(run.violated.is_empty(), "nothing was seen violated");
    }

    #[test]
    fn output_that_is_not_from_tlc_gives_nothing() {
        assert_eq!(
            read_tlc("Error: Unable to access jarfile x\n").errors.len(),
            1
        );
        assert_eq!(read_tlc(""), TlcRun::default());
        assert_eq!(
            read_tlc("a states generated, b distinct states found, 0 states left on queue.\n"),
            TlcRun::default(),
            "a count that is not a number is not read"
        );
    }

    fn checked(states: usize, rules: &[(&str, &str)]) -> Checked {
        Checked {
            result: ModelResult {
                name: "Oven".to_owned(),
                file: "oven.model.ts".to_owned(),
                line: 3,
                status: Status::Fail,
                states,
                milliseconds: 1,
                cached: false,
                reason: None,
                rules: rules
                    .iter()
                    .map(|(id, verdict)| RuleResult {
                        id: (*id).to_owned(),
                        says: String::new(),
                        verdict: (*verdict).to_owned(),
                        path: None,
                    })
                    .collect(),
                findings: Vec::new(),
            },
            detail: Vec::new(),
        }
    }

    fn rule_run(id: &str, output: &str) -> RuleRun {
        RuleRun {
            id: id.to_owned(),
            invariant: format!("Rule_{id}"),
            judged: None,
            run: read_tlc(output),
        }
    }

    #[test]
    fn the_same_count_and_verdicts_are_no_difference() {
        let ours = checked(4, &[("safe", "broken"), ("closed", "holds")]);
        let runs = [rule_run("safe", VIOLATING), rule_run("closed", PASSING)];
        assert_eq!(
            differences(&ours, &read_tlc(PASSING), &runs, Tool::Tlc),
            Vec::<String>::new()
        );
    }

    #[test]
    fn another_state_count_is_a_difference() {
        let ours = checked(5, &[]);
        assert_eq!(
            differences(&ours, &read_tlc(PASSING), &[], Tool::Tlc),
            ["states: the checker found 5, TLC found 4"]
        );
    }

    #[test]
    fn another_verdict_is_a_difference_either_way() {
        let ours = checked(4, &[("safe", "holds"), ("closed", "broken")]);
        let runs = [rule_run("safe", VIOLATING), rule_run("closed", PASSING)];
        assert_eq!(
            differences(&ours, &read_tlc(PASSING), &runs, Tool::Tlc),
            [
                "rule safe: the checker says holds, TLC says broken",
                "rule closed: the checker says broken, TLC says holds",
            ]
        );
    }

    #[test]
    fn a_violation_of_another_invariant_does_not_break_this_rule() {
        let ours = checked(4, &[("closed", "holds")]);
        let mut run = rule_run("closed", VIOLATING);
        run.run.violated = vec!["Rule_safe".to_owned()];
        assert!(
            differences(&ours, &read_tlc(PASSING), &[run], Tool::Tlc).is_empty(),
            "only the rule's own invariant counts"
        );
    }

    #[test]
    fn a_rule_tlc_can_not_evaluate_agrees_only_with_unknown() {
        let unknown = checked(4, &[("capped", "unknown")]);
        let runs = [rule_run("capped", FAILING)];
        assert!(
            differences(&unknown, &read_tlc(PASSING), &runs, Tool::Tlc).is_empty(),
            "the checker could not judge it everywhere either"
        );
        let holds = checked(4, &[("capped", "holds")]);
        assert_eq!(
            differences(&holds, &read_tlc(PASSING), &runs, Tool::Tlc),
            ["rule capped: the checker says holds, TLC says unknown \
                 (Evaluating invariant Rule_capped failed.)"]
        );
    }

    #[test]
    fn a_rule_the_checker_could_not_finish_judging_because_a_state_was_cut_short_agrees_with_tlc() {
        let finding = |kind: &str, message: &str| FindingResult {
            kind: kind.to_owned(),
            message: message.to_owned(),
            path: PathTaken {
                start: Vec::new(),
                steps: Vec::new(),
            },
        };
        let runs = [rule_run("bounded", PASSING)];
        for (kind, message) in [
            (
                "out-of-range",
                "the field `count` leaves the values it declares",
            ),
            (
                "fault",
                "the `then` of the action `price` reads a value that is null in this state",
            ),
        ] {
            let mut ours = checked(4, &[("bounded", "unknown")]);
            ours.result.findings.push(finding(kind, message));
            assert!(
                differences(&ours, &read_tlc(PASSING), &runs, Tool::Tlc).is_empty(),
                "{kind}: TLC has no way to see the state that was cut short"
            );
        }
        let mut both = checked(4, &[("bounded", "unknown"), ("other", "unknown")]);
        both.result.findings.push(finding(
            "fault",
            "the rule `bounded` reads a value that is null in this state",
        ));
        let both_runs = [rule_run("bounded", PASSING), rule_run("other", PASSING)];
        assert!(
            differences(&both, &read_tlc(PASSING), &both_runs, Tool::Tlc).is_empty(),
            "the first fault is a rule's own, and a later one of an action cut the search short"
        );
        let no_finding = checked(4, &[("bounded", "unknown")]);
        assert_eq!(
            differences(&no_finding, &read_tlc(PASSING), &runs, Tool::Tlc),
            ["rule bounded: the checker says unknown, TLC says holds"],
            "nothing cut the search short and TLC judged the rule everywhere"
        );
    }

    fn unjudged_somewhere(id: &str) -> String {
        VIOLATING.replace("Rule_safe", &format!("Judged_{id}"))
    }

    #[test]
    fn a_rule_tlc_could_not_judge_in_a_state_is_unknown_as_the_checker_says() {
        let judged = |id: &str, output: &str| RuleRun {
            judged: Some(format!("Judged_{id}")),
            ..rule_run(id, output)
        };
        let runs = [judged("capped", &unjudged_somewhere("capped"))];
        let unknown = checked(4, &[("capped", "unknown")]);
        assert!(
            differences(&unknown, &read_tlc(PASSING), &runs, Tool::Tlc).is_empty(),
            "TLC finished and saw a state where the rule has no value"
        );
        let holds = checked(4, &[("capped", "holds")]);
        assert_eq!(
            differences(&holds, &read_tlc(PASSING), &runs, Tool::Tlc),
            ["rule capped: the checker says holds, TLC says unknown"]
        );
        let everywhere = [judged("capped", PASSING)];
        assert!(
            differences(&holds, &read_tlc(PASSING), &everywhere, Tool::Tlc).is_empty(),
            "no state breaks the second invariant, so the rule holds"
        );
        let both = format!("{}{}", unjudged_somewhere("safe"), VIOLATING);
        let broken = checked(4, &[("safe", "broken")]);
        assert!(
            differences(
                &broken,
                &read_tlc(PASSING),
                &[judged("safe", &both)],
                Tool::Tlc
            )
            .is_empty(),
            "a rule broken in one state and without a value in another is broken"
        );
    }

    #[test]
    fn a_count_tlc_did_not_finish_and_a_rule_it_was_not_run_on_are_differences() {
        let ours = checked(4, &[("safe", "broken")]);
        assert_eq!(
            differences(&ours, &read_tlc(FAILING), &[], Tool::Tlc),
            [
                "states: the checker found 4, TLC did not finish its count \
                 (Evaluating invariant Rule_capped failed.)",
                "rule safe: TLC was not run on it",
            ]
        );
    }

    #[test]
    fn a_checker_result_that_stopped_at_a_limit_is_a_difference() {
        let mut ours = checked(4, &[]);
        ours.result.reason = Some("state limit of 4".to_owned());
        let found = differences(&ours, &read_tlc(PASSING), &[], Tool::Tlc);
        assert_eq!(found.len(), 1, "one difference: {found:?}");
        assert!(
            found[0].contains("state limit of 4"),
            "it says why: {found:?}"
        );
    }

    #[test]
    fn the_lines_say_same_or_differs_and_list_each_difference() {
        let ours = checked(4, &[]);
        assert_eq!(
            confirm_lines(&ours, &[], &[], Tool::Tlc),
            ["same     Oven  4 states  fail"]
        );
        assert_eq!(
            confirm_lines(&ours, &["states: 4 against 5".to_owned()], &[], Tool::Tlc),
            ["differs  Oven  4 states  fail", "  states: 4 against 5"]
        );
    }

    #[test]
    fn a_difference_in_a_model_beyond_tlc_says_tlc_cannot_confirm_it_and_why() {
        let ours = checked(4, &[]);
        assert_eq!(
            confirm_lines(
                &ours,
                &["states: 4 against 5".to_owned()],
                &["a number is too big".to_owned()],
                Tool::Tlc
            ),
            [
                "cannot   Oven  4 states  fail",
                "  TLC can't confirm this model: a number is too big",
                "  states: 4 against 5",
            ]
        );
        assert_eq!(
            confirm_lines(&ours, &[], &["a number is too big".to_owned()], Tool::Tlc),
            ["same     Oven  4 states  fail"],
            "TLC agreed, so the model is confirmed"
        );
    }

    #[test]
    fn the_lines_name_tla_rs_when_it_is_the_program() {
        let ours = checked(4, &[("safe", "holds")]);
        let runs = [RuleRun {
            id: "safe".to_owned(),
            invariant: "Rule_safe".to_owned(),
            judged: None,
            run: TlcRun {
                distinct_states: Some(4),
                violated: vec!["Rule_safe".to_owned()],
                errors: Vec::new(),
            },
        }];
        let found = differences(&ours, &read_tlc(FAILING), &runs, Tool::TlaRs);
        assert_eq!(
            found,
            [
                "states: the checker found 4, tla-rs did not finish its count \
                 (Evaluating invariant Rule_capped failed.)",
                "rule safe: the checker says holds, tla-rs says broken",
            ]
        );
        assert_eq!(
            confirm_lines(&ours, &found, &[], Tool::TlaRs)[0],
            "differs  Oven  4 states  fail  (tla-rs)",
            "the first line says which program"
        );
        assert_eq!(
            confirm_lines(&ours, &[], &[], Tool::TlaRs),
            ["same     Oven  4 states  fail  (tla-rs)"]
        );
    }

    #[test]
    fn the_runs_of_one_rule_are_merged_into_one() {
        let run = |violated: &[&str], states: Option<u64>, errors: &[&str]| TlcRun {
            distinct_states: states,
            violated: violated.iter().map(|name| (*name).to_owned()).collect(),
            errors: errors.iter().map(|error| (*error).to_owned()).collect(),
        };
        assert_eq!(
            merged(vec![
                run(&["Rule_a"], Some(4), &[]),
                run(&["Judged_a", "Rule_a"], Some(4), &["it failed"]),
            ]),
            run(&["Rule_a", "Judged_a"], Some(4), &["it failed"]),
            "violations are joined without repeats and errors are kept"
        );
        assert_eq!(
            merged(vec![run(&[], Some(4), &[]), run(&[], None, &[])]).distinct_states,
            None,
            "a run that did not finish its count means the merge did not either"
        );
        assert_eq!(
            merged(Vec::new()),
            TlcRun::default(),
            "no run, nothing found"
        );
    }
}
