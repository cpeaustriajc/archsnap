use super::{Behavior, Step, report};
use std::error::Error;
use std::fmt;

/// One step of a path from `earnest verify --json`: the action's id and each of its inputs.
///
/// A value is written as the report prints it, in the JSON of `earnest verify --json` or in
/// its text output: `Bill` or `"Bill"` for an enum variant or a text, `3` for a number, `true`
/// for a boolean, `null` for a `None` and `{row: 1, lit: true}` for a record, whose fields may
/// come in any order. The value of a `Some` is the value inside it. The step's `changes` and
/// `state` are not part of it: [`replay`] gives back every state, and the test compares them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Move {
    action: String,
    input: Vec<(String, String)>,
}

impl Move {
    /// A step of the action with this id, with no input yet.
    #[must_use]
    pub fn new(action: &str) -> Self {
        Self {
            action: action.to_owned(),
            input: Vec::new(),
        }
    }

    /// The same step with the input `name` given the value `value`, written as the report
    /// prints it. Give every input the action takes, in any order.
    #[must_use]
    pub fn with(mut self, name: &str, value: &str) -> Self {
        self.input.push((name.to_owned(), value.to_owned()));
        self
    }

    fn matches<State>(&self, step: &Step<State>) -> bool {
        step.action == self.action
            && step.input.len() == self.input.len()
            && step.input.iter().all(|(name, shown)| {
                self.input
                    .iter()
                    .any(|(given, value)| given == name && report::matches(shown, value))
            })
    }
}

impl fmt::Display for Move {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.action)?;
        for (position, (name, value)) in self.input.iter().enumerate() {
            let separator = if position == 0 { "[" } else { ", " };
            write!(formatter, "{separator}{name} = {value}")?;
        }
        if self.input.is_empty() {
            Ok(())
        } else {
            formatter.write_str("]")
        }
    }
}

/// Why a path can't be replayed: the step that can't happen and what could happen there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mismatch {
    /// The step that failed, counted from 1.
    pub step: usize,
    /// What is wrong, as a sentence.
    pub reason: String,
}

impl fmt::Display for Mismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "step {}: {}", self.step, self.reason)
    }
}

impl Error for Mismatch {}

/// Walks a path from `earnest verify --json` through the model, one step at a time, and gives
/// back every state on the way: `start` first, then the state after each step.
///
/// Pick `start` from `Model::starts()`: the one that equals the path's `start`. Each step is
/// found among `state.steps()`, so an action runs only where its `require!` conditions hold,
/// and each input is tried as the checker tries it. The states come back as the model's own
/// type, so a test compares them with `assert_eq!` or looks at them with the rule methods.
///
/// The conformance model `tasks/conformance/models/mailbox.rs` breaks `bills` after two
/// deliveries of a bill:
///
/// ```
/// include!("../../../../tasks/conformance/models/mailbox.rs");
///
/// use earnest::model::{Move, replay};
///
/// fn main() {
///     let start = Mailbox::starts().remove(0);
///     let path = [
///         Move::new("deliver").with("letter", "Bill"),
///         Move::new("deliver").with("letter", "Bill"),
///     ];
///     let states = replay(&start, &path).expect("the path is one the model can take");
///     assert_eq!(states.len(), 3);
///     assert!(states[1].bills());
///     assert!(!states[2].bills());
///
///     let full = [Move::new("deliver").with("letter", "Card")];
///     let refused = replay(&states[2], &full).expect_err("a full mailbox takes nothing");
///     assert_eq!(refused.step, 1);
/// }
/// ```
///
/// # Errors
///
/// A [`Mismatch`] that names the first step no action of the model can take in the state it
/// reaches: the id is not an action, its `require!` is false there, or an input is missing,
/// extra or not one of the values the checker tries. It lists the steps that can happen there.
pub fn replay<State: Behavior + Clone>(
    start: &State,
    path: &[Move],
) -> Result<Vec<State>, Mismatch> {
    let mut states = vec![start.clone()];
    for (position, wanted) in path.iter().enumerate() {
        let here = states.last().map(Behavior::steps).unwrap_or_default();
        let Some(found) = here.iter().position(|step| wanted.matches(step)) else {
            let listed: Vec<String> = here.iter().map(describe).collect();
            let possible = if listed.is_empty() {
                "no action can happen here".to_owned()
            } else {
                format!("the steps that can happen here are {}", listed.join(", "))
            };
            return Err(Mismatch {
                step: position.saturating_add(1),
                reason: format!("`{wanted}` can't happen in this state: {possible}"),
            });
        };
        if let Some(step) = here.into_iter().nth(found) {
            states.push(step.state);
        }
    }
    Ok(states)
}

fn describe<State>(step: &Step<State>) -> String {
    let mut text = Move::new(step.action);
    for (name, value) in &step.input {
        text = text.with(name, value);
    }
    format!("`{text}`")
}
