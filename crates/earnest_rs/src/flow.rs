use crate::reader::{Reader, name_of, unwrapped};
use crate::ty::Ty;
use earnest_value::IntType;
use proc_macro2::Span;
use std::collections::BTreeSet;
use syn::spanned::Spanned;
use syn::{
    BinOp, Block, Expr, ExprClosure, ExprIf, ExprStruct, Lit, Member, Pat, ReturnType, Signature,
    Stmt, Type, UnOp,
};

const BY_VALUE: [&str; 5] = ["unwrap_or", "map_or", "is_some_and", "unwrap", "expect"];
const FIX: &str = "derive `Copy` on its type if it can be, write `.clone()` where it is read, or \
    compare by reference";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Calls {
    Once,
    Often,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum How {
    Move,
    Look,
}

struct Var {
    name: String,
    id: usize,
    ty: Ty,
}

struct Place {
    path: String,
    ty: Ty,
    behind_a_reference: bool,
}

pub(crate) struct Walk<'w, 'r> {
    reader: &'w mut Reader<'r>,
    vars: Vec<Var>,
    moved: BTreeSet<String>,
    next: usize,
    returns: Ty,
}

fn is_conditional_early_return(branch: &ExprIf) -> bool {
    branch.else_branch.is_none()
}

impl Reader<'_> {
    pub(crate) fn walk_body(&mut self, signature: &Signature, block: &Block, returns: Ty) {
        let mut walk = Walk {
            reader: self,
            vars: Vec::new(),
            moved: BTreeSet::new(),
            next: 0,
            returns,
        };
        for input in &signature.inputs {
            walk.parameter(input);
        }
        let tail = walk.block(block);
        let declared = walk.returns.clone();
        if let Some(Stmt::Expr(last, None)) = block.stmts.last() {
            walk.expect(last.span(), &tail, &declared, "the value this gives back");
        }
    }

    pub(crate) fn walk_constant(&mut self, declared: &Type, value: &Expr, what: &'static str) {
        let returns = self.ty_of(declared);
        let mut walk = Walk {
            reader: self,
            vars: Vec::new(),
            moved: BTreeSet::new(),
            next: 0,
            returns: returns.clone(),
        };
        let found = walk.expr(value, How::Move);
        walk.expect(value.span(), &found, &returns, what);
    }
}

impl Walk<'_, '_> {
    fn parameter(&mut self, input: &syn::FnArg) {
        match input {
            syn::FnArg::Receiver(_) => {
                let model = self
                    .reader
                    .model_name
                    .clone()
                    .map_or(Ty::Unknown, Ty::Record);
                self.bind("self", Ty::Ref(Box::new(model)));
            }
            syn::FnArg::Typed(typed) => {
                let ty = self.reader.ty_of(&typed.ty);
                if let Pat::Ident(named) = typed.pat.as_ref() {
                    self.bind(&name_of(&named.ident), ty);
                }
            }
        }
    }

    fn bind(&mut self, name: &str, ty: Ty) {
        self.next = self.next.saturating_add(1);
        self.vars.push(Var {
            name: name.to_owned(),
            id: self.next,
            ty,
        });
    }

    fn refuse(&mut self, span: Span, message: &str) {
        let feature = self.reader.short_text(span);
        self.reader.refuse(span, &feature, message);
    }

    fn expect(&mut self, span: Span, have: &Ty, want: &Ty, what: &str) {
        if have.compatible(want) {
            return;
        }
        let message = format!(
            "{what} is {} where {} is expected, and `rustc` refuses the file (E0308): two \
             types are different types even when their variants or fields are named the same, \
             and two integer types are different even when a number fits both: convert with \
             `from` or use the type that is expected",
            have.shown(),
            want.shown()
        );
        self.refuse(span, &message);
    }

