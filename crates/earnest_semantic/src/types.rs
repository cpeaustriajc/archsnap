use earnest_ir::Expr;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Type {
    Unknown,
    Null,
    Bool,
    Int,
    Text,
    Optional(Box<Type>),
    Record(Vec<(String, Type)>),
    Set(Box<Type>),
    Map(Box<Type>, Box<Type>),
    Seq(Box<Type>),
    Queue(Box<Type>, usize),
    Lock(Option<Box<Type>>),
    Timer,
    Request,
    Retry(i64),
}

impl Type {
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Unknown => "a value",
            Self::Null => "null",
            Self::Bool => "a boolean",
            Self::Int => "a number",
            Self::Text => "a text value",
            Self::Optional(_) => "an optional value",
            Self::Record(_) => "a record",
            Self::Set(_) => "a set",
            Self::Map(_, _) => "a map",
            Self::Seq(_) => "a sequence",
            Self::Queue(_, _) => "a queue",
            Self::Lock(_) => "a lock",
            Self::Timer => "a timer",
            Self::Request => "a request",
            Self::Retry(_) => "a retry counter",
        }
    }

    pub(crate) fn fits(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Unknown, _)
            | (_, Self::Unknown)
            | (Self::Null | Self::Optional(_), Self::Null)
            | (Self::Null, Self::Optional(_))
            | (Self::Bool, Self::Bool)
            | (Self::Int, Self::Int)
            | (Self::Text, Self::Text)
            | (Self::Timer, Self::Timer)
            | (Self::Request, Self::Request)
            | (Self::Retry(_), Self::Retry(_))
            | (Self::Lock(None), Self::Lock(None)) => true,
            (Self::Optional(left), Self::Optional(right))
            | (Self::Set(left), Self::Set(right))
            | (Self::Lock(Some(left)), Self::Lock(Some(right)))
            | (Self::Seq(left) | Self::Queue(left, _), Self::Seq(right) | Self::Queue(right, _)) => {
                left.fits(right)
            }
            (Self::Map(left_key, left_value), Self::Map(right_key, right_value)) => {
                left_key.fits(right_key) && left_value.fits(right_value)
            }
            (Self::Record(left), Self::Record(right)) => {
                left.len() == right.len() && left.iter().zip(right.iter()).all(field_fits)
            }
            _ => false,
        }
    }

    pub(crate) fn merged(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Unknown | Self::Null, found) => found.clone(),
            (Self::Optional(left), Self::Optional(right)) => {
                Self::Optional(Box::new(left.merged(right)))
            }
            (Self::Set(left), Self::Set(right)) => Self::Set(Box::new(left.merged(right))),
            (Self::Seq(left), Self::Seq(right) | Self::Queue(right, _)) => {
                Self::Seq(Box::new(left.merged(right)))
            }
            (Self::Queue(left, capacity), Self::Seq(right) | Self::Queue(right, _)) => {
                Self::Queue(Box::new(left.merged(right)), *capacity)
            }
            (Self::Map(left_key, left_value), Self::Map(right_key, right_value)) => Self::Map(
                Box::new(left_key.merged(right_key)),
                Box::new(left_value.merged(right_value)),
            ),
            (Self::Record(left), Self::Record(right)) => Self::Record(
                left.iter()
                    .zip(right.iter())
                    .map(|((name, left_type), (_, right_type))| {
                        (name.clone(), left_type.merged(right_type))
                    })
                    .collect(),
            ),
            (kept, _) => kept.clone(),
        }
    }

    pub(crate) fn record(mut fields: Vec<(String, Self)>) -> Self {
        fields.sort_by(|(left, _), (right, _)| left.cmp(right));
        Self::Record(fields)
    }

    pub(crate) fn optional(inner: Self) -> Self {
        Self::Optional(Box::new(inner))
    }
}

fn field_fits(pair: (&(String, Type), &(String, Type))) -> bool {
    let ((left_name, left_type), (right_name, right_type)) = pair;
    left_name == right_name && left_type.fits(right_type)
}

#[derive(Clone, Debug)]
pub(crate) struct Typed {
    pub(crate) expr: Expr,
    pub(crate) ty: Type,
}

