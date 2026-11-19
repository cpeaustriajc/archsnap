use crate::reader::{Reader, at, attribute, path_text, place_of};
use earnest_ast::{Shape, ShapeKind, Term, TermKind};
use earnest_value::IntType;
use proc_macro2::Span;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Attribute, Expr, GenericArgument, PathArguments, Token, Type, TypePath};

const NEEDS_ITEM: &str = "name the type of its items between `<` and `>`";

#[derive(Clone, Copy)]
pub(crate) enum Declares<'a> {
    Values(&'a [Attribute]),
    Type,
}

impl Declares<'_> {
    fn attributes(&self) -> &[Attribute] {
        match self {
            Self::Values(attributes) => attributes,
            Self::Type => &[],
        }
    }
}

fn is_optional(shape: &Shape) -> bool {
    matches!(shape.kind, ShapeKind::Optional(_))
}

fn shaped(span: Span, kind: ShapeKind) -> Shape {
    Shape {
        place: place_of(span),
        kind,
        initial: None,
    }
}

impl Reader<'_> {
    pub(crate) fn shape(&mut self, ty: &Type, attributes: &[Attribute]) -> Shape {
        self.declared_shape(ty, Declares::Values(attributes))
    }

    pub(crate) fn type_shape(&mut self, ty: &Type) -> Shape {
        self.declared_shape(ty, Declares::Type)
    }

    pub(crate) fn of_type(&mut self, ty: &Type, value: Term) -> Term {
        let shape = self.type_shape(ty);
        Term::new(
            value.place,
            TermKind::Declared {
                shape: Box::new(shape),
                value: Box::new(value),
            },
        )
    }

    fn declared_shape(&mut self, ty: &Type, declares: Declares<'_>) -> Shape {
        let span = ty.span();
        let kind = match ty {
            Type::Paren(inner) => return self.declared_shape(&inner.elem, declares),
            Type::Group(inner) => return self.declared_shape(&inner.elem, declares),
            Type::Reference(borrowed)
                if matches!(declares, Declares::Type) && borrowed.mutability.is_none() =>
            {
                return self.declared_shape(&borrowed.elem, declares);
            }
            Type::Path(named) if named.qself.is_none() => self.named_shape(named, declares),
            _ => self.unknown_type(span),
        };
        if let Some(found) = kind {
            return shaped(span, found);
        }
        shaped(span, ShapeKind::Bool)
    }

    fn unknown_type(&mut self, span: Span) -> Option<ShapeKind> {
        let feature = self.short_text(span);
        self.refuse(
            span,
            &feature,
            "it is not a library type or a type in this file that derives `Domain`",
        );
        None
    }

    fn named_shape(&mut self, named: &TypePath, declares: Declares<'_>) -> Option<ShapeKind> {
        let span = named.span();
        let path = path_text(&named.path);
        let name = path.strip_prefix("earnest::model::").unwrap_or(&path);
        let last = named.path.segments.last()?;
        let arguments: Vec<&GenericArgument> = match &last.arguments {
            PathArguments::None => Vec::new(),
            PathArguments::AngleBracketed(listed) => listed.args.iter().collect(),
            PathArguments::Parenthesized(_) => return self.unknown_type(span),
        };
        if let Some(ty) = IntType::named(name) {
            return self.number_shape(span, ty, declares);
        }
        if let Some(variants) = self.enums.get(name) {
            let choices = variants
                .iter()
                .map(|variant| at(span, TermKind::Text(variant.clone())))
                .collect();
            return Some(ShapeKind::OneOf(choices));
        }
        if self.records.contains_key(name) {
            return self.record_shape(span, name, declares);
        }
        match (name, arguments.as_slice()) {
            ("bool", []) => Some(ShapeKind::Bool),
            ("Timer", []) => Some(ShapeKind::Timer),
            ("Request", []) => Some(ShapeKind::Request),
            ("Lock", []) => Some(ShapeKind::Lock(None)),
            ("Lock", [owner]) => {
                let owned_by = self.item_shape(span, owner, declares)?;
                Some(ShapeKind::Lock(Some(Box::new(owned_by))))
            }
            ("Option", [inner]) => {
                let item = self.item_shape(span, inner, declares)?;
                Some(ShapeKind::Optional(Box::new(item)))
            }
            ("Set", [inner]) => {
                let item = self.item_shape(span, inner, declares)?;
                Some(ShapeKind::Set(Box::new(item)))
            }
            ("Map", [key, value]) => {
                let keys = self.item_shape(span, key, declares)?;
                let values = self.held_shape(value, "Map", "get", declares)?;
                Some(ShapeKind::Map(Box::new(keys), Box::new(values)))
            }
            ("Seq", [inner, most]) => {
                let item = Box::new(self.held_shape(inner, "Seq", "first", declares)?);
                let max = self.size(span, most)?;
                Some(ShapeKind::Seq { item, max })
            }
            ("Queue", [inner, most]) => {
                let item = Box::new(self.held_shape(inner, "Queue", "head", declares)?);
                let capacity = self.size(span, most)?;
                Some(ShapeKind::Queue { item, capacity })
            }
            ("Retry", [most]) => {
                let max = i64::try_from(self.size(span, most)?).ok()?;
                Some(ShapeKind::Retry { max })
            }
            _ => self.unknown_type(span),
        }
    }

    fn item_shape(
        &mut self,
        span: Span,
        argument: &GenericArgument,
        declares: Declares<'_>,
    ) -> Option<Shape> {
        if let GenericArgument::Type(ty) = argument {
            return Some(self.declared_shape(ty, declares));
        }
        let feature = self.short_text(span);
        self.refuse(span, &feature, NEEDS_ITEM);
        None
    }

    fn held_shape(
        &mut self,
        argument: &GenericArgument,
        holder: &str,
        read: &str,
        declares: Declares<'_>,
    ) -> Option<Shape> {
        let span = argument.span();
        let held = self.item_shape(span, argument, declares)?;
        if is_optional(&held) {
            let feature = self.short_text(span);
            self.refuse(
                span,
                &feature,
                &format!(
                    "a `{holder}` can't hold an `Option`: `{read}()` would give `Some(None)`, \
                     which the checker can't tell from `None`. Hold a struct with an `Option` \
                     field, or leave the entry out"
                ),
            );
            return None;
        }
        Some(held)
    }

    fn size(&mut self, span: Span, argument: &GenericArgument) -> Option<usize> {
        let number = match argument {
            GenericArgument::Const(expr) => self.whole_number(expr),
            GenericArgument::Type(Type::Path(name)) => name
                .path
                .get_ident()
                .and_then(|ident| self.numbers.get(&ident.to_string()).copied()),
            _ => None,
        };
        let size = number.and_then(|found| usize::try_from(found).ok());
        if size.is_none() {
            let feature = self.short_text(span);
            self.refuse(
                span,
                &feature,
                "its size must be a whole number written here or as a constant in this file",
            );
        }
        size
    }

    fn number_shape(
        &mut self,
        span: Span,
        ty: IntType,
        declares: Declares<'_>,
    ) -> Option<ShapeKind> {
        let Some(range) = attribute(declares.attributes(), "range") else {
            if matches!(declares, Declares::Type) {
                return Some(ShapeKind::Int {
                    lo: ty.lowest(),
                    hi: ty.highest(),
                    ty: Some(ty),
                });
            }
            self.refuse(
                span,
                ty.name(),
                "a number needs `#[range(lo, hi)]`, so the checker knows every value it can take",
            );
            return None;
        };
        let bounds = range
            .parse_args_with(Punctuated::<Expr, Token![,]>::parse_terminated)
            .ok()
            .map(|listed| {
                listed
                    .iter()
                    .map(|bound| self.whole_number(bound))
                    .collect::<Vec<_>>()
            });
        let Some([Some(lo), Some(hi)]) = bounds.as_deref() else {
            let feature = self.short_text(range.span());
            self.refuse(
                range.span(),
                &feature,
                "write the lowest and the highest number, both included, as whole numbers or as \
                 constants of this file",
            );
            return None;
        };
        if !ty.holds(*lo) || !ty.holds(*hi) {
            let feature = self.short_text(range.span());
            self.refuse(
                range.span(),
                &feature,
                &format!(
                    "a `{}` holds {} to {}, so the range must lie inside that",
                    ty.name(),
                    ty.lowest(),
                    ty.highest()
                ),
            );
            return None;
        }
        Some(ShapeKind::Int {
            lo: *lo,
            hi: *hi,
            ty: Some(ty),
        })
    }

    fn record_shape(
        &mut self,
        span: Span,
        name: &str,
        declares: Declares<'_>,
    ) -> Option<ShapeKind> {
        if self.open_records.iter().any(|open| open == name) {
            self.refuse(
                span,
                name,
                "a record can't hold itself: it would have no end of values",
            );
            return None;
        }
        let record = self.records.get(name)?.clone();
        self.open_records.push(name.to_owned());
        let mut fields = Vec::with_capacity(record.named.len());
        for field in &record.named {
            if let Some(field_name) = &field.ident {
                let shape = match declares {
                    Declares::Values(_) => self.shape(&field.ty, &field.attrs),
                    Declares::Type => self.type_shape(&field.ty),
                };
                fields.push((field_name.to_string(), shape));
            }
        }
        self.open_records.pop();
        Some(ShapeKind::Record(fields))
    }
}
