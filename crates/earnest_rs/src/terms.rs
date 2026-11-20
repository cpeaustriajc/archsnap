use crate::reader::{
    OUTSIDE, Reader, Sign, at, name_of, path_text, start_of, unwrapped, why_outside,
};
use crate::typing::{inside_option, plain};
use earnest_ast::{Binary, HelperKind, Reach, Shape, ShapeKind, Term, TermKind, Unary};
use earnest_value::IntType;
use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::{
    Attribute, BinOp, Expr, ExprBinary, ExprCall, ExprClosure, ExprField, ExprIf, ExprLit,
    ExprMethodCall, ExprPath, ExprStruct, ExprUnary, GenericArgument, Lit, Macro, Member, Pat,
    PathArguments, Type, UnOp,
};

const PRIMITIVES: [&str; 50] = [
    "has",
    "add",
    "delete",
    "union",
    "intersect",
    "subtract",
    "is_subset",
    "size",
    "is_empty",
    "every",
    "some",
    "filter",
    "get",
    "set",
    "keys",
    "first",
    "last",
    "push",
    "shift",
    "pop",
    "includes",
    "is_full",
    "head",
    "send",
    "receive",
    "clear",
    "is_free",
    "is_held",
    "acquire",
    "release",
    "is_held_by",
    "holder",
    "is_idle",
    "is_running",
    "has_fired",
    "start",
    "fire",
    "cancel",
    "reset",
    "is_pending",
    "has_succeeded",
    "has_failed",
    "is_settled",
    "succeed",
    "fail",
    "attempts",
    "can_retry",
    "is_exhausted",
    "again",
    "is_some",
];

const AS_IS: &str = "$value";

fn unknown_method(name: &str) -> &'static str {
    match name {
        "unwrap" | "expect" => {
            "`unwrap` and `expect` panic on `None`, and earnest has no panics: a walk of the \
             model in a test would stop there instead of reporting it. Read the value with \
             `if let Some(x) = value { .. } else { .. }`, `value.unwrap_or(default)` or \
             `value.map_or(default, |x| ..)`, and in an action write `let x = value?;` where \
             `None` means the action can't happen"
        }
        "unwrap_or_default" | "unwrap_or_else" | "or" | "or_else" => {
            "write `value.unwrap_or(default)` with the default written out: the checker reads \
             no function that gives a value, only a test"
        }
        "map" | "and_then" | "filter" | "ok_or" | "map_or_else" | "take" | "replace" | "zip"
        | "flatten" => {
            "write `value.map_or(default, |x| ..)` or `if let Some(x) = value { .. } else { .. }`"
        }
        "as_ref" | "as_mut" | "as_deref" | "copied" | "cloned" | "iter" | "borrow" => {
            "the checker reads `&x` and `x.clone()` as `x` and has no references of its own: \
             leave this call out and use the value as it is"
        }
        _ => {
            "it is not one of the library's primitives, so the checker can't know what it does: \
             \"The primitives\" in the README lists what a value can do, and for the value \
             inside an `Option` write `if let Some(x) = value { .. } else { .. }` or \
             `value.map_or(default, |x| ..)`"
        }
    }
}

const OPTION_TESTS: [&str; 2] = ["is_none", "is_some_and"];
const LIBRARY_CALLS: [&str; 3] = ["forall", "exists", "Some"];

const LOOP: &str = "a loop is outside the library: use `every`, `some`, `forall` or `exists`";
const CHANGE: &str = "nothing changes in place: every operation gives back a new value, and an \
    action a new state: write `let next = value + 1;`, or put the new value in the action's \
    `Self { field: value, ..self.clone() }`";
const WAIT: &str = "waiting is outside the library: model it with a `Request` or a `Timer`";
const PRINTING: [&str; 7] = [
    "println", "print", "eprintln", "eprint", "dbg", "write", "writeln",
];

