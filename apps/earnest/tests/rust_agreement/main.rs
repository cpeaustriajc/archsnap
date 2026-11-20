//! Compares what `rustc` computes with what the checker computes, for each Rust construct the
//! reader accepts beyond the first version: the value inside an `Option`, `match` on an enum,
//! integer division, helpers that return the state, a text or a record, and the `From`
//! conversions. Each fixture is compiled as real Rust in a temporary crate that depends on
//! `earnest`; a test walks it breadth first with the library's `steps()`, and the distinct
//! states, the rules that break with the length of the shortest path to each, and whether a
//! state is stuck must be what `earnest verify --json` reports for the same file. A refusal is
//! checked next to `rustc`: a form that `rustc` rejects must be refused with a sentence, and a
//! form it accepts but the checker can't read as Rust does must be refused too.

mod agree;
mod division;
mod harness;
mod refusals;
