use earnest_check::{Limits, Named, Outcome, Path, explore};
use earnest_ir::{Action, Code, End, Expr, Field, Input, Math, Model, Op, Order, Rule, View};
use earnest_value::{Domain, Symbols, Value};

pub(crate) struct Sketch {
    pub(crate) symbols: Symbols,
    fields: Vec<Field>,
    actions: Vec<Action>,
    rules: Vec<Rule>,
    ends: Vec<End>,
    pub(crate) view: View,
}

impl Sketch {
    pub(crate) fn new() -> Self {
        Self {
            symbols: Symbols::default(),
            fields: Vec::new(),
            actions: Vec::new(),
            rules: Vec::new(),
            ends: Vec::new(),
            view: View::default(),
        }
    }

    pub(crate) fn field(&mut self, name: &str, domain: Domain, starts: Vec<Value>) -> Expr {
        self.fields.push(Field {
            name: name.to_owned(),
            sym: self.symbols.intern(name),
            domain,
            starts,
        });
        Expr::Field(self.fields.len() - 1)
    }

    pub(crate) fn number(&mut self, name: &str, lo: i64, hi: i64, starts: &[i64]) -> Expr {
        let all = starts.iter().copied().map(Value::Int).collect();
        self.field(name, Domain::Int { lo, hi }, all)
    }

    pub(crate) fn boolean(&mut self, name: &str, starts: &[bool]) -> Expr {
        let all = starts.iter().copied().map(Value::Bool).collect();
        self.field(name, Domain::Bool, all)
    }

    pub(crate) fn text(&mut self, value: &str) -> Value {
        Value::Str(self.symbols.intern(value))
    }

    pub(crate) fn action(&mut self, id: &str, guard: Expr, updates: &[(&Expr, Expr)]) {
        self.action_with(id, &[], guard, updates);
    }

    pub(crate) fn action_with(
        &mut self,
        id: &str,
        inputs: &[(&str, Vec<Value>)],
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
        let inputs: Vec<Input> = inputs
            .iter()
            .enumerate()
            .map(|(slot, (name, values))| Input {
                name: (*name).to_owned(),
                slot,
                domain: Domain::OneOf(values.clone()),
                values: values.clone(),
            })
            .collect();
        self.actions.push(Action {
            id: id.to_owned(),
            label: format!("does {id}"),
            line: 1,
            slots: inputs.len(),
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
            slots: 0,
        });
    }

    pub(crate) fn end(&mut self, id: &str, when: Expr) {
        self.ends.push(End {
            id: id.to_owned(),
            says: format!("{id} is reached"),
            when: when.into(),
            slots: 0,
        });
    }

    pub(crate) fn model(self) -> Model {
        Model {
            name: "Sketch".to_owned(),
            file: "sketch.model.ts".to_owned(),
            line: 1,
            symbols: self.symbols,
            fields: self.fields,
            actions: self.actions,
            rules: self.rules,
            ends: self.ends,
            view: self.view,
        }
    }

    pub(crate) fn explored(self) -> Outcome {
        explore(&self.model(), &Limits::default())
    }
}

pub(crate) fn int(number: i64) -> Expr {
    Expr::Const(Value::Int(number))
}

pub(crate) fn yes() -> Expr {
    Expr::Const(Value::Bool(true))
}

pub(crate) fn apply(op: Op, left: &Expr, right: Expr) -> Expr {
    Expr::Apply(op, vec![left.clone(), right])
}

pub(crate) fn plus(left: &Expr, right: i64) -> Expr {
    apply(Op::Math(Math::Add), left, int(right))
}

pub(crate) fn below(left: &Expr, right: i64) -> Expr {
    apply(Op::Order(Order::Less), left, int(right))
}

pub(crate) fn is(left: &Expr, right: i64) -> Expr {
    apply(Op::Equal, left, int(right))
}

pub(crate) fn is_not(left: &Expr, right: i64) -> Expr {
    apply(Op::Differ, left, int(right))
}

pub(crate) fn not(inner: &Expr) -> Expr {
    Expr::Apply(Op::Not, vec![inner.clone()])
}

pub(crate) fn both(left: Expr, right: Expr) -> Expr {
    Expr::And(Box::new(left), Box::new(right))
}

pub(crate) fn counter(top: i64) -> (Sketch, Expr) {
    let mut sketch = Sketch::new();
    let n = sketch.number("n", 0, top, &[0]);
    sketch.action("inc", below(&n, top), &[(&n, plus(&n, 1))]);
    (sketch, n)
}

pub(crate) fn texts(named: &[Named]) -> Vec<String> {
    named
        .iter()
        .map(|each| format!("{} = {}", each.name, each.value.text))
        .collect()
}

pub(crate) fn steps(path: &Path) -> Vec<String> {
    path.steps
        .iter()
        .map(|step| {
            let input: Vec<String> = texts(&step.input);
            let changes: Vec<String> = step
                .changes
                .iter()
                .map(|change| {
                    format!(
                        "{}: {} -> {}",
                        change.field, change.before.text, change.after.text
                    )
                })
                .collect();
            [vec![step.action.clone()], input, changes]
                .concat()
                .join("  ")
        })
        .collect()
}
