use earnest_ast::{Quoting, nesting_problem};
use proc_macro2::{Delimiter, Ident, Spacing, TokenStream, TokenTree};

const CRATE: &str = "earnest";
const RAW: &str = "r#";
const MODEL_ATTRIBUTES: [&str; 10] = [
    "model", "view", "action", "rule", "end", "state", "group", "initial", "range", "one_of",
];
const MODEL_DERIVES: [&str; 2] = ["Model", "Domain"];
const LIBRARY_TRAITS: [&str; 5] = ["Model", "Behavior", "Domain", "Start", "View"];

fn plain(name: &Ident) -> String {
    let written = name.to_string();
    written
        .strip_prefix(RAW)
        .map_or(written.clone(), str::to_owned)
}

fn is_ident(tree: Option<&TokenTree>, word: &str) -> bool {
    matches!(tree, Some(TokenTree::Ident(found)) if plain(found) == word)
}

fn is_colon(tree: Option<&TokenTree>, spacing: Spacing) -> bool {
    matches!(tree, Some(TokenTree::Punct(found)) if found.as_char() == ':' && found.spacing() == spacing)
}

fn starts_a_path(trees: &[TokenTree], index: usize) -> bool {
    let before = index.checked_sub(1).and_then(|at| trees.get(at));
    let after = trees.get(index.saturating_add(1));
    let then = trees.get(index.saturating_add(2));
    (is_colon(after, Spacing::Joint) && is_colon(then, Spacing::Alone))
        || is_ident(after, "as")
        || is_ident(before, "use")
        || is_ident(before, "crate")
}

fn names_the_crate(tokens: TokenStream) -> bool {
    let mut pending = vec![tokens];
    while let Some(stream) = pending.pop() {
        let trees: Vec<TokenTree> = stream.into_iter().collect();
        for (index, tree) in trees.iter().enumerate() {
            match tree {
                TokenTree::Group(group) => pending.push(group.stream()),
                TokenTree::Ident(name) if plain(name) == CRATE && starts_a_path(&trees, index) => {
                    return true;
                }
                TokenTree::Ident(_) | TokenTree::Punct(_) | TokenTree::Literal(_) => {}
            }
        }
    }
    false
}

fn mentions_a_derive(stream: TokenStream) -> bool {
    stream.into_iter().any(|tree| match tree {
        TokenTree::Ident(name) => MODEL_DERIVES.contains(&plain(&name).as_str()),
        TokenTree::Group(group) => mentions_a_derive(group.stream()),
        TokenTree::Punct(_) | TokenTree::Literal(_) => false,
    })
}

fn is_a_model_attribute(attribute: TokenStream) -> bool {
    let trees: Vec<TokenTree> = attribute.into_iter().collect();
    let mut last: Option<String> = None;
    for tree in &trees {
        match tree {
            TokenTree::Ident(name) => last = Some(plain(name)),
            TokenTree::Punct(mark) if mark.as_char() == ':' => {}
            TokenTree::Punct(_) | TokenTree::Literal(_) | TokenTree::Group(_) => break,
        }
    }
    let Some(name) = last else {
        return false;
    };
    MODEL_ATTRIBUTES.contains(&name.as_str())
        || ((name == "derive" || name == "cfg_attr") && mentions_a_derive(trees_to_stream(&trees)))
}

fn trees_to_stream(trees: &[TokenTree]) -> TokenStream {
    trees.iter().cloned().collect()
}

fn implements_a_library_trait(trees: &[TokenTree], index: usize) -> bool {
    let mut previous: Option<&TokenTree> = None;
    for tree in trees.iter().skip(index.saturating_add(1)) {
        match tree {
            TokenTree::Group(group) if group.delimiter() == Delimiter::Brace => return false,
            TokenTree::Punct(mark) if mark.as_char() == ';' => return false,
            TokenTree::Ident(word) if plain(word) == "for" => {
                return matches!(previous, Some(TokenTree::Ident(name))
                    if LIBRARY_TRAITS.contains(&plain(name).as_str()));
            }
            other => previous = Some(other),
        }
    }
    false
}

