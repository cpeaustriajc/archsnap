use std::collections::{HashSet, VecDeque};
use std::hash::Hash;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Walked {
    pub(crate) states: usize,
    pub(crate) broken: Vec<&'static str>,
    pub(crate) stuck: usize,
}

pub(crate) struct Walk<State> {
    pub(crate) starts: Vec<State>,
    pub(crate) next: fn(&State) -> Vec<State>,
    pub(crate) broken: fn(&State) -> Vec<&'static str>,
    pub(crate) ends: fn(&State) -> bool,
}

pub(crate) fn walk<State: Clone + Eq + Hash>(model: &Walk<State>) -> Walked {
    let mut seen: HashSet<State> = HashSet::new();
    let mut queue: VecDeque<State> = VecDeque::new();
    for start in &model.starts {
        if seen.insert(start.clone()) {
            queue.push_back(start.clone());
        }
    }
    let mut broken = Vec::new();
    let mut stuck = 0;
    while let Some(state) = queue.pop_front() {
        for rule in (model.broken)(&state) {
            if !broken.contains(&rule) {
                broken.push(rule);
            }
        }
        let next = (model.next)(&state);
        if next.is_empty() && !(model.ends)(&state) {
            stuck += 1;
        }
        for after in next {
            if seen.insert(after.clone()) {
                queue.push_back(after);
            }
        }
    }
    Walked {
        states: seen.len(),
        broken,
        stuck,
    }
}
