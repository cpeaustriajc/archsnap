use super::terms::apply;
use super::types::{Binding, Scope, Type, Typed};
use super::{Lowerer, Lowering};
use crate::ir::{Expr, MapOp, Math, Op, Order, SeqOp, SetOp, Shelf, Sweep};
use crate::surface::{Place, Term, TermKind};
use crate::value::Value;

#[derive(Clone, Copy)]
enum Takes {
    Nothing,
    Item,
    Key,
    KeyAndValue,
    Same,
    Number,
}

#[derive(Clone, Copy)]
enum Gives {
    Bool,
    Int,
    Same,
    OptionalItem,
    OptionalValue,
    KeySet,
}

struct Shelves {
    same: Type,
    item: Type,
    key: Type,
    value: Type,
}

type Signature = (Op, Takes, Gives);

fn set_signature(name: &str) -> Option<Signature> {
    Some(match name {
        "has" => (Op::Set(SetOp::Has), Takes::Item, Gives::Bool),
        "add" => (Op::Set(SetOp::Add), Takes::Item, Gives::Same),
        "delete" => (Op::Set(SetOp::Delete), Takes::Item, Gives::Same),
        "union" => (Op::Set(SetOp::Union), Takes::Same, Gives::Same),
        "intersect" => (Op::Set(SetOp::Intersect), Takes::Same, Gives::Same),
        "subtract" => (Op::Set(SetOp::Subtract), Takes::Same, Gives::Same),
        "is_subset" => (Op::Set(SetOp::IsSubset), Takes::Same, Gives::Bool),
        "size" => (Op::Set(SetOp::Size), Takes::Nothing, Gives::Int),
        _ => return None,
    })
}

fn map_signature(name: &str) -> Option<Signature> {
    Some(match name {
        "has" => (Op::Map(MapOp::Has), Takes::Key, Gives::Bool),
        "get" => (Op::Map(MapOp::Get), Takes::Key, Gives::OptionalValue),
        "set" => (Op::Map(MapOp::Set), Takes::KeyAndValue, Gives::Same),
        "delete" => (Op::Map(MapOp::Delete), Takes::Key, Gives::Same),
        "size" => (Op::Map(MapOp::Size), Takes::Nothing, Gives::Int),
        "keys" => (Op::Map(MapOp::Keys), Takes::Nothing, Gives::KeySet),
        _ => return None,
    })
}

fn seq_signature(name: &str) -> Option<Signature> {
    Some(match name {
        "size" => (Op::Seq(SeqOp::Size), Takes::Nothing, Gives::Int),
        "get" => (Op::Seq(SeqOp::Get), Takes::Number, Gives::OptionalItem),
        "first" => (Op::Seq(SeqOp::First), Takes::Nothing, Gives::OptionalItem),
        "last" => (Op::Seq(SeqOp::Last), Takes::Nothing, Gives::OptionalItem),
        "push" => (Op::Seq(SeqOp::Push), Takes::Item, Gives::Same),
        "shift" => (Op::Seq(SeqOp::Shift), Takes::Nothing, Gives::Same),
        "pop" => (Op::Seq(SeqOp::Pop), Takes::Nothing, Gives::Same),
        "includes" => (Op::Seq(SeqOp::Includes), Takes::Item, Gives::Bool),
        _ => return None,
    })
}

fn queue_signature(name: &str) -> Option<Signature> {
    Some(match name {
        "size" => (Op::Seq(SeqOp::Size), Takes::Nothing, Gives::Int),
        "head" => (Op::Seq(SeqOp::First), Takes::Nothing, Gives::OptionalItem),
        "send" => (Op::Seq(SeqOp::Push), Takes::Item, Gives::Same),
        "receive" => (Op::Seq(SeqOp::Shift), Takes::Nothing, Gives::Same),
        "includes" => (Op::Seq(SeqOp::Includes), Takes::Item, Gives::Bool),
        _ => return None,
    })
}

