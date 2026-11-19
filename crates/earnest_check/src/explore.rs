use crate::cut::cut_points;
use crate::eval::{Fault, Frame, eval, truth};
use crate::graph::{Edge, Graph, Node};
use crate::ir::{Action, Label, Model, Piece};
use crate::outcome::{
    Change, Finding, FindingKind, Named, Outcome, Path, RuleOutcome, Shown, Status, Step, Verdict,
};
use crate::value::Value;
use rustc_hash::FxHashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct Limits {
    pub max_states: usize,
    pub time_limit: Option<Duration>,
    pub jobs: usize,
    pub keep_graph: bool,
    pub only_rule: Option<String>,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_states: 5_000_000,
            time_limit: None,
            jobs: 1,
            keep_graph: false,
            only_rule: None,
        }
    }
}

type State = Arc<[Value]>;

const SMALLEST_SHARED_LEVEL: usize = 512;
const STATES_BETWEEN_CLOCK_READS: usize = 64;

#[derive(Clone, Copy)]
struct Parent {
    state: usize,
    action: usize,
    input: usize,
}

#[derive(Clone, Copy)]
enum Whence {
    Rule(usize),
    Guard(usize),
    Update(usize),
}

struct Successor {
    action: usize,
    input: usize,
    next: State,
}

#[derive(Default)]
struct Expanded {
    broken: Vec<usize>,
    outside: Option<usize>,
    trouble: Option<(Whence, Fault)>,
    enabled: bool,
    ended: bool,
    successors: Vec<Successor>,
}

impl Expanded {
    fn note(&mut self, whence: Whence, fault: Fault) {
        if self.trouble.is_none() {
            self.trouble = Some((whence, fault));
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Marks {
    start: bool,
    end: bool,
    stuck: bool,
    broken: bool,
}

struct Search<'a> {
    model: &'a Model,
    limits: &'a Limits,
    slots: usize,
    states: Vec<State>,
    index: FxHashMap<State, usize>,
    parents: Vec<Option<Parent>>,
    marks: Vec<Marks>,
    broken_rules: Vec<Vec<usize>>,
    edges: Vec<(usize, usize, usize)>,
    first_broken: Vec<Option<usize>>,
    stuck: Option<usize>,
    outside: Option<(usize, usize)>,
    trouble: Option<(usize, Whence, Fault)>,
    reason: Option<String>,
}

#[must_use]
pub fn explore(model: &Model, limits: &Limits) -> Outcome {
    let slots = model
        .actions
        .iter()
        .map(|action| action.slots)
        .chain(model.rules.iter().map(|rule| rule.slots))
        .chain(model.ends.iter().map(|end| end.slots))
        .max()
        .unwrap_or(0);
    let mut search = Search {
        model,
        limits,
        slots,
        states: Vec::new(),
        index: FxHashMap::default(),
        parents: Vec::new(),
        marks: Vec::new(),
        broken_rules: Vec::new(),
        edges: Vec::new(),
        first_broken: vec![None; model.rules.len()],
        stuck: None,
        outside: None,
        trouble: None,
        reason: None,
    };
    search.run();
    search.outcome()
}

fn start_states(model: &Model) -> Vec<Vec<Value>> {
    let mut partial: Vec<Vec<Value>> = vec![Vec::new()];
    for field in &model.fields {
        let mut grown = Vec::with_capacity(partial.len().saturating_mul(field.starts.len()));
        for prefix in &partial {
            for start in &field.starts {
                let mut next = prefix.clone();
                next.push(start.clone());
                grown.push(next);
            }
        }
        partial = grown;
    }
    partial
}

fn set_inputs(action: &Action, input: usize, frame: &mut Frame<'_>) {
    let mut rest = input;
    for each in &action.inputs {
        let count = each.values.len();
        let digit = rest.checked_rem(count).unwrap_or(0);
        rest = rest.checked_div(count).unwrap_or(0);
        if let Some(value) = each.values.get(digit) {
            frame.put(each.slot, value.clone());
        }
    }
}

fn input_count(action: &Action) -> usize {
    action.inputs.iter().fold(1_usize, |total, each| {
        total.saturating_mul(each.values.len())
    })
}

fn next_state(action: &Action, state: &[Value], frame: &mut Frame<'_>) -> Result<State, Fault> {
    let mut next = Vec::with_capacity(state.len());
    for (update, old) in action.updates.iter().zip(state) {
        next.push(match update {
            Some(expr) => eval(expr, frame)?,
            None => old.clone(),
        });
    }
    Ok(Arc::from(next))
}

fn expand_action(
    position: usize,
    action: &Action,
    state: &[Value],
    frame: &mut Frame<'_>,
    found: &mut Expanded,
) {
    for input in 0..input_count(action) {
        set_inputs(action, input, frame);
        match truth(&action.guard, frame) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(fault) => {
                found.note(Whence::Guard(position), fault);
                continue;
            }
        }
        found.enabled = true;
        match next_state(action, state, frame) {
            Ok(next) => found.successors.push(Successor {
                action: position,
                input,
                next,
            }),
            Err(fault) => found.note(Whence::Update(position), fault),
        }
    }
}

fn expand_state(model: &Model, only_rule: Option<&str>, slots: usize, state: &[Value]) -> Expanded {
    let mut found = Expanded {
        outside: model
            .fields
            .iter()
            .zip(state)
            .position(|(field, value)| !field.domain.contains(value)),
        ..Expanded::default()
    };
    if found.outside.is_some() {
        return found;
    }
    let mut frame = Frame::new(state, slots);
    for (position, rule) in model.rules.iter().enumerate() {
        if only_rule.is_some_and(|wanted| wanted != rule.id) {
            continue;
        }
        match truth(&rule.holds, &mut frame) {
            Ok(true) => {}
            Ok(false) => found.broken.push(position),
            Err(fault) => found.note(Whence::Rule(position), fault),
        }
    }
    for (position, action) in model.actions.iter().enumerate() {
        expand_action(position, action, state, &mut frame, &mut found);
    }
    if !found.enabled {
        found.ended = model
            .ends
            .iter()
            .any(|end| truth(&end.when, &mut frame).unwrap_or(false));
    }
    found
}

impl Search<'_> {
    fn run(&mut self) {
        let began = Instant::now();
        let deadline = self
            .limits
            .time_limit
            .and_then(|limit| began.checked_add(limit));
        let mut frontier = Vec::new();
        for start in start_states(self.model) {
            if let Some(id) = self.insert(Arc::from(start), None) {
                if let Some(marks) = self.marks.get_mut(id) {
                    marks.start = true;
                }
                frontier.push(id);
            }
        }
        while !frontier.is_empty() && self.reason.is_none() {
            let Some(level) = self.expand_level(&frontier, deadline) else {
                let seconds = self.limits.time_limit.map_or(0, |limit| limit.as_secs());
                self.reason = Some(format!("time limit of {seconds} s"));
                break;
            };
            frontier = self.merge(&frontier, level);
        }
    }

