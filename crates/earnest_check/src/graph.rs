use crate::explore::{Note, Search};
use earnest_ir::{Frame, Label, Piece};
use earnest_value::Value;
use rustc_hash::FxHashMap;

/// One box of a chart: every visited state with the same label and group.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Node {
    /// The view's state label, or every field as `name = value` when the view has none.
    pub label: String,
    /// The view's group label, when the view has one.
    pub group: Option<String>,
    /// Whether one of its states is a start state.
    pub start: bool,
    /// Whether one of its states has no action and is marked as an end.
    pub end: bool,
    /// Whether one of its states has no action and is not marked as an end.
    pub stuck: bool,
    /// The ids of the rules its states break, each once, in the order they were met.
    pub broken: Vec<String>,
    /// The names of the fields that are out of range in its states, each once.
    pub outside: Vec<String>,
    /// How many states it stands for.
    pub states: usize,
}

/// One arrow of a chart: every action that leads from a state of one node to a state of
/// another.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edge {
    /// The position in [`Graph::nodes`] of the node it leaves.
    pub from: usize,
    /// The position in [`Graph::nodes`] of the node it enters, which can be the same node.
    pub to: usize,
    /// The label of each action along it, each once: the view's step label or the action's
    /// sentence.
    pub labels: Vec<String>,
}

/// The visited states and the actions between them, merged by label for a chart.
///
/// When the search stopped at a limit, the graph holds what was visited so far, and a state
/// that was found but not yet expanded has no marks and no arrows out.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Graph {
    /// The view's title.
    pub title: Option<String>,
    /// The view's colors as pairs of `broken`, `start` or `end` and a CSS color.
    pub colors: Vec<(String, String)>,
    /// The nodes, in the order their first state was visited.
    pub nodes: Vec<Node>,
    /// The arrows, in the order their first action was taken.
    pub edges: Vec<Edge>,
}

fn add_once(names: &mut Vec<String>, name: &str) {
    if !names.iter().any(|known| known == name) {
        names.push(name.to_owned());
    }
}

impl Search<'_> {
    fn label(&self, label: &Label, state: &[Value]) -> String {
        let mut frame = Frame::new(state, label.slots);
        let mut text = String::new();
        for piece in &label.pieces {
            match piece {
                Piece::Text(plain) => text.push_str(plain),
                Piece::Shown(expr) => match expr.eval(&mut frame) {
                    Ok(value) => text.push_str(&self.model.symbols.plain(&value)),
                    Err(_) => text.push('?'),
                },
                _ => text.push('?'),
            }
        }
        text
    }

    fn default_label(&self, state: &[Value]) -> String {
        self.model
            .fields
            .iter()
            .zip(state)
            .map(|(field, value)| format!("{} = {}", field.name, self.model.symbols.plain(value)))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn step_label(&self, action: usize) -> String {
        let Some(found) = self.model.actions.get(action) else {
            return String::new();
        };
        self.model
            .view
            .steps
            .iter()
            .find(|(id, _)| *id == found.id)
            .map_or_else(|| found.label.clone(), |(_, label)| label.clone())
    }

    fn mark_node(&self, node: &mut Node, id: usize) {
        node.states = node.states.saturating_add(1);
        node.start |= id < self.store.len() && self.store.parent(id).is_none();
        let Some(Note {
            end,
            stuck,
            outside,
            broken,
        }) = self.notes.get(id)
        else {
            return;
        };
        node.end |= end;
        node.stuck |= stuck;
        if let Some(field) = outside.and_then(|position| self.model.fields.get(position)) {
            add_once(&mut node.outside, &field.name);
        }
        for rule in broken.iter().filter_map(|rule| self.model.rules.get(*rule)) {
            add_once(&mut node.broken, &rule.id);
        }
    }

    pub(crate) fn graph(&self) -> Graph {
        let view = &self.model.view;
        let mut graph = Graph {
            title: view.title.clone(),
            colors: view.colors.clone(),
            ..Graph::default()
        };
        let mut places: FxHashMap<(Option<String>, String), usize> = FxHashMap::default();
        let mut node_of = Vec::with_capacity(self.store.len());
        for id in 0..self.store.len() {
            let state = self.store.state(id);
            let state = state.as_slice();
            let label = view.state.as_ref().map_or_else(
                || self.default_label(state),
                |found| self.label(found, state),
            );
            let group = view.group.as_ref().map(|found| self.label(found, state));
            let position = *places
                .entry((group.clone(), label.clone()))
                .or_insert_with(|| {
                    graph.nodes.push(Node {
                        label,
                        group,
                        ..Node::default()
                    });
                    graph.nodes.len().saturating_sub(1)
                });
            node_of.push(position);
            if let Some(node) = graph.nodes.get_mut(position) {
                self.mark_node(node, id);
            }
        }
        let mut edge_at: FxHashMap<(usize, usize), usize> = FxHashMap::default();
        for arrow in &self.arrows {
            let (Some(source), Some(target)) = (node_of.get(arrow.from), node_of.get(arrow.to))
            else {
                continue;
            };
            let position = *edge_at.entry((*source, *target)).or_insert_with(|| {
                graph.edges.push(Edge {
                    from: *source,
                    to: *target,
                    labels: Vec::new(),
                });
                graph.edges.len().saturating_sub(1)
            });
            if let Some(edge) = graph.edges.get_mut(position) {
                add_once(&mut edge.labels, &self.step_label(arrow.action));
            }
        }
        graph
    }
}
