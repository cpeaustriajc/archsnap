use crate::changes::{RuleChanges, rule_change_lines};
use crate::markdown::{code_span, fenced, one_line, prose};
use crate::mermaid::chart;
use crate::text::{detail_lines, word};
use earnest_check::{Outcome, Status};

/// What the report says about one model.
#[derive(Clone, Copy, Debug)]
pub struct Section<'a> {
    /// The model's name, for the heading and the result line.
    pub name: &'a str,
    /// The path of the model's file as the report should show it.
    pub file: &'a str,
    /// What the search found. The section has a chart when its `graph` is present.
    pub outcome: &'a Outcome,
}

/// A model file that could not be read, so none of its models is in the report.
#[derive(Clone, Copy, Debug)]
pub struct Unreadable<'a> {
    /// The path of the file as the report should show it.
    pub file: &'a str,
    /// One line for each problem, as the text output of the CLI prints it:
    /// `file:line:column`, the feature and the sentence.
    pub problems: &'a [String],
}

fn result_line(section: &Section<'_>) -> String {
    let outcome = section.outcome;
    let so_far = if outcome.status == Status::Unfinished {
        " so far"
    } else {
        ""
    };
    let mut parts = vec![
        format!("**{}**", word(&outcome.status)),
        prose(section.name),
        format!("{} states{so_far}", outcome.states),
    ];
    parts.extend(outcome.reason.as_deref().map(prose));
    parts.push(code_span(section.file));
    parts.join(" · ")
}

fn unread_lines(file: &Unreadable<'_>) -> Vec<String> {
    let mut lines = vec![format!("unreadable  {}", one_line(file.file))];
    lines.extend(
        file.problems
            .iter()
            .map(|problem| format!("    {}", one_line(problem))),
    );
    lines
}

fn model(section: &Section<'_>) -> String {
    let mut blocks = vec![format!("## {}", prose(section.name)), result_line(section)];
    blocks.extend(section.outcome.graph.as_ref().map(chart));
    let detail = detail_lines(section.outcome);
    if !detail.is_empty() {
        blocks.push(fenced(&detail));
    }
    blocks.join("\n\n")
}

/// Markdown for a PR body, with no trailing line break: one section per model, in the order
/// given, then an `Unreadable files` section when `unreadable` is not empty, then a `Rule
/// changes` section when `changes` is present.
///
/// The `Unreadable files` section names each file the checker could not read and why, so a
/// reader of the report can tell a file that was checked from one that was left unchecked.
///
/// A model's section is a heading with its name, a result line with the status, the state
/// count and the file, the chart, and a fenced block with the path to each broken rule and
/// each finding. The chart is left out when the outcome holds no graph, and the fenced block
/// when nothing is wrong. Pass `changes` only when the user asked for a comparison: with no
/// changed rule the section still appears and says `none`.
#[must_use]
pub fn report(
    sections: &[Section<'_>],
    unreadable: &[Unreadable<'_>],
    changes: Option<&RuleChanges<'_>>,
) -> String {
    let mut blocks: Vec<String> = sections.iter().map(model).collect();
    if !unreadable.is_empty() {
        let lines: Vec<String> = unreadable.iter().flat_map(unread_lines).collect();
        blocks.push(format!("## Unreadable files\n\n{}", fenced(&lines)));
    }
    if let Some(changes) = changes {
        blocks.push(format!(
            "## Rule changes\n\n{}",
            fenced(&rule_change_lines(changes))
        ));
    }
    blocks.join("\n\n")
}
