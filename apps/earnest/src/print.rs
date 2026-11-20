use crate::changes::Difference;
use crate::reading::ModelFile;
use crate::run::{Checked, ModelResult, Status};
use earnest_chart::{RuleChange, RuleChanges, one_line, rule_change_lines};
use earnest_diagnostics::Problem;
use serde::Serialize;
use std::io::{self, Write};

const INDENT: &str = "  ";
const STATUS_WIDTH: usize = 10;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Since<'a> {
    pub(crate) base: &'a str,
    pub(crate) changes: &'a [Difference],
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Verified<'a> {
    pub(crate) files: &'a [ModelFile],
    pub(crate) checked: &'a [Checked],
    pub(crate) since: Option<Since<'a>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub(crate) struct Summary {
    models: usize,
    pass: usize,
    fail: usize,
    unfinished: usize,
    unreadable: usize,
}

impl Summary {
    pub(crate) fn everything_passed(&self) -> bool {
        self.fail == 0 && self.unfinished == 0 && self.unreadable == 0
    }
}

#[derive(Debug, Serialize)]
struct Document<'a> {
    models: Vec<&'a ModelResult>,
    unreadable: Vec<&'a Problem>,
    #[serde(rename = "ruleChanges")]
    rule_changes: &'a [Difference],
    summary: Summary,
}

fn counted(count: usize, one: &str, many: &str) -> String {
    let noun = if count == 1 { one } else { many };
    format!("{count} {noun}")
}

impl Verified<'_> {
    pub(crate) fn summary(&self) -> Summary {
        let with = |status: Status| {
            self.checked
                .iter()
                .filter(|checked| checked.result.status == status)
                .count()
        };
        Summary {
            models: self.checked.len(),
            pass: with(Status::Pass),
            fail: with(Status::Fail),
            unfinished: with(Status::Unfinished),
            unreadable: self
                .files
                .iter()
                .filter(|file| !file.problems().is_empty())
                .count(),
        }
    }

    pub(crate) fn text(&self) -> String {
        let mut lines = Vec::new();
        let mut checked = self.checked.iter().peekable();
        for file in self.files {
            if !file.problems().is_empty() {
                lines.extend(unreadable_lines(file));
            }
            while let Some(next) = checked.next_if(|next| next.result.file == file.path) {
                lines.push(model_line(&next.result));
                lines.extend(next.detail.iter().map(|line| format!("{INDENT}{line}")));
            }
        }
        if let Some(since) = self.since {
            let changes: Vec<RuleChange<'_>> = since
                .changes
                .iter()
                .map(Difference::for_the_chart)
                .collect();
            lines.extend(rule_change_lines(&RuleChanges {
                base: since.base,
                changes: &changes,
            }));
        }
        lines.push(summary_line(&self.summary()));
        lines.join("\n")
    }

    pub(crate) fn json(&self) -> Result<String, String> {
        let document = Document {
            models: self.checked.iter().map(|checked| &checked.result).collect(),
            unreadable: self.files.iter().flat_map(ModelFile::problems).collect(),
            rule_changes: self.since.map_or(&[], |since| since.changes),
            summary: self.summary(),
        };
        serde_json::to_string_pretty(&document)
            .map_err(|error| format!("the results can't be written as JSON: {error}"))
    }
}

fn model_line(result: &ModelResult) -> String {
    let so_far = if result.reason.is_some() {
        " so far"
    } else {
        ""
    };
    let mut fields = vec![
        format!("{:<STATUS_WIDTH$}", result.status.word()),
        one_line(&result.name),
        format!("{} states{so_far}", result.states),
        format!("{} ms", result.milliseconds),
    ];
    fields.extend(result.reason.as_ref().map(|reason| format!("({reason})")));
    if result.cached {
        fields.push("(cached)".to_owned());
    }
    fields.join(INDENT)
}

pub(crate) fn problem_lines(file: &ModelFile) -> Vec<String> {
    file.problems()
        .iter()
        .map(|problem| {
            format!(
                "{}:{}:{}{INDENT}{}{INDENT}{}",
                one_line(&problem.file),
                problem.line,
                problem.column,
                one_line(&problem.feature),
                one_line(&problem.message)
            )
        })
        .collect()
}

pub(crate) fn unreadable_lines(file: &ModelFile) -> Vec<String> {
    let mut lines = vec![format!(
        "{:<STATUS_WIDTH$}{INDENT}{}",
        "unreadable",
        one_line(&file.path)
    )];
    lines.extend(
        problem_lines(file)
            .into_iter()
            .map(|line| format!("{INDENT}{line}")),
    );
    lines
}

