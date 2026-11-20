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
mod outcome;
mod report;

pub use explore::{Limits, WORKER_STACK, explore};
pub use graph::{Edge, Graph, Node};
pub use helpers::{Helpers, Loan};
pub use outcome::{
    Change, Finding, FindingKind, Named, Outcome, Path, RuleOutcome, Shown, Status, Step, Verdict,
};
