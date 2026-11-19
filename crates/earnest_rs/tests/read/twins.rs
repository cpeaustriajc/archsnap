use crate::{actions, only_draft, search, shown};
use earnest_check::{FindingKind, Outcome, Status, Verdict};
use earnest_diagnostics::Place;

const COUNTER: &str = include_str!("../../../../tasks/conformance/models/counter.rs");
const DOWNLOAD: &str = include_str!("../../../../tasks/conformance/models/download.rs");
const MAILBOX: &str = include_str!("../../../../tasks/conformance/models/mailbox.rs");
const OVEN: &str = include_str!("../../../../tasks/conformance/models/oven.rs");
const TURNSTILE: &str = include_str!("../../../../tasks/conformance/models/turnstile.rs");

fn verdicts(outcome: &Outcome) -> Vec<(&str, Verdict)> {
    outcome
        .rules
        .iter()
        .map(|rule| (rule.id.as_str(), rule.verdict))
        .collect()
}

fn path_to<'a>(outcome: &'a Outcome, rule: &str) -> &'a earnest_check::Path {
    outcome
        .rules
        .iter()
        .find(|found| found.id == rule)
        .and_then(|found| found.path.as_ref())
        .expect("a broken rule has a path")
}

#[test]
fn the_counter_twin_passes_with_four_states() {
    let draft = only_draft("models/counter.rs", COUNTER);
    assert_eq!(draft.name, "Counter", "the model's name");
    assert_eq!(draft.file, "models/counter.rs", "the file as given");
    let ids: Vec<&str> = draft.rules.iter().map(|rule| rule.id.as_str()).collect();
    assert_eq!(ids, ["bounded", "even"], "rule ids are method names");
    assert_eq!(
        draft.rules[1].says, "Twice the count is even",
        "the sentence"
    );
    assert_eq!(
        draft.rules[1].text, "(self.count * 2) % 2 == 0",
        "a rule's text is its body"
    );
    assert_eq!(draft.ends[0].id, "full", "the end's id");
    assert_eq!(
        draft.ends[0].says, "The counter is full",
        "the end's sentence"
    );
    let outcome = search(&draft);
    assert_eq!(outcome.states, 4, "count goes 0, 1, 2, 3");
    assert_eq!(outcome.status, Status::Pass, "nothing is wrong");
    assert_eq!(
        verdicts(&outcome),
        [("bounded", Verdict::Holds), ("even", Verdict::Holds)],
        "both rules hold"
    );
    assert_eq!(
        outcome.findings,
        [],
        "3 is a marked end, so nothing is stuck"
    );
}

#[test]
fn the_download_twin_is_stuck_after_start_and_fail() {
    let draft = only_draft("models/download.rs", DOWNLOAD);
    let ids: Vec<&str> = draft
        .actions
        .iter()
        .map(|action| action.id.as_str())
        .collect();
    assert_eq!(ids, ["start", "finish", "fail"], "actions in file order");
    assert_eq!(
        draft.actions[2].label, "the connection drops",
        "the sentence"
    );
    let outcome = search(&draft);
    assert_eq!(outcome.states, 4, "idle, loading, done and failed");
    assert_eq!(
        outcome.status,
        Status::Fail,
        "a stuck state fails the model"
    );
    assert_eq!(verdicts(&outcome), [("whole", Verdict::Holds)], "the rule");
    assert_eq!(outcome.findings.len(), 1, "one finding");
    let stuck = &outcome.findings[0];
    assert_eq!(
        stuck.kind,
        FindingKind::Stuck,
        "the finding is a stuck state"
    );
    assert_eq!(actions(&stuck.path), ["start", "fail"], "the shortest path");
    assert_eq!(
        shown(&stuck.path),
        [
            "phase = \"Idle\", saved = false",
            "start[] -> phase = \"Loading\", saved = false",
            "fail[] -> phase = \"Failed\", saved = false",
        ],
        "each state on the way"
    );
}

#[test]
fn the_mailbox_twin_breaks_bills_after_two_bills() {
    let draft = only_draft("models/mailbox.rs", MAILBOX);
    assert_eq!(
        draft.rules[1].text,
        "(self.inbox.is_empty() || self.inbox.head().is_some()) && \
         self.inbox.every(|letter| self.inbox.includes(letter))",
        "a body on two lines is one line of text"
    );
    let outcome = search(&draft);
    assert_eq!(outcome.states, 7, "1 + 2 + 4 queues of at most two letters");
    assert_eq!(outcome.status, Status::Fail, "a rule is broken");
    assert_eq!(
        verdicts(&outcome),
        [("bills", Verdict::Broken), ("fronted", Verdict::Holds)],
        "one rule breaks"
    );
    assert_eq!(outcome.findings, [], "no other finding");
    assert_eq!(
        shown(path_to(&outcome, "bills")),
        [
            "inbox = []",
            "deliver[letter = \"Bill\"] -> inbox = [\"Bill\"]",
            "deliver[letter = \"Bill\"] -> inbox = [\"Bill\", \"Bill\"]",
        ],
        "a method parameter is the input of each step"
    );
}

#[test]
fn the_oven_twin_breaks_two_rules_with_a_path_for_each() {
    let draft = only_draft("models/oven.rs", OVEN);
    let outcome = search(&draft);
    assert_eq!(outcome.states, 4, "door times heat");
    assert_eq!(outcome.status, Status::Fail, "two rules are broken");
    assert_eq!(
        verdicts(&outcome),
        [("safe", Verdict::Broken), ("closed", Verdict::Broken)],
        "both rules break"
    );
    assert_eq!(outcome.findings, [], "no other finding");
    assert_eq!(
        actions(path_to(&outcome, "safe")),
        ["start", "open"],
        "the heat only starts with the door closed"
    );
    assert_eq!(
        shown(path_to(&outcome, "closed")),
        [
            "door = \"Closed\", heat = \"Off\"",
            "open[] -> door = \"Open\", heat = \"Off\"",
        ],
        "one step breaks `closed`"
    );
}

#[test]
fn the_turnstile_twin_breaks_locked_after_one_push() {
    let draft = only_draft("models/turnstile.rs", TURNSTILE);
    assert_eq!(
        draft.place,
        Place {
            line: 10,
            column: 5
        },
        "the model is declared at its `struct`"
    );
    assert_eq!(
        draft.rules[0].says, "The gate stays locked until the visitor pays",
        "the sentence comes from the `rule` attribute"
    );
    assert_eq!(
        draft.rules[0].text, "self.gate == Gate::Locked || self.paid",
        "the rule's text"
    );
    let outcome = search(&draft);
    assert_eq!(outcome.states, 4, "gate times paid");
    assert_eq!(outcome.status, Status::Fail, "the rule is broken");
    assert_eq!(
        verdicts(&outcome),
        [("locked", Verdict::Broken)],
        "the rule"
    );
    assert_eq!(outcome.findings, [], "no other finding");
    let path = path_to(&outcome, "locked");
    assert_eq!(
        shown(path),
        [
            "gate = \"Locked\", paid = false",
            "push[] -> gate = \"Open\", paid = false",
        ],
        "an enum value is known by its variant name"
    );
    let step = &path.steps[0];
    assert_eq!(step.label, "the visitor pushes", "the action's sentence");
    assert_eq!(step.changes.len(), 1, "one field of two changes");
    assert_eq!(step.changes[0].field, "gate", "the field that changes");
    assert_eq!(step.changes[0].before.text, "\"Locked\"", "before");
    assert_eq!(step.changes[0].after.text, "\"Open\"", "after");
}