    fn place(&self, expr: &Expr) -> Option<Place> {
        match unwrapped(expr) {
            Expr::Path(name) if name.qself.is_none() => {
                let ident = name.path.get_ident().map(name_of)?;
                let var = self.vars.iter().rev().find(|var| var.name == ident)?;
                Some(Place {
                    path: var.id.to_string(),
                    ty: var.ty.clone(),
                    behind_a_reference: false,
                })
            }
            Expr::Field(read) => {
                let Member::Named(field) = &read.member else {
                    return None;
                };
                let base = self.place(&read.base)?;
                let owner = base.ty.peeled().clone();
                let Ty::Record(record) = &owner else {
                    return Some(Place {
                        ty: Ty::Unknown,
                        path: format!("{}.{}", base.path, name_of(field)),
                        behind_a_reference: base.behind_a_reference || base.ty.is_ref(),
                    });
                };
                let wanted = name_of(field);
                let ty = self
                    .reader
                    .structs
                    .get(record)
                    .and_then(|fields| fields.iter().find(|(known, _)| *known == wanted))
                    .map_or(Ty::Unknown, |(_, ty)| self.reader.ty_of(ty));
                Some(Place {
                    path: format!("{}.{wanted}", base.path),
                    ty,
                    behind_a_reference: base.behind_a_reference || base.ty.is_ref(),
                })
            }
            _ => None,
        }
    }

    fn touch(&mut self, expr: &Expr, place: &Place, how: How) -> Ty {
        let span = expr.span();
        let moved_before = self.moved.iter().any(|gone| {
            gone == &place.path
                || place.path.starts_with(&format!("{gone}."))
                || gone.starts_with(&format!("{}.", place.path))
        });
        if moved_before {
            let named = self.reader.short_text(span);
            self.refuse(
                span,
                &format!(
                    "`{named}` was moved out earlier, and `rustc` refuses a later use of it \
                     (E0382): {FIX}"
                ),
            );
        }
        if how == How::Move && !self.reader.is_copy_ty(&place.ty) {
            if place.behind_a_reference {
                let named = self.reader.short_text(span);
                self.refuse(
                    span,
                    &format!(
                        "this moves {} out of `{named}`, which is only borrowed, and `rustc` \
                         refuses it (E0507): Rust moves the value out of its place when it is \
                         read this way, so the type must be `Copy`: {FIX}",
                        place.ty.shown()
                    ),
                );
            } else {
                self.moved.insert(place.path.clone());
            }
        }
        place.ty.clone()
    }

    fn expr(&mut self, expr: &Expr, how: How) -> Ty {
        if let Some(place) = self.place(expr) {
            return self.touch(expr, &place, how);
        }
        match expr {
            Expr::Paren(inner) => self.expr(&inner.expr, how),
            Expr::Group(inner) => self.expr(&inner.expr, how),
            Expr::Lit(literal) => Self::literal(literal),
            Expr::Path(name) => self.path(name),
            Expr::Field(read) => {
                let base = self.expr(&read.base, How::Look);
                self.field_of(&base, &read.member)
            }
            Expr::MethodCall(call) => self.method(call),
            Expr::Call(call) => self.call(call),
            Expr::Unary(unary) => {
                let inner = self.expr(&unary.expr, How::Move);
                match unary.op {
                    UnOp::Not(_) => Ty::Bool,
                    _ => inner,
                }
            }
            Expr::Binary(binary) => self.binary(binary),
            Expr::Reference(reference) => Ty::Ref(Box::new(self.expr(&reference.expr, How::Look))),
            Expr::If(branch) => self.branching(branch),
            Expr::Match(found) => self.matching_arms(found),
            Expr::Block(inner) if inner.label.is_none() => self.block(&inner.block),
            Expr::Struct(literal) => self.record(literal),
            Expr::Try(tried) => self.expr(&tried.expr, How::Move).inside(),
            Expr::Return(returned) => {
                if let Some(given) = &returned.expr {
                    let found = self.expr(given, How::Move);
                    let wanted = self.returns.clone();
                    self.expect(given.span(), &found, &wanted, "the value this gives back");
                }
                Ty::Never
            }
            Expr::Macro(called) if called.mac.path.is_ident("matches") => {
                self.matches_macro(&called.mac)
            }
            _ => Ty::Unknown,
        }
    }

    fn literal(literal: &syn::ExprLit) -> Ty {
        match &literal.lit {
            Lit::Bool(_) => Ty::Bool,
            Lit::Str(_) => Ty::Text,
            Lit::Int(number) => Ty::Int(IntType::named(number.suffix())),
            _ => Ty::Unknown,
        }
    }

