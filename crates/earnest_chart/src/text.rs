use crate::markdown::one_line;
use earnest_check::{Named, Outcome, Path};
use serde::Serialize;
use serde_json::Value as Json;

const UNNAMED: &str = "unknown";
const NESTED: &str = "  ";

pub(crate) fn word<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(Json::String(name)) => name.replace('-', " "),
        _ => UNNAMED.to_owned(),
    }
}

pub(crate) fn quoted(sentence: &str) -> String {
    format!("\"{}\"", one_line(sentence))
}

fn named(values: &[Named]) -> String {
    values
        .iter()
        .map(|each| format!("{} = {}", one_line(&each.name), one_line(&each.value.text)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A path in the form the text output documents: a `start` line with every field, then one
/// numbered line per action with its id, its input when it has one and each field it changed.
///
/// The lines are not indented. A line break inside a value is printed as a space, so each
/// entry is one line.
#[must_use]
pub fn path_lines(path: &Path) -> Vec<String> {
    let mut lines = vec![
        ["start".to_owned(), named(&path.start)]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("  "),
    ];
    for (number, step) in (1_usize..).zip(&path.steps) {
        let changes = step
            .changes
            .iter()
            .map(|change| {
                format!(
                    "{}: {} -> {}",
                    one_line(&change.field),
                    one_line(&change.before.text),
                    one_line(&change.after.text)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let parts = [
            format!("{number}. {}", one_line(&step.action)),
            named(&step.input),
            changes,
        ];
        lines.push(
            parts
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("  "),
        );
    }
    lines
}

/// The detail lines of a model that did not pass, as the text output documents them: for each
/// broken rule `broken`, its id and its sentence in quotes, and for each finding its kind and
/// its message, each followed by its path two spaces deeper.
///
/// The first level is not indented; the text output indents every line once more. A kind this
/// crate has not heard of prints under the name it serializes to, with spaces for hyphens. The
/// result is empty for a model with no broken rule and no finding.
#[must_use]
pub fn detail_lines(outcome: &Outcome) -> Vec<String> {
    let mut lines = Vec::new();
    let mut add = |head: String, path: &Path| {
        lines.push(head);
        lines.extend(
            path_lines(path)
                .into_iter()
                .map(|line| format!("{NESTED}{line}")),
        );
    };
    for rule in &outcome.rules {
        if let Some(path) = &rule.path {
            add(
                format!("broken  {}  {}", one_line(&rule.id), quoted(&rule.says)),
                path,
            );
        }
    }
    for finding in &outcome.findings {
        add(
            format!("{}  {}", word(&finding.kind), one_line(&finding.message)),
            &finding.path,
        );
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::word;
    use earnest_check::{FindingKind, Status, Verdict};

    #[test]
    fn a_kind_prints_as_the_words_it_serializes_to() {
        assert_eq!(word(&FindingKind::Stuck), "stuck", "one word");
        assert_eq!(
            word(&FindingKind::OutOfRange),
            "out of range",
            "hyphens become spaces"
        );
        assert_eq!(word(&FindingKind::Fault), "fault", "one word");
        assert_eq!(word(&Status::Unfinished), "unfinished", "a status");
        assert_eq!(word(&Verdict::Broken), "broken", "a verdict");
    }

    #[test]
    fn a_value_that_does_not_serialize_to_text_prints_as_unknown() {
        assert_eq!(word(&7), "unknown", "a number has no name");
    }
}
