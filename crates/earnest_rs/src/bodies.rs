use crate::reader::{Reader, at};
use earnest_ast::{Binary, Term, TermKind};
use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::{Block, Expr, ExprIf, Local, Pat, Stmt, StmtMacro};

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
        .is_some_and(|segment| segment.ident == "require")
}

fn early_return(statement: &Stmt) -> Option<&ExprIf> {
    match statement {
        Stmt::Expr(Expr::If(branch), _) if branch.else_branch.is_none() => Some(branch),
        _ => None,
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
        let Some((first, later)) = statements.split_first() else {
            return self.nothing(close, "}", "the body ends without giving a value");
        };
        match first {
            Stmt::Local(local) => {
                let (name, value) = self.binding(local);
                let inner = self.rest(later, body, close);
                at(
                    local.span(),
                    TermKind::Bind {
                        name,
                        value: Box::new(value),
                        body: Box::new(inner),
                    },
                )
            }
            Stmt::Macro(called) => {
                if !is_require(called) {
                    self.refuse_macro(&called.mac);
                } else if body == Body::Value {
                    self.refuse(
                        called.mac.path.span(),
                        "require!",
                        "only an action has a guard: here, give back `false` or use an `if`",
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
                        branch.span(),
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
                        "a body gives back a value",
                    );
                }
            },
            other => other,
        };
        let gives_none = matches!(value, Expr::Path(name) if name.path.is_ident("None"));
        if body == Body::Action && gives_none {
            return self.nothing(
                value.span(),
                "None",
                "say when an action can't happen with `require!(condition);`",
            );
        }
        self.term(value)
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
        let (first, later) = statements.split_first()?;
        match first {
            Stmt::Local(local) => {
                let inner = self.guard(later)?;
                let (name, value) = self.binding(local);
                Some(at(
                    local.span(),
                    TermKind::Bind {
                        name,
                        value: Box::new(value),
                        body: Box::new(inner),
                    },
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
                let span = branch.span();
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

    fn binding(&mut self, local: &Local) -> (String, Term) {
        let pattern = match &local.pat {
            Pat::Type(typed) => typed.pat.as_ref(),
            other => other,
        };
        let name = match pattern {
            Pat::Ident(named) if named.subpat.is_none() && named.by_ref.is_none() => {
                if named.mutability.is_some() {
                    self.refuse(
                        local.let_token.span,
                        "let mut",
                        "a binding can't change: every operation gives back a new value",
                    );
                }
                named.ident.to_string()
            }
            other => {
                let feature = self.short_text(other.span());
                self.refuse(other.span(), &feature, "a `let` binds one plain name");
                "_".to_owned()
            }
        };
        let value = match &local.init {
            Some(init) => {
                if let Some((otherwise, _)) = &init.diverge {
                    self.refuse(
                        otherwise.span,
                        "let else",
                        "`let ... else` is outside the library",
                    );
                }
                self.term(&init.expr)
            }
            None => self.nothing(
                local.let_token.span,
                "let",
                "a binding gets its value where it is declared: `let name = value;`",
            ),
        };
        (name, value)
    }
}
