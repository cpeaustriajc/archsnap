use crate::build::{
    HERE, draft, field, first_state, here, int, lowered, name, one_of, range, starting, term, text,
    the_problem,
};
use earnest_ast::{Draft, LabelDraft, TemplatePiece, Term, TermKind};
use earnest_ir::{Frame, Label, Model, Piece};

fn cart() -> Draft {
    draft(vec![
        (
            "cta",
            starting(one_of(&["Add", "Adding", "InCart"]), text("Adding")),
        ),
        ("n", starting(range(0, 3), int(2))),
    ])
}

fn label(shown: Term) -> LabelDraft {
    LabelDraft {
        state_name: Some("s".to_owned()),
        term: shown,
    }
}

fn rendered(model: &Model, label: &Label) -> String {
    let state = first_state(model);
    let mut frame = Frame::new(&state, label.slots);
    label
        .pieces
        .iter()
        .map(|piece| match piece {
            Piece::Text(plain) => plain.clone(),
            Piece::Shown(expr) => {
                let value = expr.eval(&mut frame).expect("the label has a value");
                model.symbols.plain(&value)
            }
            _ => panic!("a label piece this test does not know"),
        })
        .collect()
}

#[test]
fn a_model_without_a_view_has_no_labels() {
    let model = lowered(&cart());
    assert!(model.view.title.is_none());
    assert!(model.view.state.is_none());
    assert!(model.view.group.is_none());
    assert!(model.view.steps.is_empty(), "{:?}", model.view.steps);
    assert!(model.view.colors.is_empty(), "{:?}", model.view.colors);
}

#[test]
fn the_title_step_labels_and_colors_are_kept_as_written() {
    let mut source = cart();
    source.view.title = Some("Add to cart focus".to_owned());
    source
        .view
        .steps
        .push(("press".to_owned(), "press Add".to_owned()));
    source
        .view
        .colors
        .push(("broken".to_owned(), "#c62828".to_owned()));
    let model = lowered(&source);
    assert_eq!(model.view.title.as_deref(), Some("Add to cart focus"));
    assert_eq!(
        model.view.steps,
        [("press".to_owned(), "press Add".to_owned())]
    );
    assert_eq!(
        model.view.colors,
        [("broken".to_owned(), "#c62828".to_owned())]
    );
}

#[test]
fn a_template_label_joins_its_text_and_its_values_without_quotes() {
    let mut source = cart();
    source.view.state = Some(label(term(TermKind::Template(vec![
        TemplatePiece::Value(field("cta")),
        TemplatePiece::Text(", n is ".to_owned()),
        TemplatePiece::Value(field("n")),
    ]))));
    let model = lowered(&source);
    let state = model
        .view
        .state
        .as_ref()
        .expect("the view has a state label");
    assert_eq!(state.pieces.len(), 3);
    assert_eq!(rendered(&model, state), "Adding, n is 2");
}

#[test]
fn a_label_that_is_one_value_prints_that_value() {
    let mut source = cart();
    source.view.group = Some(label(field("cta")));
    let model = lowered(&source);
    let group = model
        .view
        .group
        .as_ref()
        .expect("the view has a group label");
    assert_eq!(group.pieces.len(), 1);
    assert_eq!(rendered(&model, group), "Adding");
}

#[test]
fn a_label_that_reads_an_unknown_name_is_refused() {
    let mut source = cart();
    let missing = TemplatePiece::Value(here(name("missing")));
    source.view.state = Some(label(term(TermKind::Template(vec![missing]))));
    let problem = the_problem(&source);
    assert_eq!(problem.place(), HERE);
    assert_eq!(problem.feature, "`missing`");
}
