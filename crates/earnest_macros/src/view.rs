use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::ext::IdentExt;
use syn::parse::Parser;
use syn::spanned::Spanned;
use syn::{Attribute, ImplItem, ImplItemFn, ItemImpl, LitStr};

const COLORED_PARTS: [&str; 3] = ["broken", "start", "end"];
const LABELS: [&str; 2] = ["state", "group"];

#[derive(Default)]
struct Settings {
    title: Option<LitStr>,
    steps: Vec<(Ident, LitStr)>,
    colors: Vec<(Ident, LitStr)>,
}

fn pairs(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<Vec<(Ident, LitStr)>> {
    let mut found: Vec<(Ident, LitStr)> = Vec::new();
    meta.parse_nested_meta(|entry| {
        let Some(name) = entry.path.get_ident() else {
            return Err(entry.error("write the name on its own: `press = \"press Add\"`"));
        };
        if found.iter().any(|(seen, _)| seen.unraw() == name.unraw()) {
            return Err(entry.error(format!("`{}` is listed twice", name.unraw())));
        }
        found.push((name.clone(), entry.value()?.parse()?));
        Ok(())
    })?;
    Ok(found)
}

fn settings(arguments: TokenStream) -> syn::Result<Settings> {
    let mut found = Settings::default();
    let reader = syn::meta::parser(|meta| {
        if meta.path.is_ident("title") {
            if found.title.is_some() {
                return Err(meta.error("`title` is given twice"));
            }
            found.title = Some(meta.value()?.parse()?);
        } else if meta.path.is_ident("step") {
            found.steps.extend(pairs(&meta)?);
        } else if meta.path.is_ident("colors") {
            for (part, color) in pairs(&meta)? {
                if !COLORED_PARTS.contains(&part.unraw().to_string().as_str()) {
                    return Err(syn::Error::new(
                        part.span(),
                        "`colors` colors `broken`, `start` and `end`, and no other part",
                    ));
                }
                found.colors.push((part, color));
            }
        } else {
            return Err(meta.error(
                "`#[view(...)]` takes `title = \"...\"`, `step(action = \"...\")` and \
                 `colors(broken = \"#c62828\")`",
            ));
        }
        Ok(())
    });
    reader.parse2(arguments)?;
    Ok(found)
}

fn take_label(attributes: &mut Vec<Attribute>) -> syn::Result<Option<&'static str>> {
    let mut found = None;
    let mut kept = Vec::with_capacity(attributes.len());
    for attribute in attributes.drain(..) {
        let label = LABELS
            .iter()
            .find(|label| attribute.path().is_ident(label))
            .copied();
        let Some(name) = label else {
            kept.push(attribute);
            continue;
        };
        if found.is_some() {
            return Err(syn::Error::new(
                attribute.span(),
                "a method labels the state or the group, never both",
            ));
        }
        attribute.meta.require_path_only().map_err(|_| {
            syn::Error::new(
                attribute.span(),
                format!("write `#[{name}]` with nothing in the brackets"),
            )
        })?;
        found = Some(name);
    }
    *attributes = kept;
    Ok(found)
}

