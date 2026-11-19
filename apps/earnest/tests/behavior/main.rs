//! Acceptance tests for the `earnest` binary, one or more for each requirement of the version 1
//! spec. Every test runs the real binary in its own temporary folder on the models in
//! `tasks/conformance/models`, and takes its expected values from `tasks/conformance/README.md`.

mod cache;
mod changes;
mod chart;
mod expected;
mod harness;
mod limits;
mod offline;
mod replay;
mod results;
mod select;
mod support;
mod tla;
mod twins;
mod typescript;
mod unreadable;
mod verify;
