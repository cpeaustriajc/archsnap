use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::ext::IdentExt;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Attribute, Expr, GenericArgument, PathArguments, Token, Type};

const INTEGER_TYPES: [&str; 12] = [
    "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
];

pub(crate) const RANGE_NEEDED: &str = "an integer needs `#[range(lo, hi)]` or `#[one_of(a, b)]`, so that every value it can take is \
     known";

const TEXT_NEEDED: &str =
    "a text needs `#[one_of(\"a\", \"b\")]`, so that every value it can take is known";

pub(crate) struct Slot {
    pub(crate) name: Ident,
    pub(crate) variable: Ident,
    pub(crate) choices: TokenStream,
}

impl Slot {
    pub(crate) fn new(name: &Ident, choices: TokenStream) -> Self {
        Self {
            name: name.clone(),
            variable: format_ident!("earnest_choice_{}", name.unraw()),
            choices,
        }
    }

    pub(crate) fn copy(&self) -> TokenStream {
        let variable = &self.variable;
        quote! { ::core::clone::Clone::clone(&#variable) }
    }
}

pub(crate) fn is_integer(ty: &Type) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    path.qself.is_none()
        && path
            .path
            .get_ident()
            .is_some_and(|name| INTEGER_TYPES.iter().any(|known| name == known))
}

fn range_of(attributes: &[Attribute]) -> syn::Result<Option<TokenStream>> {
    let Some(attribute) = attributes
        .iter()
        .find(|attribute| attribute.path().is_ident("range"))
    else {
        return Ok(None);
    };
    let bounds = attribute.parse_args_with(Punctuated::<Expr, Token![,]>::parse_terminated)?;
    let mut listed = bounds.iter();
    let (Some(lowest), Some(highest), None) = (listed.next(), listed.next(), listed.next()) else {
        return Err(syn::Error::new(
            attribute.span(),
            "`#[range(lo, hi)]` takes the lowest and the highest number, both included",
        ));
    };
    Ok(Some(quote! { (#lowest)..=(#highest) }))
}

pub(crate) fn is_text(ty: &Type) -> bool {
    matches!(ty, Type::Reference(borrowed)
        if matches!(borrowed.elem.as_ref(), Type::Path(path) if path.path.is_ident("str")))
}

fn listed_of(attributes: &[Attribute]) -> syn::Result<Option<TokenStream>> {
    let Some(attribute) = attributes
        .iter()
        .find(|attribute| attribute.path().is_ident("one_of"))
    else {
        return Ok(None);
    };
    let values = attribute.parse_args_with(Punctuated::<Expr, Token![,]>::parse_terminated)?;
    if values.is_empty() {
        return Err(syn::Error::new(
            attribute.span(),
            "`#[one_of(a, b)]` lists the values the field can take",
        ));
    }
    let listed = values.iter();
    Ok(Some(quote! { [#(#listed),*] }))
}

fn optional_listable(ty: &Type) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    let Some(last) = path.path.segments.last() else {
        return false;
    };
    let PathArguments::AngleBracketed(listed) = &last.arguments else {
        return false;
    };
    let mut arguments = listed.args.iter();
    last.ident == "Option"
        && matches!(
            (arguments.next(), arguments.next()),
            (Some(GenericArgument::Type(inner)), None) if is_integer(inner) || is_text(inner)
        )
}

pub(crate) fn ranged_choices(
    attributes: &[Attribute],
    ty: &Type,
) -> syn::Result<Option<TokenStream>> {
    let (range, listed) = (range_of(attributes)?, listed_of(attributes)?);
    let choices = match (range, listed) {
        (Some(_), Some(_)) => {
            return Err(syn::Error::new(
                ty.span(),
                "a number is listed with `#[one_of(...)]` or ranged with `#[range(lo, hi)]`, \
                 never both",
            ));
        }
        (Some(range), None) => range,
        (None, Some(listed)) => quote! { ::core::iter::IntoIterator::into_iter(#listed) },
        (None, None) => return Ok(None),
    };
    if is_integer(ty) || is_text(ty) {
        return Ok(Some(choices));
    }
    if optional_listable(ty) {
        return Ok(Some(quote! {
            ::core::iter::once(::core::option::Option::None)
                .chain((#choices).map(::core::option::Option::Some))
        }));
    }
    Ok(None)
}

pub(crate) fn text_needed(ty: &Type) -> Option<syn::Error> {
    is_text(ty).then(|| syn::Error::new(ty.span(), TEXT_NEEDED))
}

pub(crate) fn domain_choices(attributes: &[Attribute], ty: &Type) -> syn::Result<TokenStream> {
    if let Some(range) = ranged_choices(attributes, ty)? {
        return Ok(range);
    }
    if is_integer(ty) {
        return Err(syn::Error::new(ty.span(), RANGE_NEEDED));
    }
    if is_text(ty) {
        return Err(syn::Error::new(ty.span(), TEXT_NEEDED));
    }
    Ok(quote! { <#ty as ::earnest::model::Domain>::values() })
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fastest {
    First,
    Last,
}

pub(crate) fn for_each_combination(
    slots: &[Slot],
    fastest: Fastest,
    innermost: TokenStream,
) -> TokenStream {
    let order: Vec<&Slot> = match fastest {
        Fastest::First => slots.iter().collect(),
        Fastest::Last => slots.iter().rev().collect(),
    };
    order.into_iter().fold(innermost, |body, slot| {
        let variable = &slot.variable;
        let choices = &slot.choices;
        quote! {
            for #variable in #choices {
                #body
            }
        }
    })
}

pub(crate) fn every_combination(slots: &[Slot]) -> TokenStream {
    let names = slots.iter().map(|slot| &slot.name);
    let copies = slots.iter().map(Slot::copy);
    let loops = for_each_combination(
        slots,
        Fastest::Last,
        quote! {
            earnest_combinations.push(Self { #(#names: #copies),* });
        },
    );
    quote! {
        let mut earnest_combinations = ::std::vec::Vec::new();
        #loops
        earnest_combinations
    }
}