impl Typed {
    pub(crate) fn new(expr: Expr, ty: Type) -> Self {
        Self { expr, ty }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Binding {
    State,
    Inputs,
    Local(usize, Type),
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Scope {
    pub(crate) names: Vec<(String, Binding)>,
    pub(crate) inputs: Vec<(String, usize, Type)>,
    pub(crate) slots: usize,
}

impl Scope {
    pub(crate) fn starting_at(slots: usize) -> Self {
        Self {
            names: Vec::new(),
            inputs: Vec::new(),
            slots,
        }
    }

    pub(crate) fn slot(&mut self) -> usize {
        let taken = self.slots;
        self.slots = taken.saturating_add(1);
        taken
    }

    pub(crate) fn find(&self, name: &str) -> Option<Binding> {
        self.names
            .iter()
            .rev()
            .find(|(known, _)| known == name)
            .map(|(_, binding)| binding.clone())
    }

    pub(crate) fn input(&self, name: &str) -> Option<(usize, Type)> {
        self.inputs
            .iter()
            .find(|(known, _, _)| known == name)
            .map(|(_, slot, ty)| (*slot, ty.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::{Binding, Scope, Type};

    fn set(item: Type) -> Type {
        Type::Set(Box::new(item))
    }

    fn record(fields: &[(&str, Type)]) -> Type {
        Type::record(
            fields
                .iter()
                .map(|(name, ty)| ((*name).to_owned(), ty.clone()))
                .collect(),
        )
    }

    #[test]
    fn a_plain_kind_fits_only_itself() {
        assert!(Type::Bool.fits(&Type::Bool));
        assert!(Type::Int.fits(&Type::Int));
        assert!(!Type::Int.fits(&Type::Bool));
        assert!(!Type::Text.fits(&Type::Int));
        assert!(!Type::Timer.fits(&Type::Request));
    }

    #[test]
    fn a_kind_not_yet_known_fits_anything() {
        assert!(Type::Unknown.fits(&Type::Timer));
        assert!(set(Type::Unknown).fits(&set(Type::Int)));
        assert!(set(Type::Int).fits(&set(Type::Unknown)));
    }

    #[test]
    fn null_fits_an_optional_kind_but_no_plain_one() {
        assert!(Type::Null.fits(&Type::optional(Type::Int)));
        assert!(Type::optional(Type::Int).fits(&Type::Null));
        assert!(Type::Null.fits(&Type::Null));
        assert!(!Type::Null.fits(&Type::Int));
        assert!(!Type::Int.fits(&Type::Null));
    }

    #[test]
    fn a_plain_kind_does_not_fit_its_optional_kind_without_wrapping() {
        assert!(!Type::Int.fits(&Type::optional(Type::Int)));
        assert!(!Type::optional(Type::Int).fits(&Type::Int));
    }

    #[test]
    fn collections_fit_when_their_items_do() {
        assert!(set(Type::Int).fits(&set(Type::Int)));
        assert!(!set(Type::Int).fits(&set(Type::Text)));
        assert!(!set(Type::Int).fits(&Type::Seq(Box::new(Type::Int))));
    }

    #[test]
    fn a_sequence_and_a_queue_of_the_same_items_fit_each_other() {
        let sequence = Type::Seq(Box::new(Type::Int));
        let queue = Type::Queue(Box::new(Type::Int), 3);
        assert!(sequence.fits(&queue));
        assert!(queue.fits(&sequence));
        assert!(!queue.fits(&Type::Queue(Box::new(Type::Bool), 3)));
    }

    #[test]
    fn records_fit_by_field_name_whatever_the_order_they_were_written_in() {
        let written = record(&[("y", Type::Int), ("x", Type::Bool)]);
        let declared = record(&[("x", Type::Bool), ("y", Type::Int)]);
        assert!(written.fits(&declared));
        assert!(!written.fits(&record(&[("x", Type::Bool)])));
        assert!(!written.fits(&record(&[("x", Type::Bool), ("z", Type::Int)])));
        assert!(!written.fits(&record(&[("x", Type::Int), ("y", Type::Int)])));
    }

    #[test]
    fn a_lock_with_an_owner_does_not_fit_one_without() {
        let owned = Type::Lock(Some(Box::new(Type::Text)));
        assert!(owned.fits(&owned));
        assert!(Type::Lock(None).fits(&Type::Lock(None)));
        assert!(!owned.fits(&Type::Lock(None)));
    }

    #[test]
    fn merging_fills_in_what_one_side_does_not_know() {
        assert_eq!(Type::Unknown.merged(&Type::Int), Type::Int);
        assert_eq!(Type::Int.merged(&Type::Unknown), Type::Int);
        assert_eq!(set(Type::Unknown).merged(&set(Type::Int)), set(Type::Int));
        let queue = Type::Queue(Box::new(Type::Unknown), 3);
        let sequence = Type::Seq(Box::new(Type::Int));
        assert_eq!(queue.merged(&sequence), Type::Queue(Box::new(Type::Int), 3));
    }

    #[test]
    fn each_kind_reads_as_a_noun_with_its_article() {
        assert_eq!(Type::Int.kind(), "a number");
        assert_eq!(Type::optional(Type::Int).kind(), "an optional value");
        assert_eq!(Type::Retry(2).kind(), "a retry counter");
    }

    #[test]
    fn slots_are_handed_out_in_order_and_never_reused() {
        let mut scope = Scope::starting_at(4);
        assert_eq!(scope.slot(), 4);
        assert_eq!(scope.slot(), 5);
        assert_eq!(scope.slots, 6);
    }

    #[test]
    fn the_innermost_binding_of_a_name_is_the_one_found() {
        let mut scope = Scope::default();
        scope.names.push(("x".to_owned(), Binding::State));
        scope
            .names
            .push(("x".to_owned(), Binding::Local(2, Type::Int)));
        assert!(matches!(
            scope.find("x"),
            Some(Binding::Local(2, Type::Int))
        ));
        assert!(scope.find("y").is_none());
    }

    #[test]
    fn an_input_is_found_by_name_with_its_slot() {
        let mut scope = Scope::default();
        scope.inputs.push(("by".to_owned(), 1, Type::Text));
        assert_eq!(scope.input("by"), Some((1, Type::Text)));
        assert_eq!(scope.input("who"), None);
    }
}
