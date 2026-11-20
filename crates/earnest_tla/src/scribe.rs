use crate::helper::Helper;
use crate::kind::Kind;
use crate::names::Names;
use earnest_ir::{Expr, MapOp, Math, Op, Order, SeqOp, SetOp, Shelf, Sweep};
use earnest_value::{Domain, IntType, Sym, Symbols, Value};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const NULL: &str = "<<>>";
const AND: &str = "/\\";
const OR: &str = "\\/";

#[derive(Clone, Debug)]
pub(crate) struct Written {
    pub(crate) text: String,
    pub(crate) kind: Kind,
    pub(crate) sound: Option<String>,
}

impl Written {
    fn new(text: String, kind: Kind) -> Self {
        Self {
            text,
            kind,
            sound: None,
        }
    }

    fn needing(mut self, needed: impl IntoIterator<Item = Option<String>>) -> Self {
        let all: Vec<String> = needed.into_iter().flatten().collect();
        self.sound = all_of(&all);
        self
    }

    fn truth(text: String) -> Self {
        Self::new(text, Kind::Bool)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Variable {
    pub(crate) name: String,
    pub(crate) kind: Kind,
}

#[derive(Debug)]
struct Local {
    slot: usize,
    name: String,
    kind: Kind,
}

#[derive(Debug)]
pub(crate) struct Scope {
    names: Names,
    locals: Vec<Local>,
}

impl Scope {
    pub(crate) fn new(module: &Names) -> Self {
        Self {
            names: module.clone(),
            locals: Vec::new(),
        }
    }

    pub(crate) fn claim(&mut self, wanted: &str) -> String {
        self.names.claim(wanted)
    }

    pub(crate) fn bind(&mut self, slot: usize, wanted: &str, kind: Kind) -> String {
        let name = self.claim(wanted);
        self.locals.push(Local {
            slot,
            name: name.clone(),
            kind,
        });
        name
    }

    fn release(&mut self) {
        self.locals.pop();
    }

    fn find(&self, slot: usize) -> Option<&Local> {
        self.locals.iter().rev().find(|local| local.slot == slot)
    }
}

pub(crate) fn number(number: i64) -> String {
    if number < 0 {
        format!("({number})")
    } else {
        number.to_string()
    }
}

pub(crate) fn quoted(text: &str) -> String {
    let mut out = String::from("\"");
    for letter in text.chars() {
        match letter {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\u{c}' => out.push_str("\\f"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn listed(open: &str, items: &[String], close: &str) -> String {
    format!("{open}{}{close}", items.join(", "))
}

fn tuple_of(items: &[String]) -> String {
    let inner = items.join(", ");
    if inner.starts_with('<') || inner.ends_with('>') {
        format!("<< {inner} >>")
    } else {
        format!("<<{inner}>>")
    }
}

pub(crate) fn set_of(items: &[String]) -> String {
    listed("{", items, "}")
}

fn record_text(fields: &[(String, String)]) -> String {
    if fields.is_empty() {
        return NULL.to_owned();
    }
    let all: Vec<String> = fields
        .iter()
        .map(|(label, text)| format!("{label} |-> {text}"))
        .collect();
    listed("[", &all, "]")
}

#[derive(Debug)]
pub(crate) struct Scribe<'a> {
    symbols: &'a Symbols,
    variables: Vec<Variable>,
    labels: BTreeMap<Sym, String>,
    label_names: Names,
}

impl<'a> Scribe<'a> {
    pub(crate) fn new(symbols: &'a Symbols, variables: Vec<Variable>, labels: Names) -> Self {
        Self {
            symbols,
            variables,
            labels: BTreeMap::new(),
            label_names: labels,
        }
    }

    pub(crate) fn variables(&self) -> &[Variable] {
        &self.variables
    }

    fn label(&mut self, name: Sym) -> String {
        if let Some(known) = self.labels.get(&name) {
            return known.clone();
        }
        let label = self.label_names.claim(self.symbols.name(name));
        self.labels.insert(name, label.clone());
        label
    }

    pub(crate) fn value(&mut self, value: &Value, kind: &Kind) -> String {
        if let Kind::Optional(inner) = kind {
            return match value {
                Value::Null => NULL.to_owned(),
                present => tuple_of(&[self.value(present, inner)]),
            };
        }
        match value {
            Value::Null => NULL.to_owned(),
            Value::Bool(true) => "TRUE".to_owned(),
            Value::Bool(false) => "FALSE".to_owned(),
            Value::Int(found) => number(*found),
            Value::Str(text) => quoted(self.symbols.name(*text)),
            Value::Record(fields) => {
                let all: Vec<(String, String)> = fields
                    .iter()
                    .map(|(name, inner)| (self.label(*name), self.value(inner, &kind.field(*name))))
                    .collect();
                record_text(&all)
            }
            Value::Set(items) => listed("{", &self.values(items.iter(), &kind.item()), "}"),
            Value::Seq(items) => tuple_of(&self.values(items.iter(), &kind.item())),
            Value::Map(entries) => {
                let keys = self.values(entries.keys(), &kind.key());
                let values = self.values(entries.values(), &kind.value());
                let all: Vec<(String, String)> = keys.into_iter().zip(values).collect();
                map_text(&all)
            }
            _ => unknown("a value of a kind added after it was written").text,
        }
    }

    pub(crate) fn values<'v>(
        &mut self,
        values: impl Iterator<Item = &'v Value>,
        kind: &Kind,
    ) -> Vec<String> {
        values.map(|value| self.value(value, kind)).collect()
    }

    pub(crate) fn within(&mut self, subject: &str, domain: &Domain, scope: &mut Scope) -> String {
        match domain {
            Domain::Bool => format!("({subject} \\in BOOLEAN)"),
            Domain::Int { lo, hi } => format!("({subject} \\in {}..{})", number(*lo), number(*hi)),
            Domain::OneOf(values) => {
                let kind = Kind::of_values(values.iter());
                let all = self.values(values.iter(), &kind);
                format!("({subject} \\in {})", listed("{", &all, "}"))
            }
            Domain::Optional(inner) => {
                let present = self.within(&format!("{subject}[1]"), inner, scope);
                format!("(({subject} = {NULL}) \\/ {present})")
            }
            Domain::Record(fields) => {
                let mut all = Vec::with_capacity(fields.len());
                for (name, inner) in fields {
                    let label = self.label(*name);
                    all.push(self.within(&format!("{subject}.{label}"), inner, scope));
                }
                every_one(&all)
            }
            Domain::Set(item) => {
                let each = scope.claim("item");
                let fits = self.within(&each, item, scope);
                format!("(\\A {each} \\in {subject} : {fits})")
            }
            Domain::Map(key, value) => {
                let each = scope.claim("key");
                let key_fits = self.within(&each, key, scope);
                let value_fits = self.within(&format!("{subject}[{each}]"), value, scope);
                format!("(\\A {each} \\in DOMAIN {subject} : ({key_fits} /\\ {value_fits}))")
            }
            Domain::Seq { item, max } => {
                let each = scope.claim("index");
                let fits = self.within(&format!("{subject}[{each}]"), item, scope);
                format!(
                    "((Len({subject}) <= {max}) /\\ (\\A {each} \\in DOMAIN {subject} : {fits}))"
                )
            }
            _ => unknown("a declaration of a kind added after it was written").text,
        }
    }

    pub(crate) fn expr(&mut self, expr: &Expr, scope: &mut Scope) -> Written {
        match expr {
            Expr::Const(value) => {
                let kind = Kind::of_value(value);
                Written::new(self.value(value, &kind), kind)
            }
            Expr::Field(position) => match self.variables.get(*position) {
                Some(variable) => Written::new(variable.name.clone(), variable.kind.clone()),
                None => unknown("a state field the model does not have"),
            },
            Expr::Local(slot) => match scope.find(*slot) {
                Some(local) => Written::new(local.name.clone(), local.kind.clone()),
                None => unknown("a local value that nothing binds"),
            },
            Expr::Apply(op, arguments) => self.apply(*op, arguments, scope),
            Expr::And(left, right) => self.both(left, AND, right, scope),
            Expr::Or(left, right) => self.both(left, OR, right, scope),
            Expr::Choose(test, yes, no) => {
                let tested = self.expr(test, scope);
                let first = self.expr(yes, scope);
                let second = self.expr(no, scope);
                let branches = (first.sound.is_some() || second.sound.is_some()).then(|| {
                    format!(
                        "(IF {} THEN {} ELSE {})",
                        tested.text,
                        first.sound.as_deref().unwrap_or("TRUE"),
                        second.sound.as_deref().unwrap_or("TRUE")
                    )
                });
                Written::new(
                    format!(
                        "(IF {} THEN {} ELSE {})",
                        tested.text, first.text, second.text
                    ),
                    first.kind.join(&second.kind),
                )
                .needing([tested.sound, branches])
            }
            Expr::Bind { slot, value, body } => {
                let bound = self.expr(value, scope);
                let name = scope.bind(*slot, "local", bound.kind.clone());
                let inner = self.expr(body, scope);
                scope.release();
                let later = inner
                    .sound
                    .as_ref()
                    .map(|needed| format!("(LET {name} == {} IN {needed})", bound.text));
                Written::new(
                    format!("(LET {name} == {} IN {})", bound.text, inner.text),
                    inner.kind,
                )
                .needing([bound.sound, later])
            }
            Expr::Get(inner, name) => {
                let record = self.expr(inner, scope);
                let label = self.label(*name);
                Written::new(format!("{}.{label}", record.text), record.kind.field(*name))
                    .needing([record.sound])
            }
            other => self.built(other, scope),
        }
    }

    fn built(&mut self, expr: &Expr, scope: &mut Scope) -> Written {
        match expr {
            Expr::Record(fields) => {
                let written = self.named(fields, scope);
                let kind = Kind::Record(kinds_of(&written));
                let needed: Vec<Option<String>> =
                    written.iter().map(|(_, each)| each.sound.clone()).collect();
                Written::new(record_text(&self.labelled(&written)), kind).needing(needed)
            }
            Expr::With(base, changes) => self.with(base, changes, scope),
            Expr::SetOf(items) => {
                let all = self.each(items, scope);
                Written::new(listed("{", &texts_of(&all), "}"), Kind::set(joined(&all)))
                    .needing(needs_of(&all))
            }
            Expr::SeqOf(items) => {
                let all = self.each(items, scope);
                Written::new(tuple_of(&texts_of(&all)), Kind::seq(joined(&all)))
                    .needing(needs_of(&all))
            }
            Expr::MapOf(entries) => {
                let mut keys = Vec::with_capacity(entries.len());
                let mut values = Vec::with_capacity(entries.len());
                for (key, value) in entries {
                    keys.push(self.expr(key, scope));
                    values.push(self.expr(value, scope));
                }
                let kind = Kind::map(joined(&keys), joined(&values));
                let all: Vec<(String, String)> =
                    texts_of(&keys).into_iter().zip(texts_of(&values)).collect();
                let needed: Vec<Option<String>> = needs_of(&keys)
                    .into_iter()
                    .chain(needs_of(&values))
                    .collect();
                Written::new(map_text(&all), kind).needing(needed)
            }
            Expr::Sweep {
                sweep,
                shelf,
                slot,
                source,
                body,
            } => self.sweep((*sweep, *shelf), *slot, source, body, scope),
            _ => unknown("an expression of a kind added after it was written"),
        }
    }

    fn both(&mut self, left: &Expr, sign: &str, right: &Expr, scope: &mut Scope) -> Written {
        let first = self.expr(left, scope);
        let second = self.expr(right, scope);
        let later = second.sound.as_ref().map(|needed| {
            if sign == AND {
                format!("((~{}) {OR} {needed})", first.text)
            } else {
                format!("({} {OR} {needed})", first.text)
            }
        });
        Written::truth(format!("({} {sign} {})", first.text, second.text))
            .needing([first.sound, later])
    }

    fn each(&mut self, items: &[Expr], scope: &mut Scope) -> Vec<Written> {
        items.iter().map(|item| self.expr(item, scope)).collect()
    }

    fn named(&mut self, fields: &[(Sym, Expr)], scope: &mut Scope) -> Vec<(Sym, Written)> {
        fields
            .iter()
            .map(|(name, inner)| (*name, self.expr(inner, scope)))
            .collect()
    }

    fn labelled(&mut self, fields: &[(Sym, Written)]) -> Vec<(String, String)> {
        fields
            .iter()
            .map(|(name, written)| (self.label(*name), written.text.clone()))
            .collect()
    }

    fn with(&mut self, base: &Expr, changes: &[(Sym, Expr)], scope: &mut Scope) -> Written {
        let on = self.expr(base, scope);
        let replaced = self.named(changes, scope);
        if replaced.is_empty() {
            return on;
        }
        let known = match &on.kind {
            Kind::Record(fields) => Some(fields.clone()),
            _ => None,
        };
        let kind = on.kind.join(&Kind::Record(kinds_of(&replaced)));
        let needed: Vec<Option<String>> = std::iter::once(on.sound.clone())
            .chain(replaced.iter().map(|(_, each)| each.sound.clone()))
            .collect();
        let adds_a_field = known.as_ref().is_some_and(|fields| {
            replaced
                .iter()
                .any(|(name, _)| !fields.iter().any(|(had, _)| had == name))
        });
        if !adds_a_field {
            let all: Vec<String> = self
                .labelled(&replaced)
                .iter()
                .map(|(label, text)| format!("!.{label} = {text}"))
                .collect();
            return Written::new(format!("[{} EXCEPT {}]", on.text, all.join(", ")), kind)
                .needing(needed);
        }
        let old = scope.claim("local");
        let mut fields: Vec<(Sym, Written)> = Vec::new();
        for (name, _) in known.iter().flatten() {
            let label = self.label(*name);
            let kept = Written::new(format!("{old}.{label}"), Kind::Unknown);
            fields.push((*name, kept));
        }
        for (name, written) in replaced {
            match fields.iter_mut().find(|(had, _)| *had == name) {
                Some((_, place)) => *place = written,
                None => fields.push((name, written)),
            }
        }
        let record = record_text(&self.labelled(&fields));
        Written::new(format!("(LET {old} == {} IN {record})", on.text), kind).needing(needed)
    }

    fn sweep(
        &mut self,
        how: (Sweep, Shelf),
        slot: usize,
        source: &Expr,
        body: &Expr,
        scope: &mut Scope,
    ) -> Written {
        let (reach, gives) = how;
        let over = self.expr(source, scope);
        let from_a_seq = match (&over.kind, gives) {
            (Kind::Seq(_), _) => true,
            (Kind::Set(_), _) => false,
            (_, found) => found == Shelf::Seq,
        };
        let item = scope.bind(slot, "item", over.kind.item());
        let tested = self.expr(body, scope);
        let test = &tested.text;
        scope.release();
        let selects = matches!((reach, gives), (Sweep::Filter, Shelf::Seq));
        let items = if from_a_seq && (!selects || tested.sound.is_some()) {
            call(Helper::SeqItems, &[&over.text], Kind::Unknown).text
        } else {
            over.text.clone()
        };
        let order = if from_a_seq {
            Some(Visit::Index)
        } else {
            Visit::of_set(&over.kind)
        };
        let stops = matches!(reach, Sweep::Every | Sweep::Some);
        let each = match (&tested.sound, stops, order) {
            (Some(needed), true, Some(order)) => {
                let earlier = scope.bind(slot, "item", over.kind.item());
                let again = self.expr(body, scope);
                scope.release();
                let stopping_value = match reach {
                    Sweep::Every => format!("(~{})", again.text),
                    _ => again.text.clone(),
                };
                let stops_here = format!(
                    "({} /\\ {stopping_value})",
                    again.sound.as_deref().unwrap_or("TRUE")
                );
                let names = [scope.claim("Ok"), scope.claim("Hit")];
                let places = [scope.claim("at"), scope.claim("before")];
                Some(short_circuit(
                    &order,
                    (&item, &earlier),
                    &over.text,
                    (needed, &stops_here),
                    (&names, &places),
                ))
            }
            (Some(needed), _, _) => Some(format!("(\\A {item} \\in {items} : {needed})")),
            (None, _, _) => None,
        };
        let done = match (reach, gives) {
            (Sweep::Every, _) => Written::truth(format!("(\\A {item} \\in {items} : {test})")),
            (Sweep::Some, _) => Written::truth(format!("(\\E {item} \\in {items} : {test})")),
            (Sweep::Filter, Shelf::Set) => Written::new(
                format!("{{{item} \\in {items} : {test}}}"),
                Kind::set(over.kind.item()),
            ),
            (Sweep::Filter, Shelf::Seq) if from_a_seq => {
                let keep = scope.claim("Keep");
                Written::new(
                    format!(
                        "(LET {keep}({item}) == {test} IN SelectSeq({}, {keep}))",
                        over.text
                    ),
                    Kind::seq(over.kind.item()),
                )
            }
            (Sweep::Filter, Shelf::Seq) => unknown("the order of a set filtered as a sequence"),
            _ => unknown("a test over a collection of a kind added after it was written"),
        };
        done.needing([over.sound, each])
    }

    fn apply(&mut self, op: Op, arguments: &[Expr], scope: &mut Scope) -> Written {
        let given = self.each(arguments, scope);
        let mut needed = needs_of(&given);
        needed.push(has_a_value(op, &given));
        applied(op, &given).needing(needed)
    }
}

enum Visit {
    Index,
    Number,
    Truth,
}

impl Visit {
    fn of_set(items: &Kind) -> Option<Self> {
        match items.item() {
            Kind::Int => Some(Self::Number),
            Kind::Bool => Some(Self::Truth),
            _ => None,
        }
    }
}

fn short_circuit(
    order: &Visit,
    (item, earlier): (&str, &str),
    source: &str,
    (needed, decided): (&str, &str),
    (names, places): (&[String; 2], &[String; 2]),
) -> String {
    let [ok, hit] = names;
    let [at, before] = places;
    let definitions =
        format!("LET {ok}({item}) == {needed} IN LET {hit}({earlier}) == {decided} IN");
    let body = match order {
        Visit::Index => format!(
            "\\A {at} \\in 1..Len({source}) : {ok}({source}[{at}]) \\/ \\E {before} \\in \
             1..({at} - 1) : {hit}({source}[{before}])"
        ),
        Visit::Number | Visit::Truth => {
            let first = match order {
                Visit::Number => format!("({before} < {at})"),
                _ => format!("((~{before}) /\\ {at})"),
            };
            format!(
                "\\A {at} \\in {source} : {ok}({at}) \\/ \\E {before} \\in {source} : \
                 {first} /\\ {hit}({before})"
            )
        }
    };
    format!("({definitions} {body})")
}

fn applied(op: Op, given: &[Written]) -> Written {
    let Some(first) = given.first() else {
        return unknown("an operation without an argument");
    };
    if let Some(done) = alone(op, first) {
        return done;
    }
    let Some(second) = given.get(1) else {
        return unknown("an operation that lacks its second argument");
    };
    pair(op, first, second, given.get(2))
}

fn alone(op: Op, on: &Written) -> Option<Written> {
    let text = &on.text;
    Some(match op {
        Op::Not => Written::truth(format!("(~{text})")),
        Op::Negate => Written::new(format!("(-{text})"), Kind::Int),
        Op::SomeOf => Written::new(
            tuple_of(std::slice::from_ref(text)),
            Kind::optional(on.kind.clone()),
        ),
        Op::Unwrap => Written::new(format!("{text}[1]"), on.kind.present()),
        Op::Fit(_) | Op::Safe => Written::new(text.clone(), Kind::Int),
        Op::Set(SetOp::Size) => Written::new(format!("Cardinality({text})"), Kind::Int),
        Op::Map(MapOp::Size) => Written::new(format!("Cardinality(DOMAIN {text})"), Kind::Int),
        Op::Map(MapOp::Keys) => Written::new(format!("(DOMAIN {text})"), Kind::set(on.kind.key())),
        Op::Seq(SeqOp::Size) => Written::new(format!("Len({text})"), Kind::Int),
        Op::Seq(SeqOp::First) => found(
            Helper::SeqFirst,
            Helper::SeqFirstOptional,
            on.kind.item(),
            &[text],
        ),
        Op::Seq(SeqOp::Last) => found(
            Helper::SeqLast,
            Helper::SeqLastOptional,
            on.kind.item(),
            &[text],
        ),
        Op::Seq(SeqOp::Shift) => call(Helper::SeqShift, &[text], on.kind.clone()),
        Op::Seq(SeqOp::Pop) => call(Helper::SeqPop, &[text], on.kind.clone()),
        _ => return None,
    })
}

fn pair(op: Op, first: &Written, second: &Written, third: Option<&Written>) -> Written {
    let (left, right) = (&first.text, &second.text);
    match op {
        Op::Equal => Written::truth(format!("({left} = {right})")),
        Op::Differ => Written::truth(format!("({left} # {right})")),
        Op::Order(order) => {
            let sign = match order {
                Order::Less => "<",
                Order::LessOrEqual => "<=",
                Order::Greater => ">",
                Order::GreaterOrEqual => ">=",
                _ => return unknown("a comparison added after it was written"),
            };
            Written::truth(format!("({left} {sign} {right})"))
        }
        Op::Math(math) => {
            let sign = match math {
                Math::Add => "+",
                Math::Subtract => "-",
                Math::Multiply => "*",
                Math::Remainder => {
                    return call(Helper::Remainder, &[left, right], Kind::Int);
                }
                Math::Divide => {
                    return call(Helper::Division, &[left, right], Kind::Int);
                }
                _ => return unknown("arithmetic added after it was written"),
            };
            Written::new(format!("({left} {sign} {right})"), Kind::Int)
        }
        Op::Set(set_op) => on_set(set_op, first, second),
        Op::Map(map_op) => on_map(map_op, first, second, third),
        Op::Seq(seq_op) => on_seq(seq_op, first, second),
        _ => unknown("an operation added after it was written"),
    }
}

fn on_set(op: SetOp, first: &Written, second: &Written) -> Written {
    let (left, right) = (&first.text, &second.text);
    let both = first.kind.join(&second.kind);
    match op {
        SetOp::Has => Written::truth(format!("({right} \\in {left})")),
        SetOp::Add => Written::new(
            format!("({left} \\cup {{{right}}})"),
            first.kind.join(&Kind::set(second.kind.clone())),
        ),
        SetOp::Delete => Written::new(format!("({left} \\ {{{right}}})"), first.kind.clone()),
        SetOp::Union => Written::new(format!("({left} \\cup {right})"), both),
        SetOp::Intersect => Written::new(format!("({left} \\cap {right})"), both),
        SetOp::Subtract => Written::new(format!("({left} \\ {right})"), both),
        SetOp::IsSubset => Written::truth(format!("({left} \\subseteq {right})")),
        _ => unknown("a set primitive added after it was written"),
    }
}

fn on_map(op: MapOp, first: &Written, second: &Written, third: Option<&Written>) -> Written {
    let (map, key) = (&first.text, &second.text);
    match (op, third) {
        (MapOp::Has, _) => Written::truth(format!("({key} \\in DOMAIN {map})")),
        (MapOp::Get, _) => found(
            Helper::MapGet,
            Helper::MapGetOptional,
            first.kind.value(),
            &[map, key],
        ),
        (MapOp::Delete, _) => call(Helper::MapDelete, &[map, key], first.kind.clone()),
        (MapOp::Set, Some(value)) => {
            let set = Kind::map(second.kind.clone(), value.kind.clone());
            call(
                Helper::MapSet,
                &[map, key, &value.text],
                first.kind.join(&set),
            )
        }
        (MapOp::Set, None) => unknown("a map set without a value"),
        _ => unknown("a map primitive added after it was written"),
    }
}

fn on_seq(op: SeqOp, first: &Written, second: &Written) -> Written {
    let (seq, other) = (&first.text, &second.text);
    match op {
        SeqOp::Get => found(
            Helper::SeqGet,
            Helper::SeqGetOptional,
            first.kind.item(),
            &[seq, other],
        ),
        SeqOp::Push => Written::new(
            format!("Append({seq}, {other})"),
            first.kind.join(&Kind::seq(second.kind.clone())),
        ),
        SeqOp::Includes => call(Helper::SeqIncludes, &[seq, other], Kind::Bool),
        _ => unknown("a sequence primitive added after it was written"),
    }
}

fn has_a_value(op: Op, given: &[Written]) -> Option<String> {
    match (op, given) {
        (Op::Unwrap, [on, ..]) => Some(format!("({} # {NULL})", on.text)),
        (Op::Fit(ty), [on, ..]) => inside(ty, &on.text),
        (Op::Math(Math::Remainder | Math::Divide), [left, right, ..]) => Some(format!(
            "(({} >= 0) {AND} ({} >= 1))",
            left.text, right.text
        )),
        _ => None,
    }
}

fn inside(ty: IntType, text: &str) -> Option<String> {
    let lowest = Some(ty.lowest()).filter(|bound| *bound > i64::from(i32::MIN));
    let highest = Some(ty.highest()).filter(|bound| *bound < i64::from(i32::MAX));
    match (lowest, highest) {
        (Some(lo), Some(hi)) => Some(format!("({text} \\in {}..{})", number(lo), number(hi))),
        (Some(lo), None) => Some(format!("({text} >= {})", number(lo))),
        (None, Some(hi)) => Some(format!("({text} <= {})", number(hi))),
        (None, None) => None,
    }
}

fn all_of(conditions: &[String]) -> Option<String> {
    match conditions {
        [] => None,
        [only] => Some(only.clone()),
        many => Some(format!("({})", many.join(" /\\ "))),
    }
}

fn every_one(conditions: &[String]) -> String {
    all_of(conditions).unwrap_or_else(|| "TRUE".to_owned())
}

fn needs_of(written: &[Written]) -> Vec<Option<String>> {
    written.iter().map(|each| each.sound.clone()).collect()
}

fn texts_of(written: &[Written]) -> Vec<String> {
    written.iter().map(|each| each.text.clone()).collect()
}

fn kinds_of(fields: &[(Sym, Written)]) -> Vec<(Sym, Kind)> {
    fields
        .iter()
        .map(|(name, written)| (*name, written.kind.clone()))
        .collect()
}

fn joined(written: &[Written]) -> Kind {
    written
        .iter()
        .fold(Kind::Unknown, |kind, each| kind.join(&each.kind))
}

fn unknown(what: &str) -> Written {
    let message = quoted(&format!("the TLA+ export does not know {what}"));
    Written::new(
        format!("{}({message})", Helper::Fault.name()),
        Kind::Unknown,
    )
}

fn call(helper: Helper, arguments: &[&str], kind: Kind) -> Written {
    Written::new(format!("{}({})", helper.name(), arguments.join(", ")), kind)
}

fn found(plain: Helper, optional: Helper, item: Kind, on: &[&str]) -> Written {
    let helper = if item.is_optional() { optional } else { plain };
    call(helper, on, item.once_optional())
}

fn map_text(entries: &[(String, String)]) -> String {
    let mut text = Helper::EmptyMap.name().to_owned();
    for (key, value) in entries {
        text = format!("{}({text}, {key}, {value})", Helper::MapSet.name());
    }
    text
}

pub(crate) fn names_written(text: &str) -> BTreeSet<&str> {
    let mut names = BTreeSet::new();
    let mut in_a_text = false;
    let mut escaped = false;
    let mut start = None;
    for (at, letter) in text.char_indices() {
        let in_a_name = letter.is_ascii_alphanumeric() || letter == '_';
        if in_a_text {
            in_a_text = escaped || letter != '"';
            escaped = !escaped && letter == '\\';
        } else if in_a_name {
            start.get_or_insert(at);
        } else {
            names.extend(start.take().and_then(|from| text.get(from..at)));
            in_a_text = letter == '"';
        }
    }
    names.extend(start.and_then(|from| text.get(from..)));
    names
}

#[cfg(test)]
mod tests {
    use super::{names_written, number, quoted, unknown};

    #[test]
    fn the_names_in_a_text_value_are_not_names_the_module_writes() {
        let names = names_written("SeqShift(line) = \"MapGet(\\\"x\\\") \\\\\" /\\ Fault_2");
        assert!(names.contains("SeqShift"), "{names:?}");
        assert!(names.contains("line"), "{names:?}");
        assert!(names.contains("Fault_2"), "{names:?}");
        assert!(!names.contains("MapGet"), "{names:?}");
        assert!(!names.contains("x"), "{names:?}");
        assert!(!names.contains("Fault"), "{names:?}");
    }

    #[test]
    fn a_name_at_the_very_end_is_found() {
        assert!(names_written("\\/ Stop").contains("Stop"));
    }

    #[test]
    fn a_negative_number_is_put_in_brackets() {
        assert_eq!(number(0), "0");
        assert_eq!(number(12), "12");
        assert_eq!(number(-12), "(-12)");
        assert_eq!(number(i64::MIN), "(-9223372036854775808)");
    }

    #[test]
    fn a_text_is_quoted_with_its_special_letters_escaped() {
        assert_eq!(quoted("open"), "\"open\"");
        assert_eq!(quoted("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(quoted("\n\t\r\u{c}"), "\"\\n\\t\\r\\f\"");
        assert_eq!(quoted(""), "\"\"");
    }

    #[test]
    fn what_the_writer_does_not_know_is_a_fault_that_says_so() {
        assert_eq!(
            unknown("an operation added after it was written").text,
            "Fault(\"the TLA+ export does not know an operation added after it was written\")"
        );
    }
}
