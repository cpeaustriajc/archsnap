use crate::build::{draft, lowered, null, one_of, optional, set_of, set_shape, starting, text};
use earnest_ir::Model;

fn printed(model: &Model) -> Vec<String> {
    model
        .fields
        .iter()
        .flat_map(|field| field.starts.iter())
        .map(|value| model.symbols.show(value))
        .collect()
}

#[test]
fn a_set_is_in_the_order_of_its_texts_not_the_order_they_were_first_met() {
    let model = lowered(&draft(vec![(
        "bag",
        starting(
            set_shape(one_of(&["pear", "fig", "apple"])),
            set_of(vec![text("pear"), text("apple"), text("fig")]),
        ),
    )]));
    assert_eq!(printed(&model), [r#"{"apple", "fig", "pear"}"#]);
}

#[test]
fn the_order_does_not_depend_on_which_field_met_a_text_first() {
    let first = lowered(&draft(vec![
        ("a", one_of(&["zeta", "alpha"])),
        (
            "b",
            starting(
                set_shape(one_of(&["alpha", "zeta"])),
                set_of(vec![text("zeta"), text("alpha")]),
            ),
        ),
    ]));
    let second = lowered(&draft(vec![
        ("a", one_of(&["alpha", "zeta"])),
        (
            "b",
            starting(
                set_shape(one_of(&["zeta", "alpha"])),
                set_of(vec![text("alpha"), text("zeta")]),
            ),
        ),
    ]));
    let last = |model: &Model| printed(model).pop();
    assert_eq!(last(&first), Some(r#"{"alpha", "zeta"}"#.to_owned()));
    assert_eq!(last(&first), last(&second));
}

#[test]
fn an_absent_value_comes_before_every_present_one() {
    let model = lowered(&draft(vec![(
        "maybe",
        starting(
            set_shape(optional(one_of(&["b", "a"]))),
            set_of(vec![text("b"), null(), text("a")]),
        ),
    )]));
    assert_eq!(printed(&model), [r#"{null, "a", "b"}"#]);
}
