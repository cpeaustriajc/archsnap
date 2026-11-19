use crate::reader::{Reader, at, attribute, path_text, place_of};
use earnest_ast::{Shape, ShapeKind, TermKind};
use proc_macro2::Span;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Attribute, Expr, GenericArgument, PathArguments, Token, Type, TypePath};

const INTEGER_TYPES: [&str; 12] = [
    "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
];

const NEEDS_ITEM: &str = "name the type of its items between `<` and `>`";

fn shaped(span: Span, kind: ShapeKind) -> Shape {
    Shape {
        place: place_of(span),
        kind,
        initial: None,
    }
}

impl Reader<'_> {
    pub(crate) fn shape(&mut self, ty: &Type, attributes: &[Attribute]) -> Shape {
        let span = ty.span();
        let kind = match ty {
            Type::Paren(inner) => return self.shape(&inner.elem, attributes),
            Type::Group(inner) => return self.shape(&inner.elem, attributes),
            Type::Path(named) if named.qself.is_none() => self.named_shape(named, attributes),
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

    fn named_shape(&mut self, named: &TypePath, attributes: &[Attribute]) -> Option<ShapeKind> {
        let span = named.span();
        let path = path_text(&named.path);
        let name = path.strip_prefix("earnest::model::").unwrap_or(&path);
        let last = named.path.segments.last()?;
        let arguments: Vec<&GenericArgument> = match &last.arguments {
            PathArguments::None => Vec::new(),
            PathArguments::AngleBracketed(listed) => listed.args.iter().collect(),
            PathArguments::Parenthesized(_) => return self.unknown_type(span),
        };
        if INTEGER_TYPES.contains(&name) {
            return self.number_shape(span, name, attributes);
        }
        if let Some(variants) = self.enums.get(name) {
            let choices = variants
                .iter()
                .map(|variant| at(span, TermKind::Text(variant.clone())))
                .collect();
            return Some(ShapeKind::OneOf(choices));
        }
        if self.records.contains_key(name) {
            return self.record_shape(span, name);
        }
        match (name, arguments.as_slice()) {
            ("bool", []) => Some(ShapeKind::Bool),
            ("Timer", []) => Some(ShapeKind::Timer),
            ("Request", []) => Some(ShapeKind::Request),
            ("Lock", []) => Some(ShapeKind::Lock(None)),
            ("Lock", [owner]) => {
                let owned_by = self.item_shape(span, owner, attributes)?;
                Some(ShapeKind::Lock(Some(Box::new(owned_by))))
            }
            ("Option", [inner]) => {
                let item = self.item_shape(span, inner, attributes)?;
                Some(ShapeKind::Optional(Box::new(item)))
            }
            ("Set", [inner]) => {
                let item = self.item_shape(span, inner, attributes)?;
                Some(ShapeKind::Set(Box::new(item)))
            }
            ("Map", [key, value]) => {
                let keys = self.item_shape(span, key, attributes)?;
                let values = self.item_shape(span, value, attributes)?;
                Some(ShapeKind::Map(Box::new(keys), Box::new(values)))
            }
            ("Seq", [inner, most]) => {
                let item = Box::new(self.item_shape(span, inner, attributes)?);
                let max = self.size(span, most)?;
                Some(ShapeKind::Seq { item, max })
            }
            ("Queue", [inner, most]) => {
                let item = Box::new(self.item_shape(span, inner, attributes)?);
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
        attributes: &[Attribute],
    ) -> Option<Shape> {
        if let GenericArgument::Type(ty) = argument {
            return Some(self.shape(ty, attributes));
        }
        let feature = self.short_text(span);
        self.refuse(span, &feature, NEEDS_ITEM);
        None
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
        name: &str,
        attributes: &[Attribute],
    ) -> Option<ShapeKind> {
        let Some(range) = attribute(attributes, "range") else {
            self.refuse(
                span,
                name,
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
        if let Some([Some(lo), Some(hi)]) = bounds.as_deref() {
            return Some(ShapeKind::Int { lo: *lo, hi: *hi });
        }
        let feature = self.short_text(range.span());
        self.refuse(
            range.span(),
            &feature,
            "write the lowest and the highest number, both included, as whole numbers or as \
             constants of this file",
        );
        None
    }

    fn record_shape(&mut self, span: Span, name: &str) -> Option<ShapeKind> {
        if self.open_records.iter().any(|open| open == name) {
            self.refuse(
                span,
                name,
                "a record can't hold itself: it would have no end of values",
            );
            return None;
        }
        let declared = self.records.get(name)?.clone();
        self.open_records.push(name.to_owned());
        let mut fields = Vec::with_capacity(declared.named.len());
        for field in &declared.named {
            if let Some(field_name) = &field.ident {
                fields.push((field_name.to_string(), self.shape(&field.ty, &field.attrs)));
            }
        }
        self.open_records.pop();
        Some(ShapeKind::Record(fields))
    }
}
