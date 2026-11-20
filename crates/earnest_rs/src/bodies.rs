use crate::reader::{Reader, at, name_of, path_text, unwrapped};
use crate::typing::{inside_option, is_reference, plain};
use earnest_ast::{Binary, MOST_NESTING, Term, TermKind};
use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::{Block, Expr, ExprIf, Local, Pat, Stmt, StmtMacro, Type};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Body {
    Action,
    Value,
}

fn is_require(statement: &StmtMacro) -> bool {
    statement
        .mac
        .path
        .segments
        .last()
        .is_some_and(|segment| name_of(&segment.ident) == "require")
}

fn early_return(statement: &Stmt) -> Option<&ExprIf> {
    match statement {
        Stmt::Expr(Expr::If(branch), _) if branch.else_branch.is_none() => Some(branch),
        _ => None,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Gives {
    Plain,
    Some,
    SomeOfSome,
}

fn gives(term: &Term) -> Vec<Gives> {
    match &term.kind {
        TermKind::Wrap(inner) => {
            let inside = gives(inner);
            vec![if inside.iter().any(|found| *found != Gives::Plain) {
                Gives::SomeOfSome
            } else {
                Gives::Some
            }]
        }
        TermKind::Choose(_, yes, no) => {
            let mut found = gives(yes);
            found.extend(gives(no));
            found
        }
        TermKind::Bind { body, .. } => gives(body),
        TermKind::Unpack {
            present, absent, ..
        } => {
            let mut found = gives(present);
            found.extend(gives(absent));
            found
        }
        _ => vec![Gives::Plain],
    }
}

fn always() -> TermKind {
    TermKind::Bool(true)
}

impl Reader<'_> {
    pub(crate) fn value_of(&mut self, block: &Block) -> Term {
        self.rest(&block.stmts, Body::Value, block.brace_token.span.close())
    }

    pub(crate) fn rest(&mut self, statements: &[Stmt], body: Body, close: Span) -> Term {
        if let Some(refused) = self.too_deep(|| close) {
            return refused;
        }
        self.depth = self.depth.saturating_add(1);
        let read = self.statements(statements, body, close);
        self.depth = self.depth.saturating_sub(1);
        read
    }

    fn statements(&mut self, statements: &[Stmt], body: Body, close: Span) -> Term {
        let Some((first, later)) = statements.split_first() else {
            return self.nothing(close, "}", "the body ends without giving a value: end it with the value as the last expression, without a `;`");
        };
        match first {
            Stmt::Local(local) => {
                let bound = self.binding(local);
                if bound.test.is_some() && (body == Body::Value || self.returns_self == Some(true))
                {
                    self.refuse(
                        local.let_token.span,
                        "?",
                        "`?` and `let ... else { return None }` are read only in an action that \
                         returns `Option<Self>`, where `None` means the action can't happen: \
                         here, test with `if let Some(x) = value { .. } else { .. }`",
                    );
                }
                self.bound.push(plain(bound.name.clone(), bound.ty));
                let inner = self.rest(later, body, close);
                self.bound.pop();
                at(
                    local.let_token.span,
                    TermKind::Bind {
                        name: bound.name,
                        value: Box::new(bound.value),
                        body: Box::new(inner),
                    },
                )
            }
            Stmt::Macro(called) => {
                self.refuse_conditional(&called.attrs);
                if !is_require(called) {
                    self.refuse_macro(&called.mac);
                } else if body == Body::Value {
                    self.refuse(
                        called.mac.path.span(),
                        "require!",
                        "only an action has a guard: here, give back `false` or use an `if`",
                    );
                } else if self.returns_self == Some(true) {
                    self.refuse(
                        called.mac.path.span(),
                        "require!",
                        "`require!` returns `None`, which an action that returns `Self` can't \
                         (E0308): return `Option<Self>` to have a guard",
                    );
                }
                self.rest(later, body, close)
            }
            Stmt::Item(item) => {
                self.refuse(
                    item.span(),
                    "an item inside a body",
                    "declare it at the top of the file",
                );
                self.rest(later, body, close)
            }
            Stmt::Expr(expr, _) if later.is_empty() => self.last(expr, body),
            Stmt::Expr(expr, _) => {
                if let Some(branch) = early_return(first) {
                    let test = self.term(&branch.cond);
                    let yes = self.returning(branch, body);
                    let no = self.rest(later, body, close);
                    return at(
                        branch.if_token.span,
                        TermKind::Choose(Box::new(test), Box::new(yes), Box::new(no)),
                    );
                }
                self.dropped(expr);
                self.rest(later, body, close)
            }
        }
    }

    fn last(&mut self, expr: &Expr, body: Body) -> Term {
        let value = match expr {
            Expr::Return(returned) => match &returned.expr {
                Some(given) => given.as_ref(),
                None => {
                    return self.nothing(
                        returned.return_token.span,
                        "return",
                        "a body gives back a value: write `return value;`, or the value as the last expression",
                    );
                }
            },
            other => other,
        };
        let gives_none = matches!(value, Expr::Path(name) if path_text(&name.path) == "None");
        if body == Body::Action && gives_none {
            let why = if self.returns_self == Some(true) {
                "an action that returns `Self` always happens and can't give back `None`"
            } else {
                "say when an action can't happen with `require!(condition);`"
            };
            return self.nothing(value.span(), "None", why);
        }
        let term = self.term(value);
        if body == Body::Action {
            self.refuse_wrapping(value, &term);
        }
        term
    }

    fn refuse_wrapping(&mut self, value: &Expr, term: &Term) {
        let Some(returns_self) = self.returns_self else {
            return;
        };
        let wanted = if returns_self {
            Gives::Plain
        } else {
            Gives::Some
        };
        if gives(term).iter().all(|found| *found == wanted) {
            return;
        }
        let feature = self.short_text(value.span());
        let why = if returns_self {
            "an action that returns `Self` gives back the state itself, `Self { .. }`, and \
             `Some(..)` is an `Option<Self>` (E0308)"
        } else {
            "an action that returns `Option<Self>` gives back `Some(Self { .. })`, and a bare \
             `Self { .. }` or a `Some(Some(..))` is not that type (E0308)"
        };
        self.refuse(value.span(), &feature, why);
    }

    fn returning(&mut self, branch: &ExprIf, body: Body) -> Term {
        let block = &branch.then_branch;
        let ends_in_return = matches!(block.stmts.last(), Some(Stmt::Expr(Expr::Return(_), _)));
        if !ends_in_return {
            return self.nothing(
                branch.if_token.span,
                "if",
                "an `if` before the last expression must end in `return`; nothing else can \
                 happen in it, because nothing changes in place",
            );
        }
        self.rest(&block.stmts, body, block.brace_token.span.close())
    }

    fn dropped(&mut self, expr: &Expr) {
        let before = self.problems.len();
        self.term(expr);
        if self.problems.len() == before {
            let feature = self.short_text(expr.span());
            self.refuse(
                expr.span(),
                &feature,
                "its value is dropped: every operation gives back a new value, so bind it with \
                 `let` or give it back",
            );
        }
    }

    pub(crate) fn guard(&mut self, statements: &[Stmt]) -> Option<Term> {
        if self.depth >= MOST_NESTING {
            return None;
        }
        self.depth = self.depth.saturating_add(1);
        let read = self.guarded(statements);
        self.depth = self.depth.saturating_sub(1);
        read
    }

    fn guarded(&mut self, statements: &[Stmt]) -> Option<Term> {
        let (first, later) = statements.split_first()?;
        match first {
            Stmt::Local(local) => {
                let span = local.let_token.span;
                let bound = self.binding(local);
                self.bound.push(plain(bound.name.clone(), bound.ty));
                let inner = self.guard(later);
                self.bound.pop();
                let name = bound.name;
                let value = Box::new(bound.value);
                let Some(test) = bound.test else {
                    let inner = inner?;
                    return Some(at(
                        span,
                        TermKind::Bind {
                            name,
                            value,
                            body: Box::new(inner),
                        },
                    ));
                };
                let present = at(
                    span,
                    TermKind::Call {
                        target: Box::new(test),
                        method: "is_some".to_owned(),
                        arguments: Vec::new(),
                    },
                );
                let Some(inner) = inner else {
                    return Some(present);
                };
                let rest = at(
                    span,
                    TermKind::Bind {
                        name,
                        value,
                        body: Box::new(inner),
                    },
                );
                Some(at(
                    span,
                    TermKind::Binary(Binary::And, Box::new(present), Box::new(rest)),
                ))
            }
            Stmt::Macro(called) if is_require(called) => {
                let span = called.mac.span();
                let condition = match called.mac.parse_body::<Expr>() {
                    Ok(expr) => self.term(&expr),
                    Err(_) => self.nothing(
                        span,
                        "require!",
                        "write one condition in the brackets: `require!(condition);`",
                    ),
                };
                Some(match self.guard(later) {
                    Some(inner) => at(
                        span,
                        TermKind::Binary(Binary::And, Box::new(condition), Box::new(inner)),
                    ),
                    None => condition,
                })
            }
            _ => {
                let Some(branch) = early_return(first).filter(|_| !later.is_empty()) else {
                    return self.guard(later);
                };
                let inside = self.guard(&branch.then_branch.stmts);
                let after = self.guard(later);
                if inside.is_none() && after.is_none() {
                    return None;
                }
                let span = branch.if_token.span;
                let test = self.term(&branch.cond);
                let yes = inside.unwrap_or_else(|| at(span, always()));
                let no = after.unwrap_or_else(|| at(span, always()));
                Some(at(
                    span,
                    TermKind::Choose(Box::new(test), Box::new(yes), Box::new(no)),
                ))
            }
        }
    }

    pub(crate) fn scan(&mut self, block: &Block) {
        for statement in &block.stmts {
            match statement {
                Stmt::Local(local) => {
                    self.binding(local);
                }
                Stmt::Expr(expr, _) => {
                    self.term(expr);
                }
                Stmt::Macro(called) if !is_require(called) => {
                    self.refuse_macro(&called.mac);
                }
                Stmt::Macro(_) | Stmt::Item(_) => {}
            }
        }
    }

    pub(crate) fn binding(&mut self, local: &Local) -> Bound {
        self.refuse_conditional(&local.attrs);
        let (pattern, stated) = match &local.pat {
            Pat::Type(typed) => (typed.pat.as_ref(), Some(typed.ty.as_ref())),
            other => (other, None),
        };
        let (pattern, refutable) = self.some_pattern(pattern, local);
        let name = match pattern {
            Pat::Ident(named) if named.subpat.is_none() && named.by_ref.is_none() => {
                if named.mutability.is_some() {
                    self.refuse(
                        local.let_token.span,
                        "let mut",
                        "a binding can't change: drop the `mut`, and write `let next = value + 1;` under a new name",
                    );
                }
                name_of(&named.ident)
            }
            other => {
                let feature = self.short_text(other.span());
                self.refuse(
                    other.span(),
                    &feature,
                    "a `let` binds one plain name: write `let name = value;` and read a part of it with `name.field`",
                );
                "_".to_owned()
            }
        };
        let Some(init) = &local.init else {
            let value = self.nothing(
                local.let_token.span,
                "let",
                "a binding gets its value where it is declared: `let name = value;`",
            );
            return Bound {
                name,
                value,
                ty: stated.cloned(),
                test: None,
            };
        };
        let mut optional = None;
        let source = match unwrapped(&init.expr) {
            Expr::Try(tried) if !refutable => {
                optional = Some(tried.expr.as_ref());
                tried.expr.as_ref()
            }
            other => other,
        };
        if refutable {
            optional = Some(source);
            self.refuse_returns_none(init.diverge.as_ref());
        } else if let Some((otherwise, _)) = &init.diverge {
            self.refuse(
                otherwise.span,
                "let else",
                "`let ... else` is read only as `let Some(x) = value else { return None; };` in \
                 an action: write `let x = value?;`, which is the same and is what clippy asks \
                 for",
            );
        }
        let Some(optional) = optional else {
            let value = self.term(source);
            let ty = stated.cloned().or_else(|| self.static_type(source));
            let value = match stated {
                Some(declared) => self.of_type(declared, value),
                None => value,
            };
            return Bound {
                name,
                value,
                ty,
                test: None,
            };
        };
        self.refuse_borrowed_option(optional);
        let test = self.term(optional);
        let present = Term::new(test.place, TermKind::Present(Box::new(test.clone())));
        let ty = stated.cloned().or_else(|| {
            let found = self.static_type(optional)?;
            inside_option(&found).cloned()
        });
        let value = match stated {
            Some(declared) => self.of_type(declared, present),
            None => present,
        };
        Bound {
            name,
            value,
            ty,
            test: Some(test),
        }
    }

    fn some_pattern<'p>(&mut self, pattern: &'p Pat, local: &Local) -> (&'p Pat, bool) {
        let Pat::TupleStruct(found) = pattern else {
            return (pattern, false);
        };
        let is_some = found.qself.is_none() && found.path.is_ident("Some");
        if let (true, Some(inner), 1) = (is_some, found.elems.first(), found.elems.len()) {
            if local
                .init
                .as_ref()
                .and_then(|init| init.diverge.as_ref())
                .is_none()
            {
                self.refuse(
                    found.path.span(),
                    "let Some(..)",
                    "a `let` with `Some(x)` can fail, and Rust refuses it without an `else` \
                     (E0005): write `let x = value?;` in an action, or `if let Some(x) = value \
                     { .. } else { .. }`",
                );
            }
            return (inner, true);
        }
        let feature = self.short_text(pattern.span());
        self.refuse(
            pattern.span(),
            &feature,
            "a `let` binds one plain name, `let name = value;`, or `Some(name)` with `else { return None; }`: read a part with `name.field`",
        );
        (pattern, false)
    }

    fn refuse_returns_none(&mut self, diverge: Option<&(syn::token::Else, Box<Expr>)>) {
        let Some((otherwise, block)) = diverge else {
            return;
        };
        let gives_none = match unwrapped(block) {
            Expr::Block(inner) => match inner.block.stmts.as_slice() {
                [Stmt::Expr(Expr::Return(returned), _)] => returned.expr.as_deref().is_some_and(
                    |value| matches!(value, Expr::Path(name) if path_text(&name.path) == "None"),
                ),
                _ => false,
            },
            _ => false,
        };
        if !gives_none {
            self.refuse(
                otherwise.span,
                "let else",
                "the `else` of a `let Some(x) = value` is read only as `else { return None; }`, \
                 which makes the action not happen where `value` is `None`: write `let x = \
                 value?;`, or use `if let Some(x) = value { .. } else { .. }`",
            );
        }
    }

    pub(crate) fn refuse_borrowed_option(&mut self, value: &Expr) {
        let span = value.span();
        let moved = "Rust moves the value out of its place when it is read this way, so the \
                     type must be `Copy`: write `value.clone()`, or bind it by value";
        match unwrapped(value) {
            Expr::Reference(_) => {
                let feature = self.short_text(span);
                self.refuse(
                    span,
                    &feature,
                    "this binds the value inside by reference, which the checker does not read \
                     (a reference to a number does not compare with a number): write the \
                     `Option` itself, `self.field`, or `self.field.clone()`",
                );
            }
            Expr::Path(name) if name.path.get_ident().is_some() => {
                let known = self.static_type(value);
                if known.is_some_and(|found| is_reference(&found)) && !is_self_state(value) {
                    let feature = self.short_text(span);
                    self.refuse(
                        span,
                        &feature,
                        "this is a reference to an `Option`, so the value inside binds by \
                         reference: pass the `Option` by value, or write `value.clone()`",
                    );
                }
            }
            Expr::Field(_) => {
                let known = self.static_type(value);
                match known {
                    Some(found) if is_reference(&found) => {
                        let feature = self.short_text(span);
                        self.refuse(
                            span,
                            &feature,
                            "this is a reference to an `Option`, so the value inside binds by \
                             reference: pass the `Option` by value, or write `value.clone()`",
                        );
                    }
                    Some(found) if self.is_copy(&found) => {}
                    Some(_) => {
                        let feature = self.short_text(span);
                        self.refuse(span, &feature, moved);
                    }
                    None => {
                        let feature = self.short_text(span);
                        self.refuse(
                            span,
                            &feature,
                            "the checker can't tell the type of this value, so it can't tell \
                             whether Rust copies it: read a field of the state or of a record, \
                             a parameter with its type, or write `value.clone()`",
                        );
                    }
                }
            }
            _ => {}
        }
    }
}

pub(crate) struct Bound {
    pub(crate) name: String,
    pub(crate) value: Term,
    pub(crate) ty: Option<Type>,
    pub(crate) test: Option<Term>,
}

fn is_self_state(value: &Expr) -> bool {
    matches!(unwrapped(value), Expr::Path(name) if name.path.is_ident("self"))
}