fn summary_line(summary: &Summary) -> String {
    let line = format!(
        "{}: {} pass, {} fail, {} unfinished",
        counted(
            summary.models.saturating_add(summary.unreadable),
            "model",
            "models"
        ),
        summary.pass,
        summary.fail,
        summary.unfinished
    );
    if summary.unreadable > 0 {
        return format!("{line}, {} unreadable", summary.unreadable);
    }
    line
}

fn write_line(mut stream: impl Write, text: &str) {
    if text.is_empty() {
        return;
    }
    let written = stream
        .write_all(text.as_bytes())
        .and_then(|()| stream.write_all(b"\n"))
        .and_then(|()| stream.flush());
    drop(written);
}

pub(crate) fn to_stdout(text: &str) {
    write_line(io::stdout().lock(), text);
}

pub(crate) fn to_stderr(text: &str) {
    write_line(io::stderr().lock(), text);
}

#[cfg(test)]
mod tests {
    use super::{Since, Summary, Verified, summary_line, write_line};
    use crate::changes::{Difference, Kind, Wording};
    use crate::discovery::{Found, Language};
    use crate::reading::{ModelFile, load};
    use crate::run::{Checked, ModelResult, RuleResult, Status};
    use serde_json::{Value, json};
    use std::io::{self, Write};

    fn file(path: &str, body: &str) -> ModelFile {
        load(Found::model(
            path,
            Language::TypeScript,
            &format!("import {{ m }} from \"@earnest/model\";\n{body}"),
        ))
    }

    fn files() -> Vec<ModelFile> {
        vec![
            file(
                "src/a.model.ts",
                "export const A = m.model(\"A\", { on: m.boolean() });\n",
            ),
            file("src/clock.model.ts", "export const now = Date.now();\n"),
            file(
                "src/z.model.ts",
                "export const Z = m.model(\"Z\", { on: m.boolean() });\n",
            ),
        ]
    }

    fn result(name: &str, file: &str, status: Status) -> ModelResult {
        ModelResult {
            name: name.to_owned(),
            file: file.to_owned(),
            line: 2,
            status,
            states: 4,
            milliseconds: 1,
            cached: false,
            reason: None,
            rules: vec![RuleResult {
                id: "fine".to_owned(),
                says: "All is fine".to_owned(),
                verdict: "holds".to_owned(),
                path: None,
            }],
            findings: Vec::new(),
        }
    }

    fn checked() -> Vec<Checked> {
        vec![
            Checked {
                result: result("A", "src/a.model.ts", Status::Fail),
                detail: vec![
                    "stuck  nothing can happen".to_owned(),
                    "  start  on = false".to_owned(),
                ],
            },
            Checked {
                result: ModelResult {
                    states: 1000,
                    milliseconds: 3,
                    reason: Some("state limit of 1000".to_owned()),
                    cached: true,
                    ..result("Z", "src/z.model.ts", Status::Unfinished)
                },
                detail: Vec::new(),
            },
        ]
    }

    #[test]
    fn the_text_has_a_line_per_model_its_detail_each_unreadable_file_and_a_summary() {
        let (files, checked) = (files(), checked());
        let verified = Verified {
            files: &files,
            checked: &checked,
            since: None,
        };
        let problem = &files[1].problems()[0];
        assert_eq!(
            verified.text(),
            format!(
                "\
fail        A  4 states  1 ms
  stuck  nothing can happen
    start  on = false
unreadable  src/clock.model.ts
  src/clock.model.ts:2:{}  `Date.now`  {}
unfinished  Z  1000 states so far  3 ms  (state limit of 1000)  (cached)
3 models: 0 pass, 1 fail, 1 unfinished, 1 unreadable",
                problem.column, problem.message
            )
        );
    }

    #[test]
    fn the_rule_changes_come_before_the_summary() {
        let (files, checked) = (files(), checked());
        let changes = [Difference {
            file: "tank.model.ts".to_owned(),
            model: "A".to_owned(),
            id: "fresh".to_owned(),
            change: Kind::Added,
            was: None,
            now: Some(Wording {
                says: "New".to_owned(),
                holds: "s.on".to_owned(),
            }),
        }];
        let verified = Verified {
            files: &files,
            checked: &checked,
            since: Some(Since {
                base: "origin/main",
                changes: &changes,
            }),
        };
        let text = verified.text();
        let tail: Vec<&str> = text.lines().rev().take(3).collect();
        assert_eq!(
            tail,
            [
                "3 models: 0 pass, 1 fail, 1 unfinished, 1 unreadable",
                "  added      A.fresh  \"New\"  tank.model.ts",
                "rule changes since origin/main",
            ]
        );
    }

