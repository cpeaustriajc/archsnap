//! Models that `earnest` gets wrong, each with a test that states the right answer.
//!
//! The crate has no code of its own. Its test target `break_it` runs the `earnest` binary and
//! the TypeScript library on the files in `models/` and asserts what the contract in the root
//! `README.md` says, so a test fails until its finding is fixed. `README.md` beside this crate
//! lists every case with its hand-derived answer.
