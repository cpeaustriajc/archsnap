use earnest_ast::{Term, TermKind};
use earnest_diagnostics::{Place, Problem};
use proc_macro2::Span;
use std::collections::{BTreeMap, BTreeSet};
use syn::{Attribute, Expr, FieldsNamed, Lit, Path, UnOp};

const LONGEST_FEATURE: usize = 48;

pub(crate) const OUTSIDE: &str =
    "it is not part of the library or defined in this file, so the checker can't know what it does";

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

pub(crate) fn path_text(path: &Path) -> String {
    let names: Vec<String> = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    names.join("::")
}

pub(crate) fn why_outside(path: &str) -> &'static str {
    let starts = |prefixes: &[&str]| prefixes.iter().any(|prefix| path.starts_with(prefix));
    if starts(&["std::time", "Instant", "SystemTime", "chrono"]) {
        "the system clock is outside the library"
    } else if starts(&["rand", "fastrand", "getrandom"]) {
        "random values are outside the library"
    } else if starts(&[
        "std::fs",
        "std::io",
        "std::net",
        "std::env",
        "std::process",
        "tokio",
    ]) {
        "input and output are outside the library"
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

    pub(crate) fn refuse_conditional(&mut self, attributes: &[Attribute]) {
        for attribute in attributes {
            let path = attribute.path();
            if path.is_ident("cfg") || path.is_ident("cfg_attr") {
                let feature = format!("#[{}]", path_text(path));
                self.refuse(attribute.pound_token.span, &feature, ONE_BUILD);
            }
        }
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
            Expr::Lit(literal) => match &literal.lit {
                Lit::Int(number) => number.base10_parse().ok(),
                _ => None,
            },
            Expr::Unary(unary) if matches!(unary.op, UnOp::Neg(_)) => {
                self.whole_number(&unary.expr)?.checked_neg()
            }
            Expr::Paren(inner) => self.whole_number(&inner.expr),
            Expr::Group(inner) => self.whole_number(&inner.expr),
            Expr::Path(name) => {
                let ident = name.path.get_ident()?;
                self.numbers.get(&ident.to_string()).copied()
            }
            _ => None,
        }
    }
}
