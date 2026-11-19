use rustc_hash::FxHashMap;
use serde_json::Value as Json;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct Sym(u32);

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Str(Sym),
    Record(Arc<Vec<(Sym, Value)>>),
    Set(Arc<BTreeSet<Value>>),
    Map(Arc<BTreeMap<Value, Value>>),
    Seq(Arc<Vec<Value>>),
}

impl Value {
    #[must_use]
    pub fn record(mut fields: Vec<(Sym, Value)>) -> Self {
        fields.sort_by_key(|(name, _)| *name);
        Self::Record(Arc::new(fields))
    }

    #[must_use]
    pub fn set(items: BTreeSet<Value>) -> Self {
        Self::Set(Arc::new(items))
    }

    #[must_use]
    pub fn map(entries: BTreeMap<Value, Value>) -> Self {
        Self::Map(Arc::new(entries))
    }

    #[must_use]
    pub fn seq(items: Vec<Value>) -> Self {
        Self::Seq(Arc::new(items))
    }

    #[must_use]
    pub fn field(&self, name: Sym) -> Option<&Value> {
        match self {
            Self::Record(fields) => fields
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| value),
            Self::Null
            | Self::Bool(_)
            | Self::Int(_)
            | Self::Str(_)
            | Self::Set(_)
            | Self::Map(_)
            | Self::Seq(_) => None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Symbols {
    names: Vec<String>,
    ids: FxHashMap<String, Sym>,
}

impl Symbols {
    pub fn intern(&mut self, name: &str) -> Sym {
        if let Some(found) = self.ids.get(name) {
            return *found;
        }
        let sym = Sym(u32::try_from(self.names.len()).unwrap_or(u32::MAX));
        self.names.push(name.to_owned());
        self.ids.insert(name.to_owned(), sym);
        sym
    }

    #[must_use]
    pub fn find(&self, name: &str) -> Option<Sym> {
        self.ids.get(name).copied()
    }

    #[must_use]
    pub fn name(&self, sym: Sym) -> &str {
        usize::try_from(sym.0)
            .ok()
            .and_then(|index| self.names.get(index))
            .map_or("", String::as_str)
    }

    #[must_use]
    pub fn show(&self, value: &Value) -> String {
        let mut out = String::new();
        self.write(value, &mut out);
        out
    }

    #[must_use]
    pub fn plain(&self, value: &Value) -> String {
        match value {
            Value::Str(sym) => self.name(*sym).to_owned(),
            Value::Null
            | Value::Bool(_)
            | Value::Int(_)
            | Value::Record(_)
            | Value::Set(_)
            | Value::Map(_)
            | Value::Seq(_) => self.show(value),
        }
    }

    fn write(&self, value: &Value, out: &mut String) {
        match value {
            Value::Null => out.push_str("null"),
            Value::Bool(flag) => out.push_str(if *flag { "true" } else { "false" }),
            Value::Int(number) => out.push_str(&number.to_string()),
            Value::Str(sym) => {
                out.push('"');
                out.push_str(self.name(*sym));
                out.push('"');
            }
            Value::Record(fields) => {
                let shown = fields
                    .iter()
                    .map(|(name, inner)| format!("{}: {}", self.name(*name), self.show(inner)));
                write_listed(out, "{", shown, "}");
            }
            Value::Set(items) => {
                write_listed(out, "{", items.iter().map(|item| self.show(item)), "}");
            }
            Value::Map(entries) => {
                let shown = entries
                    .iter()
                    .map(|(key, inner)| format!("{} -> {}", self.show(key), self.show(inner)));
                write_listed(out, "{", shown, "}");
            }
            Value::Seq(items) => {
                write_listed(out, "[", items.iter().map(|item| self.show(item)), "]");
            }
        }
    }

    #[must_use]
    pub fn json(&self, value: &Value) -> Json {
        match value {
            Value::Null => Json::Null,
            Value::Bool(flag) => Json::Bool(*flag),
            Value::Int(number) => Json::from(*number),
            Value::Str(sym) => Json::String(self.name(*sym).to_owned()),
            Value::Record(fields) => Json::Object(
                fields
                    .iter()
                    .map(|(name, inner)| (self.name(*name).to_owned(), self.json(inner)))
                    .collect(),
            ),
            Value::Set(items) => Json::Array(items.iter().map(|item| self.json(item)).collect()),
            Value::Map(entries) => Json::Array(
                entries
                    .iter()
                    .map(|(key, inner)| Json::Array(vec![self.json(key), self.json(inner)]))
                    .collect(),
            ),
            Value::Seq(items) => Json::Array(items.iter().map(|item| self.json(item)).collect()),
        }
    }
}

fn write_listed(out: &mut String, open: &str, items: impl Iterator<Item = String>, close: &str) {
    out.push_str(open);
    for (index, item) in items.enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push_str(&item);
    }
    out.push_str(close);
}
