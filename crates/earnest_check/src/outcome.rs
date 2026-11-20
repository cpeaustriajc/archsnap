use crate::graph::Graph;
use serde::{Serialize, Serializer};
use serde_json::Value as Json;

/// One value of a model, already printed both ways.
///
/// It serializes as `json` alone, so a path in the JSON output holds plain JSON values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shown {
    /// The value as the text report prints it, such as `"locked"` or `{1, 2}`.
    pub text: String,
    /// The value as the JSON output holds it.
    pub json: Json,
}

impl Serialize for Shown {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.json.serialize(serializer)
    }
}

/// A state field or an action input with its value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Named {
    /// The field's or the input's name.
    pub name: String,
    /// Its value.
    pub value: Shown,
}

/// One field an action changed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Change {
    /// The field's name.
    pub field: String,
    /// The value before the action.
    pub before: Shown,
    /// The value after it.
    pub after: Shown,
}

/// One action taken along a path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Step {
    /// The action's id.
    pub action: String,
    /// The action's sentence.
    pub label: String,
    /// The value chosen for each input, in the action's input order.
    pub input: Vec<Named>,
    /// The fields whose value changed, in field order. Empty when the action led back to the
    /// same state.
    pub changes: Vec<Change>,
    /// Every field after the action, in field order.
    pub state: Vec<Named>,
}

/// A way to reach a state: a start state, then one action after another.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Path {
    /// Every field of the start state, in field order.
    pub start: Vec<Named>,
    /// The actions taken, first to last. Empty when the start state is the state reached.
    pub steps: Vec<Step>,
}

/// What the search learned about one rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Verdict {
    /// True in every reachable state.
    Holds,
    /// False in a reachable state.
    Broken,
    /// Not seen broken, but not judged in every state either: the search stopped at a limit,
    /// or the rule can't be evaluated in some state.
    Unknown,
}

/// One rule's result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RuleOutcome {
    /// The rule's id.
    pub id: String,
    /// The rule's sentence.
    pub says: String,
    /// What the search learned.
    pub verdict: Verdict,
    /// A shortest path to a state that breaks the rule. Present exactly when the verdict is
    /// [`Verdict::Broken`].
    pub path: Option<Path>,
}

/// What kind of trouble a [`Finding`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum FindingKind {
    /// No action can happen in the state and no end marks it.
    Stuck,
    /// A field holds a value outside what it declares.
    OutOfRange,
    /// A rule, an action or an end can't be evaluated in the state.
    Fault,
}

/// Something wrong with a model that is not a broken rule.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Finding {
    /// The kind of trouble.
    pub kind: FindingKind,
    /// A sentence for the report, naming the field, rule or action involved.
    pub message: String,
    /// A shortest path to the first state where it happens.
    pub path: Path,
}

/// The one-word result of checking a model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Status {
    /// Every reachable state was visited and nothing is wrong.
    Pass,
    /// A rule is broken or there is a finding, whether or not the search finished.
    Fail,
    /// The search stopped at a limit with nothing found wrong so far.
    Unfinished,
}

/// Everything one search found.
///
/// It serializes into the fields the JSON output documents for a model, apart from the name,
/// file, line, time and cache state, which the caller adds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Outcome {
    /// The one-word result.
    pub status: Status,
    /// How many distinct states were visited, counting any state out of range. When the search
    /// stopped at a limit, this is the count so far.
    pub states: usize,
    /// One entry per checked rule, in the model's rule order.
    pub rules: Vec<RuleOutcome>,
    /// At most one finding of each kind, in the order stuck, out of range, fault: the first
    /// state the search met with that trouble.
    pub findings: Vec<Finding>,
    /// Why the search stopped early, such as `state limit of 1000`. `None` when it visited
    /// every state.
    pub reason: Option<String>,
    /// Whether any arithmetic the search evaluated gave a number outside 32 bits, which TLC's
    /// integers can't hold. It is not serialized.
    #[serde(skip)]
    pub computes_beyond_32_bits: bool,
    /// The states and arrows for a chart, when the limits asked to keep them. It is not
    /// serialized.
    #[serde(skip)]
    pub graph: Option<Graph>,
}
