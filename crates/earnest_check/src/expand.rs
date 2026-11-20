use earnest_ir::{Action, Fault, Frame, Model, eval, truth};
use earnest_value::Value;
use rustc_hash::FxHashSet;
use std::sync::Arc;
use std::time::Instant;

pub(crate) type State = Arc<[Value]>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Whence {
    Rule(usize),
    Guard(usize),
    Update(usize),
    End(usize),
}

#[derive(Debug)]
pub(crate) struct Successor {
    pub(crate) action: usize,
    pub(crate) input: usize,
    pub(crate) next: State,
}

#[derive(Debug, Default)]
pub(crate) struct Expanded {
    pub(crate) broken: Vec<usize>,
    pub(crate) unjudged: Vec<usize>,
    pub(crate) outside: Option<usize>,
    pub(crate) trouble: Option<(Whence, Fault)>,
    pub(crate) ended: bool,
    pub(crate) stuck: bool,
    pub(crate) clock: Clock,
    pub(crate) width: Width,
    pub(crate) successors: Vec<Successor>,
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

impl Expanded {
    fn note(&mut self, whence: Whence, fault: Fault) {
        if fault == Fault::Stopped {
            self.clock = Clock::Stopped;
        } else if self.trouble.is_none() {
            self.trouble = Some((whence, fault));
        }
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

fn next_state(action: &Action, state: &[Value], frame: &mut Frame<'_>) -> Result<State, Fault> {
    let mut next = Vec::with_capacity(state.len());
    for (position, old) in state.iter().enumerate() {
        next.push(match action.updates.get(position) {
            Some(Some(expr)) => eval(expr, frame)?,
            Some(None) | None => old.clone(),
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
    budget: Budget,
) -> Stopping {
    let mut stopping = Stopping::default();
    let mut seen: FxHashSet<State> = FxHashSet::default();
    let combinations = input_count(action);
    let repeats = combinations > 1;
    for input in 0..combinations {
        if frame.out_of_time() {
            found.clock = Clock::Stopped;
            break;
        }
        if found.successors.len() > budget.successors {
            break;
        }
        set_inputs(action, input, frame);
        match truth(&action.guard, frame) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(fault) => {
                found.note(Whence::Guard(position), fault);
                stopping.unsure = true;
                continue;
            }
        }
        stopping.enabled = true;
        match next_state(action, state, frame) {
            Ok(next) => {
                if !repeats || seen.insert(Arc::clone(&next)) {
                    found.successors.push(Successor {
                        action: position,
                        input,
                        next,
                    });
                }
            }
            Err(fault) => found.note(Whence::Update(position), fault),
        }
    }
    stopping
}

fn marked_end(model: &Model, frame: &mut Frame<'_>, found: &mut Expanded) -> Stopping {
    let mut stopping = Stopping::default();
    for (position, end) in model.ends.iter().enumerate() {
        match truth(&end.when, frame) {
            Ok(true) => stopping.enabled = true,
            Ok(false) => {}
            Err(fault) => {
                found.note(Whence::End(position), fault);
                stopping.unsure = true;
            }
        }
    }
    stopping
}

pub(crate) fn expand_state(
    model: &Model,
    only_rule: Option<&str>,
    slots: usize,
    state: &[Value],
    asked: Asked,
    budget: Budget,
) -> Expanded {
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
    let mut frame = Frame::new(state, slots).until(budget.deadline);
    for (position, rule) in model.rules.iter().enumerate() {
        if only_rule.is_some_and(|wanted| wanted != rule.id) {
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
    if asked == Asked::RulesOnly || found.clock == Clock::Stopped {
        return found;
    }
    let mut moving = Stopping::default();
    for (position, action) in model.actions.iter().enumerate() {
        let each = expand_action(position, action, state, &mut frame, &mut found, budget);
        moving.enabled |= each.enabled;
        moving.unsure |= each.unsure;
        if found.clock == Clock::Stopped {
            return found;
        }
    }
    if !moving.enabled {
        let ending = marked_end(model, &mut frame, &mut found);
        found.ended = ending.enabled;
        found.stuck = !ending.enabled && !ending.unsure && !moving.unsure;
    }
    found.width = Width::of(&frame);
    found
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
