use crate::choices::{Slot, domain_choices, every_combination};
use proc_macro2::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::{Data, DataEnum, DeriveInput, Fields, FieldsNamed};

pub(crate) fn expand(input: TokenStream) -> syn::Result<TokenStream> {
    let item: DeriveInput = syn::parse2(input)?;
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new(
            item.generics.span(),
            "`Domain` can't be derived for a generic type: its values must be known",
        ));
    }
    let values = match &item.data {
        Data::Enum(choices) => variants(choices)?,
        Data::Struct(record) => match &record.fields {
            Fields::Named(fields) => combinations(fields)?,
            other @ (Fields::Unnamed(_) | Fields::Unit) => {
                return Err(syn::Error::new(
                    other.span(),
                    "`Domain` needs a struct with named fields",
                ));
            }
        },
        Data::Union(union) => {
            return Err(syn::Error::new(
                union.union_token.span,
                "`Domain` can't be derived for a union",
            ));
        }
    };
    let name = &item.ident;
    Ok(quote! {
        #[automatically_derived]
        impl ::earnest::model::Domain for #name {
            fn values() -> ::std::vec::Vec<Self> {
                #values
            }
        }
    })
}

fn variants(choices: &DataEnum) -> syn::Result<TokenStream> {
    let mut names = Vec::with_capacity(choices.variants.len());
    for variant in &choices.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(syn::Error::new(
                variant.fields.span(),
                "`Domain` needs variants without data: a value is known by its variant name",
            ));
        }
        names.push(&variant.ident);
    }
    Ok(quote! { ::std::vec![#(Self::#names),*] })
}

fn combinations(fields: &FieldsNamed) -> syn::Result<TokenStream> {
    let mut slots = Vec::with_capacity(fields.named.len());
    for field in &fields.named {
        let Some(name) = &field.ident else {
            continue;
        };
        slots.push(Slot::new(name, domain_choices(&field.attrs, &field.ty)?));
    }
    Ok(every_combination(&slots))
}

#[cfg(test)]
mod tests {
    use super::expand;
    use quote::quote;

    fn refusal(input: proc_macro2::TokenStream) -> String {
        expand(input).expect_err("refused").to_string()
    }

    #[test]
    fn an_enum_lists_its_variants_in_source_order() {
        let expanded = expand(quote! {
            enum Gate {
                Locked,
                Open,
            }
        })
        .expect("a plain enum");
        let wanted = quote! {
            #[automatically_derived]
            impl ::earnest::model::Domain for Gate {
                fn values() -> ::std::vec::Vec<Self> {
                    ::std::vec![Self::Locked, Self::Open]
                }
            }
        };
        assert_eq!(expanded.to_string(), wanted.to_string(), "the impl");
    }

    #[test]
    fn a_struct_loops_over_each_field_and_uses_a_range_where_given() {
        let expanded = expand(quote! {
            struct Seat {
                #[range(1, 2)]
                row: u8,
                aisle: bool,
            }
        })
        .expect("a struct of a ranged number and a bool")
        .to_string();
        let row = quote! { for earnest_choice_row in (1)..=(2) }.to_string();
        let aisle = quote! {
            for earnest_choice_aisle in <bool as ::earnest::model::Domain>::values()
        }
        .to_string();
        assert!(expanded.contains(&row), "the range loop in {expanded}");
        assert!(expanded.contains(&aisle), "the bool loop in {expanded}");
        assert!(
            expanded.find(&row) < expanded.find(&aisle),
            "the first field is the outer loop in {expanded}"
        );
    }

    #[test]
    fn a_variant_with_data_is_refused() {
        let message = refusal(quote! {
            enum Shape {
                Circle(u8),
            }
        });
        assert!(message.contains("variants without data"), "{message}");
    }

    #[test]
    fn an_integer_field_without_a_range_is_refused() {
        let message = refusal(quote! {
            struct Seat {
                row: u8,
            }
        });
        assert!(message.contains("#[range(lo, hi)]"), "{message}");
    }

    #[test]
    fn a_range_with_one_bound_is_refused() {
        let message = refusal(quote! {
            struct Seat {
                #[range(1)]
                row: u8,
            }
        });
        assert!(message.contains("the lowest and the highest"), "{message}");
    }

    #[test]
    fn a_tuple_struct_a_generic_type_and_a_union_are_refused() {
        let tuple = refusal(quote! {
            struct Pair(bool, bool);
        });
        assert!(tuple.contains("named fields"), "{tuple}");
        let generic = refusal(quote! {
            enum Either<T> {
                Left,
                Right,
            }
        });
        assert!(generic.contains("generic"), "{generic}");
        let union = refusal(quote! {
            union Bits {
                low: u8,
            }
        });
        assert!(union.contains("union"), "{union}");
    }

    #[test]
    fn a_listed_field_loops_over_its_list() {
        let expanded = expand(quote! {
            struct Seat {
                #[one_of(1, 3)]
                row: u8,
                #[one_of("window", "aisle")]
                side: &'static str,
            }
        })
        .expect("a struct of listed fields")
        .to_string();
        for piece in [
            quote! { for earnest_choice_row in ::core::iter::IntoIterator::into_iter([1, 3]) },
            quote! { for earnest_choice_side in ::core::iter::IntoIterator::into_iter(["window", "aisle"]) },
        ] {
            let text = piece.to_string();
            assert!(expanded.contains(&text), "{text} in {expanded}");
        }
    }
}