    fn path(&mut self, name: &syn::ExprPath) -> Ty {
        let names: Vec<String> = name
            .path
            .segments
            .iter()
            .map(|segment| name_of(&segment.ident))
            .collect();
        match names.as_slice() {
            [none] if none == "None" => Ty::option_of(Ty::Unknown),
            [constant] => self
                .reader
                .const_types
                .get(constant)
                .map_or(Ty::Unknown, |ty| self.reader.ty_of(&ty.clone())),
            [owner, variant]
                if self
                    .reader
                    .enums
                    .get(owner)
                    .is_some_and(|variants| variants.contains(variant)) =>
            {
                Ty::Enum(owner.clone())
            }
            _ => Ty::Unknown,
        }
    }

    fn field_of(&self, base: &Ty, member: &Member) -> Ty {
        let (Ty::Record(record), Member::Named(field)) = (base.peeled(), member) else {
            return Ty::Unknown;
        };
        let wanted = name_of(field);
        self.reader
            .structs
            .get(record)
            .and_then(|fields| fields.iter().find(|(known, _)| *known == wanted))
            .map_or(Ty::Unknown, |(_, ty)| self.reader.ty_of(ty))
    }

    fn binary(&mut self, binary: &syn::ExprBinary) -> Ty {
        let operands = if matches!(binary.op, BinOp::And(_) | BinOp::Or(_)) {
            How::Move
        } else {
            How::Look
        };
        let left = self.expr(&binary.left, operands);
        let right = self.expr(&binary.right, operands);
        match binary.op {
            BinOp::And(_) | BinOp::Or(_) => Ty::Bool,
            BinOp::Eq(_)
            | BinOp::Ne(_)
            | BinOp::Lt(_)
            | BinOp::Le(_)
            | BinOp::Gt(_)
            | BinOp::Ge(_) => {
                self.operands(binary, &left, &right);
                Ty::Bool
            }
            _ => {
                self.operands(binary, &left, &right);
                left.joined(&right)
            }
        }
    }

    fn operands(&mut self, binary: &syn::ExprBinary, left: &Ty, right: &Ty) {
        let numbers = matches!((left.peeled(), right.peeled()), (Ty::Int(_), Ty::Int(_)));
        if !numbers {
            self.expect(binary.right.span(), right, left, "this operand");
        }
    }

    fn arguments(&mut self, arguments: &[&Expr], wanted: &[Ty]) -> Vec<Ty> {
        let mut found = Vec::with_capacity(arguments.len());
        for (index, argument) in arguments.iter().enumerate() {
            if matches!(unwrapped(argument), Expr::Closure(_)) {
                found.push(Ty::Unknown);
                continue;
            }
            let have = self.expr(argument, How::Move);
            if let Some(want) = wanted.get(index) {
                self.expect(argument.span(), &have, want, "this argument");
            }
            found.push(have);
        }
        found
    }

    fn call(&mut self, call: &syn::ExprCall) -> Ty {
        let Expr::Path(function) = unwrapped(&call.func) else {
            return Ty::Unknown;
        };
        let names: Vec<String> = function
            .path
            .segments
            .iter()
            .map(|segment| name_of(&segment.ident))
            .collect();
        let arguments: Vec<&Expr> = call.args.iter().collect();
        match (names.as_slice(), arguments.as_slice()) {
            ([some], [inner]) if some == "Some" => Ty::option_of(self.expr(inner, How::Move)),
            ([owner, from], [value]) if from == "from" && IntType::named(owner).is_some() => {
                self.expr(value, How::Move);
                Ty::Int(IntType::named(owner))
            }
            ([owner, of], _) if of == "of" && ["Set", "Seq", "Map"].contains(&owner.as_str()) => {
                self.collection(owner, &arguments)
            }
            ([word], _) if word == "forall" || word == "exists" => {
                self.quantifier(function, &arguments);
                Ty::Bool
            }
            ([name], _) if self.reader.functions.contains(name) => {
                let wanted: Vec<Ty> = self
                    .reader
                    .fn_params
                    .get(name)
                    .map(|params| params.iter().map(|ty| self.reader.ty_of(ty)).collect())
                    .unwrap_or_default();
                self.arguments(&arguments, &wanted);
                self.reader
                    .fn_types
                    .get(name)
                    .map_or(Ty::Unknown, |ty| self.reader.ty_of(&ty.clone()))
            }
            _ => {
                self.arguments(&arguments, &[]);
                Ty::Unknown
            }
        }
    }

