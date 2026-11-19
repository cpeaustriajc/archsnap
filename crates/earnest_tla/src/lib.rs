//! The TLA+ writer: a checked model as a TLA+ module and the configuration TLC checks it with.
//!
//! [`write`] gives the text of both files. The module is written so that TLC finds the states
//! `earnest_check` finds and judges each rule the same way:
//!
//! - `Init` is every combination of the fields' start values, and `Next` is one disjunct per
//!   action: for each combination of its input values the guard, then every variable primed.
//! - `InRange` says each variable holds a value its field declares. No action can happen
//!   outside it, so a state out of range is reached and counted and nothing is searched from
//!   it. Each rule is the invariant `InRange => rule`, because the search does not judge a rule
//!   in a state out of range.
//! - `Stop` lets a state out of range or a marked end repeat itself. It adds no state, and with
//!   it the states TLC would call a deadlock are the ones the search calls stuck, apart from a
//!   state where a guard or an update faults. The configuration still turns the deadlock check
//!   off, because TLC stops at the first deadlock and would count fewer states than the search
//!   does for a model that is stuck.
//! - An optional value is the empty tuple when absent and a tuple of one value when present,
//!   so TLC never compares values of two kinds.
//! - A guard or an update that has no value in a state is a fault in the search, which then
//!   takes no step with that action and those inputs. The action's `IF` therefore first asks
//!   that every value read is present and every remainder has a value, and takes no step
//!   otherwise. A rule with no value in a state is left to stop TLC with an error, which is
//!   what the verdict `unknown` means.

mod helper;
mod kind;
mod names;
mod scribe;

use earnest_ir::{Action, Expr, Model};
use earnest_value::Value;
use helper::{EVERY_HELPER, Helper};
use kind::Kind;
use names::{EVERY_VARIABLE, IN_RANGE, Names, STARTS, STEPS, STOP};
use scribe::{Scope, Scribe, Variable, Written, names_written, set_of};
use std::collections::BTreeSet;

/// The text of the two files TLC reads to check one model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Files {
    /// The module's name. TLC wants the files named after it: `<name>.tla` and `<name>.cfg`.
    /// It is the model's name, respelled when TLA+ can't spell it or keeps the word for itself.
    pub name: String,
    /// The text of `<name>.tla`.
    pub module: String,
    /// The text of `<name>.cfg`. It names one invariant per rule, in the order of the model's
    /// rules: `Rule_` followed by the rule's id, respelled or numbered as [`write`] says.
    pub config: String,
}

const INDENT: &str = "    ";
const EXTENDED: &str = "EXTENDS Integers, Sequences, FiniteSets, TLC";
const FILLER: &str = "nothing";

/// The TLA+ module and TLC configuration for `model`.
///
/// To compare TLC with the search:
///
/// - Run TLC with `-continue`. Without it TLC stops at the first broken rule and counts fewer
///   states than the search.
/// - In one state TLC reports only the first invariant it finds broken. A rule that breaks only
///   where an earlier rule breaks too is then not reported, so give such a rule a run of its
///   own.
/// - A rule that faults in the search is an error in TLC, which then stops whatever the flags.
///   So is a construct this writer does not know, which it writes as a call of `Fault`.
///
/// A field, input, action, rule or end keeps its name when TLA+ can spell it and nothing else
/// has it; otherwise letters TLA+ lacks become `_` and a taken name gets `_2`, `_3` and so on.
#[must_use]
pub fn write(model: &Model) -> Files {
    let mut names = Names::of_a_module(EVERY_HELPER.iter().map(|helper| helper.name()));
    let name = names.claim(&model.name);
    let variables: Vec<Variable> = model
        .fields
        .iter()
        .map(|field| Variable {
            name: names.claim(&field.name),
            kind: Kind::of_domain(&field.domain),
        })
        .collect();
    let filler = variables.is_empty().then(|| names.claim(FILLER));
    let ids = |all: Vec<&str>, kind: &str, names: &mut Names| -> Vec<String> {
        all.into_iter()
            .map(|id| names.claim(&format!("{kind}_{id}")))
            .collect()
    };
    let actions = ids(
        model.actions.iter().map(|each| each.id.as_str()).collect(),
        "Action",
        &mut names,
    );
    let rules = ids(
        model.rules.iter().map(|each| each.id.as_str()).collect(),
        "Rule",
        &mut names,
    );
    let ends = ids(
        model.ends.iter().map(|each| each.id.as_str()).collect(),
        "End",
        &mut names,
    );
    let labels = Names::of_a_module(EVERY_HELPER.iter().map(|helper| helper.name()));
    let mut writer = Writer {
        model,
        scribe: Scribe::new(&model.symbols, variables, labels),
        names,
        filler,
    };
    let mut blocks = vec![writer.in_range(), writer.starts()];
    for (action, defined) in model.actions.iter().zip(&actions) {
        blocks.push(writer.action(action, defined));
    }
    for (end, defined) in model.ends.iter().zip(&ends) {
        let when = writer.condition(&end.when);
        let marked = match when.sound {
            Some(sound) => format!("((~{sound}) \\/ {})", when.text),
            None => when.text,
        };
        blocks.push(definition(defined, &[marked]));
    }
    blocks.push(stop(&ends));
    blocks.push(steps(&actions));
    for (rule, defined) in model.rules.iter().zip(&rules) {
        let holds = writer.condition(&rule.holds).text;
        blocks.push(definition(defined, &[format!("{IN_RANGE} => {holds}")]));
    }
    Files {
        module: writer.module(&name, &blocks),
        config: config(&rules),
        name,
    }
}

struct Writer<'a> {
    model: &'a Model,
    scribe: Scribe<'a>,
    names: Names,
    filler: Option<String>,
}

