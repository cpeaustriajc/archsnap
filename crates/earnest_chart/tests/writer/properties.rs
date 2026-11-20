use crate::support::{change, named, outcome, read, rule, step};
use earnest_chart::{
    ARROW_LIMIT, ChangeKind, RuleChange, RuleChanges, RuleText, Section, chart, report,
};
use earnest_check::{Edge, Finding, FindingKind, Graph, Node, Outcome, Path, Shown, Status};
use proptest::collection::vec;
use proptest::option;
use proptest::prelude::{Just, Strategy, any, prop_assert, prop_assert_eq, prop_oneof, proptest};
use serde_json::json;
use std::collections::VecDeque;

const MOST_NODES: usize = 40;
const MOST_EDGES: usize = 80;

fn written() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-z]{1,6}",
        "[a-z ,=]{0,12}",
        "(?s).{0,12}",
        "[\"#:;<>\\[\\]{}%`|\\\\&*~\\-\n\r ]{0,12}",
        Just("[*] --> s0".to_owned()),
        Just("```".to_owned()),
        Just("%%{init: {}}%%".to_owned()),
        Just("end note\n}".to_owned()),
    ]
}

fn node() -> impl Strategy<Value = Node> {
    (
        written(),
        option::of(prop_oneof![
            Just("left".to_owned()),
            Just("right".to_owned()),
            written()
        ]),
        0_u8..16,
        1_usize..5,
    )
        .prop_map(|(label, group, marks, states)| Node {
            label,
            group,
            start: marks & 1 == 1 && marks & 2 == 2,
            end: marks & 4 == 4,
            stuck: false,
            broken: if marks & 8 == 8 {
                vec!["rule".to_owned()]
            } else {
                Vec::new()
            },
            outside: Vec::new(),
            states,
        })
}

fn graphs() -> impl Strategy<Value = Graph> {
    (1..=MOST_NODES)
        .prop_flat_map(|count| {
            (
                vec(node(), count),
                vec((0..count, 0..count, vec(written(), 0..3)), 0..=MOST_EDGES),
                option::of(written()),
                vec(
                    (
                        prop_oneof![Just("broken"), Just("start"), Just("end")],
                        prop_oneof![Just("#abc".to_owned()), Just("teal".to_owned()), written()],
                    ),
                    0..3,
                ),
                any::<bool>(),
            )
        })
        .prop_map(|(mut nodes, edges, title, colors, starts_first)| {
            if starts_first {
                nodes[0].start = true;
            }
            Graph {
                title,
                colors: colors
                    .into_iter()
                    .map(|(name, color)| (name.to_owned(), color))
                    .collect(),
                nodes,
                edges: edges
                    .into_iter()
                    .map(|(from, to, labels)| Edge { from, to, labels })
                    .collect(),
            }
        })
}

