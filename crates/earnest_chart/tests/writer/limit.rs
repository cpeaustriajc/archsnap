use crate::support::{broken, edge, graph, node, read, start};
use earnest_chart::{ARROW_LIMIT, chart};
use earnest_check::{Edge, Graph, Node};

const SIDE: usize = 6;

fn chain(arrows: usize) -> Graph {
    let mut nodes = vec![start("0")];
    nodes.extend((1..=arrows).map(|position| node(&position.to_string())));
    let edges = (0..arrows)
        .map(|position| edge(position, position + 1, &["next"]))
        .collect();
    graph(nodes, edges)
}

fn dial() -> Graph {
    let mut nodes: Vec<Node> = Vec::new();
    let mut edges: Vec<Edge> = Vec::new();
    for x in 0..SIDE {
        for y in 0..SIDE {
            nodes.push(node(&format!("x{x} y{y}")));
            if x + 1 < SIDE {
                edges.push(edge(x * SIDE + y, (x + 1) * SIDE + y, &["east"]));
            }
            if y + 1 < SIDE {
                edges.push(edge(x * SIDE + y, x * SIDE + y + 1, &["north"]));
            }
        }
    }
    nodes[0].start = true;
    nodes[SIDE * SIDE - 1] = Node {
        end: true,
        ..broken("x5 y5", &["near"])
    };
    graph(nodes, edges)
}

fn pairs(arrows: &[(String, String, String)]) -> Vec<(&str, &str)> {
    arrows
        .iter()
        .map(|(from, to, _)| (from.as_str(), to.as_str()))
        .collect()
}

#[test]
fn a_graph_with_exactly_twenty_arrows_is_drawn_whole() {
    let drawn = read(&chart(&chain(ARROW_LIMIT)));
    assert_eq!(drawn.arrows.len(), 20);
    assert_eq!(drawn.states.len(), 21);
    assert_eq!(drawn.note, None);
}

#[test]
fn a_graph_with_twenty_one_arrows_loses_the_last_and_says_so() {
    let text = chart(&chain(ARROW_LIMIT + 1));
    let drawn = read(&text);
    assert_eq!(drawn.arrows.len(), 20);
    assert_eq!(drawn.states.len(), 21);
    assert_eq!(drawn.label_of("s20"), Some("20".to_owned()));
    assert_eq!(drawn.label_of("s21"), None);
    assert_eq!(
        drawn.note.as_deref(),
        Some("Not shown: 1 of 22 states and 1 of 21 arrows.")
    );
    assert!(
        text.ends_with(
            "    s19 --> s20 : next\n```\n\nNot shown: 1 of 22 states and 1 of 21 arrows."
        ),
        "the note follows the block after a blank line:\n{text}"
    );
}

#[test]
fn the_route_to_a_broken_state_far_from_the_start_is_kept_first() {
    let drawn = read(&chart(&dial()));
    assert_eq!(
        pairs(&drawn.arrows),
        [
            ("s0", "s6"),
            ("s0", "s1"),
            ("s1", "s7"),
            ("s1", "s2"),
            ("s2", "s8"),
            ("s2", "s3"),
            ("s3", "s9"),
            ("s3", "s4"),
            ("s4", "s10"),
            ("s4", "s5"),
            ("s5", "s11"),
            ("s6", "s12"),
            ("s12", "s18"),
            ("s18", "s24"),
            ("s24", "s30"),
            ("s30", "s31"),
            ("s31", "s32"),
            ("s32", "s33"),
            ("s33", "s34"),
            ("s34", "s35"),
        ],
        "five steps east, five north, then the ten arrows found first"
    );
    assert_eq!(drawn.label_of("s0"), Some("x0 y0".to_owned()));
    assert_eq!(drawn.label_of("s35"), Some("x5 y5".to_owned()));
    assert_eq!(drawn.members("broken"), ["s35"]);
    assert_eq!(drawn.starts, ["s0"]);
    assert_eq!(drawn.ends, ["s35"]);
    assert_eq!(drawn.states.len(), 21);
    assert_eq!(
        drawn.note.as_deref(),
        Some("Not shown: 15 of 36 states and 40 of 60 arrows.")
    );
}

#[test]
fn the_same_graph_gives_the_same_chart_on_every_call() {
    let dial = dial();
    let first = chart(&dial);
    for _ in 0..5 {
        assert_eq!(chart(&dial), first);
    }
}

