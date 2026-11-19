//! A place in a model file and a problem at that place.

use serde::Serialize;

/// Where something sits in a model file, as an editor shows it.
///
/// Lines and columns count from 1. A place that was never set is line 0, column 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub struct Place {
    /// Counted from 1.
    pub line: u32,
    /// Counted from 1 along the line.
    pub column: u32,
}

/// One reason a model file can't be checked.
///
/// The fields serialize under these names into the `unreadable` list of the JSON output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Problem {
    /// The path of the model file, as the user gave it.
    pub file: String,
    /// Where the feature starts, counted from 1.
    pub line: u32,
    /// Where the feature starts on its line, counted from 1.
    pub column: u32,
    /// What the file uses that the checker refuses, short enough to print inline.
    pub feature: String,
    /// Why it is refused, or what to write in its place.
    pub message: String,
}

impl Problem {
    /// Reports `feature` at `place` in `file`. `message` says why it is refused.
    #[must_use]
    pub fn new(file: &str, place: Place, feature: &str, message: &str) -> Self {
        Self {
            file: file.to_owned(),
            line: place.line,
            column: place.column,
            feature: feature.to_owned(),
            message: message.to_owned(),
        }
    }

    /// Where the feature starts, for comparing or sorting problems by position.
    #[must_use]
    pub fn place(&self) -> Place {
        Place {
            line: self.line,
            column: self.column,
        }
    }
}