fn label_call(method: &ImplItemFn) -> TokenStream {
    let name = &method.sig.ident;
    quote! { ::core::option::Option::Some(Self::#name(self)) }
}

pub(crate) fn expand(arguments: TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
    let settings = settings(arguments)?;
    let mut block: ItemImpl = syn::parse2(input)?;
    if let Some((_, path, _)) = &block.trait_ {
        return Err(syn::Error::new(
            path.span(),
            "`#[view]` goes on an `impl` block of the model's own type, not on a trait's",
        ));
    }
    let mut state = quote! { ::core::option::Option::None };
    let mut group = quote! { ::core::option::Option::None };
    let (mut states, mut groups) = (0, 0);
    for item in &mut block.items {
        let ImplItem::Fn(method) = item else {
            return Err(syn::Error::new(
                item.span(),
                "a view's block holds only `#[state]` and `#[group]` methods",
            ));
        };
        match take_label(&mut method.attrs)? {
            Some("state") => {
                states += 1;
                state = label_call(method);
            }
            Some(_) => {
                groups += 1;
                group = label_call(method);
            }
            None => {
                return Err(syn::Error::new(
                    method.sig.span(),
                    "a view's block holds only `#[state]` and `#[group]` methods",
                ));
            }
        }
        if !matches!(method.sig.inputs.first(), Some(syn::FnArg::Receiver(_))) {
            return Err(syn::Error::new(
                method.sig.span(),
                "a label takes `&self` first: it reads the state it labels",
            ));
        }
    }
    if states > 1 || groups > 1 {
        return Err(syn::Error::new(
            block.self_ty.span(),
            "a view has one `#[state]` label and one `#[group]` label at most",
        ));
    }
    let owner = &block.self_ty;
    let title = settings.title.as_ref().map_or_else(
        || quote! { ::core::option::Option::None },
        |title| quote! { ::core::option::Option::Some(#title) },
    );
    let steps = settings.steps.iter().map(|(action, label)| {
        let id = action.unraw().to_string();
        quote! { (#id, #label) }
    });
    let colors = settings.colors.iter().map(|(part, color)| {
        let id = part.unraw().to_string();
        quote! { (#id, #color) }
    });
    let known = settings.steps.iter().map(|(action, _)| {
        quote! { let _ = Self::#action; }
    });
    Ok(quote! {
        #block

        #[automatically_derived]
        impl ::earnest::model::View for #owner {
            fn title() -> ::core::option::Option<&'static str> {
                #title
            }

            fn state_label(&self) -> ::core::option::Option<::std::string::String> {
                #state
            }

            fn group_label(&self) -> ::core::option::Option<::std::string::String> {
                #group
            }

            fn step_labels() -> &'static [(&'static str, &'static str)] {
                #(#known)*
                &[#(#steps),*]
            }

            fn colors() -> &'static [(&'static str, &'static str)] {
                &[#(#colors),*]
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::expand;
    use proc_macro2::TokenStream;
    use quote::quote;

    fn refusal(arguments: TokenStream, input: TokenStream) -> String {
        expand(arguments, input).expect_err("refused").to_string()
    }

    #[test]
    fn the_labels_stay_and_the_view_is_implemented_from_them() {
        let expanded = expand(
            quote! { title = "Cart", step(press = "press Add"), colors(broken = "#c62828") },
            quote! {
                impl Cart {
                    #[state]
                    fn label(&self) -> String {
                        String::new()
                    }

                    #[group]
                    fn surface(&self) -> String {
                        String::new()
                    }
                }
            },
        )
        .expect("a view block")
        .to_string();
        let kept = quote! { fn label(&self) -> String }.to_string();
        assert!(expanded.contains(&kept), "{kept} in {expanded}");
        for gone in ["# [state", "# [group"] {
            assert!(!expanded.contains(gone), "{gone} in {expanded}");
        }
        let wanted = [
            quote! { impl ::earnest::model::View for Cart },
            quote! { ::core::option::Option::Some("Cart") },
            quote! { ::core::option::Option::Some(Self::label(self)) },
            quote! { ::core::option::Option::Some(Self::surface(self)) },
            quote! { let _ = Self::press; },
            quote! { &[("press", "press Add")] },
            quote! { &[("broken", "#c62828")] },
        ];
        for piece in wanted {
            let text = piece.to_string();
            assert!(expanded.contains(&text), "{text} in {expanded}");
        }
    }

    #[test]
    fn an_unknown_setting_an_unknown_color_part_and_a_repeat_are_refused() {
        let empty = quote! { impl Cart {} };
        let unknown = refusal(quote! { heading = "Cart" }, empty.clone());
        assert!(unknown.contains("takes `title"), "{unknown}");
        let part = refusal(quote! { colors(lost = "red") }, empty.clone());
        assert!(part.contains("no other part"), "{part}");
        let repeat = refusal(quote! { step(press = "a", press = "b") }, empty.clone());
        assert!(repeat.contains("listed twice"), "{repeat}");
        let title = refusal(quote! { title = "a", title = "b" }, empty);
        assert!(title.contains("given twice"), "{title}");
    }

    #[test]
    fn a_block_with_anything_but_labels_is_refused() {
        let helper = refusal(
            TokenStream::new(),
            quote! {
                impl Cart {
                    fn helper(&self) -> bool {
                        true
                    }
                }
            },
        );
        assert!(helper.contains("only `#[state]`"), "{helper}");
        let both = refusal(
            TokenStream::new(),
            quote! {
                impl Cart {
                    #[state]
                    #[group]
                    fn label(&self) -> String {
                        String::new()
                    }
                }
            },
        );
        assert!(both.contains("never both"), "{both}");
        let twice = refusal(
            TokenStream::new(),
            quote! {
                impl Cart {
                    #[state]
                    fn one(&self) -> String {
                        String::new()
                    }

                    #[state]
                    fn two(&self) -> String {
                        String::new()
                    }
                }
            },
        );
        assert!(twice.contains("one `#[state]` label"), "{twice}");
        let selfless = refusal(
            TokenStream::new(),
            quote! {
                impl Cart {
                    #[state]
                    fn one() -> String {
                        String::new()
                    }
                }
            },
        );
        assert!(selfless.contains("takes `&self` first"), "{selfless}");
        let on_trait = refusal(TokenStream::new(), quote! { impl Clone for Cart {} });
        assert!(on_trait.contains("not on a trait's"), "{on_trait}");
    }
}