#[test]
fn a_route_longer_than_the_limit_keeps_its_first_arrows_and_the_note_says_what_lies_beyond() {
    let mut far = chain(25);
    far.nodes[25].broken = vec!["far".to_owned()];
    let drawn = read(&chart(&far));
    assert_eq!(drawn.arrows.len(), 20, "the limit");
    assert_eq!(
        pairs(&drawn.arrows).first(),
        Some(&("s0", "s1")),
        "the route starts at the start state"
    );
    assert_eq!(
        pairs(&drawn.arrows).last(),
        Some(&("s19", "s20")),
        "and goes on for twenty arrows"
    );
    assert_eq!(drawn.label_of("s25"), None, "the broken state is not drawn");
    assert!(drawn.members("broken").is_empty(), "so nothing is marked");
    assert_eq!(
        drawn.note.as_deref(),
        Some(
            "Not shown: 5 of 26 states and 5 of 25 arrows. A broken state lies beyond what is \
             shown."
        )
    );
}

#[test]
fn two_broken_states_beyond_the_limit_are_both_counted_in_the_note() {
    let mut far = chain(25);
    far.nodes[22].broken = vec!["far".to_owned()];
    far.nodes[25].outside = vec!["n".to_owned()];
    let drawn = read(&chart(&far));
    assert_eq!(drawn.arrows.len(), 20, "the limit");
    assert_eq!(
        drawn.note.as_deref(),
        Some(
            "Not shown: 5 of 26 states and 5 of 25 arrows. 2 broken states lie beyond what is \
             shown."
        )
    );
}

#[test]
fn a_broken_state_exactly_at_the_limit_is_drawn_and_nothing_lies_beyond() {
    let mut near = chain(21);
    near.nodes[20].broken = vec!["near".to_owned()];
    let drawn = read(&chart(&near));
    assert_eq!(drawn.members("broken"), ["s20"]);
    assert_eq!(
        drawn.note.as_deref(),
        Some("Not shown: 1 of 22 states and 1 of 21 arrows.")
    );
}

#[test]
fn a_second_route_that_does_not_fit_keeps_its_first_arrows() {
    let mut nodes = vec![start("start")];
    let mut edges = Vec::new();
    for (name, first) in [("a", 1), ("b", 16)] {
        for position in 0..15 {
            nodes.push(node(&format!("{name}{position}")));
            let from = if position == 0 {
                0
            } else {
                first + position - 1
            };
            edges.push(edge(from, first + position, &["next"]));
        }
    }
    nodes[15].broken = vec!["first".to_owned()];
    nodes[30].broken = vec!["second".to_owned()];
    let drawn = read(&chart(&graph(nodes, edges)));
    assert_eq!(drawn.arrows.len(), 20);
    assert_eq!(drawn.members("broken"), ["s15"]);
    assert_eq!(drawn.label_of("s20"), Some("b4".to_owned()));
    assert_eq!(drawn.label_of("s21"), None);
    assert_eq!(drawn.label_of("s30"), None);
    assert_eq!(
        drawn.note.as_deref(),
        Some(
            "Not shown: 10 of 31 states and 10 of 30 arrows. A broken state lies beyond what is \
             shown."
        )
    );
}

#[test]
fn a_state_no_start_leads_to_is_left_out() {
    let orphan = graph(
        vec![start("home"), node("orphan")],
        vec![edge(1, 0, &["arrive"])],
    );
    let expected = "```mermaid\nstateDiagram-v2\n    state \"home\" as s0\n    [*] --> s0\n```\n\nNot shown: 1 of 2 states and 1 of 1 arrows.";
    assert_eq!(chart(&orphan), expected);
}

#[test]
fn a_start_state_whose_arrows_are_all_left_out_is_not_drawn() {
    let mut nodes: Vec<Node> = (0..25)
        .map(|position| start(&position.to_string()))
        .collect();
    nodes.push(node("shared"));
    let edges = (0..25)
        .map(|position| edge(position, 25, &["join"]))
        .collect();
    let drawn = read(&chart(&graph(nodes, edges)));
    assert_eq!(drawn.arrows.len(), 20);
    assert_eq!(drawn.starts.len(), 20);
    assert_eq!(drawn.label_of("s19"), Some("19".to_owned()));
    assert_eq!(drawn.label_of("s20"), None);
    assert_eq!(
        drawn.note.as_deref(),
        Some("Not shown: 5 of 26 states and 5 of 25 arrows.")
    );
}

#[test]
fn an_arrow_to_a_state_the_graph_does_not_hold_is_left_out() {
    let torn = graph(
        vec![start("a"), node("b")],
        vec![edge(0, 1, &["go"]), edge(1, 7, &["fall"])],
    );
    let drawn = read(&chart(&torn));
    assert_eq!(pairs(&drawn.arrows), [("s0", "s1")]);
    assert_eq!(
        drawn.note.as_deref(),
        Some("Not shown: 0 of 2 states and 1 of 2 arrows.")
    );
}
