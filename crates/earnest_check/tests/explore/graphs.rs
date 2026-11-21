use crate::build::{Sketch, below, counter, int, is, is_not, plus, yes};
use earnest_check::{Edge, Graph, Limits, Node, explore};
use earnest_ir::{Expr, Label, Model, Op, Piece};
use earnest_value::{Domain, Value};

fn graph_of(model: &Model) -> Graph {
    let limits = Limits {
        keep_graph: true,
        ..Limits::default()
    };
    explore(model, &limits)
        .graph
        .expect("the limits asked for the graph")
}

fn labels(graph: &Graph) -> Vec<&str> {
    graph.nodes.iter().map(|node| node.label.as_str()).collect()
}

fn arrows(graph: &Graph) -> Vec<String> {
    graph
        .edges
        .iter()
        .map(|edge| format!("{} -> {}: {}", edge.from, edge.to, edge.labels.join(" | ")))
        .collect()
}

fn low_or_high(sketch: &mut Sketch, n: &Expr) -> Label {
    let low = sketch.text("low");
    let high = sketch.text("high");
    let band = Expr::Choose(
        Box::new(below(n, 2)),
        Box::new(Expr::Const(low)),
        Box::new(Expr::Const(high)),
    );
    Label {
        pieces: vec![Piece::Text("band ".to_owned()), Piece::Shown(band.into())],
        slots: 0,
    }
}

fn banded_counter() -> Model {
    let (mut sketch, n) = counter(3);
    sketch.end("full", is(&n, 3));
    sketch.rule("neverOne", is_not(&n, 1));
    sketch.view.state = Some(low_or_high(&mut sketch, &n));
    sketch.view.title = Some("Counter".to_owned());
    sketch
        .view
        .steps
        .push(("inc".to_owned(), "one more".to_owned()));
    sketch
        .view
        .colors
        .push(("broken".to_owned(), "#c62828".to_owned()));
    sketch.model()
}

#[test]
fn the_graph_is_kept_only_when_the_limits_ask_for_it() {
    let (mut sketch, n) = counter(2);
    sketch.end("full", is(&n, 2));
    assert_eq!(sketch.explored().graph, None);
}

#[test]
fn without_a_view_each_state_is_a_node_labelled_with_its_fields() {
    let (mut sketch, n) = counter(2);
    sketch.end("full", is(&n, 2));
    let graph = graph_of(&sketch.model());
    assert_eq!(graph.title, None);
    assert!(graph.colors.is_empty(), "{:?}", graph.colors);
    assert_eq!(labels(&graph), ["n = 0", "n = 1", "n = 2"]);
    assert!(graph.nodes.iter().all(|node| node.states == 1));
    assert!(graph.nodes.iter().all(|node| node.group.is_none()));
    assert_eq!(arrows(&graph), ["0 -> 1: does inc", "1 -> 2: does inc"]);
}

#[test]
fn without_a_view_a_text_value_is_shown_without_quotes() {
    let mut sketch = Sketch::new();
    let locked = sketch.text("locked");
    let domain = Domain::OneOf(vec![locked.clone()]);
    sketch.field("gate", domain, vec![locked]);
    sketch.boolean("paid", &[false]);
    sketch.end("anywhere", yes());
    let graph = graph_of(&sketch.model());
    assert_eq!(labels(&graph), ["gate = locked, paid = false"]);
}

#[test]
fn start_end_and_stuck_states_are_marked() {
    let mut sketch = Sketch::new();
    let n = sketch.number("n", 0, 2, &[0]);
    sketch.action("left", is(&n, 0), &[(&n, int(1))]);
    sketch.action("right", is(&n, 0), &[(&n, int(2))]);
    sketch.end("right", is(&n, 2));
    let graph = graph_of(&sketch.model());
    let marks: Vec<(bool, bool, bool)> = graph
        .nodes
        .iter()
        .map(|node| (node.start, node.end, node.stuck))
        .collect();
    assert_eq!(
        marks,
        [
            (true, false, false),
            (false, false, true),
            (false, true, false)
        ]
    );
}

