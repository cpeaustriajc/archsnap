use crate::build::{
    binary, draft, field, first_state, helper, helper_call, here, int, name, range, rule, starting,
    text,
};
use earnest_ast::{Binary, Draft};
use earnest_ir::{Frame, Model, Rule};
use earnest_semantic::lower_with_earlier;

fn tank(level: i64) -> Draft {
    draft(vec![("level", starting(range(0, 9), int(level)))])
}

fn capped(max: i64) -> Draft {
    let mut source = tank(3);
    source.constants.push(("MAX".to_owned(), int(max)));
    source.rules.push(rule(
        "capped",
        binary(Binary::LessOrEqual, field("level"), name("MAX")),
    ));
    source
}

fn in_the_start_state(model: &Model, rule: &Rule) -> bool {
    let state = first_state(model);
    rule.holds
        .truth(&mut Frame::new(&state, rule.slots))
        .expect("the rule has a value")
}

#[test]
fn an_earlier_rule_reads_the_constants_of_its_own_draft() {
    let (model, earlier) = lower_with_earlier(&capped(3), &capped(2)).expect("both are readable");
    let was = earlier[0].as_ref().expect("the earlier rule is readable");
    assert!(
        in_the_start_state(&model, &model.rules[0]),
        "level 3 is at most the new MAX of 3"
    );
    assert!(
        !in_the_start_state(&model, was),
        "level 3 is above the earlier MAX of 2"
    );
}

#[test]
fn an_earlier_rule_calls_the_helpers_of_its_own_draft() {
    let allowed = |most: i64| {
        let mut source = tank(3);
        let body = binary(Binary::LessOrEqual, name("level"), int(most));
        source.helpers.push(helper("allowed", &["level"], body));
        source.rules.push(rule(
            "checked",
            helper_call("allowed", vec![field("level")]),
        ));
        source
    };
    let (model, earlier) = lower_with_earlier(&allowed(3), &allowed(2)).expect("readable");
    let was = earlier[0].as_ref().expect("the earlier rule is readable");
    assert!(in_the_start_state(&model, &model.rules[0]), "3 <= 3");
    assert!(!in_the_start_state(&model, was), "3 <= 2 is false");
}

#[test]
fn an_earlier_rule_reads_the_fields_of_the_new_draft_and_shares_its_texts() {
    let moody = |mood: &str| {
        let mut source = draft(vec![(
            "mood",
            starting(crate::build::one_of(&["calm", "cross"]), text("cross")),
        )]);
        source.rules.push(rule(
            "mood",
            binary(Binary::Equal, field("mood"), text(mood)),
        ));
        source
    };
    let (model, earlier) = lower_with_earlier(&moody("cross"), &moody("calm")).expect("readable");
    let was = earlier[0].as_ref().expect("the earlier rule is readable");
    assert!(
        in_the_start_state(&model, &model.rules[0]),
        "the mood is cross"
    );
    assert!(!in_the_start_state(&model, was), "the mood is not calm");
}

#[test]
fn an_earlier_rule_that_cant_be_read_against_the_new_fields_is_none_and_no_error() {
    let mut old = tank(0);
    old.rules.push(rule(
        "deep",
        binary(Binary::Less, here(field("depth")), int(2)),
    ));
    old.rules
        .push(rule("low", binary(Binary::Less, field("level"), int(2))));
    let (_, earlier) = lower_with_earlier(&tank(0), &old).expect("the new draft is readable");
    assert!(earlier[0].is_none(), "the new model has no field `depth`");
    assert!(earlier[1].is_some(), "the other rule is still read");
}

#[test]
fn the_problems_of_the_new_draft_are_the_error() {
    let mut new = tank(0);
    new.rules.push(rule(
        "deep",
        binary(Binary::Less, here(field("depth")), int(2)),
    ));
    let problems = lower_with_earlier(&new, &tank(0)).expect_err("the new draft is unreadable");
    assert_eq!(problems.len(), 1, "one problem: {problems:?}");
    assert_eq!(problems[0].feature, "the state field `depth`");
}
