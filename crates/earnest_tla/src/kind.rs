use earnest_value::{Domain, Sym, Value};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Unknown,
    Bool,
    Int,
    Text,
    Optional(Box<Kind>),
    Record(Vec<(Sym, Kind)>),
    Set(Box<Kind>),
    Map(Box<Kind>, Box<Kind>),
    Seq(Box<Kind>),
}

impl Kind {
    pub(crate) fn optional(inner: Self) -> Self {
        Self::Optional(Box::new(inner))
    }

    pub(crate) fn set(item: Self) -> Self {
        Self::Set(Box::new(item))
    }

    pub(crate) fn seq(item: Self) -> Self {
        Self::Seq(Box::new(item))
    }

    pub(crate) fn map(key: Self, value: Self) -> Self {
        Self::Map(Box::new(key), Box::new(value))
    }

    pub(crate) fn of_value(value: &Value) -> Self {
        match value {
            Value::Null => Self::optional(Self::Unknown),
            Value::Bool(_) => Self::Bool,
            Value::Int(_) => Self::Int,
            Value::Str(_) => Self::Text,
            Value::Record(fields) => Self::Record(
                fields
                    .iter()
                    .map(|(name, inner)| (*name, Self::of_value(inner)))
                    .collect(),
            ),
            Value::Set(items) => Self::set(Self::of_values(items.iter())),
            Value::Map(entries) => Self::map(
                Self::of_values(entries.keys()),
                Self::of_values(entries.values()),
            ),
            Value::Seq(items) => Self::seq(Self::of_values(items.iter())),
            _ => Self::Unknown,
        }
    }

    pub(crate) fn of_values<'a>(values: impl Iterator<Item = &'a Value>) -> Self {
        values.fold(Self::Unknown, |kind, value| {
            kind.join(&Self::of_value(value))
        })
    }

    pub(crate) fn of_domain(domain: &Domain) -> Self {
        match domain {
            Domain::Bool => Self::Bool,
            Domain::Int { .. } => Self::Int,
            Domain::OneOf(values) => Self::of_values(values.iter()),
            Domain::Optional(inner) => Self::optional(Self::of_domain(inner)),
            Domain::Record(fields) => Self::Record(
                fields
                    .iter()
                    .map(|(name, inner)| (*name, Self::of_domain(inner)))
                    .collect(),
            ),
            Domain::Set(item) => Self::set(Self::of_domain(item)),
            Domain::Map(key, value) => Self::map(Self::of_domain(key), Self::of_domain(value)),
            Domain::Seq { item, .. } => Self::seq(Self::of_domain(item)),
            _ => Self::Unknown,
        }
    }

    pub(crate) fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Unknown, found) | (found, Self::Unknown) => found.clone(),
            (Self::Optional(left), Self::Optional(right)) => Self::optional(left.join(right)),
            (Self::Optional(inner), bare) | (bare, Self::Optional(inner)) => {
                Self::optional(inner.join(bare))
            }
            (Self::Set(left), Self::Set(right)) => Self::set(left.join(right)),
            (Self::Seq(left), Self::Seq(right)) => Self::seq(left.join(right)),
            (Self::Map(left_key, left_value), Self::Map(right_key, right_value)) => {
                Self::map(left_key.join(right_key), left_value.join(right_value))
            }
            (Self::Record(left), Self::Record(right)) => Self::Record(joined_fields(left, right)),
            (kept, _) => kept.clone(),
        }
    }

    pub(crate) fn is_optional(&self) -> bool {
        matches!(self, Self::Optional(_))
    }

    pub(crate) fn once_optional(self) -> Self {
        if self.is_optional() {
            self
        } else {
            Self::optional(self)
        }
    }

    pub(crate) fn present(&self) -> Self {
        match self {
            Self::Optional(inner) => (**inner).clone(),
            _ => Self::Unknown,
        }
    }

    pub(crate) fn item(&self) -> Self {
        match self {
            Self::Set(item) | Self::Seq(item) => (**item).clone(),
            _ => Self::Unknown,
        }
    }

    pub(crate) fn key(&self) -> Self {
        match self {
            Self::Map(key, _) => (**key).clone(),
            _ => Self::Unknown,
        }
    }

    pub(crate) fn value(&self) -> Self {
        match self {
            Self::Map(_, value) => (**value).clone(),
            _ => Self::Unknown,
        }
    }

    pub(crate) fn field(&self, name: Sym) -> Self {
        match self {
            Self::Record(fields) => fields
                .iter()
                .find(|(known, _)| *known == name)
                .map_or(Self::Unknown, |(_, kind)| kind.clone()),
            _ => Self::Unknown,
        }
    }
}

