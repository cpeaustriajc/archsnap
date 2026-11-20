use crate::choices::{Fastest, Slot, domain_choices, for_each_combination};
use proc_macro2::TokenStream;
use quote::quote;
use syn::ext::IdentExt;
use syn::spanned::Spanned;
use syn::{Attribute, FnArg, ImplItem, ImplItemFn, ItemImpl, LitStr, Pat};

const ROLES: [&str; 3] = ["action", "rule", "end"];

struct Role {
    name: &'static str,
    sentence: LitStr,
}

pub(crate) fn expand(arguments: &TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
    if !arguments.is_empty() {
        return Err(syn::Error::new(
            arguments.span(),
            "`#[model]` takes no arguments",
        ));
    }
    let mut block: ItemImpl = syn::parse2(input)?;
    if let Some((_, path, _)) = &block.trait_ {
        return Err(syn::Error::new(
            path.span(),
            "`#[model]` goes on the model's own `impl` block, not on a trait's",
        ));
    }
    let mut actions = Vec::new();
    let mut rules = Vec::new();
    let mut ends = Vec::new();
    for item in &mut block.items {
        let ImplItem::Fn(method) = item else {
            continue;
        };
        let Some(role) = take_role(&mut method.attrs)? else {
            continue;
        };
        match role.name {
            "action" => actions.push(steps_of(method, &role.sentence)?),
            "rule" => rules.push(claim_of(method, &role.sentence)),
            _ => ends.push(claim_of(method, &role.sentence)),
        }
    }
    let owner = &block.self_ty;
    Ok(quote! {
        #block

        #[automatically_derived]
        impl ::earnest::model::Behavior for #owner {
            fn steps(&self) -> ::std::vec::Vec<::earnest::model::Step<Self>> {
                ::std::iter::empty()
                    #(.chain(#actions))*
                    .collect()
            }

            fn rules(&self) -> ::std::vec::Vec<::earnest::model::Claim> {
                ::std::vec![#(#rules),*]
            }

            fn ends(&self) -> ::std::vec::Vec<::earnest::model::Claim> {
                ::std::vec![#(#ends),*]
            }
        }
    })
}

fn take_role(attributes: &mut Vec<Attribute>) -> syn::Result<Option<Role>> {
    let mut found = None;
    let mut kept = Vec::with_capacity(attributes.len());
    for attribute in attributes.drain(..) {
        let role = ROLES
            .iter()
            .find(|role| attribute.path().is_ident(role))
            .copied();
        let Some(name) = role else {
            kept.push(attribute);
            continue;
        };
        if found.is_some() {
            return Err(syn::Error::new(
                attribute.span(),
                "a method is an action, a rule or an end, never two of them",
            ));
        }
        let sentence = attribute.parse_args().map_err(|_| {
            syn::Error::new(
                attribute.span(),
                format!("write the sentence in quotes: `#[{name}(\"...\")]`"),
            )
        })?;
        found = Some(Role { name, sentence });
    }
    *attributes = kept;
    Ok(found)
}

fn gives_self(signature: &syn::Signature) -> bool {
    matches!(&signature.output, syn::ReturnType::Type(_, ty)
        if matches!(ty.as_ref(), syn::Type::Path(path) if path.qself.is_none() && path.path.is_ident("Self")))
}

fn takes_self(method: &ImplItemFn) -> bool {
    matches!(method.sig.inputs.first(), Some(FnArg::Receiver(_)))
}

fn claim_of(method: &ImplItemFn, sentence: &LitStr) -> TokenStream {
    let name = &method.sig.ident;
    let id = name.unraw().to_string();
    quote! {
        ::earnest::model::Claim {
            id: #id,
            says: #sentence,
            holds: Self::#name(self),
        }
    }
}

fn steps_of(method: &mut ImplItemFn, sentence: &LitStr) -> syn::Result<TokenStream> {
    if !takes_self(method) {
        return Err(syn::Error::new(
            method.sig.span(),
            "an action takes `&self` first: it reads the state it starts from",
        ));
    }
    let mut slots = Vec::new();
    for input in &mut method.sig.inputs {
        let FnArg::Typed(typed) = input else {
            continue;
        };
        let Pat::Ident(pattern) = typed.pat.as_ref() else {
            return Err(syn::Error::new(
                typed.pat.span(),
                "an action's input is a plain name with its type",
            ));
        };
        let choices = domain_choices(&typed.attrs, &typed.ty)?;
        slots.push(Slot::new(&pattern.ident, choices));
        typed.attrs.retain(|attribute| {
            !attribute.path().is_ident("range") && !attribute.path().is_ident("one_of")
        });
    }
    let name = &method.sig.ident;
    let id = name.unraw().to_string();
    let copies = slots.iter().map(Slot::copy);
    let shown = slots.iter().map(|slot| {
        let input = slot.name.unraw().to_string();
        let variable = &slot.variable;
        quote! { (#input, ::std::format!("{:?}", #variable)) }
    });
    let step = quote! {
        earnest_steps.push(::earnest::model::Step {
            action: #id,
            label: #sentence,
            input: ::std::vec![#(#shown),*],
            state: earnest_next,
        });
    };
    let attempt = if gives_self(&method.sig) {
        quote! {
            let earnest_next = Self::#name(self #(, #copies)*);
            #step
        }
    } else {
        quote! {
            if let ::core::option::Option::Some(earnest_next) = Self::#name(self #(, #copies)*) {
                #step
            }
        }
    };
    let loops = for_each_combination(&slots, Fastest::First, attempt);
    Ok(quote! {
        {
            let mut earnest_steps = ::std::vec::Vec::new();
            #loops
            earnest_steps
        }
    })
}

#[cfg(test)]
mod tests {
    use super::expand;
    use proc_macro2::TokenStream;
    use quote::quote;

    fn refusal(input: TokenStream) -> String {
        expand(&TokenStream::new(), input)
            .expect_err("refused")
            .to_string()
    }

    #[test]
    fn the_methods_stay_and_lose_only_the_attributes_the_macro_reads() {
        let expanded = expand(
            &TokenStream::new(),
            quote! {
                impl Mailbox {
                    #[action("the postman delivers a letter")]
                    #[must_use]
                    fn deliver(&self, letter: Letter, #[range(1, 2)] copies: u8) -> Option<Self> {
                        None
                    }

                    #[rule("A full inbox holds a card")]
                    fn bills(&self) -> bool {
                        true
                    }

                    #[end("The inbox is full")]
                    fn full(&self) -> bool {
                        false
                    }

                    fn helper(&self) -> bool {
                        true
                    }
                }
            },
        )
        .expect("a model block")
        .to_string();
        let kept = quote! {
            #[must_use]
            fn deliver(&self, letter: Letter, copies: u8) -> Option<Self>
        }
        .to_string();
        assert!(expanded.contains(&kept), "{kept} in {expanded}");
        for gone in ["# [action", "# [rule", "# [end", "# [range"] {
            assert!(!expanded.contains(gone), "{gone} in {expanded}");
        }
        let wanted = [
            quote! { impl ::earnest::model::Behavior for Mailbox },
            quote! { for earnest_choice_letter in <Letter as ::earnest::model::Domain>::values() },
            quote! { for earnest_choice_copies in (1)..=(2) },
            quote! { action: "deliver", label: "the postman delivers a letter" },
            quote! { id: "bills", says: "A full inbox holds a card", holds: Self::bills(self) },
            quote! { id: "full", says: "The inbox is full", holds: Self::full(self) },
            quote! { fn helper(&self) -> bool },
        ];
        for piece in wanted {
            let text = piece.to_string();
            assert!(expanded.contains(&text), "{text} in {expanded}");
        }
    }

    #[test]
    fn arguments_a_trait_block_and_two_roles_are_refused() {
        let with_arguments = expand(&quote! { strict }, quote! { impl Mailbox {} })
            .expect_err("refused")
            .to_string();
        assert!(with_arguments.contains("no arguments"), "{with_arguments}");
        let on_trait = refusal(quote! { impl Clone for Mailbox {} });
        assert!(on_trait.contains("not on a trait's"), "{on_trait}");
        let twice = refusal(quote! {
            impl Mailbox {
                #[rule("One")]
                #[end("Two")]
                fn both(&self) -> bool {
                    true
                }
            }
        });
        assert!(twice.contains("never two of them"), "{twice}");
    }

    #[test]
    fn a_missing_sentence_an_action_without_self_and_an_unranged_integer_are_refused() {
        let bare = refusal(quote! {
            impl Mailbox {
                #[rule]
                fn bills(&self) -> bool {
                    true
                }
            }
        });
        assert!(bare.contains("#[rule(\"...\")]"), "{bare}");
        let selfless = refusal(quote! {
            impl Mailbox {
                #[action("the postman delivers a letter")]
                fn deliver(letter: Letter) -> Option<Self> {
                    None
                }
            }
        });
        assert!(selfless.contains("takes `&self` first"), "{selfless}");
        let unranged = refusal(quote! {
            impl Mailbox {
                #[action("the postman delivers some letters")]
                fn deliver(&self, copies: u8) -> Option<Self> {
                    None
                }
            }
        });
        assert!(unranged.contains("#[range(lo, hi)]"), "{unranged}");
        let pattern = refusal(quote! {
            impl Mailbox {
                #[action("the postman delivers a pair")]
                fn deliver(&self, (first, second): (Letter, Letter)) -> Option<Self> {
                    None
                }
            }
        });
        assert!(pattern.contains("a plain name"), "{pattern}");
    }

    #[test]
    fn an_action_that_gives_back_the_state_is_a_step_without_a_match() {
        let expanded = expand(
            &TokenStream::new(),
            quote! {
                impl Lamp {
                    #[action("the lamp is switched")]
                    fn toggle(&self, #[one_of("a", "b")] how: &'static str) -> Self {
                        self.clone()
                    }
                }
            },
        )
        .expect("a model block")
        .to_string();
        let wanted = [
            quote! { for earnest_choice_how in ::core::iter::IntoIterator::into_iter(["a", "b"]) },
            quote! { let earnest_next = Self::toggle(self, ::core::clone::Clone::clone(&earnest_choice_how)); },
        ];
        for piece in wanted {
            let text = piece.to_string();
            assert!(expanded.contains(&text), "{text} in {expanded}");
        }
        assert!(
            !expanded.contains("# [one_of"),
            "the attribute is read: {expanded}"
        );
    }
}
