use crate::expected::{
    Expected, MODELS, Shortest, TANK_AFTER, TANK_AFTER_RULES, TANK_BEFORE, TANK_BEFORE_RULES,
};
use crate::{drafts, search, steps};
use earnest_check::{FindingKind, Limits, Outcome, Status, Verdict};

fn verdicts(outcome: &Outcome, wanted: Verdict) -> Vec<&str> {
    let mut ids: Vec<&str> = outcome
        .rules
        .iter()
        .filter(|rule| rule.verdict == wanted)
        .map(|rule| rule.id.as_str())
        .collect();
    ids.sort_unstable();
    ids
}

fn sorted(ids: impl Iterator<Item = &'static str>) -> Vec<&'static str> {
    let mut found: Vec<&str> = ids.collect();
    found.sort_unstable();
    found
}

fn assert_broken_paths(expected: &Expected, outcome: &Outcome) {
    let name = expected.model;
    for (id, shortest) in expected.broken {
        let rule = outcome
            .rules
            .iter()
            .find(|rule| rule.id == *id)
            .unwrap_or_else(|| panic!("{name} has no rule {id}"));
        let path = rule
            .path
            .as_ref()
            .unwrap_or_else(|| panic!("{name}.{id} has no path"));
        match shortest {
            Shortest::Steps(wanted) => {
                assert_eq!(steps(path), *wanted, "{name}.{id}: the shortest path");
            }
            Shortest::Length(wanted) => {
                assert_eq!(path.steps.len(), *wanted, "{name}.{id}: the path's length");
            }
        }
        for (field, value) in expected.starts_with {
            let start = path
                .start
                .iter()
                .find(|named| named.name == *field)
                .unwrap_or_else(|| panic!("{name}.{id}: the start has no field {field}"));
            assert_eq!(start.value.text, *value, "{name}.{id}: the start's {field}");
        }
    }
}

fn assert_gives(expected: &Expected) {
    let name = expected.model;
    let outcome = search(expected.file, name, &Limits::default());
    assert_eq!(outcome.states, expected.states, "{name}: distinct states");
    assert_eq!(outcome.reason, None, "{name}: the search finished");
    assert_eq!(
        verdicts(&outcome, Verdict::Holds),
        sorted(expected.holds.iter().copied()),
        "{name}: rules that hold"
    );
    assert_eq!(
        verdicts(&outcome, Verdict::Broken),
        sorted(expected.broken.iter().map(|(id, _)| *id)),
        "{name}: rules that are broken"
    );
    assert_eq!(
        verdicts(&outcome, Verdict::Unknown),
        sorted(expected.unknown.iter().copied()),
        "{name}: rules that are unknown"
    );
    assert_broken_paths(expected, &outcome);
    let findings: Vec<(FindingKind, Vec<String>)> = outcome
        .findings
        .iter()
        .map(|finding| (finding.kind, steps(&finding.path)))
        .collect();
    let wanted: Vec<(FindingKind, Vec<String>)> = expected
        .findings
        .iter()
        .map(|(kind, path)| (*kind, path.iter().map(ToString::to_string).collect()))
        .collect();
    assert_eq!(findings, wanted, "{name}: findings and the path to each");
    let passes = expected.broken.is_empty() && expected.findings.is_empty();
    let status = if passes { Status::Pass } else { Status::Fail };
    assert_eq!(outcome.status, status, "{name}: status");
}

#[test]
fn every_model_gives_the_states_verdicts_and_findings_of_the_table() {
    assert_eq!(MODELS.len(), 21, "the table has 21 models");
    for expected in MODELS {
        assert_gives(expected);
    }
}

#[test]
fn the_table_counts_eight_passing_models_and_thirteen_failing() {
    let passing = MODELS
        .iter()
        .filter(|expected| expected.broken.is_empty() && expected.findings.is_empty())
        .count();
    assert_eq!(passing, 8, "models that pass");
    assert_eq!(MODELS.len() - passing, 13, "models that fail");
}

#[test]
fn a_file_gives_its_models_in_file_order() {
    let names: Vec<String> = drafts("switches.model.ts")
        .into_iter()
        .map(|draft| draft.name)
        .collect();
    assert_eq!(names, ["Porch", "Hall"], "the order in the file");
    for expected in MODELS {
        let found = drafts(expected.file);
        assert!(
            found.iter().any(|draft| draft.name == expected.model),
            "{} declares {}",
            expected.file,
            expected.model
        );
        let wanted = MODELS
            .iter()
            .filter(|other| other.file == expected.file)
            .count();
        assert_eq!(found.len(), wanted, "{}: models in the file", expected.file);
    }
}

#[test]
fn rules_come_in_file_order() {
    let ids = |fixture: &str| -> Vec<String> {
        drafts(fixture)
            .into_iter()
            .flat_map(|draft| draft.rules)
            .map(|rule| rule.id)
            .collect()
    };
    assert_eq!(
        ids("party.model.ts"),
        ["algebra", "members", "counted", "alone"],
        "the party's rules"
    );
    assert_eq!(ids("oven.model.ts"), ["safe", "closed"], "the oven's rules");
}

#[test]
fn the_tank_gives_its_table_before_and_after_the_change() {
    assert_gives(&TANK_BEFORE);
    assert_gives(&TANK_AFTER);
}

#[test]
fn the_tank_rules_keep_their_sentence_and_source_text() {
    for (fixture, wanted) in [
        (TANK_BEFORE.file, TANK_BEFORE_RULES),
        (TANK_AFTER.file, TANK_AFTER_RULES),
    ] {
        let found = drafts(fixture);
        let tank = found.first().expect("one model");
        let rules: Vec<(&str, &str, &str)> = tank
            .rules
            .iter()
            .map(|rule| (rule.id.as_str(), rule.says.as_str(), rule.text.as_str()))
            .collect();
        assert_eq!(rules, wanted, "{fixture}: id, sentence and condition");
    }
}

#[test]
fn the_cart_view_is_read_whole() {
    let found = drafts("cart.model.ts");
    let view = &found.first().expect("one model").view;
    assert_eq!(view.title.as_deref(), Some("Add to cart focus"), "title");
    assert!(view.state.is_some(), "a state label");
    assert!(view.group.is_some(), "a group label");
    assert_eq!(
        view.steps,
        [("press".to_owned(), "press Add".to_owned())],
        "arrow labels"
    );
    assert_eq!(
        view.colors,
        [("broken".to_owned(), "#c62828".to_owned())],
        "colors"
    );
}
