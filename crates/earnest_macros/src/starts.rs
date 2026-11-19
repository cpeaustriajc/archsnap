use crate::choices::{RANGE_NEEDED, Slot, every_combination, is_integer, ranged_choices};
use proc_macro2::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::{Data, DeriveInput, Expr, Field, Fields};

pub(crate) fn expand(input: TokenStream) -> syn::Result<TokenStream> {
    let item: DeriveInput = syn::parse2(input)?;
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new(
            item.generics.span(),
            "`Model` can't be derived for a generic type: its start states must be known",
        ));
    }
    let Data::Struct(state) = &item.data else {
        return Err(syn::Error::new(
            item.ident.span(),
            "`Model` goes on the struct that holds the model's state",
        ));
    };
    let Fields::Named(fields) = &state.fields else {
        return Err(syn::Error::new(
            state.fields.span(),
            "`Model` needs a struct with named fields",
        ));
    };
    let mut slots = Vec::with_capacity(fields.named.len());
    for field in &fields.named {
        let Some(name) = &field.ident else {
            continue;
        };
        slots.push(Slot::new(name, start_choices(field)?));
    }
    let starts = every_combination(&slots);
    let name = &item.ident;
    Ok(quote! {
        #[automatically_derived]
        impl ::earnest::model::Model for #name {
            fn starts() -> ::std::vec::Vec<Self> {
                #starts
            }
        }
    })
}

fn start_choices(field: &Field) -> syn::Result<TokenStream> {
    let ty = &field.ty;
    let range = ranged_choices(&field.attrs, ty)?;
    if range.is_none() && is_integer(ty) {
        return Err(syn::Error::new(ty.span(), RANGE_NEEDED));
    }
    let initial = field
        .attrs
        .iter()
        .find(|attribute| attribute.path().is_ident("initial"));
    if let Some(attribute) = initial {
        let value: Expr = attribute.parse_args()?;
        return Ok(quote! { ::std::vec::Vec::<#ty>::from([#value]) });
    }
    Ok(range.unwrap_or_else(|| quote! { <#ty as ::earnest::model::Start>::starts() }))
}

#[cfg(test)]
mod tests {
    use super::expand;
    use quote::quote;

    fn refusal(input: proc_macro2::TokenStream) -> String {
        expand(input).expect_err("refused").to_string()
    }

    #[test]
    fn each_kind_of_field_gets_its_own_start_choices() {
        let expanded = expand(quote! {
            struct Lamp {
                #[initial(Mode::Dim)]
                mode: Mode,
                #[range(0, 2)]
                level: u8,
                #[range(0, 9)]
                #[initial(4)]
                heat: u8,
                power: bool,
            }
        })
        .expect("a model struct")
        .to_string();
        let wanted = [
            quote! { impl ::earnest::model::Model for Lamp },
            quote! { for earnest_choice_mode in ::std::vec::Vec::<Mode>::from([Mode::Dim]) },
            quote! { for earnest_choice_level in (0)..=(2) },
            quote! { for earnest_choice_heat in ::std::vec::Vec::<u8>::from([4]) },
            quote! { for earnest_choice_power in <bool as ::earnest::model::Start>::starts() },
        ];
        for piece in wanted {
            let text = piece.to_string();
            assert!(expanded.contains(&text), "{text} in {expanded}");
        }
    }

    #[test]
    fn an_integer_field_without_a_range_is_refused_even_with_an_initial_value() {
        let message = refusal(quote! {
            struct Counter {
                #[initial(0)]
                count: u8,
            }
        });
        assert!(message.contains("#[range(lo, hi)]"), "{message}");
    }

    #[test]
    fn an_enum_a_tuple_struct_and_a_generic_struct_are_refused() {
        let on_enum = refusal(quote! {
            enum Gate {
                Locked,
            }
        });
        assert!(on_enum.contains("the struct that holds"), "{on_enum}");
        let tuple = refusal(quote! {
            struct Pair(bool, bool);
        });
        assert!(tuple.contains("named fields"), "{tuple}");
        let generic = refusal(quote! {
            struct Holder<T> {
                held: T,
            }
        });
        assert!(generic.contains("generic"), "{generic}");
    }
}
