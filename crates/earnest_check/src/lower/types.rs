use crate::ir::Expr;

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
            Self::Unknown => "value",
            Self::Null => "null",
            Self::Bool => "boolean",
            Self::Int => "number",
            Self::Text => "text value",
            Self::Optional(_) => "optional value",
            Self::Record(_) => "record",
            Self::Set(_) => "set",
            Self::Map(_, _) => "map",
            Self::Seq(_) => "sequence",
            Self::Queue(_, _) => "queue",
            Self::Lock(_) => "lock",
            Self::Timer => "timer",
            Self::Request => "request",
            Self::Retry(_) => "retry counter",
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
