use earnest_ast::{Quoting, nesting_problem};
use proc_macro2::{Ident, Spacing, TokenStream, TokenTree};

const CRATE: &str = "earnest";
const RAW: &str = "r#";

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
    use super::uses_library;

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
