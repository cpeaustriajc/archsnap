use crate::reader::{Reader, at, name_of, path_text, unwrapped};
use crate::typing::{inside_option, path_name, plain};
use earnest_ast::{Binary, Term, TermKind};
use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::{Arm, Expr, ExprIf, ExprLet, ExprMatch, Macro, Pat, Token};

#[derive(Clone)]
enum Pick {
    Variant(String),
    Anything,
}

type Alternative = Vec<Pick>;

struct Matched {
    scrutinees: Vec<(String, Term, Vec<String>)>,
    arms: Vec<(Vec<Alternative>, Term)>,
}

const NAMES: [&str; 2] = ["$match0", "$match1"];

const NEEDS_ENUM: &str = "`match` is read on a value of an enum that derives `Domain`, or on a \
    pair of two such values written `(a, b)`: for an `Option`, write `if let Some(x) = value { \
    .. } else { .. }`, and for a number, compare with `==` in an `if`";

fn is_true(term: &Term) -> bool {
    matches!(term.kind, TermKind::Bool(true))
}

impl Reader<'_> {
    pub(crate) fn matching(&mut self, found: &ExprMatch) -> Term {
        self.refuse_conditional(&found.attrs);
        let span = found.match_token.span;
        let Some(scrutinees) = self.scrutinees(&found.expr) else {
            for arm in &found.arms {
                self.term(&arm.body);
            }
            return at(span, TermKind::Null);
        };
        let mut arms = Vec::with_capacity(found.arms.len());
        let mut sound = true;
        for arm in &found.arms {
            let body = self.arm_body(arm);
            match self.alternatives(&arm.pat, &scrutinees) {
                Some(listed) => arms.push((listed, body)),
                None => sound = false,
            }
            if let Some((guard, _)) = &arm.guard {
                self.refuse(
                    guard.span,
                    "if",
                    "a guard is outside the library: nest an `if` in the arm, or match the \
                     variants alone",
                );
                sound = false;
            }
            self.refuse_conditional(&arm.attrs);
        }
        let matched = Matched { scrutinees, arms };
        if !sound {
            return at(span, TermKind::Null);
        }
        self.build(matched, span)
    }

    pub(crate) fn matches_macro(&mut self, called: &Macro) -> Term {
        let span = called.path.span();
        let parsed = called.parse_body_with(|input: syn::parse::ParseStream<'_>| {
            let value: Expr = input.parse()?;
            input.parse::<Token![,]>()?;
            let pattern = Pat::parse_multi_with_leading_vert(input)?;
            let guard = if input.peek(Token![if]) {
                Some(input.parse::<Token![if]>()?.span)
            } else {
                None
            };
            Ok((value, pattern, guard))
        });
        let Ok((value, pattern, guard)) = parsed else {
            return self.nothing(
                span,
                "matches!",
                "write `matches!(value, Enum::A | Enum::B)`: the value, a comma, then the \
                 variants",
            );
        };
        if let Some(at_if) = guard {
            return self.nothing(
                at_if,
                "if",
                "a guard is outside the library: use `&&` after the `matches!`",
            );
        }
        let Some(scrutinees) = self.scrutinees(&value) else {
            return at(span, TermKind::Null);
        };
        let Some(listed) = self.alternatives(&pattern, &scrutinees) else {
            return at(span, TermKind::Null);
        };
        let yes = at(span, TermKind::Bool(true));
        let no = at(span, TermKind::Bool(false));
        let wild = vec![Pick::Anything; scrutinees.len()];
        let matched = Matched {
            scrutinees,
            arms: vec![(listed, yes), (vec![wild], no)],
        };
        Self::build_unchecked(matched, span)
    }

    fn arm_body(&mut self, arm: &Arm) -> Term {
        match unwrapped(&arm.body) {
            Expr::Block(block) if block.label.is_none() => {
                self.refuse_conditional(&block.attrs);
                self.value_of(&block.block)
            }
            other => self.term(other),
        }
    }

    fn scrutinees(&mut self, value: &Expr) -> Option<Vec<(String, Term, Vec<String>)>> {
        let parts: Vec<&Expr> = match unwrapped(value) {
            Expr::Tuple(pair) if pair.elems.len() == 2 => pair.elems.iter().collect(),
            Expr::Tuple(pair) => {
                self.refuse(
                    pair.span(),
                    "match on a tuple",
                    "`match` is read on one enum value or on a pair of two: nest a `match` for \
                     more",
                );
                return None;
            }
            single => vec![single],
        };
        let mut found = Vec::with_capacity(parts.len());
        for (index, part) in parts.into_iter().enumerate() {
            let typed = self.static_type(part);
            let variants = typed
                .as_ref()
                .filter(|ty| inside_option(ty).is_none())
                .and_then(path_name)
                .and_then(|name| self.enums.get(&name).cloned());
            let Some(variants) = variants else {
                let feature = self.short_text(part.span());
                let message = if typed.is_some() {
                    NEEDS_ENUM
                } else {
                    "the checker can't tell which enum this value is, so it can't tell \
                     whether the arms cover it: match on a field of the state or of a record, \
                     an action's input, a helper's parameter or result with its type written, \
                     or a `let` with its type written"
                };
                self.refuse(part.span(), &feature, message);
                return None;
            };
            let term = self.term(part);
            found.push((NAMES[index].to_owned(), term, variants));
        }
        Some(found)
    }

    fn alternatives(
        &mut self,
        pattern: &Pat,
        scrutinees: &[(String, Term, Vec<String>)],
    ) -> Option<Vec<Alternative>> {
        let cases: Vec<&Pat> = match pattern {
            Pat::Or(cases) => cases.cases.iter().collect(),
            other => vec![other],
        };
        let mut listed = Vec::with_capacity(cases.len());
        for case in cases {
            listed.extend(self.alternative(case, scrutinees)?);
        }
        Some(listed)
    }

    fn alternative(
        &mut self,
        pattern: &Pat,
        scrutinees: &[(String, Term, Vec<String>)],
    ) -> Option<Vec<Alternative>> {
        match (pattern, scrutinees) {
            (Pat::Paren(inner), _) => self.alternative(&inner.pat, scrutinees),
            (Pat::Tuple(pair), [_, _]) if pair.elems.len() == 2 => {
                let mut parts = Vec::with_capacity(2);
                for (element, scrutinee) in pair.elems.iter().zip(scrutinees) {
                    parts.push(self.either_of(element, scrutinee)?);
                }
                let mut product: Vec<Alternative> = vec![Vec::new()];
                for options in parts {
                    product = product
                        .into_iter()
                        .flat_map(|so_far| {
                            options.iter().map(move |option| {
                                let mut next = so_far.clone();
                                next.push(option.clone());
                                next
                            })
                        })
                        .collect();
                }
                Some(product)
            }
            (other, [only]) => Some(vec![vec![self.variant_of(other, only)?]]),
            (other, _) => {
                let feature = self.short_text(other.span());
                self.refuse(
                    other.span(),
                    &feature,
                    "this value is a pair, so each arm is a pair: `(Enum::A, _)`",
                );
                None
            }
        }
    }

    fn either_of(
        &mut self,
        pattern: &Pat,
        scrutinee: &(String, Term, Vec<String>),
    ) -> Option<Vec<Pick>> {
        match pattern {
            Pat::Or(cases) => {
                let mut options = Vec::with_capacity(cases.cases.len());
                for case in &cases.cases {
                    if matches!(case, Pat::Wild(_)) {
                        let feature = self.short_text(case.span());
                        self.refuse(
                            case.span(),
                            &feature,
                            "a `_` joined to other patterns with `|` makes them pointless: write \
                             `_` alone",
                        );
                        return None;
                    }
                    options.push(self.variant_of(case, scrutinee)?);
                }
                Some(options)
            }
            Pat::Paren(inner) => self.either_of(&inner.pat, scrutinee),
            other => Some(vec![self.variant_of(other, scrutinee)?]),
        }
    }

    fn variant_of(
        &mut self,
        pattern: &Pat,
        scrutinee: &(String, Term, Vec<String>),
    ) -> Option<Pick> {
        let span = pattern.span();
        let feature = self.short_text(span);
        match pattern {
            Pat::Wild(_) => Some(Pick::Anything),
            Pat::Paren(inner) => self.variant_of(&inner.pat, scrutinee),
            Pat::Path(named) if named.qself.is_none() => {
                let text = path_text(&named.path);
                let parts: Vec<&str> = text.split("::").collect();
                let [owner, variant] = parts.as_slice() else {
                    self.refuse(span, &feature, "name a variant as `Enum::Variant`");
                    return None;
                };
                if scrutinee.2.iter().any(|known| known == variant)
                    && self.owner_of(&scrutinee.2) == Some((*owner).to_owned())
                {
                    return Some(Pick::Variant((*variant).to_owned()));
                }
                self.refuse(
                    span,
                    &feature,
                    "this is not a variant of the enum being matched, and Rust refuses the \
                     arm (E0308 or E0599)",
                );
                None
            }
            Pat::Ident(named) => {
                let _ = named;
                self.refuse(
                    span,
                    &feature,
                    "a name catches every value and binds it, which the checker does not read: \
                     write `_` for the rest, or each variant as `Enum::Variant`",
                );
                None
            }
            Pat::Or(_) => {
                self.refuse(
                    span,
                    &feature,
                    "an `|` here is outside the library: put it around a whole arm, \
                     `Enum::A | Enum::B =>`, or around one side of a pair, `(Enum::A | Enum::B, _)`",
                );
                None
            }
            _ => {
                self.refuse(
                    span,
                    &feature,
                    "a nested pattern, a binding and a reference pattern are outside the \
                     library: match a variant as `Enum::Variant`, `_` for the rest, and \
                     `A | B` for several",
                );
                None
            }
        }
    }

    fn owner_of(&self, variants: &[String]) -> Option<String> {
        self.enums
            .iter()
            .find(|(_, known)| known.as_slice() == variants)
            .map(|(name, _)| name.clone())
    }

    fn build(&mut self, matched: Matched, span: Span) -> Term {
        if let Some(missing) = missing_case(&matched) {
            self.refuse(
                span,
                "match",
                &format!(
                    "the arms do not cover {missing}, and Rust refuses a `match` that leaves a \
                     value out (E0004): add the arm, or a `_` arm"
                ),
            );
            return at(span, TermKind::Null);
        }
        if matched.arms.is_empty() {
            return self.nothing(span, "match", "a `match` needs at least one arm");
        }
        Self::build_unchecked(matched, span)
    }

    fn build_unchecked(matched: Matched, span: Span) -> Term {
        let Matched { scrutinees, arms } = matched;
        let mut chain: Option<Term> = None;
        for (alternatives, body) in arms.into_iter().rev() {
            chain = Some(match chain {
                None => body,
                Some(otherwise) => {
                    let test = anyone(&alternatives, &scrutinees, span);
                    if is_true(&test) {
                        body
                    } else {
                        at(
                            span,
                            TermKind::Choose(Box::new(test), Box::new(body), Box::new(otherwise)),
                        )
                    }
                }
            });
        }
        let mut result = chain.unwrap_or_else(|| at(span, TermKind::Null));
        for (name, value, _) in scrutinees.into_iter().rev() {
            result = at(
                span,
                TermKind::Bind {
                    name,
                    value: Box::new(value),
                    body: Box::new(result),
                },
            );
        }
        result
    }

    pub(crate) fn unpack(&mut self, branch: &ExprIf, binding: &ExprLet) -> Term {
        let span = branch.if_token.span;
        let Pat::TupleStruct(some) = binding.pat.as_ref() else {
            return self.nothing(
                binding.let_token.span,
                "if let",
                "`if let` is read as `if let Some(x) = value { .. } else { .. }` on an `Option`",
            );
        };
        let plain_name = match some.elems.first() {
            Some(Pat::Ident(named))
                if some.elems.len() == 1
                    && named.subpat.is_none()
                    && named.by_ref.is_none()
                    && named.mutability.is_none() =>
            {
                Some(name_of(&named.ident))
            }
            _ => None,
        };
        let (true, Some(name)) = (some.path.is_ident("Some"), plain_name) else {
            return self.nothing(
                some.span(),
                "if let",
                "`if let` is read with `Some(name)`: for `Some(_)` write `is_some()`, for \
                 `None` write `is_none()`, and name the value to use it",
            );
        };
        self.refuse_borrowed_option(&binding.expr);
        let value = self.term(&binding.expr);
        let ty = self
            .static_type(&binding.expr)
            .and_then(|found| inside_option(&found).cloned());
        self.bound.push(plain(name.clone(), ty));
        let present = self.value_of(&branch.then_branch);
        self.bound.pop();
        let absent = match &branch.else_branch {
            Some((_, otherwise)) => self.term(otherwise),
            None => self.nothing(
                span,
                "if let",
                "an `if let` used as a value needs an `else`: write `if let Some(x) = value { a } else { b }`",
            ),
        };
        at(
            span,
            TermKind::Unpack {
                value: Box::new(value),
                name,
                present: Box::new(present),
                absent: Box::new(absent),
                absent_first: false,
            },
        )
    }
}

