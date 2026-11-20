use crate::typing::Local;
use earnest_ast::{MOST_NESTING, TOO_DEEP, Term, TermKind};
use earnest_diagnostics::{Place, Problem};
use proc_macro2::{Ident, Span};
use std::collections::{BTreeMap, BTreeSet};
use syn::ext::IdentExt;
use syn::spanned::Spanned;
use syn::{Attribute, Expr, ExprLit, FieldsNamed, Lit, Path, Type, UnOp};

const LONGEST_FEATURE: usize = 48;

pub(crate) const OUTSIDE: &str = "the checker reads only the library and what this file defines, so it can't know what this does: define it in this file as a constant or a helper function, or use a primitive from \"The primitives\" in the README";

pub(crate) struct Reader<'a> {
    pub(crate) file: &'a str,
    pub(crate) source: &'a str,
    pub(crate) problems: Vec<Problem>,
    pub(crate) enums: BTreeMap<String, Vec<String>>,
    pub(crate) records: BTreeMap<String, FieldsNamed>,
    pub(crate) numbers: BTreeMap<String, i64>,
    pub(crate) models: BTreeSet<String>,
    pub(crate) functions: BTreeSet<String>,
    pub(crate) methods: BTreeSet<String>,
    pub(crate) open_records: Vec<String>,
    pub(crate) bound: Vec<Local>,
    pub(crate) structs: BTreeMap<String, Vec<(String, Type)>>,
    pub(crate) copies: BTreeSet<String>,
    pub(crate) fn_types: BTreeMap<String, Type>,
    pub(crate) method_types: BTreeMap<String, Type>,
    pub(crate) fn_params: BTreeMap<String, Vec<Type>>,
    pub(crate) method_params: BTreeMap<String, Vec<Type>>,
    pub(crate) const_types: BTreeMap<String, Type>,
    pub(crate) const_values: BTreeMap<String, Expr>,
    pub(crate) model_name: Option<String>,
    pub(crate) returns_self: Option<bool>,
    pub(crate) depth: usize,
}

#[derive(Clone, Copy)]
pub(crate) enum Sign {
    Plus,
    Minus,
}

pub(crate) fn unwrapped(expr: &Expr) -> &Expr {
    match expr {
        Expr::Paren(inner) => unwrapped(&inner.expr),
        Expr::Group(inner) => unwrapped(&inner.expr),
        other => other,
    }
}

pub(crate) fn start_of(expr: &Expr) -> Span {
    let mut current = expr;
    loop {
        current = match current {
            Expr::Binary(found) => &found.left,
            Expr::MethodCall(found) => &found.receiver,
            Expr::Field(found) => &found.base,
            Expr::Cast(found) => &found.expr,
            Expr::Index(found) => &found.expr,
            Expr::Try(found) => &found.expr,
            Expr::Call(found) => &found.func,
            Expr::Paren(found) => return found.paren_token.span.open(),
            other => return other.span(),
        };
    }
}

pub(crate) fn place_of(span: Span) -> Place {
    let start = span.start();
    Place {
        line: u32::try_from(start.line).unwrap_or(u32::MAX),
        column: u32::try_from(start.column.saturating_add(1)).unwrap_or(u32::MAX),
    }
}

pub(crate) fn at(span: Span, kind: TermKind) -> Term {
    Term::new(place_of(span), kind)
}

pub(crate) fn name_of(ident: &Ident) -> String {
    ident.unraw().to_string()
}

pub(crate) fn path_text(path: &Path) -> String {
    let names: Vec<String> = path
        .segments
        .iter()
        .map(|segment| name_of(&segment.ident))
        .collect();
    names.join("::")
}

pub(crate) fn why_outside(path: &str) -> &'static str {
    let starts = |prefixes: &[&str]| prefixes.iter().any(|prefix| path.starts_with(prefix));
    if starts(&["std::time", "Instant", "SystemTime", "chrono"]) {
        "the system clock is outside the library: model time with a `Timer`, or as a number that actions step up"
    } else if starts(&["rand", "fastrand", "getrandom"]) {
        "random values are outside the library: model each possible value as an action input, with `#[one_of(..)]` or an enum that derives `Domain`, so every one is tried"
    } else if starts(&[
        "std::fs",
        "std::io",
        "std::net",
        "std::env",
        "std::process",
        "tokio",
    ]) {
        "input and output are outside the library: model the answer as an action input, or the call as a `Request`"
    } else {
        OUTSIDE
    }
}

const ONE_BUILD: &str = "the checker reads the file as it is written, for every build at once: \
    a part kept or left out by `cfg` would be checked in one build and missing in another";

pub(crate) fn attribute<'a>(attributes: &'a [Attribute], name: &str) -> Option<&'a Attribute> {
    attributes
        .iter()
        .find(|attribute| attribute.path().is_ident(name))
}

