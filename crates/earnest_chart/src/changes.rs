use crate::markdown::one_line;
use crate::text::quoted;

/// How a rule differs between the base commit and the working tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ChangeKind {
    /// Only the new model has the rule.
    Added,
    /// Only the old model has the rule.
    Removed,
    /// A reachable state passes the new rule and breaks the old one.
    Loosened,
    /// A reachable state breaks the new rule and passes the old one, and none loosens it.
    Tightened,
    /// The condition differs, and no reachable state tells the two apart.
    Changed,
    /// Only the sentence differs.
    Reworded,
}

impl ChangeKind {
    fn word(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::Loosened => "loosened",
            Self::Tightened => "tightened",
            Self::Changed => "changed",
            Self::Reworded => "reworded",
        }
    }
}

/// One side of a rule change: the rule as it was, or as it is now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleText<'a> {
    /// The rule's sentence.
    pub says: &'a str,
    /// The source text of the rule's condition. It may span lines; it is printed on one.
    pub holds: &'a str,
}

/// One rule that differs from the base commit, with the same fields as an entry of
/// `ruleChanges` in the JSON output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleChange<'a> {
    /// The name of the model the rule belongs to.
    pub model: &'a str,
    /// The rule's id.
    pub id: &'a str,
    /// How the rule differs.
    pub change: ChangeKind,
    /// The rule at the base commit. `None` for an added rule.
    pub was: Option<RuleText<'a>>,
    /// The rule in the working tree. `None` for a removed rule.
    pub now: Option<RuleText<'a>>,
}

/// Every rule that differs from one base commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleChanges<'a> {
    /// The ref the models were compared with, as the user gave it to `--base`.
    pub base: &'a str,
    /// The changes, in the order to print them.
    pub changes: &'a [RuleChange<'a>],
}

fn side(name: &str, text: RuleText<'_>) -> String {
    format!(
        "    {name}  {}  {}",
        quoted(text.says),
        one_line(text.holds)
    )
}

/// The rule-changes section the text output documents: the line `rule changes since <base>`,
/// then one indented line per rule. An added or removed rule has its sentence on that line;
/// any other change is followed by a `was` and a `now` line with the sentence and the
/// condition.
///
/// When no rule changed, the header is followed by the one line `  none`.
#[must_use]
pub fn rule_change_lines(changes: &RuleChanges<'_>) -> Vec<String> {
    let mut lines = vec![format!("rule changes since {}", one_line(changes.base))];
    if changes.changes.is_empty() {
        lines.push("  none".to_owned());
    }
    for each in changes.changes {
        let head = format!(
            "  {:<9}  {}.{}",
            each.change.word(),
            one_line(each.model),
            one_line(each.id)
        );
        let only = match each.change {
            ChangeKind::Added => each.now,
            ChangeKind::Removed => each.was,
            ChangeKind::Loosened
            | ChangeKind::Tightened
            | ChangeKind::Changed
            | ChangeKind::Reworded => None,
        };
        if let Some(text) = only {
            lines.push(format!("{head}  {}", quoted(text.says)));
            continue;
        }
        lines.push(head);
        if matches!(each.change, ChangeKind::Added | ChangeKind::Removed) {
            continue;
        }
        lines.extend(each.was.map(|text| side("was", text)));
        lines.extend(each.now.map(|text| side("now", text)));
    }
    lines
}
