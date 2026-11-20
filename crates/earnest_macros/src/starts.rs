use crate::choices::{
    RANGE_NEEDED, Slot, every_combination, is_integer, ranged_choices, text_needed,
};
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

        const _: () = {
            const fn earnest_is_used<Used>() {}
            earnest_is_used::<#name>();
        };
    })
}

fn start_choices(field: &Field) -> syn::Result<TokenStream> {
    let ty = &field.ty;
    let range = ranged_choices(&field.attrs, ty)?;
    if range.is_none() && is_integer(ty) {
        return Err(syn::Error::new(ty.span(), RANGE_NEEDED));
    }
    if let Some(refused) = text_needed(ty).filter(|_| range.is_none()) {
        return Err(refused);
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

    #[test]
    fn a_listed_number_and_a_listed_text_start_at_each_value_listed() {
        let expanded = expand(quote! {
            struct Pick {
                #[one_of(1, 5, 9)]
                size: u8,
                #[one_of("Kim", "Lee")]
                owner: Option<&'static str>,
                #[one_of("a", "b")]
                #[initial("a")]
                kind: &'static str,
            }
        })
        .expect("a model struct")
        .to_string();
        let wanted = [
            quote! {
                for earnest_choice_size in ::core::iter::IntoIterator::into_iter([1, 5, 9])
            },
            quote! {
                for earnest_choice_owner in ::core::iter::once(::core::option::Option::None)
                    .chain((::core::iter::IntoIterator::into_iter(["Kim", "Lee"]))
                        .map(::core::option::Option::Some))
            },
            quote! {
                for earnest_choice_kind in ::std::vec::Vec::<&'static str>::from(["a"])
            },
        ];
        for piece in wanted {
            let text = piece.to_string();
            assert!(expanded.contains(&text), "{text} in {expanded}");
        }
    }

    #[test]
    fn the_model_is_used_where_rustc_looks_for_dead_code() {
        let expanded = expand(quote! {
            struct Lamp {
                power: bool,
            }
        })
        .expect("a model struct")
        .to_string();
        let used = quote! { earnest_is_used::<Lamp>(); }.to_string();
        assert!(expanded.contains(&used), "{used} in {expanded}");
    }

    #[test]
    fn a_text_without_a_list_and_a_number_with_both_a_range_and_a_list_are_refused() {
        let unlisted = refusal(quote! {
            struct Pick {
                owner: &'static str,
            }
        });
        assert!(unlisted.contains("#[one_of("), "{unlisted}");
        let both = refusal(quote! {
            struct Pick {
                #[range(0, 3)]
                #[one_of(1, 2)]
                size: u8,
            }
        });
        assert!(both.contains("never both"), "{both}");
        let empty = refusal(quote! {
            struct Pick {
                #[one_of()]
                size: u8,
            }
        });
        assert!(empty.contains("lists the values"), "{empty}");
    }
}