fn declares_in(tokens: TokenStream) -> bool {
    let mut pending = vec![tokens];
    while let Some(stream) = pending.pop() {
        let trees: Vec<TokenTree> = stream.into_iter().collect();
        for (index, tree) in trees.iter().enumerate() {
            let before = index.checked_sub(1).and_then(|at| trees.get(at));
            match tree {
                TokenTree::Group(group) => {
                    let attribute = group.delimiter() == Delimiter::Bracket
                        && (matches!(before, Some(TokenTree::Punct(mark)) if mark.as_char() == '#')
                            || matches!(before, Some(TokenTree::Punct(mark)) if mark.as_char() == '!'));
                    if attribute && is_a_model_attribute(group.stream()) {
                        return true;
                    }
                    pending.push(group.stream());
                }
                TokenTree::Ident(name) => {
                    let word = plain(name);
                    let calls_require = word == "require"
                        && matches!(trees.get(index.saturating_add(1)),
                            Some(TokenTree::Punct(mark)) if mark.as_char() == '!');
                    if calls_require
                        || (word == "impl" && implements_a_library_trait(&trees, index))
                    {
                        return true;
                    }
                }
                TokenTree::Punct(_) | TokenTree::Literal(_) => {}
            }
        }
    }
    false
}

/// Whether the Rust file `source` declares a model, or anything only a model file has, so it
/// must be read, and a file that uses the library without one is left out and said to be.
///
/// It is true when the file holds, anywhere, also inside a `mod`, a function or a macro, behind
/// `cfg` or `cfg_attr`, and written with raw identifiers or a path: a `derive` of `Model` or
/// `Domain`, an attribute `#[model]`, `#[view]`, `#[action]`, `#[rule]`, `#[end]`, `#[state]`,
/// `#[group]`, `#[initial]`, `#[range]` or `#[one_of]`, a call of `require!`, or an `impl` of
/// one of the library's traits. The file is read as tokens, so a word in a comment or a text
/// counts for nothing. A file that can't be read as Rust tokens, or is nested too deep to be,
/// is true: it can't be shown to declare nothing, so the reader refuses what it can't read.
#[must_use]
pub fn declares_a_model(source: &str) -> bool {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    if nesting_problem("", source, Quoting::Rust).is_some() {
        return true;
    }
    source.parse::<TokenStream>().map_or(true, declares_in)
}

/// Whether the Rust file `source` names the `earnest` crate, so it is a model file and must be
/// read, or refused as unreadable, and never left out.
///
/// The crate is named by a path that starts with it, such as `earnest::model::Model`, by `use
/// earnest as kit` and by `extern crate earnest`, wherever the path is spaced, grouped or
/// broken by a comment: the file is read as tokens, so what lies between them does not count.
/// A raw identifier is the same name, so `r#earnest` names the crate too. A word `earnest` in
/// a comment or a text does not name the crate. A file that mentions
/// `earnest` and can't be read as Rust tokens is true too, because it can't be shown to leave
/// the crate alone and the reader then refuses what it can't read.
#[must_use]
pub fn uses_library(source: &str) -> bool {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    if !source.contains(CRATE) {
        return false;
    }
    if nesting_problem("", source, Quoting::Rust).is_some() {
        return true;
    }
    source.parse::<TokenStream>().map_or(true, names_the_crate)
}

#[cfg(test)]
mod tests {
    use super::{declares_a_model, uses_library};