    fn collection(&mut self, owner: &str, arguments: &[&Expr]) -> Ty {
        let Some(Expr::Array(items)) = arguments.first().map(|first| unwrapped(first)) else {
            return Ty::Unknown;
        };
        let mut item = Ty::Unknown;
        let mut value = Ty::Unknown;
        for element in &items.elems {
            if owner == "Map" {
                if let Expr::Tuple(pair) = unwrapped(element) {
                    let mut sides = pair.elems.iter();
                    if let (Some(key), Some(entry)) = (sides.next(), sides.next()) {
                        item = item.joined(&self.expr(key, How::Move));
                        value = value.joined(&self.expr(entry, How::Move));
                    }
                }
            } else {
                let found = self.expr(element, How::Move);
                self.expect(element.span(), &found, &item, "this item");
                item = item.joined(&found);
            }
        }
        match owner {
            "Set" => Ty::Set(Box::new(item)),
            "Seq" => Ty::Seq(Box::new(item)),
            _ => Ty::Map(Box::new(item), Box::new(value)),
        }
    }

    fn quantifier(&mut self, function: &syn::ExprPath, arguments: &[&Expr]) {
        let Some(Expr::Closure(closure)) = arguments.first().map(|first| unwrapped(first)) else {
            return;
        };
        let named = function
            .path
            .segments
            .last()
            .and_then(|segment| match &segment.arguments {
                syn::PathArguments::AngleBracketed(listed) => listed.args.first(),
                _ => None,
            })
            .and_then(|argument| match argument {
                syn::GenericArgument::Type(ty) => Some(self.reader.ty_of(ty)),
                _ => None,
            })
            .unwrap_or(Ty::Unknown);
        self.closure(closure, &named, Calls::Often);
    }

    fn closure(&mut self, closure: &ExprClosure, takes: &Ty, calls: Calls) -> Ty {
        let kept = self.vars.len();
        let before = self.moved.clone();
        for input in &closure.inputs {
            let (plain, annotated) = match input {
                Pat::Type(typed) => (typed.pat.as_ref(), Some(typed.ty.as_ref())),
                other => (other, None),
            };
            let mut ty = takes.clone();
            if let Some(written) = annotated {
                let given = self.reader.ty_of(written);
                if takes.is_known() {
                    if !given.compatible(takes) || given.is_ref() != takes.is_ref() {
                        self.refuse(
                            input.span(),
                            &format!(
                                "this function takes {} where {} is passed to it, and `rustc` \
                                 refuses the file (E0631): write the type that is passed, or \
                                 leave the annotation out",
                                given.shown(),
                                takes.shown()
                            ),
                        );
                    }
                } else if !given.is_known() {
                    self.refuse(
                        input.span(),
                        "the checker can't tell the type of the value this function is passed, \
                         so it can't check this annotation, and it never ignores one: leave the \
                         annotation out",
                    );
                }
                if given.is_known() {
                    ty = given;
                }
            }
            if let Pat::Ident(named) = plain {
                self.bind(&name_of(&named.ident), ty);
            }
        }
        let body = self.expr(&closure.body, How::Move);
        self.vars.truncate(kept);
        if calls == Calls::Often {
            let outside: Vec<String> = self.moved.difference(&before).cloned().collect();
            if !outside.is_empty() {
                self.refuse(
                    closure.body.span(),
                    &format!(
                        "this function moves a value it captured, and it is called once for each \
                         item, so `rustc` refuses it (E0507): {FIX}"
                    ),
                );
                for gone in outside {
                    self.moved.remove(&gone);
                }
            }
        }
        body
    }

