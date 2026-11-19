//! A tree is plain data: built by hand, compared by value.

use earnest_ast::{Reading, Term, TermKind, ViewDraft, condition_text};
use earnest_diagnostics::Place;

#[test]
fn a_new_term_keeps_its_place_and_kind() {
    let place = Place { line: 4, column: 9 };
    let term = Term::new(place, TermKind::Int(3));
    assert_eq!(term.place, place);
    assert_eq!(term.kind, TermKind::Int(3));
}

#[test]
fn terms_differ_when_only_their_place_differs() {
    let first = Term::new(Place { line: 1, column: 1 }, TermKind::Null);
    let second = Term::new(Place { line: 1, column: 2 }, TermKind::Null);
    assert_ne!(first, second);
}

#[test]
fn a_file_without_models_reads_as_nothing_found_and_nothing_refused() {
    let reading = Reading::default();
    assert!(reading.drafts.is_empty());
    assert!(reading.problems.is_empty());
}

#[test]
fn a_model_without_a_view_has_no_title_labels_steps_or_colors() {
    let view = ViewDraft::default();
    assert_eq!(view.title, None);
    assert_eq!(view.state, None);
    assert_eq!(view.group, None);
    assert!(view.steps.is_empty());
    assert!(view.colors.is_empty());
}

#[test]
fn the_text_of_a_condition_has_one_space_for_each_run_of_white_space() {
    assert_eq!(
        condition_text("  s.n <= 2 &&\n\t\ts.n   >= 0\r\n"),
        "s.n <= 2 && s.n >= 0"
    );
    assert_eq!(condition_text("s.n<=2"), "s.n<=2");
    assert_eq!(condition_text(" \n\t"), "");
}