fn optional_once(inner: Type) -> Type {
    match inner {
        Type::Optional(_) | Type::Null => inner,
        other => Type::optional(other),
    }
}

fn is(expr: Expr, wanted: Expr) -> Expr {
    apply(Op::Equal, vec![expr, wanted])
}

fn int(number: i64) -> Expr {
    Expr::Const(Value::Int(number))
}

impl Lowerer<'_> {
    pub(super) fn method(
        &mut self,
        on: Typed,
        name: &str,
        arguments: &[Term],
        place: Place,
        scope: &mut Scope,
    ) -> Lowering<Typed> {
        let call = Call {
            name,
            arguments,
            place,
        };
        match on.ty.clone() {
            Type::Optional(inner) => self.on_optional(on.expr, *inner, &call, scope),
            Type::Set(item) => {
                let shelves = Shelves {
                    same: on.ty,
                    item: *item,
                    key: Type::Unknown,
                    value: Type::Unknown,
                };
                self.on_shelf(
                    on.expr,
                    &shelves,
                    Shelf::Set,
                    set_signature(name),
                    &call,
                    scope,
                )
            }
            Type::Map(key, value) => {
                let shelves = Shelves {
                    same: on.ty,
                    item: Type::Unknown,
                    key: *key,
                    value: *value,
                };
                self.on_map(on.expr, &shelves, &call, scope)
            }
            Type::Seq(item) => {
                let shelves = Shelves {
                    same: on.ty,
                    item: *item,
                    key: Type::Unknown,
                    value: Type::Unknown,
                };
                self.on_shelf(
                    on.expr,
                    &shelves,
                    Shelf::Seq,
                    seq_signature(name),
                    &call,
                    scope,
                )
            }
            Type::Queue(item, capacity) => {
                let shelves = Shelves {
                    same: on.ty,
                    item: *item,
                    key: Type::Unknown,
                    value: Type::Unknown,
                };
                self.on_queue(on.expr, &shelves, capacity, &call, scope)
            }
            Type::Lock(Some(owner)) => self.on_owned_lock(on.expr, &owner, &call, scope),
            Type::Lock(None) => self.on_plain_lock(on.expr, &call),
            Type::Timer => self.on_timer(on.expr, &call),
            Type::Request => self.on_request(on.expr, &call),
            Type::Retry(most) => self.on_retry(on.expr, most, &call),
            Type::Unknown | Type::Null | Type::Bool | Type::Int | Type::Text | Type::Record(_) => {
                Err(self.unknown(&on.ty, &call))
            }
        }
    }

    fn unknown(&mut self, ty: &Type, call: &Call<'_>) -> super::Stop {
        self.stop(
            call.place,
            &format!("`{}` on a {}", call.name, ty.kind()),
            "it is not one of the library's primitives, so the checker can't know what it does",
        )
    }

    fn arity(&mut self, call: &Call<'_>, wanted: usize) -> Lowering<()> {
        if call.arguments.len() == wanted {
            return Ok(());
        }
        Err(self.stop(
            call.place,
            &format!("`{}` with {} arguments", call.name, call.arguments.len()),
            &format!("this primitive takes {wanted}"),
        ))
    }

    fn argument(
        &mut self,
        call: &Call<'_>,
        index: usize,
        want: &Type,
        scope: &mut Scope,
    ) -> Lowering<Typed> {
        let Some(term) = call.arguments.get(index) else {
            return Err(self.stop(
                call.place,
                &format!("`{}` with too few arguments", call.name),
                "an argument is missing",
            ));
        };
        let lowered = self.term(term, scope)?;
        let ty = want.merged(&lowered.ty);
        let expr = self.coerce(lowered, want, term.place)?;
        Ok(Typed::new(expr, ty))
    }

    fn signed(
        &mut self,
        on: Expr,
        shelves: &Shelves,
        signature: Signature,
        call: &Call<'_>,
        scope: &mut Scope,
    ) -> Lowering<Typed> {
        let (op, takes, gives) = signature;
        let mut arguments = vec![on];
        let mut same = shelves.same.clone();
        match takes {
            Takes::Nothing => self.arity(call, 0)?,
            Takes::Item | Takes::Key | Takes::Same | Takes::Number => {
                self.arity(call, 1)?;
                let want = match takes {
                    Takes::Item => &shelves.item,
                    Takes::Key => &shelves.key,
                    Takes::Number => &Type::Int,
                    Takes::Same | Takes::Nothing | Takes::KeyAndValue => &shelves.same,
                };
                let given = self.argument(call, 0, want, scope)?;
                if matches!(takes, Takes::Item) {
                    same = with_item(&same, given.ty);
                }
                arguments.push(given.expr);
            }
            Takes::KeyAndValue => {
                self.arity(call, 2)?;
                let key = self.argument(call, 0, &shelves.key, scope)?;
                let value = self.argument(call, 1, &shelves.value, scope)?;
                same = Type::Map(Box::new(key.ty), Box::new(value.ty));
                arguments.push(key.expr);
                arguments.push(value.expr);
            }
        }
        let ty = match gives {
            Gives::Bool => Type::Bool,
            Gives::Int => Type::Int,
            Gives::Same => same,
            Gives::OptionalItem => optional_once(shelves.item.clone()),
            Gives::OptionalValue => optional_once(shelves.value.clone()),
            Gives::KeySet => Type::Set(Box::new(shelves.key.clone())),
        };
        Ok(Typed::new(apply(op, arguments), ty))
    }

    fn on_shelf(
        &mut self,
        on: Expr,
        shelves: &Shelves,
        shelf: Shelf,
        signature: Option<Signature>,
        call: &Call<'_>,
        scope: &mut Scope,
    ) -> Lowering<Typed> {
        if let Some(found) = signature {
            return self.signed(on, shelves, found, call, scope);
        }
        let size = match shelf {
            Shelf::Set => Op::Set(SetOp::Size),
            Shelf::Seq => Op::Seq(SeqOp::Size),
        };
        let sweep = match call.name {
            "is_empty" => {
                self.arity(call, 0)?;
                let empty = is(apply(size, vec![on]), int(0));
                return Ok(Typed::new(empty, Type::Bool));
            }
            "every" => Sweep::Every,
            "some" => Sweep::Some,
            "filter" => Sweep::Filter,
            _ => return Err(self.unknown(&shelves.same, call)),
        };
        self.arity(call, 1)?;
        let (slot, body) = self.test(call, &shelves.item, scope)?;
        let ty = match sweep {
            Sweep::Every | Sweep::Some => Type::Bool,
            Sweep::Filter => shelves.same.clone(),
        };
        Ok(Typed::new(
            Expr::Sweep {
                sweep,
                shelf,
                slot,
                source: Box::new(on),
                body: Box::new(body),
            },
            ty,
        ))
    }

    fn test(&mut self, call: &Call<'_>, item: &Type, scope: &mut Scope) -> Lowering<(usize, Expr)> {
        let lambda = call.arguments.first().and_then(|term| match &term.kind {
            TermKind::Lambda { parameters, body } => match parameters.as_slice() {
                [parameter] => Some((parameter, body)),
                _ => None,
            },
            _ => None,
        });
        let Some((parameter, body)) = lambda else {
            return Err(self.stop(
                call.place,
                &format!("`{}` without a one-parameter function", call.name),
                "pass a function of one item that gives true or false",
            ));
        };
        let slot = scope.slot();
        scope
            .names
            .push((parameter.clone(), Binding::Local(slot, item.clone())));
        let lowered = self.truth(body, scope);
        scope.names.pop();
        Ok((slot, lowered?))
    }

    fn on_map(
        &mut self,
        on: Expr,
        shelves: &Shelves,
        call: &Call<'_>,
        scope: &mut Scope,
    ) -> Lowering<Typed> {
        if let Some(found) = map_signature(call.name) {
            return self.signed(on, shelves, found, call, scope);
        }
        if call.name == "is_empty" {
            self.arity(call, 0)?;
            let empty = is(apply(Op::Map(MapOp::Size), vec![on]), int(0));
            return Ok(Typed::new(empty, Type::Bool));
        }
        Err(self.unknown(&shelves.same, call))
    }

    fn on_queue(
        &mut self,
        on: Expr,
        shelves: &Shelves,
        capacity: usize,
        call: &Call<'_>,
        scope: &mut Scope,
    ) -> Lowering<Typed> {
        if let Some(found) = queue_signature(call.name) {
            return self.signed(on, shelves, found, call, scope);
        }
        let full = i64::try_from(capacity).unwrap_or(i64::MAX);
        match call.name {
            "is_full" => {
                self.arity(call, 0)?;
                let size = apply(Op::Seq(SeqOp::Size), vec![on]);
                let at_capacity = apply(Op::Order(Order::GreaterOrEqual), vec![size, int(full)]);
                Ok(Typed::new(at_capacity, Type::Bool))
            }
            "clear" => {
                self.arity(call, 0)?;
                Ok(Typed::new(Expr::SeqOf(Vec::new()), shelves.same.clone()))
            }
            _ => self.on_shelf(on, shelves, Shelf::Seq, None, call, scope),
        }
    }

    fn on_optional(
        &mut self,
        on: Expr,
        inner: Type,
        call: &Call<'_>,
        scope: &mut Scope,
    ) -> Lowering<Typed> {
        let null = Expr::Const(Value::Null);
        match call.name {
            "is_none" => {
                self.arity(call, 0)?;
                Ok(Typed::new(is(on, null), Type::Bool))
            }
            "is_some" => {
                self.arity(call, 0)?;
                Ok(Typed::new(apply(Op::Differ, vec![on, null]), Type::Bool))
            }
            "is_some_and" => {
                self.arity(call, 1)?;
                let (slot, body) = self.test(call, &inner, scope)?;
                let present = apply(Op::Differ, vec![on.clone(), null]);
                let bound = Expr::Bind {
                    slot,
                    value: Box::new(apply(Op::Unwrap, vec![on])),
                    body: Box::new(body),
                };
                Ok(Typed::new(
                    Expr::And(Box::new(present), Box::new(bound)),
                    Type::Bool,
                ))
            }
            _ => {
                let unwrapped = Typed::new(apply(Op::Unwrap, vec![on]), inner);
                self.method(unwrapped, call.name, call.arguments, call.place, scope)
            }
        }
    }

    fn on_owned_lock(
        &mut self,
        on: Expr,
        owner: &Type,
        call: &Call<'_>,
        scope: &mut Scope,
    ) -> Lowering<Typed> {
        let null = Expr::Const(Value::Null);
        let lock = Type::Lock(Some(Box::new(owner.clone())));
        let takes_owner = matches!(call.name, "is_held_by" | "acquire");
        self.arity(call, usize::from(takes_owner))?;
        if takes_owner {
            let given = self.argument(call, 0, owner, scope)?;
            let held = apply(Op::SomeOf, vec![given.expr]);
            return Ok(if call.name == "acquire" {
                Typed::new(held, lock)
            } else {
                Typed::new(is(on, held), Type::Bool)
            });
        }
        match call.name {
            "is_free" => Ok(Typed::new(is(on, null), Type::Bool)),
            "is_held" => Ok(Typed::new(apply(Op::Differ, vec![on, null]), Type::Bool)),
            "holder" => Ok(Typed::new(on, Type::optional(owner.clone()))),
            "release" => Ok(Typed::new(null, lock)),
            _ => Err(self.unknown(&lock, call)),
        }
    }

    fn on_plain_lock(&mut self, on: Expr, call: &Call<'_>) -> Lowering<Typed> {
        self.arity(call, 0)?;
        let lock = Type::Lock(None);
        match call.name {
            "is_free" => Ok(Typed::new(apply(Op::Not, vec![on]), Type::Bool)),
            "is_held" => Ok(Typed::new(on, Type::Bool)),
            "acquire" => Ok(Typed::new(Expr::Const(Value::Bool(true)), lock)),
            "release" => Ok(Typed::new(Expr::Const(Value::Bool(false)), lock)),
            _ => Err(self.unknown(&lock, call)),
        }
    }

    fn stage(&mut self, name: &str) -> Expr {
        self.text(name).expr
    }

    fn on_timer(&mut self, on: Expr, call: &Call<'_>) -> Lowering<Typed> {
        self.arity(call, 0)?;
        let (stage, moves) = match call.name {
            "is_idle" => ("idle", false),
            "is_running" => ("running", false),
            "has_fired" => ("fired", false),
            "start" => ("running", true),
            "fire" => ("fired", true),
            "cancel" | "reset" => ("idle", true),
            _ => return Err(self.unknown(&Type::Timer, call)),
        };
        let wanted = self.stage(stage);
        Ok(if moves {
            Typed::new(wanted, Type::Timer)
        } else {
            Typed::new(is(on, wanted), Type::Bool)
        })
    }

    fn on_request(&mut self, on: Expr, call: &Call<'_>) -> Lowering<Typed> {
        self.arity(call, 0)?;
        let (stage, moves) = match call.name {
            "is_idle" => ("idle", false),
            "is_pending" => ("pending", false),
            "has_succeeded" => ("succeeded", false),
            "has_failed" => ("failed", false),
            "send" => ("pending", true),
            "succeed" => ("succeeded", true),
            "fail" => ("failed", true),
            "reset" => ("idle", true),
            "is_settled" => {
                let succeeded = is(on.clone(), self.stage("succeeded"));
                let failed = is(on, self.stage("failed"));
                return Ok(Typed::new(
                    Expr::Or(Box::new(succeeded), Box::new(failed)),
                    Type::Bool,
                ));
            }
            _ => return Err(self.unknown(&Type::Request, call)),
        };
        let wanted = self.stage(stage);
        Ok(if moves {
            Typed::new(wanted, Type::Request)
        } else {
            Typed::new(is(on, wanted), Type::Bool)
        })
    }

    fn on_retry(&mut self, on: Expr, most: i64, call: &Call<'_>) -> Lowering<Typed> {
        self.arity(call, 0)?;
        let retry = Type::Retry(most);
        match call.name {
            "attempts" => Ok(Typed::new(on, Type::Int)),
            "can_retry" => Ok(Typed::new(
                apply(Op::Order(Order::Less), vec![on, int(most)]),
                Type::Bool,
            )),
            "is_exhausted" => Ok(Typed::new(
                apply(Op::Order(Order::GreaterOrEqual), vec![on, int(most)]),
                Type::Bool,
            )),
            "again" => Ok(Typed::new(
                apply(Op::Math(Math::Add), vec![on, int(1)]),
                retry,
            )),
            "reset" => Ok(Typed::new(int(0), retry)),
            _ => Err(self.unknown(&retry, call)),
        }
    }
}

struct Call<'a> {
    name: &'a str,
    arguments: &'a [Term],
    place: Place,
}

fn with_item(same: &Type, given: Type) -> Type {
    match same {
        Type::Set(_) => Type::Set(Box::new(given)),
        Type::Seq(_) => Type::Seq(Box::new(given)),
        Type::Queue(_, capacity) => Type::Queue(Box::new(given), *capacity),
        Type::Unknown
        | Type::Null
        | Type::Bool
        | Type::Int
        | Type::Text
        | Type::Optional(_)
        | Type::Record(_)
        | Type::Map(_, _)
        | Type::Lock(_)
        | Type::Timer
        | Type::Request
        | Type::Retry(_) => same.clone(),
    }
}