fn attributes_of(expr: &Expr) -> &[Attribute] {
    match expr {
        Expr::Binary(found) => &found.attrs,
        Expr::Block(found) => &found.attrs,
        Expr::Call(found) => &found.attrs,
        Expr::Closure(found) => &found.attrs,
        Expr::Field(found) => &found.attrs,
        Expr::If(found) => &found.attrs,
        Expr::Lit(found) => &found.attrs,
        Expr::Macro(found) => &found.attrs,
        Expr::MethodCall(found) => &found.attrs,
        Expr::Paren(found) => &found.attrs,
        Expr::Path(found) => &found.attrs,
        Expr::Reference(found) => &found.attrs,
        Expr::Return(found) => &found.attrs,
        Expr::Struct(found) => &found.attrs,
        Expr::Unary(found) => &found.attrs,
        _ => &[],
    }
}

fn is_state(expr: &Expr) -> bool {
    matches!(unwrapped(expr), Expr::Path(name) if name.path.is_ident("self"))
}

fn operator(op: &BinOp) -> Option<Binary> {
    Some(match op {
        BinOp::And(_) => Binary::And,
        BinOp::Or(_) => Binary::Or,
        BinOp::Eq(_) => Binary::Equal,
        BinOp::Ne(_) => Binary::Differ,
        BinOp::Lt(_) => Binary::Less,
        BinOp::Le(_) => Binary::LessOrEqual,
        BinOp::Gt(_) => Binary::Greater,
        BinOp::Ge(_) => Binary::GreaterOrEqual,
        BinOp::Add(_) => Binary::Add,
        BinOp::Sub(_) => Binary::Subtract,
        BinOp::Mul(_) => Binary::Multiply,
        BinOp::Rem(_) => Binary::Remainder,
        BinOp::Div(_) => Binary::Divide,
        _ => return None,
    })
}

fn changes_in_place(op: &BinOp) -> bool {
    matches!(
        op,
        BinOp::AddAssign(_)
            | BinOp::SubAssign(_)
            | BinOp::MulAssign(_)
            | BinOp::DivAssign(_)
            | BinOp::RemAssign(_)
            | BinOp::BitXorAssign(_)
            | BinOp::BitAndAssign(_)
            | BinOp::BitOrAssign(_)
            | BinOp::ShlAssign(_)
            | BinOp::ShrAssign(_)
    )
}

