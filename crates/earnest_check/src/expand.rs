use earnest_ir::{Action, Fault, Frame, Model};
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

    fn done(&self) -> bool {
        false
    }
}

#[derive(Debug, Default)]
pub(crate) struct Scratch {
    updates: Vec<(usize, Value)>,
    seen: Seen,
    tried: Trials,
}

#[derive(Debug, Default)]
struct Seen {
    values: Vec<Value>,
    at: HashTable<usize>,
}

#[derive(Debug, Default)]
struct Trials {
    of_action: Vec<Option<Tried>>,
}

impl Trials {
    fn skipping(&mut self, position: usize, action: &Action) -> Option<&[Vec<bool>]> {
        if self.of_action.len() <= position {
            self.of_action
                .resize_with(position.saturating_add(1), || None);
        }
        match self
            .of_action
            .get_mut(position)?
            .get_or_insert_with(|| Tried::of(action))
        {
            Tried::Every => None,
            Tried::FirstOfEach(told) => Some(told),
        }
    }
}

#[derive(Debug)]
enum Tried {
    Every,
    FirstOfEach(Vec<Vec<bool>>),
}

impl Tried {
    fn of(action: &Action) -> Self {
        let told = action.told_apart();
        if told.iter().flatten().all(|apart| *apart) {
            Self::Every
        } else {
            Self::FirstOfEach(told)
        }
    }
}

fn told_apart_from_earlier(action: &Action, told: &[Vec<bool>], input: usize) -> bool {
    let mut rest = input;
    action.inputs.iter().zip(told).all(|(each, apart)| {
        let count = each.values.len();
        let digit = rest.checked_rem(count);
        rest = rest.checked_div(count).unwrap_or(0);
        digit.and_then(|at| apart.get(at)).copied().unwrap_or(true)
    })
}

fn hash_values(values: &[Value]) -> u64 {
    let mut hasher = FxHasher::default();
    values.hash(&mut hasher);
    hasher.finish()
}

impl Seen {
    fn first_time(&mut self, updates: &[(usize, Value)]) -> bool {
        let width = updates.len();
        let start = self.values.len();
        self.values
            .extend(updates.iter().map(|(_, value)| value.clone()));
        let (seen, values) = (&mut self.at, &self.values);
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
                self.values.truncate(start);
                false
            }
            hashbrown::hash_table::Entry::Vacant(vacant) => {
                vacant.insert(start);
                true
            }
        }
    }

    fn forget(&mut self) {
        self.at.clear();
        self.values.clear();
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
            updates.push((position, expr.eval(frame)?));
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
        self.scratch.seen.forget();
        let skipping = if repeats {
            self.scratch.tried.skipping(position, action)
        } else {
            None
        };
        for input in 0..combinations {
            if self.frame.out_of_time() {
                self.found.clock = Clock::Stopped;
                break;
            }
            if self.offered > self.budget.successors || self.sink.done() {
                break;
            }
            if let Some(told) = skipping
                && !told_apart_from_earlier(action, told, input)
            {
                continue;
            }
            set_inputs(action, input, &mut self.frame);
            match action.guard.truth(&mut self.frame) {
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
                    if (!repeats || self.scratch.seen.first_time(&self.scratch.updates))
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
            match end.when.truth(&mut self.frame) {
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
            match rule.holds.truth(&mut frame) {
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
            if expansion.sink.done() {
                break;
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
    use super::{chosen, input_count, told_apart_from_earlier};
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
            guard: Expr::Const(Value::Bool(true)).into(),
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

    #[test]
    fn a_combination_is_tried_only_when_each_of_its_values_is_told_apart_from_earlier_ones() {
        let two_by_three = action(vec![
            input("a", 0, numbers(&[1, 2])),
            input("b", 1, numbers(&[10, 20, 30])),
        ]);
        let told = [vec![true, true], vec![true, false, true]];
        let tried: Vec<usize> = (0..6)
            .filter(|combination| told_apart_from_earlier(&two_by_three, &told, *combination))
            .collect();
        assert_eq!(
            tried,
            vec![0, 1, 4, 5],
            "the combinations with b at 20 repeat those with b at 10"
        );
    }
}
