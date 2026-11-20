use earnest::model::{Domain, Model, Set, exists, forall, model, require, view};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Kind {
    Model,
    Unreadable,
    SkippedTest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Skipped {
    Listed,
    Removed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Selection {
    Everything,
    ModelOfFirst,
    NoSuchModel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Phase {
    Searching,
    Exhausted,
    StateLimit,
    TimeLimit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Stop {
    Exhausted,
    StateLimit,
    TimeLimit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Found {
    Broken,
    RuleFaults,
    Stuck,
    CutShort,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum File {
    First,
    Second,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Word {
    Pending,
    Pass,
    Fail,
    Unfinished,
    Unreadable,
    Skipped,
    NotChecked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Verdict {
    Pending,
    Holds,
    Broken,
    Unknown,
    Unasked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Exit {
    Pending,
    Zero,
    One,
    Two,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Outcome {
    selection: Selection,
    first: Kind,
    second: Kind,
    #[initial(Phase::Searching)]
    phase_first: Phase,
    #[initial(Set::of([]))]
    found_first: Set<Found>,
    #[initial(Phase::Searching)]
    phase_second: Phase,
    #[initial(Set::of([]))]
    found_second: Set<Found>,
    #[initial(Word::Pending)]
    word_first: Word,
    #[initial(Verdict::Pending)]
    verdict_first: Verdict,
    #[initial(Word::Pending)]
    word_second: Word,
    #[initial(Verdict::Pending)]
    verdict_second: Verdict,
    #[initial(Exit::Pending)]
    exit: Exit,
    #[initial(Exit::Pending)]
    exit_without_skipped: Exit,
    #[initial(false)]
    error_names_skipped: bool,
    #[initial(false)]
    error_names_unreadable: bool,
}

fn matched(selection: Selection, first: Kind) -> bool {
    selection == Selection::Everything
        || (selection == Selection::ModelOfFirst
            && (first == Kind::Model || first == Kind::Unreadable))
}

fn selected(selection: Selection, file: File) -> bool {
    selection == Selection::Everything || file == File::First
}

fn kind_of(outcome: &Outcome, file: File) -> Kind {
    if file == File::First {
        outcome.first
    } else {
        outcome.second
    }
}

fn phase_of(outcome: &Outcome, file: File) -> Phase {
    if file == File::First {
        outcome.phase_first
    } else {
        outcome.phase_second
    }
}

fn found_of(outcome: &Outcome, file: File) -> Set<Found> {
    if file == File::First {
        outcome.found_first.clone()
    } else {
        outcome.found_second.clone()
    }
}

fn searched(outcome: &Outcome, file: File) -> bool {
    matched(outcome.selection, outcome.first)
        && selected(outcome.selection, file)
        && kind_of(outcome, file) == Kind::Model
}

fn names_a_declared_model(selection: Selection, first: Kind) -> bool {
    matched(selection, first)
}

fn asked_for(selection: Selection, first: Kind, kind: Kind, file: File) -> bool {
    names_a_declared_model(selection, first)
        && kind == Kind::Model
        && (selection == Selection::Everything || file == File::First)
}

fn wrong(found: &Set<Found>) -> bool {
    !found.is_empty()
}

fn status(outcome: &Outcome, file: File) -> Word {
    let kind = kind_of(outcome, file);
    if kind == Kind::Unreadable {
        Word::Unreadable
    } else if kind == Kind::SkippedTest {
        Word::Skipped
    } else if !searched(outcome, file) {
        Word::NotChecked
    } else if wrong(&found_of(outcome, file)) {
        Word::Fail
    } else if phase_of(outcome, file) == Phase::Exhausted {
        Word::Pass
    } else {
        Word::Unfinished
    }
}

fn verdict(outcome: &Outcome, file: File) -> Verdict {
    let found = found_of(outcome, file);
    if !searched(outcome, file) {
        Verdict::Unasked
    } else if found.has(Found::Broken) {
        Verdict::Broken
    } else if phase_of(outcome, file) == Phase::Exhausted
        && !found.has(Found::RuleFaults)
        && !found.has(Found::CutShort)
    {
        Verdict::Holds
    } else {
        Verdict::Unknown
    }
}

fn not_passed(word: Word) -> bool {
    word == Word::Fail || word == Word::Unfinished || word == Word::Unreadable
}

fn counted(word: Word, skipped: Skipped) -> Word {
    if skipped == Skipped::Removed && word == Word::Skipped {
        Word::NotChecked
    } else {
        word
    }
}

fn exit_code(outcome: &Outcome, skipped: Skipped) -> Exit {
    let first = counted(status(outcome, File::First), skipped);
    let second = counted(status(outcome, File::Second), skipped);
    if !matched(outcome.selection, outcome.first) {
        Exit::Two
    } else if not_passed(first) || not_passed(second) {
        Exit::One
    } else {
        Exit::Zero
    }
}

#[model]
impl Outcome {
    fn searching(&self, file: File) -> bool {
        searched(self, file) && phase_of(self, file) == Phase::Searching
    }

    fn settled(&self) -> bool {
        !self.searching(File::First) && !self.searching(File::Second)
    }

    fn phase(&self, file: File) -> Phase {
        phase_of(self, file)
    }

    fn found(&self, file: File) -> Set<Found> {
        found_of(self, file)
    }

    fn word(&self, file: File) -> Word {
        if file == File::First {
            self.word_first
        } else {
            self.word_second
        }
    }

    fn verdict(&self, file: File) -> Verdict {
        if file == File::First {
            self.verdict_first
        } else {
            self.verdict_second
        }
    }

    fn passed(&self, file: File) -> bool {
        self.phase(file) == Phase::Exhausted && self.found(file).is_empty()
    }

    fn unreadable(&self) -> bool {
        self.first == Kind::Unreadable || self.second == Kind::Unreadable
    }

    fn asked(&self, file: File) -> bool {
        if file == File::First {
            asked_for(self.selection, self.first, self.first, File::First)
        } else {
            asked_for(self.selection, self.first, self.second, File::Second)
        }
    }

    fn reported(&self) -> bool {
        self.exit != Exit::Pending
    }

    #[action("the search of a model finds something")]
    fn find(&self, file: File, found: Found) -> Option<Self> {
        require!(self.searching(file) && !self.reported());
        if file == File::First {
            require!(!self.found_first.has(found));
            return Some(Self {
                found_first: self.found_first.add(found),
                ..self.clone()
            });
        }
        require!(!self.found_second.has(found));
        Some(Self {
            found_second: self.found_second.add(found),
            ..self.clone()
        })
    }

    #[action("the search of a model ends, at the last state or at a limit")]
    fn stop(&self, file: File, how: Stop) -> Option<Self> {
        require!(self.searching(file) && !self.reported());
        let phase = if how == Stop::Exhausted {
            Phase::Exhausted
        } else if how == Stop::StateLimit {
            Phase::StateLimit
        } else {
            Phase::TimeLimit
        };
        if file == File::First {
            return Some(Self {
                phase_first: phase,
                ..self.clone()
            });
        }
        Some(Self {
            phase_second: phase,
            ..self.clone()
        })
    }

    #[action("earnest prints the results and exits")]
    fn report(&self) -> Option<Self> {
        require!(self.settled() && !self.reported());
        let printed = matched(self.selection, self.first);
        Some(Self {
            word_first: if printed {
                status(self, File::First)
            } else {
                Word::Pending
            },
            verdict_first: verdict(self, File::First),
            word_second: if printed {
                status(self, File::Second)
            } else {
                Word::Pending
            },
            verdict_second: verdict(self, File::Second),
            exit: exit_code(self, Skipped::Listed),
            exit_without_skipped: exit_code(self, Skipped::Removed),
            error_names_skipped: !printed && self.first == Kind::SkippedTest,
            error_names_unreadable: !printed && self.unreadable(),
            ..self.clone()
        })
    }

    #[rule("A limit that stops a model with something broken found is a fail")]
    fn limit_with_broken_is_fail(&self) -> bool {
        forall::<File>(|file| {
            !self.reported()
                || !self.asked(file)
                || self.found(file).is_empty()
                || self.word(file) == Word::Fail
        })
    }

    #[rule("A model that stops at a limit with nothing broken found is unfinished")]
    fn limit_without_broken_is_unfinished(&self) -> bool {
        forall::<File>(|file| {
            !self.reported()
                || !self.asked(file)
                || !self.found(file).is_empty()
                || self.phase(file) == Phase::Exhausted
                || self.word(file) == Word::Unfinished
        })
    }

    #[rule("A model passes only when every reachable state was visited and nothing is wrong")]
    fn pass_only_when_whole(&self) -> bool {
        forall::<File>(|file| self.word(file) != Word::Pass || self.passed(file))
    }

    #[rule(
        "A rule holds only when the search finished, the rule had a value in every state and no state was left unjudged or unreached"
    )]
    fn holds_only_when_judged(&self) -> bool {
        forall::<File>(|file| {
            self.verdict(file) != Verdict::Holds
                || (self.phase(file) == Phase::Exhausted
                    && !self.found(file).has(Found::RuleFaults)
                    && !self.found(file).has(Found::CutShort))
        })
    }

    #[rule("A rule is broken exactly when a state the search counted broke it")]
    fn broken_exactly_when_seen(&self) -> bool {
        forall::<File>(|file| {
            !self.reported()
                || !self.asked(file)
                || (self.verdict(file) == Verdict::Broken) == self.found(file).has(Found::Broken)
        })
    }

    #[rule("Exit 0 only when every model passed and no file was unreadable")]
    fn exit_zero_only_when_all_passed(&self) -> bool {
        self.exit != Exit::Zero
            || (!self.unreadable() && forall::<File>(|file| !self.asked(file) || self.passed(file)))
    }

    #[rule("Exit 1 when a checked model did not pass or a file was unreadable")]
    fn exit_one_when_something_is_wrong(&self) -> bool {
        !self.reported()
            || !names_a_declared_model(self.selection, self.first)
            || !(self.unreadable() || exists::<File>(|file| self.asked(file) && !self.passed(file)))
            || self.exit == Exit::One
    }

    #[rule("Exit 2 exactly when --model names no model that was read")]
    fn exit_two_when_nothing_matched(&self) -> bool {
        !self.reported()
            || (self.exit == Exit::Two) != names_a_declared_model(self.selection, self.first)
    }

    #[rule("A skipped test file never changes the exit code")]
    fn skipped_never_changes_the_exit(&self) -> bool {
        self.exit == self.exit_without_skipped
    }

    #[rule(
        "A --model that names a model only a skipped test file holds is exit 2, and the error names the file"
    )]
    fn named_skipped_file_is_reported(&self) -> bool {
        !self.reported()
            || self.selection != Selection::ModelOfFirst
            || self.first != Kind::SkippedTest
            || (self.exit == Exit::Two && self.error_names_skipped)
    }

    #[rule(
        "A --model that names a model an unreadable file holds is exit 1, and the file is reported as unreadable"
    )]
    fn named_unreadable_file_is_reported(&self) -> bool {
        !self.reported()
            || self.selection != Selection::ModelOfFirst
            || self.first != Kind::Unreadable
            || (self.exit == Exit::One && self.word_first == Word::Unreadable)
    }

    #[rule(
        "A --model that names no declared model is exit 2, and the error names every unreadable file, which may hold it"
    )]
    fn unreadable_files_are_named_with_no_such_model(&self) -> bool {
        self.exit != Exit::Two || !self.unreadable() || self.error_names_unreadable
    }

    #[end("earnest has printed the results and exited")]
    fn exited(&self) -> bool {
        self.reported()
    }
}

#[view(
    title = "Run outcome",
    step(find = "found", stop = "search ends", report = "report")
)]
impl Outcome {
    #[state]
    fn state(&self) -> String {
        format!("exit {:?}", self.exit)
    }

    #[group]
    fn group(&self) -> String {
        format!("{:?}", self.selection)
    }
}

#[cfg(test)]
mod tests {
    use super::Outcome;
    use crate::walk::{Walk, Walked, walk};
    use earnest::model::{Behavior, Model};

    #[test]
    fn rustc_reaches_the_states_the_checker_counts() {
        let walked = walk(&Walk {
            starts: Outcome::starts(),
            next: |state| state.steps().into_iter().map(|step| step.state).collect(),
            broken: |state| {
                state
                    .rules()
                    .into_iter()
                    .filter(|rule| !rule.holds)
                    .map(|rule| rule.id)
                    .collect()
            },
            ends: |state| state.ends().iter().any(|end| end.holds),
        });
        assert_eq!(
            walked,
            Walked {
                states: 7222,
                broken: Vec::new(),
                stuck: 0,
            },
            "the model run as Rust agrees with the count in README.md"
        );
    }
}
