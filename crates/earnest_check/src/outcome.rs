use crate::graph::Graph;
use serde::{Serialize, Serializer};
use serde_json::Value as Json;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shown {
    pub text: String,
    pub json: Json,
}

impl Serialize for Shown {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.json.serialize(serializer)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Named {
    pub name: String,
    pub value: Shown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Change {
    pub field: String,
    pub before: Shown,
    pub after: Shown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Step {
    pub action: String,
    pub label: String,
    pub input: Vec<Named>,
    pub changes: Vec<Change>,
    pub state: Vec<Named>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Path {
    pub start: Vec<Named>,
    pub steps: Vec<Step>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Holds,
    Broken,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RuleOutcome {
    pub id: String,
    pub says: String,
    pub verdict: Verdict,
    pub path: Option<Path>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FindingKind {
    Stuck,
    OutOfRange,
    Fault,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub kind: FindingKind,
    pub message: String,
    pub path: Path,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Pass,
    Fail,
    Unfinished,
}

#[derive(Clone, Debug, Serialize)]
pub struct Outcome {
    pub status: Status,
    pub states: usize,
    pub rules: Vec<RuleOutcome>,
    pub findings: Vec<Finding>,
    pub reason: Option<String>,
    #[serde(skip)]
    pub graph: Option<Graph>,
}
