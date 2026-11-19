use crate::workers::run_each;
use earnest_chart::detail_lines;
use earnest_check::{Limits, Outcome, explore};
use earnest_ir::Model;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::num::NonZeroUsize;
use std::thread::available_parallelism;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Bounds {
    pub(crate) max_states: usize,
    pub(crate) time_limit: Option<Duration>,
    pub(crate) jobs: usize,
}

impl Bounds {
    pub(crate) fn new(
        max_states: usize,
        time_limit_seconds: Option<u64>,
        jobs: Option<NonZeroUsize>,
    ) -> Self {
        Self {
            max_states,
            time_limit: time_limit_seconds.map(Duration::from_secs),
            jobs: jobs
                .or_else(|| available_parallelism().ok())
                .map_or(1, NonZeroUsize::get),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Graphs {
    Kept,
    Dropped,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Search<'a> {
    pub(crate) model: &'a Model,
    pub(crate) only_rule: Option<&'a str>,
}

#[derive(Clone, Debug)]
pub(crate) struct Ran {
    pub(crate) outcome: Outcome,
    pub(crate) elapsed: Duration,
}

pub(crate) fn explore_each(searches: &[Search<'_>], bounds: Bounds, graphs: Graphs) -> Vec<Ran> {
    run_each(searches, bounds.jobs, |search, threads| {
        let limits = Limits {
            max_states: bounds.max_states,
            time_limit: bounds.time_limit,
            jobs: threads,
            keep_graph: graphs == Graphs::Kept,
            only_rule: search.only_rule.map(str::to_owned),
        };
        let began = Instant::now();
        let outcome = explore(search.model, &limits);
        Ran {
            outcome,
            elapsed: began.elapsed(),
        }
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Status {
    Pass,
    Fail,
    Unfinished,
}

impl Status {
    pub(crate) fn word(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Unfinished => "unfinished",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct NamedValue {
    pub(crate) name: String,
    pub(crate) value: Json,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FieldChange {
    pub(crate) field: String,
    pub(crate) before: Json,
    pub(crate) after: Json,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PathStep {
    pub(crate) action: String,
    pub(crate) label: String,
    pub(crate) input: Vec<NamedValue>,
    pub(crate) changes: Vec<FieldChange>,
    pub(crate) state: Vec<NamedValue>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PathTaken {
    pub(crate) start: Vec<NamedValue>,
    pub(crate) steps: Vec<PathStep>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RuleResult {
    pub(crate) id: String,
    pub(crate) says: String,
    pub(crate) verdict: String,
    pub(crate) path: Option<PathTaken>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FindingResult {
    pub(crate) kind: String,
    pub(crate) message: String,
    pub(crate) path: PathTaken,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct Searched {
    status: Status,
    states: usize,
    rules: Vec<RuleResult>,
    findings: Vec<FindingResult>,
    reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ModelResult {
    pub(crate) name: String,
    pub(crate) file: String,
    pub(crate) line: u32,
    pub(crate) status: Status,
    pub(crate) states: usize,
    pub(crate) milliseconds: u64,
    pub(crate) cached: bool,
    pub(crate) reason: Option<String>,
    pub(crate) rules: Vec<RuleResult>,
    pub(crate) findings: Vec<FindingResult>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Checked {
    pub(crate) result: ModelResult,
    pub(crate) detail: Vec<String>,
}

pub(crate) fn milliseconds(elapsed: Duration) -> u64 {
    u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
}

pub(crate) fn checked(model: &Model, ran: &Ran) -> Result<Checked, String> {
    let searched: Searched = serde_json::to_value(&ran.outcome)
        .and_then(serde_json::from_value)
        .map_err(|error| {
            format!(
                "the result of the model `{}` can't be reported: {error}",
                model.name
            )
        })?;
    Ok(Checked {
        result: ModelResult {
            name: model.name.clone(),
            file: model.file.clone(),
            line: model.line,
            status: searched.status,
            states: searched.states,
            milliseconds: milliseconds(ran.elapsed),
            cached: false,
            reason: searched.reason,
            rules: searched.rules,
            findings: searched.findings,
        },
        detail: detail_lines(&ran.outcome),
    })
}
