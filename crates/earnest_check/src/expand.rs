use earnest_ir::{Action, Fault, Frame, Model, eval, truth};
use earnest_value::Value;
use hashbrown::HashTable;
use rustc_hash::FxHasher;
use std::hash::{Hash, Hasher};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Whence {
    Rule(usize),
    Guard(usize),
    Update(usize),
    End(usize),
}

#[derive(Debug, Default)]
pub(crate) struct Mark {
    pub(crate) broken: Vec<usize>,
    pub(crate) unjudged: Vec<usize>,
    pub(crate) outside: Option<usize>,
    pub(crate) trouble: Option<(Whence, Fault)>,
    pub(crate) ended: bool,
    pub(crate) stuck: bool,
    pub(crate) cut_short: bool,
    pub(crate) clock: Clock,
    pub(crate) width: Width,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Clock {
    #[default]
    Running,
    Stopped,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Width {
    #[default]
    Narrow,
    Beyond32Bits,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Budget {
    pub(crate) deadline: Option<Instant>,
    pub(crate) successors: usize,
}

impl Width {
    fn of(frame: &Frame<'_>) -> Self {
        if frame.computed_beyond_32_bits() {
            Self::Beyond32Bits
        } else {
            Self::Narrow
        }
    }
}

impl Mark {
    fn note(&mut self, whence: Whence, fault: Fault) {
        if fault == Fault::Stopped {
            self.clock = Clock::Stopped;
            return;
        }
        self.cut_short |= !matches!(whence, Whence::Rule(_));
        if self.trouble.is_none() {
            self.trouble = Some((whence, fault));
        }
    }

    pub(crate) fn is_plain(&self) -> bool {
        self.broken.is_empty()
            && self.unjudged.is_empty()
            && self.outside.is_none()
            && self.trouble.is_none()
            && !self.ended
            && !self.stuck
            && !self.cut_short
            && self.clock == Clock::Running
            && self.width == Width::Narrow
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Asked {
    Everything,
    RulesOnly,
}

#[derive(Clone, Copy, Default)]
struct Stopping {
    enabled: bool,
    unsure: bool,
}

pub(crate) trait Successors {
    fn offer(
        &mut self,
        action: usize,
        input: usize,
        state: &[Value],
        updates: &[(usize, Value)],
    ) -> bool;
}

#[derive(Debug, Default)]
pub(crate) struct Scratch {
    updates: Vec<(usize, Value)>,
    seen_values: Vec<Value>,
    seen: HashTable<usize>,
}

fn hash_values(values: &[Value]) -> u64 {
    let mut hasher = FxHasher::default();
    values.hash(&mut hasher);
    hasher.finish()
}

impl Scratch {
    fn first_time(&mut self) -> bool {
        let width = self.updates.len();
        let start = self.seen_values.len();
        self.seen_values
            .extend(self.updates.iter().map(|(_, value)| value.clone()));
        let (seen, values) = (&mut self.seen, &self.seen_values);
        let tuple = |at: usize| values.get(at..at.saturating_add(width)).unwrap_or(&[]);
        let Some(candidate) = values.get(start..) else {
            return false;
        };
        let hash = hash_values(candidate);
        match seen.entry(
            hash,
            |at| tuple(*at) == candidate,
            |at| hash_values(tuple(*at)),
        ) {
            hashbrown::hash_table::Entry::Occupied(_) => {
                self.seen_values.truncate(start);
                false
            }
            hashbrown::hash_table::Entry::Vacant(vacant) => {
                vacant.insert(start);
                true
            }
        }
    }

    fn forget(&mut self) {
        self.seen.clear();
        self.seen_values.clear();
    }
}

pub(crate) fn chosen(action: &Action, input: usize) -> impl Iterator<Item = Option<&Value>> {
    let mut rest = input;
    action.inputs.iter().map(move |each| {
        let count = each.values.len();
        let digit = rest.checked_rem(count)?;
        rest = rest.checked_div(count)?;
        each.values.get(digit)
    })
}

fn input_count(action: &Action) -> usize {
    action.inputs.iter().fold(1_usize, |total, each| {
        total.saturating_mul(each.values.len())
    })
}

fn set_inputs(action: &Action, input: usize, frame: &mut Frame<'_>) {
    for (each, value) in action.inputs.iter().zip(chosen(action, input)) {
        if let Some(found) = value {
            frame.put(each.slot, found.clone());
        }
    }
}

fn next_updates(
    action: &Action,
    state: &[Value],
    frame: &mut Frame<'_>,
    updates: &mut Vec<(usize, Value)>,
) -> Result<(), Fault> {
    updates.clear();
    for (position, update) in action.updates.iter().take(state.len()).enumerate() {
        if let Some(expr) = update {
            updates.push((position, eval(expr, frame)?));
        }
    }
    Ok(())
}

struct Expansion<'s, 'f, S> {
    state: &'s [Value],
    frame: Frame<'f>,
    found: Mark,
    budget: Budget,
    offered: usize,
    sink: &'s mut S,
    scratch: &'s mut Scratch,
}

impl<S: Successors> Expansion<'_, '_, S> {
    fn action(&mut self, position: usize, action: &Action) -> Stopping {
        let mut stopping = Stopping::default();
        let combinations = input_count(action);
        let repeats = combinations > 1;
        self.scratch.forget();
        for input in 0..combinations {
            if self.frame.out_of_time() {
                self.found.clock = Clock::Stopped;
                break;
            }
            if self.offered > self.budget.successors {
                break;
            }
            set_inputs(action, input, &mut self.frame);
            match truth(&action.guard, &mut self.frame) {
                Ok(true) => {}
                Ok(false) => continue,
                Err(fault) => {
                    self.found.note(Whence::Guard(position), fault);
                    stopping.unsure = true;
                    continue;
                }
            }
            stopping.enabled = true;
            match next_updates(
                action,
                self.state,
                &mut self.frame,
                &mut self.scratch.updates,
            ) {
                Ok(()) => {
                    if (!repeats || self.scratch.first_time())
                        && self
                            .sink
                            .offer(position, input, self.state, &self.scratch.updates)
                    {
                        self.offered = self.offered.saturating_add(1);
                    }
                }
                Err(fault) => self.found.note(Whence::Update(position), fault),
            }
        }
        stopping
    }

    fn marked_end(&mut self, model: &Model) -> Stopping {
        let mut stopping = Stopping::default();
        for (position, end) in model.ends.iter().enumerate() {
            match truth(&end.when, &mut self.frame) {
                Ok(true) => stopping.enabled = true,
                Ok(false) => {}
                Err(fault) => {
                    self.found.note(Whence::End(position), fault);
                    stopping.unsure = true;
                }
            }
        }
        stopping
    }
}

pub(crate) struct Expand<'a> {
    pub(crate) model: &'a Model,
    pub(crate) only_rule: Option<&'a str>,
    pub(crate) slots: usize,
    pub(crate) asked: Asked,
    pub(crate) budget: Budget,
}

impl Expand<'_> {
    pub(crate) fn state<S: Successors>(
        &self,
        state: &[Value],
        outside: Option<usize>,
        sink: &mut S,
        scratch: &mut Scratch,
    ) -> Mark {
        let model = self.model;
        let mut found = Mark {
            outside,
            ..Mark::default()
        };
        if found.outside.is_some() {
            found.cut_short = true;
            return found;
        }
        let mut frame = Frame::new(state, self.slots).until(self.budget.deadline);
        for (position, rule) in model.rules.iter().enumerate() {
            if self.only_rule.is_some_and(|wanted| wanted != rule.id) {
                continue;
            }
            match truth(&rule.holds, &mut frame) {
                Ok(true) => {}
                Ok(false) => found.broken.push(position),
                Err(fault) => {
                    found.note(Whence::Rule(position), fault);
                    found.unjudged.push(position);
                }
            }
        }
        found.width = Width::of(&frame);
        if self.asked == Asked::RulesOnly || found.clock == Clock::Stopped {
            return found;
        }
        let mut expansion = Expansion {
            state,
            frame,
            found,
            budget: self.budget,
            offered: 0,
            sink,
            scratch,
        };
        let mut moving = Stopping::default();
        for (position, action) in model.actions.iter().enumerate() {
            let each = expansion.action(position, action);
            moving.enabled |= each.enabled;
            moving.unsure |= each.unsure;
            if expansion.found.clock == Clock::Stopped {
                return expansion.found;
            }
        }
        if !moving.enabled {
            let ending = expansion.marked_end(model);
            expansion.found.ended = ending.enabled;
            expansion.found.stuck = !ending.enabled && !ending.unsure && !moving.unsure;
        }
        expansion.found.width = Width::of(&expansion.frame);
        expansion.found
    }
}

#[cfg(test)]
mod tests {
    use super::{chosen, input_count};
    use earnest_ir::{Action, Expr, Input};
    use earnest_value::{Domain, Value};

    fn input(name: &str, slot: usize, values: Vec<Value>) -> Input {
        Input {
            name: name.to_owned(),
            slot,
            domain: Domain::OneOf(values.clone()),
            values,
        }
    }

    fn action(inputs: Vec<Input>) -> Action {
        Action {
            id: "act".to_owned(),
            label: "acts".to_owned(),
            line: 1,
            inputs,
            guard: Expr::Const(Value::Bool(true)),
            updates: Vec::new(),
            slots: 2,
        }
    }

    fn numbers(all: &[i64]) -> Vec<Value> {
        all.iter().copied().map(Value::Int).collect()
    }

    #[test]
    fn an_action_without_inputs_is_tried_once() {
        assert_eq!(input_count(&action(Vec::new())), 1);
    }

    #[test]
    fn an_action_is_tried_once_per_combination_of_input_values() {
        let two_by_three = action(vec![
            input("a", 0, numbers(&[1, 2])),
            input("b", 1, numbers(&[10, 20, 30])),
        ]);
        assert_eq!(input_count(&two_by_three), 6);
    }

    #[test]
    fn an_input_without_values_leaves_nothing_to_try() {
        let none = action(vec![input("a", 0, Vec::new())]);
        assert_eq!(input_count(&none), 0);
    }

    #[test]
    fn every_combination_number_names_a_different_choice_and_the_first_input_turns_fastest() {
        let two_by_three = action(vec![
            input("a", 0, numbers(&[1, 2])),
            input("b", 1, numbers(&[10, 20, 30])),
        ]);
        let all: Vec<Vec<Value>> = (0..6)
            .map(|combination| {
                chosen(&two_by_three, combination)
                    .map(|value| value.cloned().unwrap())
                    .collect()
            })
            .collect();
        let expected: Vec<Vec<Value>> = [[1, 10], [2, 10], [1, 20], [2, 20], [1, 30], [2, 30]]
            .iter()
            .map(|pair| numbers(pair))
            .collect();
        assert_eq!(all, expected);
    }
}
