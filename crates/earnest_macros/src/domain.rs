use crate::choices::{Slot, domain_choices, every_combination};
use proc_macro2::TokenStream;
use quote::quote;
use syn::ext::IdentExt;
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
    let (values, report, walk) = match &item.data {
        Data::Enum(choices) => (
            variants(choices)?,
            reported_variants(choices),
            walked_variants(choices),
        ),
        Data::Struct(record) => match &record.fields {
            Fields::Named(fields) => (
                combinations(fields)?,
                reported_fields(fields),
                walked_fields(fields),
            ),
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

        #[automatically_derived]
        impl ::earnest::model::Report for #name {
            fn report(&self) -> ::std::string::String {
                #report
            }
        }

        #[automatically_derived]
        impl ::earnest::model::Walk for #name {
            fn walk_cmp(&self, other: &Self) -> ::core::cmp::Ordering {
                #walk
            }
        }
    })
}

fn walked_variants(choices: &DataEnum) -> TokenStream {
    let mut by_name: Vec<(String, &syn::Ident)> = choices
        .variants
        .iter()
        .map(|variant| (variant.ident.unraw().to_string(), &variant.ident))
        .collect();
    by_name.sort_by(|left, right| left.0.cmp(&right.0));
    let names = by_name.iter().map(|(_, ident)| ident);
    let ranks = 0..by_name.len();
    quote! {
        let rank = |value: &Self| match value {
            #(Self::#names => #ranks,)*
        };
        ::core::cmp::Ord::cmp(&rank(self), &rank(other))
    }
}

fn walked_fields(fields: &FieldsNamed) -> TokenStream {
    let mut by_name: Vec<(String, &syn::Ident)> = fields
        .named
        .iter()
        .filter_map(|field| field.ident.as_ref())
        .map(|ident| (ident.unraw().to_string(), ident))
        .collect();
    by_name.sort_by(|left, right| left.0.cmp(&right.0));
    let names = by_name.iter().map(|(_, ident)| ident);
    quote! {
        ::core::cmp::Ordering::Equal
            #(.then_with(|| ::earnest::model::Walk::walk_cmp(&self.#names, &other.#names)))*
    }
}

fn reported_variants(choices: &DataEnum) -> TokenStream {
    let names = choices.variants.iter().map(|variant| &variant.ident);
    let shown = choices
        .variants
        .iter()
        .map(|variant| format!("\"{}\"", variant.ident.unraw()));
    quote! {
        match *self {
            #(Self::#names => ::std::string::String::from(#shown),)*
        }
    }
}

fn reported_fields(fields: &FieldsNamed) -> TokenStream {
    let mut by_name: Vec<(String, &syn::Ident)> = fields
        .named
        .iter()
        .filter_map(|field| field.ident.as_ref())
        .map(|ident| (ident.unraw().to_string(), ident))
        .collect();
    by_name.sort_by(|left, right| left.0.cmp(&right.0));
    let names: Vec<&syn::Ident> = by_name.iter().map(|(_, ident)| *ident).collect();
    let labels = by_name.iter().map(|(label, _)| label);
    quote! {
        let earnest_parts: ::std::vec::Vec<::std::string::String> = ::std::vec![
            #(::std::format!("{}: {}", #labels, ::earnest::model::Report::report(&self.#names))),*
        ];
        ::std::format!("{{{}}}", earnest_parts.join(", "))
    }
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

            #[automatically_derived]
            impl ::earnest::model::Report for Gate {
                fn report(&self) -> ::std::string::String {
                    match *self {
                        Self::Locked => ::std::string::String::from("\"Locked\""),
                        Self::Open => ::std::string::String::from("\"Open\""),
                    }
                }
            }

            #[automatically_derived]
            impl ::earnest::model::Walk for Gate {
                fn walk_cmp(&self, other: &Self) -> ::core::cmp::Ordering {
                    let rank = |value: &Self| match value {
                        Self::Locked => 0usize,
                        Self::Open => 1usize,
                    };
                    ::core::cmp::Ord::cmp(&rank(self), &rank(other))
                }
            }
        };
        assert_eq!(expanded.to_string(), wanted.to_string(), "the impls");
    }

    #[test]
    fn a_struct_reports_each_field_by_name_in_the_order_of_the_names() {
        let expanded = expand(quote! {
            struct Spot {
                #[range(0, 1)]
                row: u8,
                r#type: bool,
            }
        })
        .expect("a struct")
        .to_string();
        let wanted = quote! {
            ::std::format!("{}: {}", "row", ::earnest::model::Report::report(&self.row))
        }
        .to_string();
        assert!(expanded.contains(&wanted), "{wanted} in {expanded}");
        assert!(
            expanded.contains("\"type\""),
            "a raw name is its plain spelling: {expanded}"
        );
    }

    #[test]
    fn a_struct_is_reported_and_walked_by_its_field_names_not_in_the_order_declared() {
        let expanded = expand(quote! {
            struct Spot {
                zebra: bool,
                apple: bool,
            }
        })
        .expect("a struct")
        .to_string();
        let reported = quote! { "apple" }.to_string();
        let other = quote! { "zebra" }.to_string();
        assert!(
            expanded.find(&reported) < expanded.find(&other),
            "the report lists apple first: {expanded}"
        );
        let walk = quote! {
            ::core::cmp::Ordering::Equal
                .then_with(|| ::earnest::model::Walk::walk_cmp(&self.apple, &other.apple))
                .then_with(|| ::earnest::model::Walk::walk_cmp(&self.zebra, &other.zebra))
        }
        .to_string();
        assert!(
            expanded.contains(&walk),
            "the walk compares apple first: {expanded}"
        );
        let values = quote! { for earnest_choice_zebra }.to_string();
        let inner = quote! { for earnest_choice_apple }.to_string();
        assert!(
            expanded.find(&values) < expanded.find(&inner),
            "every value is still listed in the order declared: {expanded}"
        );
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
