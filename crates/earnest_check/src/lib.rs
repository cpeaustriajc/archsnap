//! The search over every reachable state of a checked model.
//!
//! [`explore`] takes a model from `earnest_semantic` and [`Limits`], visits every state the
//! model can reach, and gives an [`Outcome`]: a verdict and a shortest path for each rule,
//! findings for a stuck state, a value out of range and a fault, and a [`Graph`] for a chart
//! when asked.

mod cut;
mod expand;
mod explore;
mod graph;
mod helpers;
mod limits;
mod outcome;
mod report;
mod store;

pub use explore::explore;
pub use graph::{Edge, Graph, Node};
pub use helpers::{Helpers, Loan};
pub use limits::{Limits, WORKER_STACK, stopped_by_time_limit};
pub use outcome::{
    Change, Finding, FindingKind, Named, Outcome, Path, RuleOutcome, Shown, Status, Step, Verdict,
};
pub use store::{NumberSlot, RowLayout};