    #[test]
    fn the_summary_counts_models_and_names_unreadable_files_only_when_there_are_some() {
        let summary = |models, pass, unreadable| Summary {
            models,
            pass,
            fail: 0,
            unfinished: 0,
            unreadable,
        };
        assert_eq!(
            summary_line(&summary(1, 1, 0)),
            "1 model: 1 pass, 0 fail, 0 unfinished"
        );
        assert_eq!(
            summary_line(&summary(0, 0, 0)),
            "0 models: 0 pass, 0 fail, 0 unfinished"
        );
        assert_eq!(
            summary_line(&summary(2, 2, 2)),
            "4 models: 2 pass, 0 fail, 0 unfinished, 2 unreadable"
        );
        let readme_example = Summary {
            models: 3,
            pass: 1,
            fail: 1,
            unfinished: 1,
            unreadable: 1,
        };
        assert_eq!(
            summary_line(&readme_example),
            "4 models: 1 pass, 1 fail, 1 unfinished, 1 unreadable",
            "the last line of the README's text example"
        );
        assert!(summary(2, 2, 0).everything_passed(), "two models pass");
        assert!(
            !summary(2, 2, 1).everything_passed(),
            "a file is unreadable"
        );
        assert!(summary(0, 0, 0).everything_passed(), "nothing to check");
    }

    #[test]
    fn the_json_has_the_documented_members() {
        let (files, checked) = (files(), checked());
        let verified = Verified {
            files: &files,
            checked: &checked,
            since: None,
        };
        let document: Value = serde_json::from_str(&verified.json().unwrap()).unwrap();
        assert_eq!(
            document["summary"],
            json!({ "models": 2, "pass": 0, "fail": 1, "unfinished": 1, "unreadable": 1 })
        );
        assert_eq!(document["ruleChanges"], json!([]), "no base, no changes");
        assert_eq!(
            document["models"][1],
            json!({
                "name": "Z",
                "file": "src/z.model.ts",
                "line": 2,
                "status": "unfinished",
                "states": 1000,
                "milliseconds": 3,
                "cached": true,
                "reason": "state limit of 1000",
                "rules": [{ "id": "fine", "says": "All is fine", "verdict": "holds", "path": null }],
                "findings": [],
            })
        );
        let problem = &document["unreadable"][0];
        assert_eq!(problem["file"], "src/clock.model.ts", "the file");
        assert_eq!(problem["line"], 2, "the line");
        assert_eq!(problem["feature"], "`Date.now`", "the feature");
        assert!(problem["column"].is_u64(), "the column");
        assert!(problem["message"].is_string(), "the sentence");
        assert!(
            !verified.json().unwrap().contains("detail"),
            "the text lines are not part of the JSON"
        );
    }

    #[test]
    fn a_model_or_file_name_with_a_line_break_stays_on_one_line() {
        let named = Checked {
            result: result(
                "First\nSecond\u{b}Third",
                "first\nsecond.model.ts",
                Status::Pass,
            ),
            detail: Vec::new(),
        };
        let unreadable = file("first\nsecond.model.ts", "export const now = Date.now();\n");
        let files = [unreadable];
        let checked = [named];
        let text = Verified {
            files: &files,
            checked: &checked,
            since: None,
        }
        .text();
        let lines: Vec<&str> = text.lines().collect();
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("pass") && line.contains("First Second Third")),
            "the model is one line: {text}"
        );
        assert!(
            lines.iter().any(
                |line| line.starts_with("unreadable") && line.contains("first second.model.ts")
            ),
            "the file is one line: {text}"
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("first second.model.ts:") && line.contains("`Date.now`")),
            "each problem names the file on its one line: {text}"
        );
    }

    struct ClosedPipe;

    impl Write for ClosedPipe {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
    }

    #[test]
    fn a_closed_pipe_is_not_a_panic() {
        write_line(ClosedPipe, "anything");
    }

    #[test]
    fn text_is_written_with_one_line_break_and_nothing_is_written_for_no_text() {
        let mut written = Vec::new();
        write_line(&mut written, "one\ntwo");
        assert_eq!(written, b"one\ntwo\n", "one line break at the end");
        let mut nothing = Vec::new();
        write_line(&mut nothing, "");
        assert!(nothing.is_empty(), "no empty line");
    }
}