fn spaced(label: &str) -> String {
    label
        .chars()
        .map(|character| {
            if character.is_whitespace() || character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect()
}

fn steps_from_a_start(graph: &Graph) -> Vec<Option<usize>> {
    let mut steps: Vec<Option<usize>> = graph
        .nodes
        .iter()
        .map(|node| node.start.then_some(0))
        .collect();
    let mut queue: VecDeque<usize> = (0..graph.nodes.len())
        .filter(|node| steps[*node].is_some())
        .collect();
    while let Some(node) = queue.pop_front() {
        for edge in graph.edges.iter().filter(|edge| edge.from == node) {
            if steps[edge.to].is_none() {
                steps[edge.to] = steps[node].map(|taken| taken + 1);
                queue.push_back(edge.to);
            }
        }
    }
    steps
}

proptest! {
    #[test]
    fn a_chart_is_one_well_formed_block_with_at_most_twenty_arrows(graph in graphs()) {
        let text = chart(&graph);
        let fences = text.lines().filter(|line| line.trim_start().starts_with("```")).count();
        prop_assert_eq!(fences, 2, "one block opens and closes in\n{}", text);
        let drawn = read(&text);
        prop_assert!(drawn.arrows.len() <= ARROW_LIMIT, "{} arrows in\n{}", drawn.arrows.len(), text);
        prop_assert_eq!(chart(&graph), text, "the second call differs");
    }

    #[test]
    fn every_state_drawn_is_reached_from_a_start_by_the_arrows_drawn(graph in graphs()) {
        let text = chart(&graph);
        let drawn = read(&text);
        let reached = drawn.reached();
        for (id, _) in &drawn.states {
            prop_assert!(reached.contains(id), "{} is cut off in\n{}", id, text);
        }
        for id in drawn.starts.iter().chain(&drawn.ends) {
            prop_assert!(drawn.label_of(id).is_some(), "{} is not declared in\n{}", id, text);
        }
    }

    #[test]
    fn every_state_drawn_keeps_its_label_and_its_marks(graph in graphs()) {
        let text = chart(&graph);
        let drawn = read(&text);
        let broken = drawn.members("broken");
        for (id, _) in &drawn.states {
            let position: usize = id[1..].parse().expect("an id is `s` and a number");
            let node = &graph.nodes[position];
            if !node.label.is_empty() {
                prop_assert_eq!(drawn.label_of(id), Some(spaced(&node.label)), "the label of {}", id);
            }
            prop_assert_eq!(drawn.starts.contains(id), node.start, "the start arrow of {}", id);
            prop_assert_eq!(drawn.ends.contains(id), node.end, "the end arrow of {}", id);
            prop_assert_eq!(broken.contains(id), !node.broken.is_empty(), "the class of {}", id);
        }
    }

    #[test]
    fn the_note_counts_what_the_chart_leaves_out(graph in graphs()) {
        let text = chart(&graph);
        let drawn = read(&text);
        let boxes = graph.nodes.len() - drawn.states.len();
        let all: usize = graph.nodes.iter().map(|node| node.states).sum();
        let shown: usize = drawn
            .states
            .iter()
            .map(|(id, _)| graph.nodes[id[1..].parse::<usize>().expect("an id is `s` and a number")].states)
            .sum();
        let states = all - shown;
        let arrows = graph.edges.len() - drawn.arrows.len();
        let from_a_start = steps_from_a_start(&graph);
        let beyond = graph
            .nodes
            .iter()
            .zip(&from_a_start)
            .enumerate()
            .filter(|(position, (node, steps))| {
                let marked = !node.broken.is_empty() || !node.outside.is_empty();
                marked && steps.is_some() && drawn.label_of(&format!("s{position}")).is_none()
            })
            .count();
        let shown = graph
            .nodes
            .iter()
            .enumerate()
            .filter(|(position, node)| {
                let marked = !node.broken.is_empty() || !node.outside.is_empty();
                marked && drawn.label_of(&format!("s{position}")).is_some()
            })
            .count();
        let lies_beyond = match (beyond, shown) {
            (0, _) => String::new(),
            (1, 0) => " A broken state lies beyond what is shown.".to_owned(),
            (many, 0) => format!(" {many} broken states lie beyond what is shown."),
            (1, _) => " Another broken state lies beyond what is shown.".to_owned(),
            (many, _) => format!(" {many} more broken states lie beyond what is shown."),
        };
        let expected = (boxes > 0 || arrows > 0).then(|| format!(
            "Not shown: {states} of {all} states and {arrows} of {} arrows.{lies_beyond}",
            graph.edges.len()
        ));
        prop_assert_eq!(drawn.note, expected, "the note of\n{}", text);
        let reachable = from_a_start.iter().all(Option::is_some);
        if graph.edges.len() <= ARROW_LIMIT && reachable {
            prop_assert_eq!(boxes + arrows, 0, "a small graph is drawn whole:\n{}", text);
        }
    }

    #[test]
    fn the_first_broken_state_within_twenty_steps_is_drawn(graph in graphs()) {
        let steps = steps_from_a_start(&graph);
        let first = graph
            .nodes
            .iter()
            .zip(&steps)
            .position(|(node, taken)| !node.broken.is_empty() && taken.is_some());
        if let Some(position) = first.filter(|position| steps[*position] <= Some(ARROW_LIMIT)) {
            let text = chart(&graph);
            let drawn = read(&text);
            prop_assert!(
                drawn.members("broken").contains(&format!("s{position}")),
                "s{} is missing from\n{}",
                position,
                text
            );
        }
    }

    #[test]
    fn a_report_has_only_the_blocks_and_headings_the_writer_opens(
        name in written(),
        file in written(),
        said in written(),
        value in written(),
        base in written(),
    ) {
        let shown = Shown { text: value.clone(), json: json!(value) };
        let path = Path {
            start: vec![named(&said, shown.clone())],
            steps: vec![step(
                &said,
                vec![named(&value, shown.clone())],
                vec![change(&said, shown.clone(), shown)],
            )],
        };
        let model = Outcome {
            rules: vec![rule(&value, &said, Some(path.clone()))],
            findings: vec![Finding { kind: FindingKind::Fault, message: said.clone(), path }],
            reason: Some(said.clone()),
            computes_beyond_32_bits: false,
            ..outcome(Status::Fail, 3)
        };
        let sections = [Section { name: &name, file: &file, outcome: &model }];
        let side = Some(RuleText { says: &said, holds: &value });
        let changes = [RuleChange {
            file: "src/cart.model.ts",
            model: &name,
            id: &value,
            change: ChangeKind::Tightened,
            was: side,
            now: side,
        }];
        let since = RuleChanges { base: &base, changes: &changes };
        let text = report(&sections, &[], Some(&since));
        let fences: Vec<&str> = text
            .split(['\n', '\r'])
            .filter(|line| line.trim_start().starts_with("```"))
            .collect();
        prop_assert_eq!(fences, ["```text", "```", "```text", "```"], "the blocks of\n{}", text);
        let headings = text
            .split(['\n', '\r'])
            .filter(|line| line.trim_start().starts_with('#'))
            .count();
        prop_assert_eq!(headings, 2, "the headings of\n{}", text);
        prop_assert_eq!(text.split(['\n', '\r']).count(), 23,"the lines of\n{}", text);
    }
}