    fn insert(&mut self, state: State, parent: Option<Parent>) -> Option<usize> {
        if self.index.contains_key(&state) {
            return None;
        }
        if self.states.len() >= self.limits.max_states {
            self.reason = Some(format!("state limit of {}", self.limits.max_states));
            return None;
        }
        let id = self.states.len();
        self.index.insert(Arc::clone(&state), id);
        self.states.push(state);
        self.parents.push(parent);
        self.marks.push(Marks::default());
        self.broken_rules.push(Vec::new());
        Some(id)
    }

    fn expand_ids(&self, ids: &[usize], deadline: Option<Instant>) -> Option<Vec<Expanded>> {
        let only_rule = self.limits.only_rule.as_deref();
        let mut level = Vec::with_capacity(ids.len());
        for (position, id) in ids.iter().enumerate() {
            let clock_due = position.checked_rem(STATES_BETWEEN_CLOCK_READS) == Some(0);
            if clock_due && deadline.is_some_and(|limit| Instant::now() >= limit) {
                return None;
            }
            let state = self.states.get(*id)?;
            level.push(expand_state(self.model, only_rule, self.slots, state));
        }
        Some(level)
    }

    fn expand_level(&self, frontier: &[usize], deadline: Option<Instant>) -> Option<Vec<Expanded>> {
        let jobs = self.limits.jobs.max(1);
        if jobs == 1 || frontier.len() < SMALLEST_SHARED_LEVEL {
            return self.expand_ids(frontier, deadline);
        }
        let share = frontier.len().div_ceil(jobs);
        let cuts = cut_points(frontier.len(), share);
        std::thread::scope(|scope| {
            let workers: Vec<_> = cuts
                .windows(2)
                .filter_map(|pair| match pair {
                    [from, to] => frontier.get(*from..*to),
                    _ => None,
                })
                .map(|ids| scope.spawn(move || self.expand_ids(ids, deadline)))
                .collect();
            let mut level = Vec::with_capacity(frontier.len());
            for worker in workers {
                level.extend(worker.join().ok().flatten()?);
            }
            Some(level)
        })
    }