impl Writer<'_> {
    fn every_variable(&self) -> Vec<String> {
        self.scribe
            .variables()
            .iter()
            .map(|variable| variable.name.clone())
            .chain(self.filler.clone())
            .collect()
    }

    fn module(&self, name: &str, blocks: &[String]) -> String {
        let every = self.every_variable().join(", ");
        let written: BTreeSet<&str> = blocks
            .iter()
            .flat_map(|block| names_written(block))
            .collect();
        let used: Vec<Helper> = EVERY_HELPER
            .into_iter()
            .filter(|helper| written.contains(helper.name()))
            .collect();
        let helpers: Vec<String> = EVERY_HELPER
            .into_iter()
            .filter(|helper| {
                used.iter()
                    .any(|other| other == helper || other.needs() == Some(*helper))
            })
            .map(|helper| helper.definition(&self.names))
            .collect();
        let mut sections = vec![
            format!("---- MODULE {name} ----\n{EXTENDED}"),
            format!("VARIABLES {every}"),
            format!("{EVERY_VARIABLE} == <<{every}>>"),
        ];
        if !helpers.is_empty() {
            sections.push(helpers.join("\n"));
        }
        sections.extend(blocks.iter().cloned());
        sections.push("====\n".to_owned());
        sections.join("\n\n")
    }

    fn in_range(&mut self) -> String {
        let mut scope = Scope::new(&self.names);
        let mut lines = Vec::with_capacity(self.model.fields.len());
        for (field, variable) in self.model.fields.iter().zip(self.every_variable()) {
            let fits = self.scribe.within(&variable, &field.domain, &mut scope);
            lines.push(format!("/\\ {fits}"));
        }
        if lines.is_empty() {
            lines.push("TRUE".to_owned());
        }
        definition(IN_RANGE, &lines)
    }

    fn starts(&mut self) -> String {
        let mut lines = Vec::with_capacity(self.model.fields.len());
        let variables = self.scribe.variables().to_vec();
        for (field, variable) in self.model.fields.iter().zip(variables) {
            let values = self.scribe.values(field.starts.iter(), &variable.kind);
            lines.push(match values.as_slice() {
                [only] => format!("/\\ {} = {only}", variable.name),
                many => format!("/\\ {} \\in {}", variable.name, set_of(many)),
            });
        }
        if let Some(filler) = &self.filler {
            lines.push(format!("/\\ {filler} = 0"));
        }
        definition(STARTS, &lines)
    }

    fn condition(&mut self, expr: &Expr) -> Written {
        let mut scope = Scope::new(&self.names);
        self.scribe.expr(expr, &mut scope)
    }

    fn action(&mut self, action: &Action, defined: &str) -> String {
        let mut scope = Scope::new(&self.names);
        let mut choices = Vec::with_capacity(action.inputs.len());
        for input in &action.inputs {
            let kind = Kind::of_domain(&input.domain);
            let values = self.scribe.values(input.values.iter(), &kind);
            let name = scope.bind(input.slot, &input.name, kind);
            choices.push(format!("{name} \\in {}", set_of(&values)));
        }
        let mut needed = vec![IN_RANGE.to_owned()];
        if !matches!(action.guard, Expr::Const(Value::Bool(true))) {
            let allowed = self.scribe.expr(&action.guard, &mut scope);
            needed.extend(allowed.sound);
            needed.push(allowed.text);
        }
        let mut primed = Vec::new();
        let mut kept = Vec::new();
        for (position, variable) in self.scribe.variables().to_vec().iter().enumerate() {
            if let Some(Some(update)) = action.updates.get(position) {
                let next = self.scribe.expr(update, &mut scope);
                needed.extend(next.sound);
                primed.push(format!("/\\ {}' = {}", variable.name, next.text));
            } else {
                kept.push(variable.name.clone());
            }
        }
        kept.extend(self.filler.clone());
        if !kept.is_empty() {
            primed.push(format!("/\\ UNCHANGED <<{}>>", kept.join(", ")));
        }
        let mut lines = vec![format!("IF {}", needed.join(" /\\ "))];
        for (position, conjunct) in primed.iter().enumerate() {
            let lead = if position == 0 { "THEN" } else { "    " };
            lines.push(format!("{lead} {conjunct}"));
        }
        lines.push("ELSE FALSE".to_owned());
        if choices.is_empty() {
            return definition(defined, &lines);
        }
        let mut chosen = vec![format!("\\E {} :", choices.join(", "))];
        chosen.extend(lines.iter().map(|line| format!("{INDENT}{line}")));
        definition(defined, &chosen)
    }
}

fn definition(name: &str, lines: &[String]) -> String {
    let mut text = format!("{name} ==");
    for line in lines {
        text.push('\n');
        text.push_str(INDENT);
        text.push_str(line);
    }
    text
}

fn stop(ends: &[String]) -> String {
    let mut reasons = vec![format!("(~{IN_RANGE})")];
    reasons.extend(ends.iter().cloned());
    definition(
        STOP,
        &[
            format!("IF {}", reasons.join(" \\/ ")),
            format!("THEN UNCHANGED {EVERY_VARIABLE}"),
            "ELSE FALSE".to_owned(),
        ],
    )
}

fn steps(actions: &[String]) -> String {
    let lines: Vec<String> = actions
        .iter()
        .map(String::as_str)
        .chain([STOP])
        .map(|action| format!("\\/ {action}"))
        .collect();
    definition(STEPS, &lines)
}

fn config(rules: &[String]) -> String {
    let mut text = format!("INIT {STARTS}\nNEXT {STEPS}\nCHECK_DEADLOCK FALSE\n");
    for rule in rules {
        text.push_str("INVARIANT ");
        text.push_str(rule);
        text.push('\n');
    }
    text
}
