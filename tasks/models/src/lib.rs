//! Models of three of earnest's own state machines, written with the `earnest` crate and checked
//! with `earnest verify`: the shared thread pool, the result cache and a run's outcome.
//!
//! The crate has no code to call. `README.md` beside it says what each model stands for in the
//! checker's code, and the test target runs `earnest verify` on the three files.

mod cache;
mod outcome;
mod pool;
#[cfg(test)]
mod walk;
