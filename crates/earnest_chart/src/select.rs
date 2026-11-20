use earnest_check::{Edge, Graph, Node};
use std::collections::VecDeque;

/// The most arrows a chart draws, not counting the arrows from and to `[*]`.
pub const ARROW_LIMIT: usize = 20;

pub(crate) struct Selection {
    pub(crate) states: Vec<usize>,
    pub(crate) arrows: Vec<usize>,
    pub(crate) beyond: usize,
    pub(crate) shown: usize,
}

pub(crate) fn marked(node: &Node) -> bool {
    !node.broken.is_empty() || !node.outside.is_empty()
}

fn sound(graph: &Graph) -> impl Iterator<Item = (usize, &Edge)> {
    let count = graph.nodes.len();
    graph
        .edges
        .iter()
        .enumerate()
        .filter(move |(_, edge)| edge.from < count && edge.to < count)
}

fn arrows_into(graph: &Graph, leaving: &[Vec<usize>]) -> Vec<Option<usize>> {
    let mut seen: Vec<bool> = graph.nodes.iter().map(|node| node.start).collect();
    let mut into = vec![None; graph.nodes.len()];
    let mut queue: VecDeque<usize> = (0..graph.nodes.len()).filter(|node| seen[*node]).collect();
    while let Some(node) = queue.pop_front() {
        for position in &leaving[node] {
            let target = graph.edges[*position].to;
            if !seen[target] {
                seen[target] = true;
                into[target] = Some(*position);
                queue.push_back(target);
            }
        }
    }
    into
}

fn route(graph: &Graph, into: &[Option<usize>], kept: &[bool], target: usize) -> Vec<usize> {
    let mut arrows = Vec::new();
    let mut node = target;
    while let Some(position) = into[node] {
        if kept[position] {
            break;
        }
        arrows.push(position);
        node = graph.edges[position].from;
    }
    arrows
}

pub(crate) fn select(graph: &Graph, limit: usize) -> Selection {
    let mut leaving = vec![Vec::new(); graph.nodes.len()];
    for (position, edge) in sound(graph) {
        leaving[edge.from].push(position);
    }
    let into = arrows_into(graph, &leaving);
    let mut reached: Vec<bool> = graph.nodes.iter().map(|node| node.start).collect();
    let mut drawn: Vec<bool> = graph
        .nodes
        .iter()
        .zip(&leaving)
        .map(|(node, arrows)| node.start && arrows.is_empty())
        .collect();
    let mut kept = vec![false; graph.edges.len()];
    let mut count = 0_usize;
    for (target, node) in graph.nodes.iter().enumerate() {
        if !marked(node) || (!node.start && into[target].is_none()) {
            continue;
        }
        let whole = route(graph, &into, &kept, target);
        let room = limit.saturating_sub(count);
        let left_out = whole.len().saturating_sub(room);
        let arrows = whole.get(left_out..).unwrap_or_default();
        count = count.saturating_add(arrows.len());
        if left_out == 0 {
            drawn[target] = true;
        }
        for position in arrows.iter().copied() {
            let edge = &graph.edges[position];
            kept[position] = true;
            drawn[edge.from] = true;
            drawn[edge.to] = true;
            reached[edge.to] = true;
        }
    }
    while count < limit {
        let Some((position, edge)) =
            sound(graph).find(|(position, edge)| !kept[*position] && reached[edge.from])
        else {
            break;
        };
        kept[position] = true;
        drawn[edge.from] = true;
        drawn[edge.to] = true;
        reached[edge.to] = true;
        count = count.saturating_add(1);
    }
    let beyond = graph
        .nodes
        .iter()
        .enumerate()
        .filter(|(target, node)| {
            marked(node) && !drawn[*target] && (node.start || into[*target].is_some())
        })
        .count();
    let shown = graph
        .nodes
        .iter()
        .enumerate()
        .filter(|(target, node)| marked(node) && drawn[*target])
        .count();
    Selection {
        states: (0..graph.nodes.len()).filter(|node| drawn[*node]).collect(),
        arrows: (0..graph.edges.len())
            .filter(|position| kept[*position])
            .collect(),
        beyond,
        shown,
    }
}
