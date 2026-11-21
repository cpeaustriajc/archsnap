//! The fixed benchmark models and the number of states each must have, derived by hand in
//! `tasks/benchmark/README.md`.

use std::path::{Path, PathBuf};

pub(crate) const FOLDER: &str = "tasks/benchmark/models";

pub(crate) const LARGEST: &str = "grid_10m";
pub(crate) const R12_LARGE: &str = "grid_1m";
pub(crate) const LARGEST_STATE_LIMIT: u64 = 20_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Language {
    TypeScript,
    Rust,
}

impl Language {
    pub(crate) const fn extension(self) -> &'static str {
        match self {
            Self::TypeScript => "model.ts",
            Self::Rust => "rs",
        }
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::TypeScript => "TypeScript",
            Self::Rust => "Rust",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Scaled {
    pub(crate) file: &'static str,
    pub(crate) model: &'static str,
}

pub(crate) const SCALED: [Scaled; 6] = [
    Scaled {
        file: "grid_1k",
        model: "Grid1k",
    },
    Scaled {
        file: "ledger",
        model: "Ledger",
    },
    Scaled {
        file: "wide",
        model: "Wide",
    },
    Scaled {
        file: "grid_100k",
        model: "Grid100k",
    },
    Scaled {
        file: "grid_1m",
        model: "Grid1m",
    },
    Scaled {
        file: "grid_10m",
        model: "Grid10m",
    },
];

pub(crate) const MIXED_SMALL: [&str; 10] = [
    "counter100",
    "traffic",
    "plane",
    "basket",
    "mailbox4",
    "trail3",
    "shelf",
    "seats",
    "votes",
    "pairs",
];

pub(crate) const MIXED_LARGE: [&str; 2] = ["ledger", "wide"];

pub(crate) const EXPECTED_STATES: [(&str, u64); 16] = [
    ("Grid1k", 1_000),
    ("Grid100k", 100_000),
    ("Grid1m", 1_000_000),
    ("Grid10m", 10_000_000),
    ("Ledger", 36_400),
    ("Wide", 10_000),
    ("Counter", 100),
    ("Traffic", 30),
    ("Plane", 400),
    ("Basket", 16),
    ("Mailbox", 31),
    ("Trail", 40),
    ("Shelf", 64),
    ("Seats", 18),
    ("Votes", 21),
    ("Pairs", 120),
];

pub(crate) fn expected_states(model: &str) -> Option<u64> {
    EXPECTED_STATES
        .iter()
        .find(|(name, _)| *name == model)
        .map(|(_, states)| *states)
}

pub(crate) fn state_limit(files: &[PathBuf]) -> Option<u64> {
    files
        .iter()
        .filter_map(|file| file.file_stem()?.to_str())
        .any(|stem| stem.trim_end_matches(".model") == LARGEST)
        .then_some(LARGEST_STATE_LIMIT)
}

pub(crate) fn scaled_path(repository: &Path, file: &str, language: Language) -> PathBuf {
    repository
        .join(FOLDER)
        .join(format!("{file}.{}", language.extension()))
}

pub(crate) fn mixed_path(repository: &Path, file: &str) -> PathBuf {
    if MIXED_LARGE.contains(&file) {
        return scaled_path(repository, file, Language::TypeScript);
    }
    repository
        .join(FOLDER)
        .join("mixed")
        .join(format!("{file}.{}", Language::TypeScript.extension()))
}

pub(crate) fn mixed_files(repository: &Path) -> Vec<PathBuf> {
    MIXED_SMALL
        .iter()
        .chain(MIXED_LARGE.iter())
        .map(|file| mixed_path(repository, file))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_largest_model_gets_a_higher_state_limit() {
        let repository = Path::new(".");
        for language in [Language::TypeScript, Language::Rust] {
            let largest = scaled_path(repository, LARGEST, language);
            let smaller = scaled_path(repository, R12_LARGE, language);
            assert_eq!(
                state_limit(std::slice::from_ref(&largest)),
                Some(LARGEST_STATE_LIMIT),
                "{} is raised",
                largest.display()
            );
            assert_eq!(
                state_limit(&[smaller.clone(), largest]),
                Some(LARGEST_STATE_LIMIT),
                "a set with the largest is raised"
            );
            assert_eq!(state_limit(&[smaller]), None, "grid_1m keeps the default");
        }
        assert!(
            expected_states("Grid10m").is_some_and(|states| states < LARGEST_STATE_LIMIT),
            "the limit is above the largest count"
        );
    }
}