    #[test]
    fn a_model_hidden_by_spelling_or_place_is_still_declared() {
        for source in [
            "#[derive(Model)] struct A;",
            "#[derive(Clone, Debug, earnest::model::Model)] struct A;",
            "#[cfg(test)]\n#[derive(Clone, r#Model)]\nstruct A;",
            "mod inner { #[derive(earnest::model::Domain)] enum E { A } }",
            "fn f() { #[derive(Model)] struct A; }",
            "#[model] impl A {}",
            "#[earnest::model::model] impl A {}",
            "#[r#model]\nimpl A {}",
            "#[view(title = \"T\")] impl A {}",
            "impl A { #[action(\"go\")] fn go(&self) {} }",
            "impl A { #[rule(\"r\")] fn r(&self) -> bool { true } }",
            "#[cfg_attr(test, derive(Model))] struct A;",
            "impl earnest::model::Model for A { fn starts() -> Vec<Self> { vec![] } }",
            "fn f() { require!(true); }",
            "macro_rules! m { () => { #[derive(Model)] struct A; } }",
            "struct A { #[range(0, 3)] n: u8 }",
        ] {
            assert!(declares_a_model(source), "{source:?} declares a model");
        }
    }

    #[test]
    fn a_file_that_only_uses_the_library_declares_none() {
        for source in [
            "use earnest::model::{Behavior, Model};\nfn walk<M: Model + Behavior>() { let _ = M::starts(); }",
            "// #[derive(Model)]\nfn f() {}",
            "/* #[model] */ fn f() {}",
            "const S: &str = \"#[derive(Model)] require!\";",
            "#[derive(Clone)] struct A;",
            "#[test] fn t() {}",
            "impl<M: Model> Walk for M {}",
            "use earnest::model::Model;\nfn count() -> usize { Model::starts().len() }",
            "pub fn one() -> u8 { 1 }",
        ] {
            assert!(!declares_a_model(source), "{source:?} declares none");
        }
    }

    #[test]
    fn a_file_that_can_not_be_shown_to_declare_nothing_declares_a_model() {
        assert!(declares_a_model(
            "use earnest::model::Model; fn f() { \"unclosed"
        ));
        let deep = format!("fn f() {{ {} }}", "(".repeat(5_000));
        assert!(declares_a_model(&deep), "left for the reader to refuse");
        assert!(
            declares_a_model("\u{feff}#[derive(Model)] struct A;"),
            "a byte order mark"
        );
    }

    #[test]
    fn a_path_that_starts_with_the_crate_names_it_however_it_is_written() {
        for source in [
            "use earnest::model::Model;",
            "use earnest::{model::Model};",
            "use earnest :: model :: Model;",
            "use earnest::\n    model::Model;",
            "use earnest::/* the model module */model::{Model, model};",
            "use earnest:: // the model module\n    model::Model;",
            "use ::earnest::model::Model;",
            "use earnest as kit;",
            "extern crate earnest;",
            "use r#earnest::model::Model;",
            "use ::r#earnest::{model::Model};",
            "extern crate r#earnest;",
            "use earnest;",
            "fn f() -> earnest::model::Timer { todo() }",
        ] {
            assert!(uses_library(source), "{source:?} names the crate");
        }
    }

    #[test]
    fn a_word_that_is_not_a_path_to_the_crate_does_not() {
        for source in [
            "pub const MOOD: &str = \"earnest\";",
            "// use earnest::model::Model;\nfn f() {}",
            "/* earnest::model */ fn f() {}",
            "use earnest_rs::read;",
            "fn f(earnest: u8) -> u8 { earnest }",
            "pub fn one() -> u8 { 1 }",
        ] {
            assert!(!uses_library(source), "{source:?} does not name the crate");
        }
    }

    #[test]
    fn a_file_that_mentions_the_crate_and_is_not_rust_is_not_left_out() {
        assert!(
            uses_library("use earnest::model::{Model; \"unclosed"),
            "it can't be shown to leave the crate alone"
        );
        assert!(!uses_library("use other::{Model; \"unclosed"), "no mention");
    }

    #[test]
    fn a_byte_order_mark_does_not_hide_the_crate() {
        assert!(
            uses_library("\u{feff}use earnest::model::Model;"),
            "skipped"
        );
    }

    #[test]
    fn a_file_nested_too_deep_to_lex_safely_that_mentions_the_crate_is_not_left_out() {
        let deep = format!(
            "use earnest::model::Model;\nfn f() {{ {} }}",
            "(".repeat(5_000)
        );
        assert!(uses_library(&deep), "left for the reader to refuse");
    }
}
