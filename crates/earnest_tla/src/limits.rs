use earnest_ir::{Expr, Model};
use earnest_value::{Domain, Symbols, Value};

#[derive(Default)]
struct Found {
    widest: Option<i64>,
    text: Option<String>,
}

impl Found {
    fn number(&mut self, number: i64) {
        let out = i32::try_from(number).is_err();
        if out
            && self
                .widest
                .is_none_or(|known| number.unsigned_abs() > known.unsigned_abs())
        {
            self.widest = Some(number);
        }
    }

    fn name(&mut self, symbols: &Symbols, sym: earnest_value::Sym) {
        let text = symbols.name(sym);
        if !text.is_ascii() && self.text.is_none() {
            self.text = Some(text.to_owned());
        }
    }

    fn value(&mut self, symbols: &Symbols, value: &Value) {
        match value {
            Value::Int(number) => self.number(*number),
            Value::Str(sym) => self.name(symbols, *sym),
            Value::Record(fields) => {
                for (name, inner) in fields.iter() {
                    self.name(symbols, *name);
                    self.value(symbols, inner);
                }
            }
            Value::Set(items) => {
                for item in items.iter() {
                    self.value(symbols, item);
                }
            }
            Value::Seq(items) => {
                for item in items.iter() {
                    self.value(symbols, item);
                }
            }
            Value::Map(entries) => {
                for (key, inner) in entries.iter() {
                    self.value(symbols, key);
                    self.value(symbols, inner);
                }
            }
            _ => {}
        }
    }

    fn domain(&mut self, symbols: &Symbols, domain: &Domain) {
        match domain {
            Domain::Int { lo, hi } => {
                self.number(*lo);
                self.number(*hi);
            }
            Domain::OneOf(values) => {
                for value in values {
                    self.value(symbols, value);
                }
            }
            Domain::Optional(inner) | Domain::Set(inner) => self.domain(symbols, inner),
            Domain::Record(fields) => {
                for (name, inner) in fields {
                    self.name(symbols, *name);
                    self.domain(symbols, inner);
                }
            }
            Domain::Map(key, value) => {
                self.domain(symbols, key);
                self.domain(symbols, value);
            }
            Domain::Seq { item, .. } => self.domain(symbols, item),
            _ => {}
        }
    }

    fn expr(&mut self, symbols: &Symbols, expr: &Expr) {
        match expr {
            Expr::Const(value) => self.value(symbols, value),
            Expr::Apply(_, arguments) => {
                for each in arguments {
                    self.expr(symbols, each);
                }
            }
            Expr::And(left, right) | Expr::Or(left, right) => {
                self.expr(symbols, left);
                self.expr(symbols, right);
            }
            Expr::Choose(test, yes, no) => {
                for part in [test, yes, no] {
                    self.expr(symbols, part);
                }
            }
            Expr::Bind { value, body, .. } => {
                self.expr(symbols, value);
                self.expr(symbols, body);
            }
            Expr::Record(fields) => {
                for (name, inner) in fields {
                    self.name(symbols, *name);
                    self.expr(symbols, inner);
                }
            }
            Expr::Get(inner, name) => {
                self.name(symbols, *name);
                self.expr(symbols, inner);
            }
            Expr::With(base, changes) => {
                self.expr(symbols, base);
                for (name, inner) in changes {
                    self.name(symbols, *name);
                    self.expr(symbols, inner);
                }
            }
            Expr::SetOf(items) | Expr::SeqOf(items) => {
                for item in items {
                    self.expr(symbols, item);
                }
            }
            Expr::MapOf(entries) => {
                for (key, inner) in entries {
                    self.expr(symbols, key);
                    self.expr(symbols, inner);
                }
            }
            Expr::Sweep { source, body, .. } => {
                self.expr(symbols, source);
                self.expr(symbols, body);
            }
            _ => {}
        }
    }
}

/// The reasons TLC can't confirm `model`, from what the model declares: the largest number
/// outside TLC's 32-bit integers, and a text that is not ASCII. Each reason is one sentence.
/// A number the model computes is known only by running the search; see
/// [`BEYOND_32_BITS_COMPUTED`].
pub(crate) fn declared(model: &Model) -> Vec<String> {
    let symbols = &model.symbols;
    let mut found = Found::default();
    for field in &model.fields {
        found.domain(symbols, &field.domain);
        for start in &field.starts {
            found.value(symbols, start);
        }
    }
    for action in &model.actions {
        for input in &action.inputs {
            found.domain(symbols, &input.domain);
            for value in &input.values {
                found.value(symbols, value);
            }
        }
        found.expr(symbols, action.guard.expr());
        for update in action.updates.iter().flatten() {
            found.expr(symbols, update.expr());
        }
    }
    for rule in &model.rules {
        found.expr(symbols, rule.holds.expr());
    }
    for end in &model.ends {
        found.expr(symbols, end.when.expr());
    }
    let mut reasons = Vec::new();
    if let Some(number) = found.widest {
        reasons.push(format!(
            "the model declares the number {number}, and TLC's integers hold 32 bits, from \
             -2147483648 to 2147483647"
        ));
    }
    if let Some(text) = found.text {
        reasons.push(format!(
            "the model has the text \"{text}\", which is not ASCII, and TLC compares some texts \
             above ASCII as equal when it counts states"
        ));
    }
    reasons
}

/// The sentence for a number the search computed outside 32 bits: TLC's integers can't hold it.
pub const BEYOND_32_BITS_COMPUTED: &str = "the search computed a number outside 32 bits, and \
    TLC's integers hold 32 bits, from -2147483648 to 2147483647";