    fn method(&mut self, call: &syn::ExprMethodCall) -> Ty {
        let name = name_of(&call.method);
        let receiver_how = if BY_VALUE.contains(&name.as_str()) {
            How::Move
        } else {
            How::Look
        };
        let on = self.expr(&call.receiver, receiver_how);
        let arguments: Vec<&Expr> = call.args.iter().collect();
        let wanted = self.wanted_by(&name, &on, &arguments);
        let found = self.arguments(&arguments, &wanted);
        let closure = arguments
            .iter()
            .find_map(|argument| match unwrapped(argument) {
                Expr::Closure(closure) => Some(closure),
                _ => None,
            });
        let result = closure.map_or(Ty::Unknown, |closure| {
            let takes = match name.as_str() {
                "map_or" | "is_some_and" => on.inside(),
                _ => on.item(),
            };
            self.closure(
                closure,
                &takes,
                if BY_VALUE.contains(&name.as_str()) {
                    Calls::Once
                } else {
                    Calls::Often
                },
            )
        });
        self.method_result(&name, &on, &found, &result)
    }

    fn wanted_by(&self, name: &str, on: &Ty, arguments: &[&Expr]) -> Vec<Ty> {
        let _ = arguments;
        match (on.peeled(), name) {
            (
                Ty::Set(item) | Ty::Seq(item) | Ty::Queue(item),
                "has" | "add" | "delete" | "push" | "includes" | "send",
            ) => {
                vec![(**item).clone()]
            }
            (Ty::Set(_), "union" | "intersect" | "subtract" | "is_subset") => {
                vec![on.peeled().clone()]
            }
            (Ty::Map(key, value), "has" | "get" | "delete") => {
                let _ = value;
                vec![(**key).clone()]
            }
            (Ty::Map(key, value), "set") => vec![(**key).clone(), (**value).clone()],
            (Ty::Lock(Some(owner)), "acquire" | "is_held_by") => vec![(**owner).clone()],
            (Ty::Option(inside), "unwrap_or") => vec![(**inside).clone()],
            (Ty::Record(model), _) if Some(model) == self.reader.model_name.as_ref() => self
                .reader
                .method_params
                .get(name)
                .map(|params| params.iter().map(|ty| self.reader.ty_of(ty)).collect())
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    fn method_result(&self, name: &str, on: &Ty, found: &[Ty], closure: &Ty) -> Ty {
        let usize_type = Ty::Int(Some(IntType::Usize));
        let item = on.item();
        match (on.peeled(), name) {
            (
                _,
                "is_none" | "is_some" | "is_some_and" | "is_empty" | "has" | "is_subset"
                | "includes" | "every" | "some" | "is_free" | "is_held" | "is_held_by" | "is_idle"
                | "is_running" | "has_fired" | "is_pending" | "has_succeeded" | "has_failed"
                | "is_settled" | "can_retry" | "is_exhausted" | "is_full" | "is_multiple_of",
            ) => Ty::Bool,
            (Ty::Option(inside), "unwrap_or") => (**inside)
                .clone()
                .joined(&found.first().cloned().unwrap_or(Ty::Unknown)),
            (Ty::Option(_), "map_or") => found
                .first()
                .cloned()
                .unwrap_or(Ty::Unknown)
                .joined(closure),
            (_, "size") => usize_type,
            (Ty::Seq(_) | Ty::Queue(_), "get" | "first" | "last" | "head") => Ty::option_of(item),
            (Ty::Map(_, value), "get") => Ty::option_of((**value).clone()),
            (Ty::Map(key, _), "keys") => Ty::Set(key.clone()),
            (Ty::Lock(Some(owner)), "holder") => Ty::option_of((**owner).clone()),
            (_, "clone")
            | (
                Ty::Set(_)
                | Ty::Seq(_)
                | Ty::Queue(_)
                | Ty::Map(_, _)
                | Ty::Timer
                | Ty::Request
                | Ty::Retry
                | Ty::Lock(_),
                "add" | "delete" | "union" | "intersect" | "subtract" | "filter" | "push" | "shift"
                | "pop" | "set" | "send" | "receive" | "clear" | "acquire" | "release" | "start"
                | "fire" | "cancel" | "reset" | "succeed" | "fail" | "again",
            ) => on.peeled().clone(),
            (Ty::Record(model), _) if Some(model) == self.reader.model_name.as_ref() => self
                .reader
                .method_types
                .get(name)
                .map_or(Ty::Unknown, |ty| self.reader.ty_of(ty)),
            _ => Ty::Unknown,
        }
    }

    fn record(&mut self, literal: &ExprStruct) -> Ty {
        let written = literal
            .path
            .segments
            .iter()
            .map(|segment| name_of(&segment.ident))
            .collect::<Vec<_>>()
            .join("::");
        let record = if written == "Self" {
            self.reader.model_name.clone()
        } else {
            Some(written)
        };
        let fields: Vec<(String, Type)> = record
            .as_ref()
            .and_then(|name| self.reader.structs.get(name).cloned())
            .unwrap_or_default();
        let mut listed = BTreeSet::new();
        for field in &literal.fields {
            let found = self.expr(&field.expr, How::Move);
            let Member::Named(name) = &field.member else {
                continue;
            };
            listed.insert(name_of(name));
            if let Some((_, declared)) = fields.iter().find(|(known, _)| *known == name_of(name)) {
                let wanted = self.reader.ty_of(declared);
                self.expect(
                    field.expr.span(),
                    &found,
                    &wanted,
                    "the value of this field",
                );
            }
        }
        if let Some(rest) = &literal.rest {
            self.spread(rest, &fields, &listed);
        }
        record.map_or(Ty::Unknown, Ty::Record)
    }

    fn spread(&mut self, rest: &Expr, fields: &[(String, Type)], listed: &BTreeSet<String>) {
        let Some(base) = self.place(rest) else {
            self.expr(rest, How::Look);
            return;
        };
        let untouched: Vec<&(String, Type)> = fields
            .iter()
            .filter(|(name, _)| !listed.contains(name))
            .collect();
        for (name, declared) in untouched {
            let ty = self.reader.ty_of(declared);
            let part = Place {
                path: format!("{}.{name}", base.path),
                ty,
                behind_a_reference: base.behind_a_reference || base.ty.is_ref(),
            };
            self.touch(rest, &part, How::Move);
        }
    }

    fn branching(&mut self, branch: &ExprIf) -> Ty {
        let kept = self.vars.len();
        let (present, moves, matched) = if let Expr::Let(binding) = unwrapped(&branch.cond) {
            let (names, moves) = self.scrutinee(binding);
            (names, moves, Some(binding))
        } else {
            self.expr(&branch.cond, How::Move);
            (Vec::new(), false, None)
        };
        let start = self.moved.clone();
        if let (true, Some(binding)) = (moves, matched) {
            self.expr(&binding.expr, How::Move);
        }
        for (name, ty) in present {
            self.bind(&name, ty);
        }
        let yes = self.block(&branch.then_branch);
        self.vars.truncate(kept);
        let after_yes = std::mem::replace(&mut self.moved, start.clone());
        let no = match &branch.else_branch {
            Some((_, otherwise)) => self.expr(otherwise, How::Move),
            None => Ty::Unknown,
        };
        let after_no = std::mem::take(&mut self.moved);
        self.moved = start;
        if yes != Ty::Never {
            self.moved.extend(after_yes);
        }
        if no != Ty::Never || is_conditional_early_return(branch) {
            self.moved.extend(after_no);
        }
        if branch.else_branch.is_some() && yes.is_known() && no.is_known() {
            self.expect(branch.if_token.span, &no, &yes, "this branch");
        }
        yes.joined(&no)
    }

    fn scrutinee(&mut self, binding: &syn::ExprLet) -> (Vec<(String, Ty)>, bool) {
        let found = self.expr(&binding.expr, How::Look);
        let inside = found.inside();
        let names = self.pattern_names(&binding.pat, &inside);
        let moves = names.iter().any(|(_, ty)| !self.reader.is_copy_ty(ty));
        (names, moves)
    }

    fn pattern_names(&self, pattern: &Pat, inside: &Ty) -> Vec<(String, Ty)> {
        match pattern {
            Pat::TupleStruct(found) => found
                .elems
                .iter()
                .flat_map(|element| self.pattern_names(element, inside))
                .collect(),
            Pat::Ident(named) if !self.reader.is_constant_name(&name_of(&named.ident)) => {
                vec![(name_of(&named.ident), inside.clone())]
            }
            Pat::Paren(inner) => self.pattern_names(&inner.pat, inside),
            _ => Vec::new(),
        }
    }

    fn matching_arms(&mut self, found: &syn::ExprMatch) -> Ty {
        self.expr(&found.expr, How::Look);
        let start = self.moved.clone();
        let mut joined = start.clone();
        let mut result = Ty::Never;
        for arm in &found.arms {
            self.moved.clone_from(&start);
            let body = self.expr(&arm.body, How::Move);
            if body != Ty::Never {
                joined.extend(self.moved.iter().cloned());
            }
            result = result.joined(&body);
        }
        self.moved = joined;
        result
    }

    fn matches_macro(&mut self, called: &syn::Macro) -> Ty {
        let parsed = called.parse_body_with(|input: syn::parse::ParseStream<'_>| {
            let value: Expr = input.parse()?;
            input.parse::<syn::Token![,]>()?;
            let rest: proc_macro2::TokenStream = input.parse()?;
            Ok((value, rest))
        });
        if let Ok((value, _)) = parsed {
            self.expr(&value, How::Look);
        }
        Ty::Bool
    }

    fn block(&mut self, block: &Block) -> Ty {
        let kept = self.vars.len();
        let mut last = Ty::Unknown;
        for statement in &block.stmts {
            last = self.statement(statement);
            if last == Ty::Never {
                break;
            }
        }
        self.vars.truncate(kept);
        last
    }

    fn statement(&mut self, statement: &Stmt) -> Ty {
        match statement {
            Stmt::Local(local) => {
                self.local(local);
                Ty::Unknown
            }
            Stmt::Expr(expr, semicolon) => {
                let found = self.expr(expr, How::Move);
                if semicolon.is_some() && found != Ty::Never {
                    Ty::Unknown
                } else {
                    found
                }
            }
            Stmt::Macro(called) => {
                if called.mac.path.is_ident("require")
                    && let Ok(condition) = called.mac.parse_body::<Expr>()
                {
                    self.expr(&condition, How::Move);
                }
                Ty::Unknown
            }
            Stmt::Item(_) => Ty::Unknown,
        }
    }

    fn local(&mut self, local: &syn::Local) {
        let (pattern, annotated) = match &local.pat {
            Pat::Type(typed) => (typed.pat.as_ref(), Some(self.reader.ty_of(&typed.ty))),
            other => (other, None),
        };
        let Some(init) = &local.init else {
            return;
        };
        let (names, found) = if let Pat::TupleStruct(_) = pattern {
            let binding = syn::ExprLet {
                attrs: Vec::new(),
                let_token: local.let_token,
                pat: Box::new(pattern.clone()),
                eq_token: init.eq_token,
                expr: init.expr.clone(),
            };
            let (names, moves) = self.scrutinee(&binding);
            if moves {
                self.expr(&binding.expr, How::Move);
            }
            (names, Ty::Unknown)
        } else {
            let found = self.expr(&init.expr, How::Move);
            if let Some(want) = &annotated {
                self.expect(init.expr.span(), &found, want, "this value");
            }
            let ty = annotated
                .clone()
                .filter(Ty::is_known)
                .unwrap_or(found.clone());
            let names = match pattern {
                Pat::Ident(named) if !self.reader.is_constant_name(&name_of(&named.ident)) => {
                    vec![(name_of(&named.ident), ty)]
                }
                _ => Vec::new(),
            };
            (names, found)
        };
        let _ = found;
        if let Some((_, otherwise)) = &init.diverge {
            let kept = self.moved.clone();
            self.expr(otherwise, How::Move);
            self.moved = kept;
        }
        for (name, ty) in names {
            self.bind(&name, ty);
        }
    }
}

impl Reader<'_> {
    pub(crate) fn is_constant_name(&self, name: &str) -> bool {
        self.const_types.contains_key(name)
    }

    pub(crate) fn returns_of(&self, signature: &Signature) -> Ty {
        match &signature.output {
            ReturnType::Type(_, ty) => self.ty_of(ty),
            ReturnType::Default => Ty::Unknown,
        }
    }
}