fn joined_fields(left: &[(Sym, Kind)], right: &[(Sym, Kind)]) -> Vec<(Sym, Kind)> {
    let mut fields: Vec<(Sym, Kind)> = left
        .iter()
        .map(|(name, kind)| {
            let other = right.iter().find(|(known, _)| known == name);
            let both = other.map_or_else(|| kind.clone(), |(_, found)| kind.join(found));
            (*name, both)
        })
        .collect();
    for (name, kind) in right {
        if !left.iter().any(|(known, _)| known == name) {
            fields.push((*name, kind.clone()));
        }
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::Kind;
    use earnest_value::{Domain, Symbols, Value};
    use std::collections::{BTreeMap, BTreeSet};

    fn set_of(items: Vec<Value>) -> Value {
        Value::set(items.into_iter().collect::<BTreeSet<Value>>())
    }

    #[test]
    fn joining_fills_in_what_one_side_does_not_know() {
        assert_eq!(Kind::Unknown.join(&Kind::Int), Kind::Int);
        assert_eq!(Kind::Int.join(&Kind::Unknown), Kind::Int);
        assert_eq!(
            Kind::seq(Kind::Unknown).join(&Kind::seq(Kind::Text)),
            Kind::seq(Kind::Text)
        );
        assert_eq!(
            Kind::map(Kind::Unknown, Kind::Int).join(&Kind::map(Kind::Text, Kind::Unknown)),
            Kind::map(Kind::Text, Kind::Int)
        );
    }

    #[test]
    fn joining_null_with_a_present_value_gives_an_optional_of_it() {
        let null = Kind::of_value(&Value::Null);
        assert_eq!(null, Kind::optional(Kind::Unknown));
        assert_eq!(null.join(&Kind::Text), Kind::optional(Kind::Text));
        assert_eq!(Kind::Text.join(&null), Kind::optional(Kind::Text));
        assert_eq!(
            Kind::optional(Kind::Text).join(&Kind::Text),
            Kind::optional(Kind::Text)
        );
    }

    #[test]
    fn two_kinds_that_do_not_fit_keep_the_first() {
        assert_eq!(Kind::Int.join(&Kind::Text), Kind::Int);
        assert_eq!(
            Kind::set(Kind::Int).join(&Kind::seq(Kind::Int)),
            Kind::set(Kind::Int)
        );
    }

    #[test]
    fn records_join_field_by_field_and_keep_the_fields_of_both() {
        let mut symbols = Symbols::default();
        let a = symbols.intern("a");
        let b = symbols.intern("b");
        let left = Kind::Record(vec![(a, Kind::optional(Kind::Unknown))]);
        let right = Kind::Record(vec![(a, Kind::Int), (b, Kind::Bool)]);
        assert_eq!(
            left.join(&right),
            Kind::Record(vec![(a, Kind::optional(Kind::Int)), (b, Kind::Bool)])
        );
    }

    #[test]
    fn a_set_that_holds_null_is_a_set_of_optional_items() {
        let mut symbols = Symbols::default();
        let text = Value::Str(symbols.intern("a"));
        let every = set_of(vec![Value::Null, text]);
        assert_eq!(
            Kind::of_value(&every),
            Kind::set(Kind::optional(Kind::Text))
        );
    }

    #[test]
    fn an_empty_collection_does_not_know_its_items() {
        assert_eq!(
            Kind::of_value(&Value::seq(Vec::new())),
            Kind::seq(Kind::Unknown)
        );
        assert_eq!(
            Kind::of_value(&Value::map(BTreeMap::new())),
            Kind::map(Kind::Unknown, Kind::Unknown)
        );
    }

    #[test]
    fn a_domain_has_the_kind_of_its_values() {
        let mut symbols = Symbols::default();
        let name = symbols.intern("qty");
        let choices = Domain::OneOf(vec![Value::Str(symbols.intern("x"))]);
        assert_eq!(Kind::of_domain(&Domain::Bool), Kind::Bool);
        assert_eq!(Kind::of_domain(&Domain::Int { lo: 0, hi: 3 }), Kind::Int);
        assert_eq!(Kind::of_domain(&choices), Kind::Text);
        assert_eq!(
            Kind::of_domain(&Domain::Optional(Box::new(Domain::Record(vec![(
                name,
                Domain::Int { lo: 1, hi: 2 }
            )])))),
            Kind::optional(Kind::Record(vec![(name, Kind::Int)]))
        );
        assert_eq!(
            Kind::of_domain(&Domain::Map(
                Box::new(choices.clone()),
                Box::new(Domain::Bool)
            )),
            Kind::map(Kind::Text, Kind::Bool)
        );
        assert_eq!(
            Kind::of_domain(&Domain::Seq {
                item: Box::new(choices.clone()),
                max: 2
            }),
            Kind::seq(Kind::Text)
        );
        assert_eq!(
            Kind::of_domain(&Domain::Set(Box::new(choices))),
            Kind::set(Kind::Text)
        );
    }

    #[test]
    fn the_parts_of_a_kind_are_unknown_where_it_has_none() {
        assert_eq!(Kind::Int.item(), Kind::Unknown);
        assert_eq!(Kind::Int.key(), Kind::Unknown);
        assert_eq!(Kind::Int.value(), Kind::Unknown);
        assert_eq!(Kind::Int.present(), Kind::Unknown);
        assert_eq!(Kind::map(Kind::Text, Kind::Int).key(), Kind::Text);
        assert_eq!(Kind::map(Kind::Text, Kind::Int).value(), Kind::Int);
        assert_eq!(Kind::optional(Kind::Int).present(), Kind::Int);
    }

    #[test]
    fn an_optional_kind_is_not_made_optional_twice() {
        assert_eq!(Kind::Int.once_optional(), Kind::optional(Kind::Int));
        assert_eq!(
            Kind::optional(Kind::Int).once_optional(),
            Kind::optional(Kind::Int)
        );
    }
}
