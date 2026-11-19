use crate::build::{
    FILE, HERE, action, after, binary, bind, boolean, choose, draft, equal, field, first_state,
    here, int, lowered, name, no, null, one_of, optional, placed, prop, range, record, refused,
    rule, spread, starting, text, the_problem, wrap,
};
use earnest_ast::{ActionDraft, Binary, Draft, EndDraft, Shape, Term};
use earnest_diagnostics::{Place, Problem};
use earnest_ir::{Frame, Model, truth};
use earnest_value::Value;

fn press() -> Shape {
    one_of(&["keyboard", "virtual", "mouse"])
}

fn cart() -> Draft {
    draft(vec![
        (
            "cta",
            starting(one_of(&["Add", "Adding", "InCart"]), text("Add")),
        ),
        ("pressBy", starting(optional(press()), null())),
        ("keyboardAdd", starting(boolean(), no())),
        ("n", starting(range(0, 3), int(0))),
    ])
}

fn press_action() -> ActionDraft {
    let by_mouse = equal(name("by"), text("mouse"));
    let guard = binary(
        Binary::And,
        equal(field("cta"), text("Add")),
        binary(Binary::Or, by_mouse, equal(field("n"), int(0))),
    );
    let change = record(vec![
        ("cta", text("Adding")),
        ("pressBy", name("by")),
        ("keyboardAdd", equal(name("by"), text("keyboard"))),
    ]);
    let mut pressed = action("press", Some(guard), change);
    pressed.inputs.push(("by".to_owned(), press()));
    pressed
}

fn with_action(added: ActionDraft) -> Draft {
    let mut source = cart();
    source.actions.push(added);
    source
}

fn changing(change: Term) -> Draft {
    with_action(action("change", None, change))
}

fn shown(model: &Model, state: &[Value]) -> Vec<String> {
    state
        .iter()
        .map(|value| model.symbols.show(value))
        .collect()
}

fn text_value(model: &Model, known: &str) -> Value {
    Value::Str(model.symbols.find(known).expect("the model uses this text"))
}

fn enabled(model: &Model, inputs: &[Value]) -> bool {
    let state = first_state(model);
    let taken = &model.actions[0];
    let mut frame = Frame::new(&state, taken.slots);
    for (each, value) in taken.inputs.iter().zip(inputs) {
        frame.put(each.slot, value.clone());
    }
    truth(&taken.guard, &mut frame).expect("the guard has a value in the start state")
}

#[test]
fn an_action_keeps_its_id_sentence_line_and_inputs() {
    let model = lowered(&with_action(press_action()));
    let pressed = &model.actions[0];
    assert_eq!(pressed.id, "press");
    assert_eq!(pressed.label, "press happens");
    assert_eq!(pressed.line, 20);
    assert_eq!(pressed.inputs.len(), 1);
    assert_eq!(pressed.inputs[0].name, "by");
    assert_eq!(
        shown(&model, &pressed.inputs[0].values),
        ["\"keyboard\"", "\"virtual\"", "\"mouse\""]
    );
    assert!(pressed.slots > pressed.inputs[0].slot);
}

#[test]
fn an_action_updates_only_the_fields_its_result_names() {
    let model = lowered(&with_action(press_action()));
    let updated: Vec<bool> = model.actions[0]
        .updates
        .iter()
        .map(Option::is_some)
        .collect();
    assert_eq!(updated, [true, true, true, false]);
}

#[test]
fn an_action_s_result_is_computed_from_the_state_and_the_input() {
    let model = lowered(&with_action(press_action()));
    let by_keyboard = after(&model, 0, &[text_value(&model, "keyboard")]);
    assert_eq!(
        shown(&model, &by_keyboard),
        ["\"Adding\"", "\"keyboard\"", "true", "0"]
    );
    let by_mouse = after(&model, 0, &[text_value(&model, "mouse")]);
    assert_eq!(
        shown(&model, &by_mouse),
        ["\"Adding\"", "\"mouse\"", "false", "0"]
    );
}

#[test]
fn a_guard_reads_the_state_and_the_input() {
    let mut source = with_action(press_action());
    let at_zero = lowered(&source);
    assert!(enabled(&at_zero, &[text_value(&at_zero, "virtual")]));
    source.fields[3].shape = starting(range(0, 3), int(1));
    let model = lowered(&source);
    assert!(!enabled(&model, &[text_value(&model, "virtual")]));
    assert!(enabled(&model, &[text_value(&model, "mouse")]));
}

#[test]
fn an_action_without_a_guard_can_always_happen() {
    let model = lowered(&changing(record(vec![("n", int(1))])));
    assert!(enabled(&model, &[]));
}

#[test]
fn an_input_is_also_read_through_the_name_of_the_whole_input() {
    let mut pressed = action(
        "press",
        Some(equal(prop(name("input"), "by"), text("mouse"))),
        record(vec![("pressBy", prop(name("input"), "by"))]),
    );
    pressed.inputs.push(("by".to_owned(), press()));
    pressed.input_name = Some("input".to_owned());
    let model = lowered(&with_action(pressed));
    assert!(enabled(&model, &[text_value(&model, "mouse")]));
    assert!(!enabled(&model, &[text_value(&model, "virtual")]));
    let state = after(&model, 0, &[text_value(&model, "mouse")]);
    assert_eq!(shown(&model, &state)[1], "\"mouse\"");
}

#[test]
fn a_rust_result_wraps_the_changed_fields_and_spreads_the_state() {
    let result = wrap(spread(Some(name("s")), vec![("n", int(2))]));
    let model = lowered(&changing(result));
    let state = after(&model, 0, &[]);
    assert_eq!(shown(&model, &state), ["\"Add\"", "null", "false", "2"]);
}