impl Reader<'_> {
    pub(crate) fn term(&mut self, expr: &Expr) -> Term {
        if let Some(refused) = self.too_deep(|| start_of(expr)) {
            return refused;
        }
        self.depth = self.depth.saturating_add(1);
        let read = self.term_inside(expr);
        self.depth = self.depth.saturating_sub(1);
        read
    }

    fn term_inside(&mut self, expr: &Expr) -> Term {
        self.refuse_conditional(attributes_of(expr));
        match expr {
            Expr::Paren(inner) => self.term(&inner.expr),
            Expr::Group(inner) => self.term(&inner.expr),
            Expr::Lit(literal) => self.literal(literal, Sign::Plus, literal.span()),
            Expr::Path(name) => self.path(name),
            Expr::Field(read) => self.field_read(read),
            Expr::MethodCall(call) => self.method_call(call),
            Expr::Call(call) => self.call(call),
            Expr::Unary(unary) => self.unary(unary),
            Expr::Binary(binary) => self.binary(binary),
            Expr::Reference(reference) if reference.mutability.is_none() => {
                self.term(&reference.expr)
            }
            Expr::If(branch) => self.choice(branch),
            Expr::Match(found) => self.matching(found),
            Expr::Block(inner) if inner.label.is_none() => self.value_of(&inner.block),
            Expr::Struct(literal) => self.record(literal),
            Expr::Closure(closure) => self.lambda(closure),
            other => self.outside(other),
        }
    }

    fn literal(&mut self, literal: &ExprLit, sign: Sign, span: Span) -> Term {
        match &literal.lit {
            Lit::Bool(flag) => at(span, TermKind::Bool(flag.value)),
            Lit::Str(text) => at(span, TermKind::Text(text.value())),
            Lit::Int(number) => {
                let Some(value) = Self::literal_number(literal, sign) else {
                    let feature = self.short_text(span);
                    return self.nothing(span, &feature, "the number is too large for the checker, which holds a number in 64 bits: use a smaller number, or a constant computed from smaller ones");
                };
                let plain = at(span, TermKind::Int(value));
                if number.suffix().is_empty() {
                    return plain;
                }
                let Some(ty) = IntType::named(number.suffix()) else {
                    let feature = self.short_text(span);
                    return self.nothing(span, &feature, "a model holds whole numbers only: write the number with an integer suffix such as `1_u8`, or count in whole units (cents, not 1.5 dollars)");
                };
                let shape = Shape {
                    place: plain.place,
                    kind: ShapeKind::Int {
                        lo: ty.lowest(),
                        hi: ty.highest(),
                        ty: Some(ty),
                    },
                    initial: None,
                };
                at(
                    span,
                    TermKind::Declared {
                        shape: Box::new(shape),
                        value: Box::new(plain),
                    },
                )
            }
            _ => {
                let feature = self.short_text(span);
                self.nothing(
                    span,
                    &feature,
                    "a model holds booleans, whole numbers, texts and enum variants: write a `&'static str` for a text, an enum for a choice, and a whole number for anything counted",
                )
            }
        }
    }

    fn path(&mut self, name: &ExprPath) -> Term {
        let span = name.span();
        let text = path_text(&name.path);
        let parts: Vec<&str> = text.split("::").collect();
        match parts.as_slice() {
            ["None"] => at(span, TermKind::Null),
            [single] if name.qself.is_none() && *single != "Self" => {
                at(span, TermKind::Name((*single).to_owned()))
            }
            [owner, variant] if self.enums.contains_key(*owner) => {
                let variants = self.enums.get(*owner).cloned().unwrap_or_default();
                if variants.iter().any(|known| known == variant) {
                    at(
                        span,
                        TermKind::Variant {
                            owner: (*owner).to_owned(),
                            name: (*variant).to_owned(),
                        },
                    )
                } else {
                    let message = format!(
                        "`{owner}` has no variant with this name: its variants are {}",
                        variants.join(", ")
                    );
                    self.nothing(span, &text, &message)
                }
            }
            _ => self.nothing(span, &text, why_outside(&text)),
        }
    }

    fn field_read(&mut self, read: &ExprField) -> Term {
        let target = self.term(&read.base);
        match &read.member {
            Member::Named(field) => at(
                start_of(&read.base),
                TermKind::Member(Box::new(target), name_of(field)),
            ),
            Member::Unnamed(index) => {
                let feature = format!(".{}", index.index);
                self.nothing(
                    index.span,
                    &feature,
                    "a tuple is outside the library: use a struct that derives `Domain`",
                )
            }
        }
    }

    fn method_call(&mut self, call: &ExprMethodCall) -> Term {
        let span = start_of(&call.receiver);
        let name = name_of(&call.method);
        if name == "clone" && call.args.is_empty() {
            return self.term(&call.receiver);
        }
        if is_state(&call.receiver) {
            return self.own_method(call, &name);
        }
        if let Some(read) = self.option_method(call, &name) {
            return read;
        }
        let target = self.term(&call.receiver);
        let arguments: Vec<Term> = call
            .args
            .iter()
            .map(|argument| self.term(argument))
            .collect();
        let is_multiple_of = name == "is_multiple_of" && arguments.len() == 1;
        if call.turbofish.is_some()
            || !(is_multiple_of
                || PRIMITIVES.contains(&name.as_str())
                || OPTION_TESTS.contains(&name.as_str()))
        {
            let feature = format!(".{name}()");
            return self.nothing(call.method.span(), &feature, unknown_method(&name));
        }
        at(
            span,
            TermKind::Call {
                target: Box::new(target),
                method: name,
                arguments,
            },
        )
    }

    fn option_method(&mut self, call: &ExprMethodCall, name: &str) -> Option<Term> {
        let span = start_of(&call.receiver);
        match (name, call.args.len()) {
            ("unwrap_or", 1) => {
                self.refuse_borrowed_option(&call.receiver);
                let value = self.term(&call.receiver);
                let default = self.term(&call.args[0]);
                Some(at(
                    span,
                    TermKind::Unpack {
                        value: Box::new(value),
                        name: AS_IS.to_owned(),
                        present: Box::new(at(span, TermKind::Name(AS_IS.to_owned()))),
                        absent: Box::new(default),
                        absent_first: true,
                    },
                ))
            }
            ("map_or", 2) => {
                self.refuse_borrowed_option(&call.receiver);
                let value = self.term(&call.receiver);
                let default = self.term(&call.args[0]);
                let Expr::Closure(closure) = unwrapped(&call.args[1]) else {
                    return Some(self.nothing(
                        call.args[1].span(),
                        &self.short_text(call.args[1].span()),
                        "pass `map_or` a function of one value written in place, as \
                         `value.map_or(0, |x| x + 1)`",
                    ));
                };
                let inputs: Vec<&Pat> = closure.inputs.iter().collect();
                let [input] = inputs.as_slice() else {
                    return Some(self.nothing(
                        closure.or1_token.span,
                        "map_or",
                        "the function of `map_or` takes the one value inside: `|x| ..`",
                    ));
                };
                let (parameter, annotated) = self.parameter(input);
                let inside = annotated.cloned().or_else(|| {
                    let found = self.static_type(&call.receiver)?;
                    inside_option(&found).cloned()
                });
                self.bound.push(plain(parameter.clone(), inside));
                let body = self.term(&closure.body);
                self.bound.pop();
                Some(at(
                    span,
                    TermKind::Unpack {
                        value: Box::new(value),
                        name: parameter,
                        present: Box::new(body),
                        absent: Box::new(default),
                        absent_first: true,
                    },
                ))
            }
            _ => None,
        }
    }

    fn own_method(&mut self, call: &ExprMethodCall, name: &str) -> Term {
        let mut arguments = vec![self.term(&call.receiver)];
        arguments.extend(call.args.iter().map(|argument| self.term(argument)));
        if !self.methods.contains(name) {
            let feature = format!("self.{name}()");
            return self.nothing(
                call.method.span(),
                &feature,
                "the model's block has no helper method with this name; an action, a rule or \
                 an end can't be called from inside the model",
            );
        }
        at(
            call.receiver.span(),
            TermKind::Helper {
                kind: HelperKind::Method,
                name: name.to_owned(),
                arguments,
            },
        )
    }

    fn call(&mut self, call: &ExprCall) -> Term {
        let span = call.func.span();
        let Expr::Path(function) = unwrapped(&call.func) else {
            let feature = self.short_text(call.func.span());
            return self.nothing(call.func.span(), &feature, OUTSIDE);
        };
        let text = path_text(&function.path);
        let parts: Vec<&str> = text.split("::").collect();
        let arguments: Vec<&Expr> = call.args.iter().collect();
        if let [name] = parts.as_slice()
            && self.bound.iter().any(|local| local.name == *name)
            && (self.functions.contains(*name) || LIBRARY_CALLS.contains(name))
        {
            return self.nothing(
                function.span(),
                &text,
                "a value of this body has this name, so a call of it is not a call of the \
                 library's or the file's function of the same name: give the value another name",
            );
        }
        match (parts.as_slice(), arguments.as_slice()) {
            (["Some"], [inner]) => {
                let wrapped = self.term(inner);
                at(span, TermKind::Wrap(Box::new(wrapped)))
            }
            ([int, "from"], [value])
                if function.qself.is_none() && IntType::named(int).is_some() =>
            {
                let ty = IntType::named(int).unwrap_or(IntType::I32);
                let converted = self.term(value);
                at(
                    span,
                    TermKind::Convert {
                        ty,
                        value: Box::new(converted),
                    },
                )
            }
            (["Set", "of"], _) => {
                let items = self.listed(call, &text);
                at(span, TermKind::SetOf(items))
            }
            (["Seq", "of"], _) => {
                let items = self.listed(call, &text);
                at(span, TermKind::SeqOf(items))
            }
            (["Map", "of"], _) => {
                let entries = self.pairs(call);
                at(span, TermKind::MapOf(entries))
            }
            (["forall"], _) => self.across(call, function, Reach::Every),
            (["exists"], _) => self.across(call, function, Reach::Some),
            ([name], _) if self.functions.contains(*name) => {
                let passed = arguments.iter().map(|argument| self.term(argument));
                at(
                    span,
                    TermKind::Helper {
                        kind: HelperKind::Function,
                        name: (*name).to_owned(),
                        arguments: passed.collect(),
                    },
                )
            }
            _ => self.nothing(function.span(), &text, why_outside(&text)),
        }
    }

    fn listed(&mut self, call: &ExprCall, constructor: &str) -> Vec<Term> {
        let arguments: Vec<&Expr> = call.args.iter().collect();
        if let [Expr::Array(items)] = arguments.as_slice() {
            return items.elems.iter().map(|item| self.term(item)).collect();
        }
        self.refuse(
            call.span(),
            constructor,
            "list the items in square brackets, as in `Set::of([a, b])`",
        );
        Vec::new()
    }

    fn pairs(&mut self, call: &ExprCall) -> Vec<(Term, Term)> {
        const WANTED: &str = "list the entries as pairs in square brackets, as in \
                              `Map::of([(key, value)])`";
        let arguments: Vec<&Expr> = call.args.iter().collect();
        let [Expr::Array(entries)] = arguments.as_slice() else {
            self.refuse(call.span(), "Map::of", WANTED);
            return Vec::new();
        };
        let mut read = Vec::with_capacity(entries.elems.len());
        for entry in &entries.elems {
            let sides: Vec<&Expr> = match entry {
                Expr::Tuple(pair) => pair.elems.iter().collect(),
                _ => Vec::new(),
            };
            if let [key, value] = sides.as_slice() {
                read.push((self.term(key), self.term(value)));
            } else {
                self.refuse(entry.span(), "Map::of", WANTED);
            }
        }
        read
    }

    fn parameter<'b>(&mut self, pattern: &'b Pat) -> (String, Option<&'b Type>) {
        let (plain, ty) = match pattern {
            Pat::Type(typed) => (typed.pat.as_ref(), Some(typed.ty.as_ref())),
            other => (other, None),
        };
        match plain {
            Pat::Ident(named) if named.subpat.is_none() && named.by_ref.is_none() => {
                self.refuse_constant_name(&named.ident);
                (name_of(&named.ident), ty)
            }
            Pat::Wild(_) => ("_".to_owned(), ty),
            other => {
                let feature = self.short_text(other.span());
                self.refuse(
                    other.span(),
                    &feature,
                    "a parameter is one plain name: take the value whole and read its fields with `.field`",
                );
                ("_".to_owned(), ty)
            }
        }
    }

    fn lambda(&mut self, closure: &ExprClosure) -> Term {
        let typed: Vec<(String, Option<Type>)> = closure
            .inputs
            .iter()
            .map(|input| {
                let (name, ty) = self.parameter(input);
                (name, ty.cloned())
            })
            .collect();
        let parameters: Vec<String> = typed.iter().map(|(name, _)| name.clone()).collect();
        let spans: Vec<(String, Span)> = parameters
            .iter()
            .cloned()
            .zip(closure.inputs.iter().map(Spanned::span))
            .collect();
        self.refuse_repeats(&spans, "parameter", "E0415");
        let kept = self.bound.len();
        self.bound
            .extend(typed.into_iter().map(|(name, ty)| plain(name, ty)));
        let body = self.term(&closure.body);
        self.bound.truncate(kept);
        at(
            closure.or1_token.span,
            TermKind::Lambda {
                parameters,
                body: Box::new(body),
            },
        )
    }

    fn across(&mut self, call: &ExprCall, function: &ExprPath, reach: Reach) -> Term {
        let span = call.func.span();
        let word = path_text(&function.path);
        let arguments: Vec<&Expr> = call.args.iter().collect();
        let inputs: Vec<&Pat> = match arguments.as_slice() {
            [Expr::Closure(closure)] => closure.inputs.iter().collect(),
            _ => Vec::new(),
        };
        let ([Expr::Closure(closure)], [input]) = (arguments.as_slice(), inputs.as_slice()) else {
            return self.nothing(
                span,
                &word,
                "pass one function of one value that gives true or false",
            );
        };
        let (parameter, annotated) = self.parameter(input);
        let named = function
            .path
            .segments
            .last()
            .and_then(|segment| match &segment.arguments {
                PathArguments::AngleBracketed(listed) => listed.args.first(),
                _ => None,
            })
            .and_then(|argument| match argument {
                GenericArgument::Type(ty) => Some(ty),
                _ => None,
            });
        let Some(ty) = named.or(annotated) else {
            return self.nothing(
                span,
                &word,
                "name the type whose values are tried, as in `forall::<Guest>(|guest| ...)`",
            );
        };
        let shape = self.shape(ty, &[]);
        self.bound.push(plain(parameter.clone(), Some(ty.clone())));
        let body = self.term(&closure.body);
        self.bound.pop();
        at(
            span,
            TermKind::Across {
                reach,
                shape: Box::new(shape),
                parameter,
                body: Box::new(body),
            },
        )
    }

    fn unary(&mut self, unary: &ExprUnary) -> Term {
        let op = match unary.op {
            UnOp::Not(_) => Unary::Not,
            UnOp::Neg(_) => {
                if let Expr::Lit(literal) = unwrapped(&unary.expr)
                    && matches!(literal.lit, Lit::Int(_))
                {
                    return self.literal(literal, Sign::Minus, unary.op.span());
                }
                Unary::Negate
            }
            _ => {
                return self.nothing(
                    unary.span(),
                    "*",
                    "a `*` reads through a reference, and the checker has no references: it reads \
                     `&x` as `x`. Take the parameter by value (`last: Option<u8>`, which clippy \
                     asks for on a `Copy` type), or compare two references, `last == \
                     &Some(1)`, and write the value itself everywhere else",
                );
            }
        };
        let inner = self.term(&unary.expr);
        at(unary.op.span(), TermKind::Unary(op, Box::new(inner)))
    }

    fn binary(&mut self, binary: &ExprBinary) -> Term {
        let left = self.term(&binary.left);
        let right = self.term(&binary.right);
        if let Some(op) = operator(&binary.op) {
            return at(
                start_of(&binary.left),
                TermKind::Binary(op, Box::new(left), Box::new(right)),
            );
        }
        let feature = self.text(binary.op.span());
        let message = if changes_in_place(&binary.op) {
            CHANGE
        } else {
            "this operator is outside the library"
        };
        self.nothing(binary.op.span(), &feature, message)
    }

    fn choice(&mut self, branch: &ExprIf) -> Term {
        if let Expr::Let(binding) = unwrapped(&branch.cond) {
            return self.unpack(branch, binding);
        }
        let test = self.term(&branch.cond);
        let yes = self.value_of(&branch.then_branch);
        let no = match &branch.else_branch {
            Some((_, otherwise)) => self.term(otherwise),
            None => self.nothing(
                branch.if_token.span,
                "if",
                "an `if` used as a value needs an `else`: write `if condition { a } else { b }`",
            ),
        };
        at(
            branch.if_token.span,
            TermKind::Choose(Box::new(test), Box::new(yes), Box::new(no)),
        )
    }

    fn record(&mut self, literal: &ExprStruct) -> Term {
        let span = literal.path.span();
        let name = path_text(&literal.path);
        if name != "Self" && !self.models.contains(&name) && !self.records.contains_key(&name) {
            return self.nothing(
                literal.path.span(),
                &name,
                "it is not `Self`, the model, or a struct in this file that derives `Domain`: \
                 derive `Domain` on the struct, or build the state as `Self { .. }`",
            );
        }
        let mut fields = Vec::with_capacity(literal.fields.len());
        for field in &literal.fields {
            self.refuse_conditional(&field.attrs);
            let value = self.term(&field.expr);
            match &field.member {
                Member::Named(field_name) => fields.push((name_of(field_name), value)),
                Member::Unnamed(index) => {
                    let feature = index.index.to_string();
                    self.refuse(index.span, &feature, "a record's fields have names");
                }
            }
        }
        let base = literal.rest.as_ref().map(|rest| Box::new(self.term(rest)));
        let owner = self.records.contains_key(&name).then_some(name);
        at(
            span,
            TermKind::Record {
                owner,
                base,
                fields,
            },
        )
    }

    pub(crate) fn refuse_macro(&mut self, called: &Macro) -> Term {
        let name = called
            .path
            .segments
            .last()
            .map(|segment| name_of(&segment.ident))
            .unwrap_or_default();
        let message = if PRINTING.contains(&name.as_str()) {
            "printing is input and output, which is outside the library"
        } else if name == "format" {
            "text is built only in the `#[state]` and `#[group]` labels of a `#[view]` block"
        } else if name == "require" {
            "`require!(condition);` is a statement at the top of an action, not a value"
        } else {
            "the checker does not know this macro: only `require!` is part of the library"
        };
        self.nothing(called.path.span(), &format!("{name}!"), message)
    }

    fn outside(&mut self, expr: &Expr) -> Term {
        let (span, feature, message): (Span, &str, &str) = match expr {
            Expr::ForLoop(repeated) => {
                self.scan(&repeated.body);
                (repeated.for_token.span, "for", LOOP)
            }
            Expr::While(repeated) => {
                self.scan(&repeated.body);
                (repeated.while_token.span, "while", LOOP)
            }
            Expr::Loop(repeated) => {
                self.scan(&repeated.body);
                (repeated.loop_token.span, "loop", LOOP)
            }
            Expr::Break(stop) => (stop.break_token.span, "break", LOOP),
            Expr::Continue(skip) => (skip.continue_token.span, "continue", LOOP),
            Expr::Unsafe(block) => {
                self.scan(&block.block);
                (
                    block.unsafe_token.span,
                    "unsafe",
                    "unsafe code is outside the library",
                )
            }
            Expr::Assign(assignment) => (assignment.eq_token.span, "=", CHANGE),
            Expr::Reference(reference) => (reference.and_token.span, "&mut", CHANGE),
            Expr::Let(binding) => (
                binding.let_token.span,
                "let",
                "a `let` condition is read only as `if let Some(x) = value { .. } else { .. }`, \
                 with nothing joined to it by `&&`",
            ),
            Expr::Await(waited) => (waited.await_token.span, "await", WAIT),
            Expr::Async(block) => (block.async_token.span, "async", WAIT),
            Expr::Try(tried) => (
                tried.question_token.span,
                "?",
                "`?` is read only on an `Option` in an action that returns `Option<Self>`, as `let x = value?;`, which makes the action not happen where `value` is `None`: otherwise write `require!(condition);` at the top of the action",
            ),
            Expr::Cast(cast) => (
                cast.as_token.span,
                "as",
                "a cast is outside the library: widen a number with `u16::from(small)`, make a number of a boolean with `u8::from(flag)`, and keep a number in the wider type where it would narrow",
            ),
            Expr::Return(returned) => (
                returned.return_token.span,
                "return",
                "`return` is read only as the last statement of a body, or of an `if` block \
                 before the last expression",
            ),
            Expr::Index(indexed) => (
                indexed.bracket_token.span.open(),
                "[]",
                "read an item of a sequence with `get(index)`",
            ),
            Expr::Macro(called) if called.mac.path.is_ident("matches") => {
                return self.matches_macro(&called.mac);
            }
            Expr::Macro(called) => return self.refuse_macro(&called.mac),
            other => {
                let feature = self.short_text(other.span());
                return self.nothing(other.span(), &feature, OUTSIDE);
            }
        };
        self.nothing(span, feature, message)
    }
}
