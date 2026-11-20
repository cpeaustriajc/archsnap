use crate::build::{Sketch, int, yes};
use earnest_tla::limits;

#[test]
fn a_model_within_32_bits_and_ascii_has_no_limit() {
    let mut sketch = Sketch::new("Small");
    sketch.number("n", -2_147_483_648, 2_147_483_647, 0);
    sketch.rule("fine", yes());
    sketch.end("any", yes());
    assert!(limits(&sketch.model()).is_empty());
}

#[test]
fn a_declared_number_beyond_32_bits_is_a_limit_that_names_the_widest() {
    let mut sketch = Sketch::new("Big");
    sketch.number("n", 2_147_483_645, 2_147_483_650, 2_147_483_645);
    sketch.rule("fine", yes());
    let found = limits(&sketch.model());
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("2147483650"), "{found:?}");
}

#[test]
fn a_number_written_in_a_rule_beyond_32_bits_is_a_limit() {
    let mut sketch = Sketch::new("Wide");
    sketch.number("n", 0, 3, 0);
    sketch.rule(
        "fine",
        crate::build::two(
            earnest_ir::Op::Order(earnest_ir::Order::Greater),
            &int(3_000_000_000),
            int(0),
        ),
    );
    let found = limits(&sketch.model());
    assert!(
        found.iter().any(|why| why.contains("3000000000")),
        "{found:?}"
    );
}

#[test]
fn a_text_above_ascii_is_a_limit_and_an_ascii_text_is_not() {
    let mut sketch = Sketch::new("Letters");
    let domain = sketch.one_of(&["Z"]);
    let start = sketch.text("Z");
    sketch.field("letter", domain, vec![start]);
    sketch.rule("fine", yes());
    assert!(limits(&sketch.model()).is_empty(), "ASCII");
    let mut wide = Sketch::new("Letters");
    let domain = wide.one_of(&["Z", "\u{15a}"]);
    let start = wide.text("Z");
    wide.field("letter", domain, vec![start]);
    wide.rule("fine", yes());
    let found = limits(&wide.model());
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains('\u{15a}'), "{found:?}");
}