    fn merge(&mut self, frontier: &[usize], level: Vec<Expanded>) -> Vec<usize> {
        let mut next = Vec::new();
        for (id, expanded) in frontier.iter().copied().zip(level) {
            self.record(id, &expanded);
            for successor in expanded.successors {
                let parent = Parent {
                    state: id,
                    action: successor.action,
                    input: successor.input,
                };
                let target = match self.index.get(&successor.next) {
                    Some(known) => Some(*known),
                    None => self.insert(successor.next, Some(parent)).inspect(|new| {
                        next.push(*new);
                    }),
                };
                let Some(to) = target else {
                    return next;
                };
                if self.limits.keep_graph {
                    self.edges.push((id, to, successor.action));
                }
            }
        }
        next
    }

    fn record(&mut self, id: usize, expanded: &Expanded) {
        if let Some(field) = expanded.outside {
            self.outside.get_or_insert((id, field));
        }
        for rule in &expanded.broken {
            if let Some(first) = self.first_broken.get_mut(*rule) {
                first.get_or_insert(id);
            }
        }
        if let Some((whence, fault)) = expanded.trouble {
            self.trouble.get_or_insert((id, whence, fault));
        }
        let stuck = !expanded.enabled && !expanded.ended && expanded.outside.is_none();
        if stuck {
            self.stuck.get_or_insert(id);
        }
        if let Some(marks) = self.marks.get_mut(id) {
            marks.end = expanded.ended;
            marks.stuck = stuck;
            marks.broken = !expanded.broken.is_empty() || expanded.outside.is_some();
        }
        if let Some(broken) = self.broken_rules.get_mut(id) {
            broken.clone_from(&expanded.broken);
        }
    }

    fn shown(&self, value: &Value) -> Shown {
        Shown {
            text: self.model.symbols.show(value),
            json: self.model.symbols.json(value),
        }
    }

    fn named(&self, state: &[Value]) -> Vec<Named> {
        self.model
            .fields
            .iter()
            .zip(state)
            .map(|(field, value)| Named {
                name: field.name.clone(),
                value: self.shown(value),
            })
            .collect()
    }

    fn step(&self, parent: Parent, before: &[Value], after: &[Value]) -> Option<Step> {
        let action = self.model.actions.get(parent.action)?;
        let mut rest = parent.input;
        let mut input = Vec::with_capacity(action.inputs.len());
        for each in &action.inputs {
            let count = each.values.len();
            let digit = rest.checked_rem(count).unwrap_or(0);
            rest = rest.checked_div(count).unwrap_or(0);
            input.push(Named {
                name: each.name.clone(),
                value: self.shown(each.values.get(digit)?),
            });
        }
        let changes = self
            .model
            .fields
            .iter()
            .zip(before.iter().zip(after))
            .filter(|(_, (old, new))| old != new)
            .map(|(field, (old, new))| Change {
                field: field.name.clone(),
                before: self.shown(old),
                after: self.shown(new),
            })
            .collect();
        Some(Step {
            action: action.id.clone(),
            label: action.label.clone(),
            input,
            changes,
            state: self.named(after),
        })
    }

    fn path_to(&self, target: usize) -> Path {
        let mut chain = vec![target];
        let mut at = target;
        while let Some(Some(parent)) = self.parents.get(at) {
            chain.push(parent.state);
            at = parent.state;
        }
        chain.reverse();
        let start = chain
            .first()
            .and_then(|id| self.states.get(*id))
            .map(|state| self.named(state))
            .unwrap_or_default();
        let steps = chain
            .windows(2)
            .filter_map(|pair| {
                let [from, to] = pair else { return None };
                let parent = (*self.parents.get(*to)?)?;
                self.step(parent, self.states.get(*from)?, self.states.get(*to)?)
            })
            .collect();
        Path { start, steps }
    }

