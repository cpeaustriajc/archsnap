use serde_json::json;

use crate::support::{DIFFERS_BY_LANGUAGE, Sandbox, actions, rule, source, without};

const PICK_TWINS: (&str, &str) = ("twins/pick.rs", "twins/pick.model.ts");
const TICKET_TWINS: (&str, &str) = ("twins/ticket.rs", "models/ticket.model.ts");

fn both(name: &str, twins: (&str, &str)) -> (serde_json::Value, serde_json::Value) {
    let sandbox = Sandbox::new();
    sandbox.write("model.rs", &source(twins.0));
    sandbox.write("model.model.ts", &source(twins.1));
    (
        sandbox.verified("model.rs", name),
        sandbox.verified("model.model.ts", name),
    )
}

#[test]
fn a_rust_choice_among_listed_values_gives_the_states_and_verdicts_of_one_of() {
    let (twin, original) = both("Pick", PICK_TWINS);
    assert_eq!(twin["states"], 9, "3 sizes times no buyer, Kim or Lee");
    assert_eq!(original["states"], 9, "the same count from `m.oneOf`");
    assert_eq!(twin["status"], "fail", "the largest size breaks `fits`");
    assert_eq!(
        without(&twin, &DIFFERS_BY_LANGUAGE),
        without(&original, &DIFFERS_BY_LANGUAGE),
        "the same result in both languages"
    );
    assert_eq!(
        rule(&twin, "listed")["verdict"],
        "holds",
        "the buyer is listed"
    );
    let path = &rule(&twin, "fits")["path"];
    assert_eq!(
        actions(path),
        ["sell"],
        "one sale from the start with size 9"
    );
    assert_eq!(
        path["start"],
        json!([
            { "name": "size", "value": 9 },
            { "name": "buyer", "value": null },
        ]),
        "a listed number starts at every number listed, and the broken one comes first"
    );
    assert_eq!(
        path["steps"][0]["input"],
        json!([{ "name": "who", "value": "Kim" }]),
        "an input that lists texts"
    );
}

#[test]
fn a_rust_listed_text_in_an_optional_field_and_a_record_gives_the_states_of_its_twin() {
    let (twin, original) = both("Ticket", TICKET_TWINS);
    assert_eq!(twin["states"], 12, "2 rows, 2 sides, no owner, Kim or Lee");
    assert_eq!(original["states"], 12, "the same count from `m.oneOf`");
    assert_eq!(
        without(&twin, &DIFFERS_BY_LANGUAGE),
        without(&original, &DIFFERS_BY_LANGUAGE),
        "the same result in both languages"
    );
}

#[test]
fn the_tla_export_of_a_rust_choice_among_listed_values_is_the_one_of_its_twin() {
    let sandbox = Sandbox::new();
    sandbox.write("model.rs", &source(PICK_TWINS.0));
    sandbox.write("model.model.ts", &source(PICK_TWINS.1));
    let twin = sandbox.tla("model.rs", "Pick", "rust");
    let original = sandbox.tla("model.model.ts", "Pick", "typescript");
    assert_eq!(twin, original, "the module and the configuration");
    assert!(
        twin.0.contains("size \\in {1, 5, 9}") && twin.0.contains("who \\in {\"Kim\", \"Lee\"}"),
        "the listed numbers and texts are sets in the module\n{}",
        twin.0
    );
}
