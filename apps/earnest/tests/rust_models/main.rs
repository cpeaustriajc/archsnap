//! Rust models as the spec asks for them: statechart labels (R9), a choice among listed values
//! (the primitives table), the timer, request and retry parts, a path from the report replayed
//! with the Rust library (R21), and model files that the app's compiler, linter and formatter
//! accept (R2). Each Rust model is compared with its TypeScript twin or built in a real crate
//! that depends on `earnest`. The fixtures are in `tasks/conformance/twins` and
//! `tasks/conformance/models`.

mod charts;
mod crates;
mod listed;
mod parts;
mod replay;
mod support;