    fn findings(&self) -> Vec<Finding> {
        let mut findings = Vec::new();
        if let Some(id) = self.stuck {
            findings.push(Finding {
                kind: FindingKind::Stuck,
                message: "no action can happen here, and the model does not mark it as an end"
                    .to_owned(),
                path: self.path_to(id),
            });
        }
        if let Some((id, field)) = self.outside {
            let name = self
                .model
                .fields
                .get(field)
                .map_or("", |found| found.name.as_str());
            findings.push(Finding {
                kind: FindingKind::OutOfRange,
                message: format!("the field `{name}` leaves the values it declares"),
                path: self.path_to(id),
            });
        }
        if let Some((id, whence, fault)) = self.trouble {
            let part = match whence {
                Whence::Rule(rule) => format!(
                    "the rule `{}`",
                    self.model.rules.get(rule).map_or("", |found| &found.id)
                ),
                Whence::Guard(action) => format!(
                    "the `when` of the action `{}`",
                    self.model.actions.get(action).map_or("", |found| &found.id)
                ),
                Whence::Update(action) => format!(
                    "the `then` of the action `{}`",
                    self.model.actions.get(action).map_or("", |found| &found.id)
                ),
            };
            findings.push(Finding {
                kind: FindingKind::Fault,
                message: format!("{part} {} in this state", fault.describe()),
                path: self.path_to(id),
            });
        }
        findings
    }

    fn outcome(&self) -> Outcome {
        let only_rule = self.limits.only_rule.as_deref();
        let rules: Vec<RuleOutcome> = self
            .model
            .rules
            .iter()
            .zip(&self.first_broken)
            .filter(|(rule, _)| only_rule.is_none_or(|wanted| wanted == rule.id))
            .map(|(rule, first)| RuleOutcome {
                id: rule.id.clone(),
                says: rule.says.clone(),
                verdict: match (first, &self.reason) {
                    (Some(_), _) => Verdict::Broken,
                    (None, Some(_)) => Verdict::Unknown,
                    (None, None) => Verdict::Holds,
                },
                path: first.map(|id| self.path_to(id)),
            })
            .collect();
        let findings = self.findings();
        let failed =
            !findings.is_empty() || rules.iter().any(|rule| rule.verdict == Verdict::Broken);
        let status = match (failed, &self.reason) {
            (true, _) => Status::Fail,
            (false, Some(_)) => Status::Unfinished,
            (false, None) => Status::Pass,
        };
        Outcome {
            status,
            states: self.states.len(),
            rules,
            findings,
            reason: self.reason.clone(),
            graph: self.limits.keep_graph.then(|| self.graph()),
        }
    }

    fn label(&self, label: &Label, state: &[Value]) -> String {
        let mut frame = Frame::new(state, label.slots);
        let mut text = String::new();
        for piece in &label.pieces {
            match piece {
                Piece::Text(plain) => text.push_str(plain),
                Piece::Shown(expr) => match eval(expr, &mut frame) {
                    Ok(value) => text.push_str(&self.model.symbols.plain(&value)),
                    Err(_) => text.push('?'),
                },
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

    fn graph(&self) -> Graph {
        let view = &self.model.view;
        let mut graph = Graph {
            title: view.title.clone(),
            colors: view.colors.clone(),
            ..Graph::default()
        };
        let mut places: FxHashMap<(Option<String>, String), usize> = FxHashMap::default();
        let mut node_of = Vec::with_capacity(self.states.len());
        for (id, state) in self.states.iter().enumerate() {
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
            self.mark_node(graph.nodes.get_mut(position), id);
        }
        let mut edge_at: FxHashMap<(usize, usize), usize> = FxHashMap::default();
        for (from, to, action) in &self.edges {
            let (Some(source), Some(target)) = (node_of.get(*from), node_of.get(*to)) else {
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
            let label = self.step_label(*action);
            if let Some(edge) = graph.edges.get_mut(position)
                && !edge.labels.contains(&label)
            {
                edge.labels.push(label);
            }
        }
        graph
    }

    fn mark_node(&self, node: Option<&mut Node>, id: usize) {
        let (Some(found), Some(marks)) = (node, self.marks.get(id)) else {
            return;
        };
        found.states = found.states.saturating_add(1);
        found.start |= marks.start;
        found.end |= marks.end;
        found.stuck |= marks.stuck;
        if marks.broken && found.broken.is_empty() {
            found.broken.push("values out of range".to_owned());
        }
        for rule in self.broken_rules.get(id).into_iter().flatten() {
            let Some(broken) = self.model.rules.get(*rule) else {
                continue;
            };
            if found
                .broken
                .first()
                .is_some_and(|only| only == "values out of range")
            {
                found.broken.clear();
            }
            if !found.broken.contains(&broken.id) {
                found.broken.push(broken.id.clone());
            }
        }
    }
}
