use rustc_hash::FxHashMap;
use serde_json::Value as Json;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// An interned name: a text value, or the name of a record field.
///
/// Only the [`Symbols`] that handed it out can turn it back into text. Two names from the same
/// `Symbols` are equal exactly when their text is equal, and they sort in the order they were
/// first interned, not alphabetically.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct Sym(u32);

/// What one field of a model holds in one state.
///
/// An optional value has no wrapper: it is [`Value::Null`] when absent and the plain value when
/// present. A queue is a [`Value::Seq`], an owned lock is its owner or `Null`, and a timer or a
/// request is the [`Value::Str`] naming its stage. Collections are shared, so a clone is cheap.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
#[non_exhaustive]
pub enum Value {
    /// An absent optional value.
    Null,
    /// True or false.
    Bool(bool),
    /// A whole number.
    Int(i64),
    /// A text value or an enum value.
    Str(Sym),
    /// Named fields, sorted by name. Build one with [`Value::record`] to keep them sorted.
    Record(Arc<Vec<(Sym, Value)>>),
    /// Distinct items in sorted order.
    Set(Arc<BTreeSet<Value>>),
    /// Keys in sorted order, each with one value.
    Map(Arc<BTreeMap<Value, Value>>),
    /// Items in the order they were added.
    Seq(Arc<Vec<Value>>),
}

impl Value {
    /// A record of `fields`, in any order. Two records with the same fields are equal however
    /// they were listed.
    #[must_use]
    pub fn record(mut fields: Vec<(Sym, Self)>) -> Self {
        fields.sort_by_key(|(name, _)| *name);
        Self::Record(Arc::new(fields))
    }

    /// A set holding `items`.
    #[must_use]
    pub fn set(items: BTreeSet<Self>) -> Self {
        Self::Set(Arc::new(items))
    }

    /// A map holding `entries`.
    #[must_use]
    pub fn map(entries: BTreeMap<Self, Self>) -> Self {
        Self::Map(Arc::new(entries))
    }

    /// A sequence or a queue holding `items`, first item first.
    #[must_use]
    pub fn seq(items: Vec<Self>) -> Self {
        Self::Seq(Arc::new(items))
    }

    /// The value of the record field `name`. `None` when this is not a record or the record has
    /// no such field.
    #[must_use]
    pub fn field(&self, name: Sym) -> Option<&Self> {
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

/// Every name one model uses, and the way its values are printed.
///
/// A [`Sym`] means something only to the `Symbols` that made it. Keep one per model and pass it
/// along with the model's values.
#[derive(Clone, Debug, Default)]
pub struct Symbols {
    names: Vec<String>,
    ids: FxHashMap<String, Sym>,
}

impl Symbols {
    /// The name for `text`. Interning the same text again gives the same name back.
    pub fn intern(&mut self, text: &str) -> Sym {
        if let Some(found) = self.ids.get(text) {
            return *found;
        }
        let sym = Sym(u32::try_from(self.names.len()).unwrap_or(u32::MAX));
        self.names.push(text.to_owned());
        self.ids.insert(text.to_owned(), sym);
        sym
    }

    /// The name for `text` if it was interned before. Unlike [`Symbols::intern`], it adds
    /// nothing.
    #[must_use]
    pub fn find(&self, text: &str) -> Option<Sym> {
        self.ids.get(text).copied()
    }

    /// The text behind `sym`. Empty when `sym` came from a different `Symbols` that holds more
    /// names than this one.
    #[must_use]
    pub fn name(&self, sym: Sym) -> &str {
        usize::try_from(sym.0)
            .ok()
            .and_then(|index| self.names.get(index))
            .map_or("", String::as_str)
    }

    /// `value` as the text report prints it: `"text"`, `12`, `true`, `null`, `{a, b}` for a set,
    /// `[a, b]` for a sequence or queue, `{k -> v}` for a map and `{name: value}` for a record.
    #[must_use]
    pub fn show(&self, value: &Value) -> String {
        let mut out = String::new();
        self.write(value, &mut out);
        out
    }

    /// `value` as a chart label prints it: a text value loses its quotes, and everything else is
    /// as [`Symbols::show`] prints it.
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

    /// `value` as the JSON output holds it: a set and a sequence are arrays, a map is an array
    /// of `[key, value]` pairs and a record is an object.
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