impl<'a> Reader<'a> {
    pub(crate) fn new(file: &'a str, source: &'a str) -> Self {
        Self {
            file,
            source,
            problems: Vec::new(),
            enums: BTreeMap::new(),
            records: BTreeMap::new(),
            numbers: BTreeMap::new(),
            models: BTreeSet::new(),
            functions: BTreeSet::new(),
            methods: BTreeSet::new(),
            open_records: Vec::new(),
            bound: Vec::new(),
            structs: BTreeMap::new(),
            copies: BTreeSet::new(),
            fn_types: BTreeMap::new(),
            method_types: BTreeMap::new(),
            fn_params: BTreeMap::new(),
            method_params: BTreeMap::new(),
            const_types: BTreeMap::new(),
            const_values: BTreeMap::new(),
            model_name: None,
            returns_self: None,
            depth: 0,
        }
    }

    pub(crate) fn refuse(&mut self, span: Span, feature: &str, message: &str) {
        self.problems.push(Problem::new(
            self.file,
            place_of(span),
            &format!("`{feature}`"),
            message,
        ));
    }

    pub(crate) fn refuse_repeats(&mut self, names: &[(String, Span)], what: &str, code: &str) {
        for (position, (name, span)) in names.iter().enumerate() {
            let seen_before = names
                .iter()
                .take(position)
                .any(|(earlier, _)| earlier == name);
            if name != "_" && seen_before {
                self.refuse(
                    *span,
                    &format!("{what} {name}"),
                    &format!(
                        "`{name}` is a {what} that is written twice in one place, and Rust refuses \
                         the file ({code}): it has no meaning to check"
                    ),
                );
            }
        }
    }

    pub(crate) fn refuse_conditional(&mut self, attributes: &[Attribute]) {
        for attribute in attributes {
            let path = attribute.path();
            if path.is_ident("cfg") || path.is_ident("cfg_attr") {
                let feature = format!("#[{}]", path_text(path));
                self.refuse(attribute.pound_token.span, &feature, ONE_BUILD);
            }
        }
    }

    pub(crate) fn refuse_constant_name(&mut self, ident: &Ident) -> bool {
        let name = name_of(ident);
        let why = if self.const_types.contains_key(&name) {
            format!(
                "`{name}` is a constant, so as a pattern it is the constant's value to compare \
                 with and not a new name, and Rust refuses a pattern that can fail where it \
                 must always match, in a `let`, a parameter and a closure's parameter (E0005): \
                 give the value another name"
            )
        } else if ["Some", "None", "Ok", "Err"].contains(&name.as_str()) {
            format!(
                "`{name}` is a variant that Rust has in scope, so as a pattern it is that \
                 variant and not a new name (E0530 or E0005): give the value another name"
            )
        } else {
            return false;
        };
        self.refuse(ident.span(), &name, &why);
        true
    }

    pub(crate) fn refuse_constant_name_of(&mut self, name: &str, span: Span) -> bool {
        if ["Some", "None", "Ok", "Err"].contains(&name) {
            self.refuse(
                span,
                name,
                &format!(
                    "`{name}` is a variant that Rust has in scope, so as a pattern it is that \
                     variant and not a new name: give the value another name"
                ),
            );
            return true;
        }
        false
    }

    pub(crate) fn too_deep(&mut self, at: impl FnOnce() -> Span) -> Option<Term> {
        if self.depth < MOST_NESTING {
            return None;
        }
        let span = at();
        let feature = self.short_text(span);
        Some(self.nothing(span, &feature, TOO_DEEP))
    }

    pub(crate) fn nothing(&mut self, span: Span, feature: &str, message: &str) -> Term {
        self.refuse(span, feature, message);
        at(span, TermKind::Null)
    }

    pub(crate) fn text(&self, span: Span) -> String {
        let words: Vec<&str> = self
            .source
            .get(span.byte_range())
            .unwrap_or_default()
            .split_whitespace()
            .collect();
        words.join(" ")
    }

    pub(crate) fn short_text(&self, span: Span) -> String {
        let text = self.text(span);
        if text.chars().count() <= LONGEST_FEATURE {
            return text;
        }
        let kept: String = text.chars().take(LONGEST_FEATURE).collect();
        format!("{kept}...")
    }

    pub(crate) fn whole_number(&self, expr: &Expr) -> Option<i64> {
        match expr {
            Expr::Lit(literal) => Self::literal_number(literal, Sign::Plus),
            Expr::Unary(unary) if matches!(unary.op, UnOp::Neg(_)) => {
                match unwrapped(&unary.expr) {
                    Expr::Lit(literal) => Self::literal_number(literal, Sign::Minus),
                    inner => self.whole_number(inner)?.checked_neg(),
                }
            }
            Expr::Paren(inner) => self.whole_number(&inner.expr),
            Expr::Group(inner) => self.whole_number(&inner.expr),
            Expr::Path(name) => {
                let ident = name.path.get_ident()?;
                self.numbers.get(&name_of(ident)).copied()
            }
            _ => None,
        }
    }

    pub(crate) fn literal_number(literal: &ExprLit, sign: Sign) -> Option<i64> {
        let Lit::Int(number) = &literal.lit else {
            return None;
        };
        let magnitude: i128 = number.base10_parse().ok()?;
        let signed = match sign {
            Sign::Plus => magnitude,
            Sign::Minus => -magnitude,
        };
        i64::try_from(signed).ok()
    }
}
