use crate::build::{Sketch, int, two};
use earnest_ir::{Op, Order};

fn two_rules() -> Sketch {
    let mut sketch = Sketch::new("Pair");
    let count = sketch.number("count", 0, 3, 0);
    sketch.rule("low", two(Op::Order(Order::Less), &count, int(2)));
    sketch.rule("high", two(Op::Order(Order::LessOrEqual), &count, int(3)));
    sketch
}

#[test]
fn the_invariants_are_the_rules_in_order() {
    let files = two_rules().written();
    assert_eq!(
        files.invariants(),
        ["Rule_low", "Rule_high"],
        "one invariant per rule"
    );
}

#[test]
fn the_rules_are_listed_with_their_invariants_in_order() {
    let files = two_rules().written();
    let listed: Vec<(&str, &str, Option<&str>)> = files
        .rules
        .iter()
        .map(|each| {
            (
                each.rule.as_str(),
                each.holds.as_str(),
                each.judged.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        listed,
        [("low", "Rule_low", None), ("high", "Rule_high", None)],
        "a comparison of a number always has a value"
    );
}

#[test]
fn a_model_without_rules_has_no_invariant() {
    let files = Sketch::new("Bare").written();
    assert!(files.invariants().is_empty(), "no rule, no invariant");
}

#[test]
fn a_configuration_for_one_invariant_drops_the_others_and_keeps_the_rest() {
    let files = two_rules().written();
    assert_eq!(
        files.config_checking(&["Rule_high"]),
        "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\nINVARIANT Rule_high\n",
        "only the kept invariant is checked"
    );
}

#[test]
fn a_configuration_for_no_invariant_only_counts_the_states() {
    let files = two_rules().written();
    assert_eq!(
        files.config_checking(&[]),
        "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n",
        "no invariant is checked"
    );
}

#[test]
fn a_configuration_for_every_invariant_is_the_whole_configuration() {
    let files = two_rules().written();
    assert_eq!(
        files.config_checking(&files.invariants()),
        files.config,
        "keeping every invariant changes nothing"
    );
    assert_eq!(
        files.config_checking(&["Rule_missing"]),
        files.config_checking(&[]),
        "a name the configuration lacks is ignored"
    );
}
