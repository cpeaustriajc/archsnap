use crate::build::{
    action, boolean, draft, equal, field, holds, int, lowered, name, one_of, owned_by, range,
    refusal, refused, rust, starting, struct_shape, take,
};
use earnest_ast::{Draft, Shape, Term};
use earnest_diagnostics::Problem;

fn line(owner: &str) -> Shape {
    struct_shape(Some(owner), vec![("qty", range(0, 2))])
}

fn built(owner: &str) -> Term {
    owned_by(Some(owner), None, vec![("qty", int(1))])
}

fn said(problem: &Problem) -> String {
    format!("{} {}", problem.feature, problem.message)
}

fn with_start(declared: &str, written: &str) -> Draft {
    rust(draft(vec![(
        "line",
        starting(line(declared), built(written)),
    )]))
}

#[test]
fn a_record_of_another_struct_with_the_same_fields_is_not_a_starting_value() {
    let problems = refused(&with_start("Line", "Line2"));
    let all = problems.iter().map(said).collect::<Vec<_>>().join(" | ");
    assert!(
        all.contains("`Line2`") && all.contains("`Line`"),
        "the sentence names both structs: {all}"
    );
}

#[test]
fn a_record_of_the_same_struct_is_a_starting_value() {
    let model = lowered(&with_start("Line", "Line"));
    assert_eq!(model.fields[0].starts.len(), 1);
}

#[test]
fn comparing_a_record_with_a_record_of_another_struct_is_refused() {
    let fields = || vec![("line", starting(line("Line"), built("Line")))];
    let problem = refusal(fields(), equal(field("line"), built("Line2")));
    assert!(
        said(&problem).contains("two sides can never be equal"),
        "the comparison is refused: {}",
        said(&problem)
    );
    assert!(holds(fields(), equal(field("line"), built("Line"))));
}

#[test]
fn a_record_written_without_a_struct_fits_a_struct_with_the_same_fields() {
    let fields = vec![("line", starting(line("Line"), built("Line")))];
    let unnamed = owned_by(None, None, vec![("qty", int(1))]);
    assert!(holds(fields, equal(field("line"), unnamed)));
}

#[test]
fn spreading_a_record_of_another_struct_is_refused() {
    let mut source = rust(draft(vec![("line", starting(line("Line"), built("Line")))]));
    let spread = owned_by(Some("Line2"), Some(field("line")), vec![("qty", int(2))]);
    let change = owned_by(None, None, vec![("line", spread)]);
    source.actions.push(action("grow", None, change));
    let problems = refused(&source);
    assert!(
        problems
            .iter()
            .any(|problem| said(problem).contains("Line2")),
        "{problems:?}"
    );
}

#[test]
fn an_action_that_gives_a_field_a_record_of_another_struct_is_refused() {
    let mut source = rust(draft(vec![("line", starting(line("Line"), built("Line")))]));
    let change = owned_by(None, None, vec![("line", built("Line2"))]);
    source.actions.push(action("swap", None, change));
    let problems = refused(&source);
    assert!(!problems.is_empty(), "{problems:?}");
}

#[test]
fn an_input_that_is_a_struct_is_not_a_value_of_another_struct() {
    let mut source = rust(draft(vec![("line", starting(line("Line"), built("Line")))]));
    let mut swap = action(
        "swap",
        None,
        owned_by(None, None, vec![("line", name("given"))]),
    );
    take(&mut swap, "given", line("Line2"));
    source.actions.push(swap);
    let problems = refused(&source);
    assert!(!problems.is_empty(), "{problems:?}");
    let mut fine = rust(draft(vec![("line", starting(line("Line"), built("Line")))]));
    let mut swap = action(
        "swap",
        None,
        owned_by(None, None, vec![("line", name("given"))]),
    );
    take(&mut swap, "given", line("Line"));
    fine.actions.push(swap);
    assert_eq!(lowered(&fine).actions.len(), 1);
}

#[test]
fn a_struct_is_still_a_value_of_its_fields_for_a_state_and_a_twin() {
    let flag = || one_of(&["a", "b"]);
    let twin = |owner: &str| struct_shape(Some(owner), vec![("on", boolean()), ("which", flag())]);
    let first = lowered(&rust(draft(vec![("spot", twin("Spot"))])));
    let second = lowered(&rust(draft(vec![("spot", twin("Place"))])));
    assert_eq!(
        first.fields[0].starts, second.fields[0].starts,
        "the owner is a type, not a value: both start at the same four values"
    );
}
