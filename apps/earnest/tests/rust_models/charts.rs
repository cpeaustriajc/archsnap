use crate::support::{Sandbox, source};

const TWINS: [(&str, &str, &str); 2] = [
    ("Cart", "twins/cart.rs", "models/cart.model.ts"),
    ("Dial", "twins/dial.rs", "models/dial.model.ts"),
];

fn chart(text: &str, file: &str, name: &str) -> String {
    let sandbox = Sandbox::new();
    sandbox.write(file, text);
    let run = sandbox.earnest(&["chart", file, "--model", name]);
    run.exits(0);
    run.stdout
}

fn report_chart(text: &str, file: &str) -> String {
    let sandbox = Sandbox::new();
    sandbox.write(file, text);
    let run = sandbox.earnest(&["report", file]);
    let opening = run
        .stdout
        .find("```mermaid")
        .unwrap_or_else(|| panic!("the report has a chart\n{run}"));
    let closing = run.stdout[opening..]
        .find("\n```\n")
        .unwrap_or_else(|| panic!("the chart is closed\n{run}"));
    run.stdout[opening..opening + closing].to_owned()
}

#[test]
fn r09_a_rust_model_and_its_typescript_twin_give_the_same_chart() {
    for (name, rust, typescript) in TWINS {
        let twin = chart(&source(rust), "model.rs", name);
        let original = chart(&source(typescript), "model.model.ts", name);
        assert!(
            twin.contains("stateDiagram-v2"),
            "{rust} gives a chart\n{twin}"
        );
        assert_eq!(twin, original, "the chart of {rust} and of {typescript}");
    }
}

#[test]
fn r09_the_chart_in_the_report_of_a_rust_model_is_the_chart_of_its_typescript_twin() {
    for (_, rust, typescript) in TWINS {
        assert_eq!(
            report_chart(&source(rust), "model.rs"),
            report_chart(&source(typescript), "model.model.ts"),
            "the report chart of {rust} and of {typescript}"
        );
    }
}

#[test]
fn r09_the_chart_carries_the_views_title_labels_groups_and_colors() {
    let twin = chart(&source("twins/cart.rs"), "model.rs", "Cart");
    for wanted in [
        "title: \"Add to cart focus\"",
        "state \"results\" as g0 {",
        "state \"shortlist\" as g1 {",
        "state \"Add, focus on Add\" as s0",
        "s0 --> s2 : press Add",
        "s2 --> s4 : the add lands",
        "classDef broken fill:#c62828,color:#ffffff",
    ] {
        assert!(twin.contains(wanted), "{wanted} in\n{twin}");
    }
}

#[test]
fn r09_without_a_view_block_a_rust_chart_shows_fields_and_sentences() {
    let text = source("twins/cart.rs");
    let bare = text
        .split("#[view(")
        .next()
        .expect("the model comes before its view");
    let found = chart(bare, "model.rs", "Cart");
    assert!(found.contains("surface = "), "raw field boxes in\n{found}");
    assert!(
        found.contains(" : the buyer presses Add"),
        "sentences in\n{found}"
    );
    assert!(!found.contains("Add to cart focus"), "no title in\n{found}");
}

fn refused(change: (&str, &str)) -> String {
    let text = source("twins/cart.rs").replacen(change.0, change.1, 1);
    let sandbox = Sandbox::new();
    sandbox.write("model.rs", &text);
    let run = sandbox.earnest(&["verify", "model.rs", "--no-cache"]);
    run.exits(1);
    assert!(
        run.stdout.contains("unreadable  model.rs"),
        "the file is unreadable\n{run}"
    );
    run.stdout
}

#[test]
fn r09_a_rust_view_is_refused_where_the_typescript_view_is() {
    let unknown_action = refused(("step(press = ", "step(prez = "));
    assert!(
        unknown_action.contains("`prez`") && unknown_action.contains("no action with this id"),
        "{unknown_action}"
    );
    let invalid_color = refused(("#c62828", "#c6282"));
    assert!(
        invalid_color.contains("`\"#c6282\"`") && invalid_color.contains("hexadecimal digits"),
        "{invalid_color}"
    );
    let unknown_part = refused(("broken = ", "lost = "));
    assert!(
        unknown_part.contains("`lost`") && unknown_part.contains("`broken`, `start` and `end`"),
        "{unknown_part}"
    );
}