fn anyone(
    alternatives: &[Alternative],
    scrutinees: &[(String, Term, Vec<String>)],
    span: Span,
) -> Term {
    let mut tests: Vec<Term> = Vec::new();
    for alternative in alternatives {
        let mut parts: Vec<Term> = Vec::new();
        for (variant, (name, _, _)) in alternative.iter().zip(scrutinees) {
            let Pick::Variant(variant) = variant else {
                continue;
            };
            parts.push(at(
                span,
                TermKind::Binary(
                    Binary::Equal,
                    Box::new(at(span, TermKind::Name(name.clone()))),
                    Box::new(at(span, TermKind::Variant(variant.clone()))),
                ),
            ));
        }
        let all = parts.into_iter().reduce(|left, right| {
            at(
                span,
                TermKind::Binary(Binary::And, Box::new(left), Box::new(right)),
            )
        });
        match all {
            Some(test) => tests.push(test),
            None => return at(span, TermKind::Bool(true)),
        }
    }
    tests
        .into_iter()
        .reduce(|left, right| {
            at(
                span,
                TermKind::Binary(Binary::Or, Box::new(left), Box::new(right)),
            )
        })
        .unwrap_or_else(|| at(span, TermKind::Bool(false)))
}

fn missing_case(matched: &Matched) -> Option<String> {
    let lists: Vec<&Vec<String>> = matched.scrutinees.iter().map(|(_, _, list)| list).collect();
    let mut combinations: Vec<Vec<&String>> = vec![Vec::new()];
    for list in lists {
        combinations = combinations
            .into_iter()
            .flat_map(|so_far| {
                list.iter().map(move |variant| {
                    let mut next = so_far.clone();
                    next.push(variant);
                    next
                })
            })
            .collect();
    }
    for combination in combinations {
        let covered = matched.arms.iter().any(|(alternatives, _)| {
            alternatives.iter().any(|alternative| {
                alternative
                    .iter()
                    .zip(&combination)
                    .all(|(wanted, have)| match wanted {
                        Pick::Anything => true,
                        Pick::Variant(variant) => variant == *have,
                    })
            })
        });
        if !covered {
            let shown: Vec<&str> = combination.iter().map(|variant| variant.as_str()).collect();
            return Some(if shown.len() == 1 {
                format!("`{}`", shown.join(", "))
            } else {
                format!("`({})`", shown.join(", "))
            });
        }
    }
    None
}
