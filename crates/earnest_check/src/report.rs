use crate::expand::{Whence, chosen};
use crate::explore::Search;
use crate::outcome::{
    Change, Finding, FindingKind, Named, Outcome, Path, RuleOutcome, Shown, Status, Step, Verdict,
};
use earnest_value::Value;

const STUCK: &str = "no action can happen here, and the model does not mark it as an end";

impl Search<'_> {
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

    fn step(&self, from: usize, to: usize) -> Option<Step> {
        let (position, input) = self.choice(from, to)?;
        let before = self.store.state(from);
        let after = self.store.state(to);
        let action = self.model.actions.get(position)?;
        let input = action
            .inputs
            .iter()
            .zip(chosen(action, input))
            .map(|(each, value)| {
                Some(Named {
                    name: each.name.clone(),
                    value: self.shown(value?),
                })
            })
            .collect::<Option<Vec<Named>>>()?;
        let changes = self
            .model
            .fields
            .iter()
            .zip(before.iter().zip(&after))
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
            state: self.named(&after),
        })
    }

    fn path_to(&self, target: usize) -> Path {
        let mut chain = vec![target];
        let mut at = target;
        while let Some(parent) = self.store.parent(at) {
            chain.push(parent);
            at = parent;
        }
        chain.reverse();
        let start = chain
            .first()
            .map(|id| self.named(&self.store.state(*id)))
            .unwrap_or_default();
        let steps = chain
            .windows(2)
            .filter_map(|pair| {
                let [from, to] = pair else { return None };
                self.step(*from, *to)
            })
            .collect();
        Path { start, steps }
    }

    fn fault_source(&self, whence: Whence) -> String {
        let model = self.model;
        let action_id = |position: usize| model.actions.get(position).map_or("", |found| &found.id);
        match whence {
            Whence::Rule(rule) => format!(
                "the rule `{}`",
                model.rules.get(rule).map_or("", |found| &found.id)
            ),
            Whence::Guard(action) => format!("the condition of the action `{}`", action_id(action)),
            Whence::Update(action) => format!("the change of the action `{}`", action_id(action)),
            Whence::End(end) => format!(
                "the end `{}`",
                model.ends.get(end).map_or("", |found| &found.id)
            ),
        }
    }

    fn findings(&self) -> Vec<Finding> {
        let mut findings = Vec::new();
        if let Some(id) = self.stuck {
            findings.push(Finding {
                kind: FindingKind::Stuck,
                message: STUCK.to_owned(),
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
            let source = self.fault_source(whence);
            findings.push(Finding {
                kind: FindingKind::Fault,
                message: format!("{source} {} in this state", fault.describe()),
                path: self.path_to(id),
            });
        }
        findings
    }

    pub(crate) fn outcome(&self) -> Outcome {
        let only_rule = self.limits.only_rule.as_deref();
        let judged_everywhere =
            |unjudged: bool| self.reason.is_none() && !self.cut_short && !unjudged;
        let rules: Vec<RuleOutcome> = self
            .model
            .rules
            .iter()
            .zip(self.first_broken.iter().zip(&self.unjudged))
            .filter(|(rule, _)| only_rule.is_none_or(|wanted| wanted == rule.id))
            .map(|(rule, (first, unjudged))| RuleOutcome {
                id: rule.id.clone(),
                says: rule.says.clone(),
                verdict: match first {
                    Some(_) => Verdict::Broken,
                    None if judged_everywhere(*unjudged) => Verdict::Holds,
                    None => Verdict::Unknown,
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
            states: self.store.len(),
            rules,
            findings,
            reason: self.reason.clone(),
            computes_beyond_32_bits: self.wide,
            graph: self.limits.keep_graph.then(|| self.graph()),
        }
    }
}