#[test]
fn a_binding_before_the_result_is_visible_in_it() {
    let next = binary(Binary::Add, field("n"), int(1));
    let result = bind("next", next, record(vec![("n", name("next"))]));
    let model = lowered(&changing(result));
    assert_eq!(shown(&model, &after(&model, 0, &[]))[3], "1");
}

#[test]
fn an_early_return_changes_only_its_own_fields() {
    let early = record(vec![("n", int(2))]);
    let late = record(vec![("cta", text("InCart"))]);
    let at_zero = choose(equal(field("n"), int(0)), early.clone(), late.clone());
    let model = lowered(&changing(at_zero));
    assert_eq!(
        shown(&model, &after(&model, 0, &[])),
        ["\"Add\"", "null", "false", "2"]
    );
    let not_at_zero = choose(equal(field("n"), int(1)), early, late);
    let other = lowered(&changing(not_at_zero));
    assert_eq!(
        shown(&other, &after(&other, 0, &[])),
        ["\"InCart\"", "null", "false", "0"]
    );
}

#[test]
fn a_plain_value_or_null_can_be_given_to_an_optional_field() {
    let set = lowered(&changing(record(vec![("pressBy", text("mouse"))])));
    assert_eq!(shown(&set, &after(&set, 0, &[]))[1], "\"mouse\"");
    let cleared = lowered(&changing(record(vec![("pressBy", null())])));
    assert_eq!(shown(&cleared, &after(&cleared, 0, &[]))[1], "null");
}

#[test]
fn a_result_that_is_not_a_record_is_refused() {
    let problem = the_problem(&changing(here(int(1))));
    assert_eq!(problem.file, FILE);
    assert_eq!(problem.place(), HERE);
    assert_eq!(
        problem.feature,
        "an action result that is not a record of fields"
    );
}

#[test]
fn a_result_that_names_a_field_the_model_lacks_is_refused() {
    let problem = the_problem(&changing(record(vec![("total", here(int(1)))])));
    assert_eq!(problem.place(), HERE);
    assert_eq!(problem.feature, "the state field `total`");
}

#[test]
fn a_result_of_the_wrong_kind_for_its_field_is_refused() {
    let problem = the_problem(&changing(record(vec![("n", here(text("one")))])));
    assert_eq!(problem.place(), HERE);
    assert_eq!(problem.feature, "a text value where a number is needed");
}

#[test]
fn a_result_that_spreads_something_other_than_the_state_is_refused() {
    let result = spread(Some(here(field("n"))), vec![("n", int(1))]);
    let problem = the_problem(&changing(result));
    assert_eq!(problem.place(), HERE);
    assert_eq!(problem.feature, "a spread in an action result");
}

#[test]
fn an_input_the_action_does_not_declare_is_refused() {
    let mut pressed = action(
        "press",
        None,
        record(vec![("pressBy", here(prop(name("input"), "who")))]),
    );
    pressed.inputs.push(("by".to_owned(), press()));
    pressed.input_name = Some("input".to_owned());
    let problem = the_problem(&with_action(pressed));
    assert_eq!(problem.place(), HERE);
    assert_eq!(problem.feature, "the input `who`");
}

#[test]
fn an_input_with_more_than_a_million_values_is_refused_at_its_declaration() {
    let mut counted = action("count", None, record(vec![("n", int(1))]));
    let mut wide = range(0, 1_000_000);
    wide.place = HERE;
    counted.inputs.push(("amount".to_owned(), wide));
    let problem = the_problem(&with_action(counted));
    assert_eq!(problem.place(), HERE);
    assert_eq!(
        problem.feature,
        "a declaration with more than a million values"
    );
}

#[test]
fn a_rule_keeps_its_id_sentence_line_and_source_text() {
    let mut source = cart();
    let mut stays = rule("staysSmall", binary(Binary::Less, field("n"), int(3)));
    stays.text = "s.n < 3".to_owned();
    source.rules.push(stays);
    let model = lowered(&source);
    assert_eq!(model.rules[0].id, "staysSmall");
    assert_eq!(model.rules[0].says, "staysSmall holds");
    assert_eq!(model.rules[0].line, 30);
    assert_eq!(model.rules[0].text, "s.n < 3");
}

#[test]
fn an_end_keeps_its_id_and_sentence_and_its_condition_reads_the_state() {
    let mut source = cart();
    source.ends.push(EndDraft {
        id: "inCart".to_owned(),
        says: "The item is in the cart".to_owned(),
        place: Place {
            line: 40,
            column: 1,
        },
        state_name: Some("s".to_owned()),
        when: equal(field("cta"), text("InCart")),
    });
    let model = lowered(&source);
    let end = &model.ends[0];
    assert_eq!(end.id, "inCart");
    assert_eq!(end.says, "The item is in the cart");
    let state = first_state(&model);
    assert_eq!(
        truth(&end.when, &mut Frame::new(&state, end.slots)),
        Ok(false)
    );
}

#[test]
fn every_problem_in_a_draft_is_reported_and_no_model_is_given() {
    let mut source = cart();
    let first = Place {
        line: 31,
        column: 5,
    };
    let second = Place {
        line: 32,
        column: 9,
    };
    source
        .rules
        .push(rule("one", placed(name("missing"), first)));
    source
        .rules
        .push(rule("two", placed(name("absent"), second)));
    let problems = refused(&source);
    let places: Vec<Place> = problems.iter().map(Problem::place).collect();
    assert_eq!(places, [first, second]);
    assert!(problems.iter().all(|problem| problem.file == FILE));
}
