use earnest_ir::{Action, Code, End, Expr, Field, Input, Model, Op, Rule, View};
use earnest_tla::{Files, write};
use earnest_value::{Domain, Sym, Symbols, Value};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct Sketch {
    pub(crate) symbols: Symbols,
    name: String,
    fields: Vec<Field>,
    actions: Vec<Action>,
    rules: Vec<Rule>,
    ends: Vec<End>,
}

impl Sketch {
    pub(crate) fn new(name: &str) -> Self {
        Self {
            symbols: Symbols::default(),
            name: name.to_owned(),
            fields: Vec::new(),
            actions: Vec::new(),
            rules: Vec::new(),
            ends: Vec::new(),
        }
    }

    pub(crate) fn sym(&mut self, name: &str) -> Sym {
        self.symbols.intern(name)
    }

    pub(crate) fn text(&mut self, value: &str) -> Value {
        Value::Str(self.sym(value))
    }

    pub(crate) fn texts(&mut self, values: &[&str]) -> Vec<Value> {
        values.iter().map(|value| self.text(value)).collect()
    }

    pub(crate) fn one_of(&mut self, values: &[&str]) -> Domain {
        Domain::OneOf(self.texts(values))
    }

    pub(crate) fn record(&mut self, fields: &[(&str, Domain)]) -> Domain {
        let mut all: Vec<(Sym, Domain)> = fields
            .iter()
            .map(|(name, domain)| (self.sym(name), domain.clone()))
            .collect();
        all.sort_by_key(|(name, _)| *name);
        Domain::Record(all)
    }

    pub(crate) fn field(&mut self, name: &str, domain: Domain, starts: Vec<Value>) -> Expr {
        let sym = self.sym(name);
        self.fields.push(Field {
            name: name.to_owned(),
            sym,
            domain,
            starts,
        });
        Expr::Field(self.fields.len() - 1)
    }

    pub(crate) fn number(&mut self, name: &str, lo: i64, hi: i64, start: i64) -> Expr {
        self.field(name, Domain::Int { lo, hi }, vec![Value::Int(start)])
    }

    pub(crate) fn action(&mut self, id: &str, guard: Expr, updates: &[(&Expr, Expr)]) {
        self.action_with(id, Vec::new(), guard, updates);
    }

    pub(crate) fn action_with(
        &mut self,
        id: &str,
        inputs: Vec<Input>,
        guard: Expr,
        updates: &[(&Expr, Expr)],
    ) {
        let mut changed = vec![None; self.fields.len()];
        for (target, value) in updates {
            let Expr::Field(position) = target else {
                panic!("an update names a state field");
            };
            changed[*position] = Some(value.clone());
        }
        self.actions.push(Action {
            id: id.to_owned(),
            label: format!("does {id}"),
            line: 1,
            slots: 8,
            inputs,
            guard: guard.into(),
            updates: changed
                .into_iter()
                .map(|update| update.map(Code::new))
                .collect(),
        });
    }

    pub(crate) fn rule(&mut self, id: &str, holds: Expr) {
        self.rules.push(Rule {
            id: id.to_owned(),
            says: format!("{id} always"),
            line: 1,
            text: String::new(),
            holds: holds.into(),
            slots: 8,
        });
    }

    pub(crate) fn end(&mut self, id: &str, when: Expr) {
        self.ends.push(End {
            id: id.to_owned(),
            says: format!("{id} is reached"),
            when: when.into(),
            slots: 8,
        });
    }

    pub(crate) fn model(self) -> Model {
        Model {
            name: self.name,
            file: "sketch.model.ts".to_owned(),
            line: 1,
            symbols: self.symbols,
            fields: self.fields,
            actions: self.actions,
            rules: self.rules,
            ends: self.ends,
            view: View::default(),
        }
    }

    pub(crate) fn written(self) -> Files {
        write(&self.model())
    }

    pub(crate) fn shown(mut self, expr: Expr) -> String {
        self.rule("shown", expr);
        let module = self.written().module;
        let judged_in_range = |name: &str| -> Option<String> {
            let lines = body_of(&module, name);
            let [only] = lines.as_slice() else {
                return None;
            };
            only.strip_prefix("InRange => ").map(str::to_owned)
        };
        let rule = judged_in_range("Rule_shown")
            .unwrap_or_else(|| panic!("a rule is one line judged only in range: {module}"));
        let Some(has_a_value) = judged_in_range("Judged_shown") else {
            return rule;
        };
        rule.strip_prefix(&format!("((~{has_a_value}) \\/ "))
            .and_then(|rest| rest.strip_suffix(')'))
            .unwrap_or_else(|| {
                panic!("a rule that can fault is judged where it has a value: {rule}")
            })
            .to_owned()
    }
}

pub(crate) fn body_of(module: &str, name: &str) -> Vec<String> {
    let heading = format!("{name} ==");
    module
        .lines()
        .skip_while(|line| *line != heading)
        .skip(1)
        .take_while(|line| !line.is_empty())
        .map(|line| {
            line.strip_prefix("    ")
                .unwrap_or_else(|| panic!("a body line is indented, not `{line}`"))
                .to_owned()
        })
        .collect()
}

pub(crate) fn helpers_of(module: &str) -> Vec<String> {
    module
        .lines()
        .skip_while(|line| !line.starts_with("vars =="))
        .skip(2)
        .take_while(|line| !line.is_empty() && *line != "InRange ==")
        .map(str::to_owned)
        .collect()
}

pub(crate) fn input(name: &str, slot: usize, domain: Domain) -> Input {
    Input {
        name: name.to_owned(),
        slot,
        values: domain.values(),
        domain,
    }
}

pub(crate) fn int(number: i64) -> Expr {
    Expr::Const(Value::Int(number))
}

pub(crate) fn yes() -> Expr {
    Expr::Const(Value::Bool(true))
}

pub(crate) fn no() -> Expr {
    Expr::Const(Value::Bool(false))
}

pub(crate) fn null() -> Expr {
    Expr::Const(Value::Null)
}

pub(crate) fn constant(value: Value) -> Expr {
    Expr::Const(value)
}

pub(crate) fn one(op: Op, on: &Expr) -> Expr {
    Expr::Apply(op, vec![on.clone()])
}

pub(crate) fn two(op: Op, left: &Expr, right: Expr) -> Expr {
    Expr::Apply(op, vec![left.clone(), right])
}

pub(crate) fn both(left: Expr, right: Expr) -> Expr {
    Expr::And(Box::new(left), Box::new(right))
}

pub(crate) fn either(left: Expr, right: Expr) -> Expr {
    Expr::Or(Box::new(left), Box::new(right))
}

pub(crate) fn choose(test: Expr, yes: Expr, no: Expr) -> Expr {
    Expr::Choose(Box::new(test), Box::new(yes), Box::new(no))
}

pub(crate) fn bind(slot: usize, value: Expr, body: Expr) -> Expr {
    Expr::Bind {
        slot,
        value: Box::new(value),
        body: Box::new(body),
    }
}

pub(crate) fn set_value(items: Vec<Value>) -> Value {
    Value::set(items.into_iter().collect::<BTreeSet<Value>>())
}

pub(crate) fn map_value(entries: Vec<(Value, Value)>) -> Value {
    Value::map(entries.into_iter().collect::<BTreeMap<Value, Value>>())
}

pub(crate) fn boxed(domain: Domain) -> Box<Domain> {
    Box::new(domain)
}

pub(crate) const SMALL: Domain = Domain::Int { lo: 0, hi: 2 };