#[test]
fn a_node_lists_the_rules_its_states_break_and_the_fields_out_of_range() {
    let mut sketch = Sketch::new();
    let n = sketch.number("n", 0, 2, &[0]);
    sketch.action("inc", yes(), &[(&n, plus(&n, 1))]);
    sketch.rule("neverOne", is_not(&n, 1));
    sketch.rule("belowTwo", below(&n, 2));
    let graph = graph_of(&sketch.model());
    let broken: Vec<&[String]> = graph
        .nodes
        .iter()
        .map(|node| node.broken.as_slice())
        .collect();
    let none: &[String] = &[];
    assert_eq!(
        broken,
        [
            none,
            &["neverOne".to_owned()],
            &["belowTwo".to_owned()],
            none
        ]
    );
    let outside: Vec<&[String]> = graph
        .nodes
        .iter()
        .map(|node| node.outside.as_slice())
        .collect();
    assert_eq!(outside, [none, none, none, &["n".to_owned()]]);
}

#[test]
fn a_view_label_merges_the_states_that_share_it() {
    let graph = graph_of(&banded_counter());
    let low = Node {
        label: "band low".to_owned(),
        group: None,
        start: true,
        end: false,
        stuck: false,
        broken: vec!["neverOne".to_owned()],
        outside: Vec::new(),
        states: 2,
    };
    let high = Node {
        label: "band high".to_owned(),
        end: true,
        start: false,
        broken: Vec::new(),
        ..low.clone()
    };
    assert_eq!(graph.nodes, [low, high]);
}

#[test]
fn arrows_between_merged_states_are_merged_and_use_the_view_s_step_labels() {
    let graph = graph_of(&banded_counter());
    let arrow = |from: usize, to: usize| Edge {
        from,
        to,
        labels: vec!["one more".to_owned()],
    };
    assert_eq!(graph.edges, [arrow(0, 0), arrow(0, 1), arrow(1, 1)]);
}

#[test]
fn the_view_s_title_and_colors_are_copied_to_the_graph() {
    let graph = graph_of(&banded_counter());
    assert_eq!(graph.title.as_deref(), Some("Counter"));
    assert_eq!(graph.colors, [("broken".to_owned(), "#c62828".to_owned())]);
}

#[test]
fn a_group_label_keeps_states_with_the_same_state_label_apart() {
    let (mut sketch, n) = counter(3);
    sketch.end("full", is(&n, 3));
    sketch.view.state = Some(Label {
        pieces: vec![Piece::Text("count".to_owned())],
        slots: 0,
    });
    sketch.view.group = Some(low_or_high(&mut sketch, &n));
    let graph = graph_of(&sketch.model());
    let groups: Vec<(&str, Option<&str>, usize)> = graph
        .nodes
        .iter()
        .map(|node| (node.label.as_str(), node.group.as_deref(), node.states))
        .collect();
    assert_eq!(
        groups,
        [
            ("count", Some("band low"), 2),
            ("count", Some("band high"), 2)
        ]
    );
}

#[test]
fn two_actions_between_the_same_nodes_share_one_arrow_with_both_labels() {
    let mut sketch = Sketch::new();
    let n = sketch.number("n", 0, 1, &[0]);
    sketch.action("walk", is(&n, 0), &[(&n, int(1))]);
    sketch.action("run", is(&n, 0), &[(&n, int(1))]);
    sketch.end("there", is(&n, 1));
    let graph = graph_of(&sketch.model());
    assert_eq!(arrows(&graph), ["0 -> 1: does walk | does run"]);
}

#[test]
fn a_label_that_can_not_be_evaluated_shows_a_question_mark() {
    let (mut sketch, n) = counter(1);
    sketch.end("full", is(&n, 1));
    let unreadable = Expr::Apply(Op::Unwrap, vec![Expr::Const(Value::Null)]);
    sketch.view.state = Some(Label {
        pieces: vec![
            Piece::Text("n is ".to_owned()),
            Piece::Shown(unreadable.into()),
        ],
        slots: 0,
    });
    let graph = graph_of(&sketch.model());
    assert_eq!(labels(&graph), ["n is ?"]);
    assert_eq!(graph.nodes[0].states, 2);
}

#[test]
fn a_graph_cut_short_by_the_state_limit_holds_the_states_visited_so_far() {
    let (mut sketch, n) = counter(9);
    sketch.end("full", is(&n, 9));
    let limits = Limits {
        keep_graph: true,
        max_states: 3,
        ..Limits::default()
    };
    let graph = explore(&sketch.model(), &limits)
        .graph
        .expect("the limits asked for the graph");
    assert_eq!(labels(&graph), ["n = 0", "n = 1", "n = 2"]);
    assert_eq!(arrows(&graph), ["0 -> 1: does inc", "1 -> 2: does inc"]);
}
