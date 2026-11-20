use crate::reader::{Reader, name_of};
use earnest_value::IntType;
use syn::{GenericArgument, PathArguments, Type};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Ty {
    Unknown,
    Never,
    Bool,
    Int(Option<IntType>),
    Text,
    Enum(String),
    Record(String),
    Option(Box<Ty>),
    Set(Box<Ty>),
    Seq(Box<Ty>),
    Queue(Box<Ty>),
    Map(Box<Ty>, Box<Ty>),
    Lock(Option<Box<Ty>>),
    Timer,
    Request,
    Retry,
    Ref(Box<Ty>),
}

impl Ty {
    pub(crate) fn peeled(&self) -> &Self {
        match self {
            Self::Ref(inner) => inner.peeled(),
            other => other,
        }
    }

    pub(crate) fn is_ref(&self) -> bool {
        matches!(self, Self::Ref(_))
    }

    pub(crate) fn is_known(&self) -> bool {
        match self {
            Self::Unknown | Self::Never => false,
            Self::Ref(inner) | Self::Option(inner) => inner.is_known(),
            _ => true,
        }
    }

    pub(crate) fn option_of(inner: Self) -> Self {
        Self::Option(Box::new(inner))
    }

    pub(crate) fn inside(&self) -> Self {
        match self.peeled() {
            Self::Option(inner) => (**inner).clone(),
            _ => Self::Unknown,
        }
    }

    pub(crate) fn item(&self) -> Self {
        match self.peeled() {
            Self::Set(item) | Self::Seq(item) | Self::Queue(item) => (**item).clone(),
            Self::Map(key, _) => (**key).clone(),
            _ => Self::Unknown,
        }
    }

    pub(crate) fn joined(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Unknown | Self::Never, found) => found.clone(),
            (Self::Int(None), Self::Int(typed)) => Self::Int(*typed),
            (Self::Option(left), Self::Option(right)) => Self::option_of(left.joined(right)),
            (kept, _) => kept.clone(),
        }
    }

    pub(crate) fn compatible(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Unknown | Self::Never, _) | (_, Self::Unknown | Self::Never) => true,
            (Self::Ref(left), right) => left.compatible(right),
            (left, Self::Ref(right)) => left.compatible(right),
            (Self::Int(Some(left)), Self::Int(Some(right))) => left == right,
            (Self::Int(_), Self::Int(_))
            | (Self::Bool, Self::Bool)
            | (Self::Text, Self::Text)
            | (Self::Timer, Self::Timer)
            | (Self::Request, Self::Request)
            | (Self::Retry, Self::Retry) => true,
            (Self::Enum(left), Self::Enum(right)) | (Self::Record(left), Self::Record(right)) => {
                left == right
            }
            (Self::Option(left), Self::Option(right))
            | (Self::Set(left), Self::Set(right))
            | (Self::Seq(left), Self::Seq(right))
            | (Self::Queue(left), Self::Queue(right)) => left.compatible(right),
            (Self::Lock(left), Self::Lock(right)) => match (left, right) {
                (Some(left), Some(right)) => left.compatible(right),
                (None, None) => true,
                _ => false,
            },
            (Self::Map(left_key, left_value), Self::Map(right_key, right_value)) => {
                left_key.compatible(right_key) && left_value.compatible(right_value)
            }
            _ => false,
        }
    }

    pub(crate) fn shown(&self) -> String {
        match self {
            Self::Unknown | Self::Never => "a value".to_owned(),
            Self::Bool => "a `bool`".to_owned(),
            Self::Int(Some(ty)) => format!("a `{}`", ty.name()),
            Self::Int(None) => "a number".to_owned(),
            Self::Text => "a `&'static str`".to_owned(),
            Self::Enum(name) | Self::Record(name) => format!("a `{name}`"),
            Self::Option(inner) => format!("an `Option` of {}", inner.shown()),
            Self::Set(item) => format!("a `Set` of {}", item.shown()),
            Self::Seq(item) => format!("a `Seq` of {}", item.shown()),
            Self::Queue(item) => format!("a `Queue` of {}", item.shown()),
            Self::Map(key, value) => format!("a `Map` of {} to {}", key.shown(), value.shown()),
            Self::Lock(_) => "a `Lock`".to_owned(),
            Self::Timer => "a `Timer`".to_owned(),
            Self::Request => "a `Request`".to_owned(),
            Self::Retry => "a `Retry`".to_owned(),
            Self::Ref(inner) => inner.shown(),
        }
    }
}

impl Reader<'_> {
    pub(crate) fn ty_of(&self, ty: &Type) -> Ty {
        match ty {
            Type::Paren(inner) => self.ty_of(&inner.elem),
            Type::Group(inner) => self.ty_of(&inner.elem),
            Type::Reference(borrowed) => match borrowed.elem.as_ref() {
                Type::Path(named) if named.path.is_ident("str") => Ty::Text,
                other => Ty::Ref(Box::new(self.ty_of(other))),
            },
            Type::Path(named) if named.qself.is_none() => self.path_ty(named),
            _ => Ty::Unknown,
        }
    }

    fn path_ty(&self, named: &syn::TypePath) -> Ty {
        let Some(last) = named.path.segments.last() else {
            return Ty::Unknown;
        };
        let name = name_of(&last.ident);
        let arguments: Vec<Ty> = match &last.arguments {
            PathArguments::AngleBracketed(listed) => listed
                .args
                .iter()
                .filter_map(|argument| match argument {
                    GenericArgument::Type(inner) => Some(self.ty_of(inner)),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        let first = arguments.first().cloned().unwrap_or(Ty::Unknown);
        match name.as_str() {
            "bool" => Ty::Bool,
            "Option" => Ty::option_of(first),
            "Set" => Ty::Set(Box::new(first)),
            "Seq" => Ty::Seq(Box::new(first)),
            "Queue" => Ty::Queue(Box::new(first)),
            "Map" => Ty::Map(
                Box::new(first),
                Box::new(arguments.get(1).cloned().unwrap_or(Ty::Unknown)),
            ),
            "Lock" => Ty::Lock(arguments.first().cloned().map(Box::new)),
            "Timer" => Ty::Timer,
            "Request" => Ty::Request,
            "Retry" => Ty::Retry,
            "Self" => self.model_name.clone().map_or(Ty::Unknown, Ty::Record),
            other if IntType::named(other).is_some() => Ty::Int(IntType::named(other)),
            other if self.enums.contains_key(other) => Ty::Enum(other.to_owned()),
            other if self.structs.contains_key(other) || self.records.contains_key(other) => {
                Ty::Record(other.to_owned())
            }
            _ => Ty::Unknown,
        }
    }

    pub(crate) fn is_copy_ty(&self, ty: &Ty) -> bool {
        match ty {
            Ty::Enum(name) | Ty::Record(name) => self.copies.contains(name),
            Ty::Option(inner) => self.is_copy_ty(inner),
            Ty::Lock(Some(owner)) => self.is_copy_ty(owner),
            Ty::Set(_) | Ty::Seq(_) | Ty::Queue(_) | Ty::Map(_, _) => false,
            Ty::Unknown
            | Ty::Never
            | Ty::Bool
            | Ty::Int(_)
            | Ty::Text
            | Ty::Lock(None)
            | Ty::Timer
            | Ty::Request
            | Ty::Retry
            | Ty::Ref(_) => true,
        }
    }
}
