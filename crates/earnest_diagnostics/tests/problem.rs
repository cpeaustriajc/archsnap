//! A problem keeps the place it was reported at and serializes as the JSON output names it.

use earnest_diagnostics::{Place, Problem};
use serde_json::json;

const CLOCK: Place = Place {
    line: 12,
    column: 17,
};

#[test]
fn a_place_that_was_never_set_is_line_zero_column_zero() {
    assert_eq!(Place::default(), Place { line: 0, column: 0 });
}

#[test]
fn places_sort_by_line_before_column() {
    let early = Place {
        line: 2,
        column: 40,
    };
    let late = Place { line: 3, column: 1 };
    assert!(early < late);
}

#[test]
fn a_problem_copies_the_line_and_column_of_its_place() {
    let problem = Problem::new(
        "src/clock.model.ts",
        CLOCK,
        "`Date.now`",
        "the system clock is outside the library",
    );
    assert_eq!(problem.file, "src/clock.model.ts");
    assert_eq!(problem.line, 12);
    assert_eq!(problem.column, 17);
    assert_eq!(problem.feature, "`Date.now`");
    assert_eq!(problem.message, "the system clock is outside the library");
}

#[test]
fn a_problem_gives_back_the_place_it_was_reported_at() {
    let problem = Problem::new("a.ts", CLOCK, "`x`", "why");
    assert_eq!(problem.place(), CLOCK);
}

#[test]
fn a_problem_serializes_with_the_field_names_of_the_json_output() {
    let problem = Problem::new("src/clock.model.ts", CLOCK, "`Date.now`", "outside");
    let expected = json!({
        "file": "src/clock.model.ts",
        "line": 12,
        "column": 17,
        "feature": "`Date.now`",
        "message": "outside"
    });
    assert_eq!(serde_json::to_value(&problem).unwrap(), expected);
}
