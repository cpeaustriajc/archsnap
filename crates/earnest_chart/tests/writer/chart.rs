use crate::support::{broken, edge, end, graph, inside, node, read, start};
use earnest_chart::chart;
use earnest_check::{Graph, Node};

#[test]
fn a_graph_without_a_view_is_drawn_with_every_field_as_the_label() {
    let turnstile = graph(
        vec![
            start("gate = locked, paid = false"),
            node("gate = locked, paid = true"),
            broken("gate = open, paid = false", &["locked"]),
            node("gate = open, paid = true"),
        ],
        vec![
            edge(0, 1, &["the visitor pays"]),
            edge(0, 2, &["the visitor pushes"]),
            edge(1, 3, &["the visitor pushes"]),
            edge(2, 0, &["the visitor walks through"]),
            edge(3, 0, &["the visitor walks through"]),
        ],
    );
    let expected = r#"```mermaid
stateDiagram-v2
    state "gate = locked, paid = false" as s0
    state "gate = locked, paid = true" as s1
    state "gate = open, paid = false" as s2
    state "gate = open, paid = true" as s3
    [*] --> s0
    s0 --> s1 : the visitor pays
    s0 --> s2 : the visitor pushes
    s1 --> s3 : the visitor pushes
    s2 --> s0 : the visitor walks through
    s3 --> s0 : the visitor walks through
    classDef broken fill:#c62828,color:#ffffff
    class s2 broken
```"#;
    assert_eq!(chart(&turnstile), expected);
    read(expected);
}

fn cart() -> Graph {
    Graph {
        title: Some("Add to cart focus".to_owned()),
        colors: vec![("broken".to_owned(), "#c62828".to_owned())],
        nodes: vec![
            inside("results", start("Add, focus on Add")),
            inside("shortlist", start("Add, focus on Add")),
            inside("results", node("Adding, focus on Add")),
            inside("shortlist", node("Adding, focus on Add")),
            inside("results", end("InCart, focus on InCart")),
            inside(
                "shortlist",
                Node {
                    end: true,
                    ..broken("InCart, focus on Lost", &["focusLands"])
                },
            ),
        ],
        edges: vec![
            edge(0, 2, &["press Add"]),
            edge(1, 3, &["press Add"]),
            edge(2, 4, &["the add lands"]),
            edge(3, 5, &["the add lands"]),
        ],
    }
}

#[test]
fn a_view_with_groups_puts_each_state_inside_its_group() {
    let expected = r#"```mermaid
---
title: "Add to cart focus"
---
stateDiagram-v2
    state "results" as g0 {
        state "Add, focus on Add" as s0
        state "Adding, focus on Add" as s2
        state "InCart, focus on InCart" as s4
        [*] --> s0
        s0 --> s2 : press Add
        s2 --> s4 : the add lands
        s4 --> [*]
    }
    state "shortlist" as g1 {
        state "Add, focus on Add" as s1
        state "Adding, focus on Add" as s3
        state "InCart, focus on Lost" as s5
        [*] --> s1
        s1 --> s3 : press Add
        s3 --> s5 : the add lands
        s5 --> [*]
    }
    classDef broken fill:#c62828,color:#ffffff
    class s5 broken
```"#;
    assert_eq!(chart(&cart()), expected);
    read(expected);
}

#[test]
fn an_arrow_between_groups_is_drawn_outside_them() {
    let mixed = graph(
        vec![
            inside("a", start("one")),
            node("two"),
            inside("a", end("three")),
        ],
        vec![
            edge(0, 1, &["leave"]),
            edge(1, 2, &["enter"]),
            edge(0, 2, &["stay"]),
        ],
    );
    let expected = r#"```mermaid
stateDiagram-v2
    state "a" as g0 {
        state "one" as s0
        state "three" as s2
        [*] --> s0
        s0 --> s2 : stay
        s2 --> [*]
    }
    state "two" as s1
    s0 --> s1 : leave
    s1 --> s2 : enter
```"#;
    assert_eq!(chart(&mixed), expected);
    read(expected);
}

#[test]
fn actions_between_the_same_two_states_share_one_arrow() {
    let waiting = graph(
        vec![start("idle"), end("done")],
        vec![edge(0, 0, &["wait"]), edge(0, 1, &["finish", "give up"])],
    );
    let expected = r#"```mermaid
stateDiagram-v2
    state "idle" as s0
    state "done" as s1
    [*] --> s0
    s0 --> s0 : wait
    s0 --> s1 : finish / give up
    s1 --> [*]
```"#;
    assert_eq!(chart(&waiting), expected);
    read(expected);
}

#[test]
fn an_arrow_with_no_label_is_drawn_bare() {
    let bare = graph(
        vec![start("a"), node("b")],
        vec![edge(0, 1, &[]), edge(1, 0, &["", " "])],
    );
    let expected = r#"```mermaid
stateDiagram-v2
    state "a" as s0
    state "b" as s1
    [*] --> s0
    s0 --> s1
    s1 --> s0
```"#;
    assert_eq!(chart(&bare), expected);
}

fn colored(colors: &[(&str, &str)]) -> Graph {
    Graph {
        colors: colors
            .iter()
            .map(|(name, color)| ((*name).to_owned(), (*color).to_owned()))
            .collect(),
        nodes: vec![
            start("a"),
            Node {
                outside: vec!["count".to_owned()],
                ..node("b")
            },
            end("c"),
        ],
        edges: vec![edge(0, 1, &["x"]), edge(0, 2, &["y"])],
        ..Graph::default()
    }
}

#[test]
fn the_colors_of_the_view_fill_the_broken_start_and_end_states() {
    let custom = colored(&[("broken", "#ffcdd2"), ("start", "#e8f5e9"), ("end", "navy")]);
    let expected = r#"```mermaid
stateDiagram-v2
    state "a" as s0
    state "b" as s1
    state "c" as s2
    [*] --> s0
    s0 --> s1 : x
    s0 --> s2 : y
    s2 --> [*]
    classDef broken fill:#ffcdd2,color:#000000
    class s1 broken
    classDef ending fill:navy
    class s2 ending
    classDef starting fill:#e8f5e9,color:#000000
    class s0 starting
```"#;
    assert_eq!(chart(&custom), expected);
    read(expected);
}

#[test]
fn a_color_that_could_carry_more_than_a_color_is_ignored() {
    let hostile = colored(&[
        ("broken", "red;stroke:1px"),
        ("start", "rgb(1, 2, 3)"),
        ("end", "#12345"),
    ]);
    let expected = r#"```mermaid
stateDiagram-v2
    state "a" as s0
    state "b" as s1
    state "c" as s2
    [*] --> s0
    s0 --> s1 : x
    s0 --> s2 : y
    s2 --> [*]
    classDef broken fill:#c62828,color:#ffffff
    class s1 broken
```"#;
    assert_eq!(chart(&hostile), expected);
}

#[test]
fn a_state_that_is_broken_and_an_end_has_the_class_broken_alone() {
    let mut both = colored(&[("end", "#000")]);
    both.nodes[1].end = true;
    let drawn = read(&chart(&both));
    assert_eq!(drawn.members("broken"), ["s1"]);
    assert_eq!(drawn.members("ending"), ["s2"]);
    assert_eq!(
        drawn.styles,
        [
            ("broken".to_owned(), "fill:#c62828,color:#ffffff".to_owned()),
            ("ending".to_owned(), "fill:#000,color:#ffffff".to_owned()),
        ]
    );
}

#[test]
fn a_graph_with_no_state_is_an_empty_chart() {
    assert_eq!(chart(&Graph::default()), "```mermaid\nstateDiagram-v2\n```");
}
