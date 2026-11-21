use crate::wellformed::assert_well_formed;
use earnest_ir::{
    Action, Code, End, Expr, Field, Input, MapOp, Math, Model, Op, Order, Rule, SeqOp, SetOp,
    Shelf, Sweep, View,
};
use earnest_tla::write;
use earnest_value::{Domain, Sym, Symbols, Value};
use proptest::prelude::{BoxedStrategy, Just, Strategy, any, prop_oneof, proptest};
use proptest::sample::select;
use std::collections::{BTreeMap, BTreeSet};

const WORDS: [&str; 34] = [
    "a",
    "b",
    "count",
    "IF",
    "Init",
    "Next",
    "vars",
    "seq",
    "item",
    "local",
    "Keep",
    "key",
    "x y",
    "",
    "WF_x",
    "_",
    "9lives",
    "café",
    "say \"hi\"",
    "back\\slash",
    "InRange",
    "Stop",
    "Len",
    "nothing",
    "message",
    "Fault",
    "DOMAIN",
    "index",
    "other",
    "map",
    "value",
    "left",
    "Rule_a",
    "Action_a",
];

fn words() -> (Symbols, Vec<Sym>) {
    let mut symbols = Symbols::default();
    let syms = WORDS.iter().map(|word| symbols.intern(word)).collect();
    (symbols, syms)
}

fn word() -> BoxedStrategy<String> {
    select(WORDS.to_vec()).prop_map(str::to_owned).boxed()
}

fn sym() -> BoxedStrategy<Sym> {
    select(words().1).boxed()
}

fn value() -> BoxedStrategy<Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        (-3_i64..4).prop_map(Value::Int),
        sym().prop_map(Value::Str),
    ];
    leaf.prop_recursive(3, 12, 3, |inner| {
        let list = proptest::collection::vec(inner.clone(), 0..3);
        prop_oneof![
            proptest::collection::btree_map(sym(), inner.clone(), 0..3)
                .prop_map(|fields| Value::record(fields.into_iter().collect())),
            list.clone()
                .prop_map(|items| Value::set(items.into_iter().collect::<BTreeSet<Value>>())),
            list.prop_map(Value::seq),
            proptest::collection::btree_map(inner.clone(), inner, 0..3).prop_map(Value::map),
        ]
    })
    .boxed()
}

fn domain() -> BoxedStrategy<Domain> {
    let leaf = prop_oneof![
        Just(Domain::Bool),
        (-2_i64..3, -2_i64..3).prop_map(|(lo, hi)| Domain::Int { lo, hi }),
        proptest::collection::vec(value(), 0..3).prop_map(Domain::OneOf),
    ];
    leaf.prop_recursive(3, 8, 3, |inner| {
        prop_oneof![
            inner
                .clone()
                .prop_map(|each| Domain::Optional(Box::new(each))),
            proptest::collection::btree_map(sym(), inner.clone(), 0..3)
                .prop_map(|fields| Domain::Record(fields.into_iter().collect())),
            inner.clone().prop_map(|each| Domain::Set(Box::new(each))),
            (inner.clone(), inner.clone())
                .prop_map(|(key, each)| Domain::Map(Box::new(key), Box::new(each))),
            (inner, 0_usize..3).prop_map(|(each, max)| Domain::Seq {
                item: Box::new(each),
                max
            }),
        ]
    })
    .boxed()
}

fn op() -> BoxedStrategy<Op> {
    select(vec![
        Op::Not,
        Op::Negate,
        Op::SomeOf,
        Op::Unwrap,
        Op::Equal,
        Op::Differ,
        Op::Order(Order::Less),
        Op::Order(Order::LessOrEqual),
        Op::Order(Order::Greater),
        Op::Order(Order::GreaterOrEqual),
        Op::Math(Math::Add),
        Op::Math(Math::Subtract),
        Op::Math(Math::Multiply),
        Op::Math(Math::Remainder),
        Op::Set(SetOp::Has),
        Op::Set(SetOp::Add),
        Op::Set(SetOp::Delete),
        Op::Set(SetOp::Union),
        Op::Set(SetOp::Intersect),
        Op::Set(SetOp::Subtract),
        Op::Set(SetOp::IsSubset),
        Op::Set(SetOp::Size),
        Op::Map(MapOp::Has),
        Op::Map(MapOp::Get),
        Op::Map(MapOp::Set),
        Op::Map(MapOp::Delete),
        Op::Map(MapOp::Size),
        Op::Map(MapOp::Keys),
        Op::Seq(SeqOp::Size),
        Op::Seq(SeqOp::Get),
        Op::Seq(SeqOp::First),
        Op::Seq(SeqOp::Last),
        Op::Seq(SeqOp::Push),
        Op::Seq(SeqOp::Shift),
        Op::Seq(SeqOp::Pop),
        Op::Seq(SeqOp::Includes),
    ])
    .boxed()
}

fn named(inner: &BoxedStrategy<Expr>) -> BoxedStrategy<Vec<(Sym, Expr)>> {
    proptest::collection::btree_map(sym(), inner.clone(), 0..3)
        .prop_map(|fields| fields.into_iter().collect())
        .boxed()
}

fn tested(inner: &BoxedStrategy<Expr>) -> BoxedStrategy<Expr> {
    (
        select(vec![Sweep::Every, Sweep::Some, Sweep::Filter]),
        select(vec![Shelf::Set, Shelf::Seq]),
        0_usize..5,
        inner.clone(),
        inner.clone(),
    )
        .prop_map(|(sweep, shelf, slot, source, body)| Expr::Sweep {
            sweep,
            shelf,
            slot,
            source: Box::new(source),
            body: Box::new(body),
        })
        .boxed()
}

fn expr() -> BoxedStrategy<Expr> {
    let leaf = prop_oneof![
        value().prop_map(Expr::Const),
        (0_usize..5).prop_map(Expr::Field),
        (0_usize..5).prop_map(Expr::Local),
    ];
    leaf.prop_recursive(4, 24, 3, |inner| {
        let list = proptest::collection::vec(inner.clone(), 0..4);
        let pair = (inner.clone(), inner.clone());
        prop_oneof![
            (op(), list.clone()).prop_map(|(found, arguments)| Expr::Apply(found, arguments)),
            pair.clone()
                .prop_map(|(left, right)| Expr::And(Box::new(left), Box::new(right))),
            pair.clone()
                .prop_map(|(left, right)| Expr::Or(Box::new(left), Box::new(right))),
            (inner.clone(), inner.clone(), inner.clone()).prop_map(|(test, yes, no)| {
                Expr::Choose(Box::new(test), Box::new(yes), Box::new(no))
            }),
            (0_usize..5, inner.clone(), inner.clone()).prop_map(|(slot, bound, body)| {
                Expr::Bind {
                    slot,
                    value: Box::new(bound),
                    body: Box::new(body),
                }
            }),
            named(&inner).prop_map(Expr::Record),
            (inner.clone(), sym()).prop_map(|(record, name)| Expr::Get(Box::new(record), name)),
            (inner.clone(), named(&inner))
                .prop_map(|(record, changes)| Expr::With(Box::new(record), changes)),
            list.clone().prop_map(Expr::SetOf),
            list.prop_map(Expr::SeqOf),
            proptest::collection::vec(pair, 0..3).prop_map(Expr::MapOf),
            tested(&inner),
        ]
    })
    .boxed()
}

fn field() -> BoxedStrategy<Field> {
    (word(), domain(), proptest::collection::vec(value(), 0..3))
        .prop_map(|(name, declared, starts)| {
            let (_, syms) = words();
            Field {
                name,
                sym: syms[0],
                domain: declared,
                starts,
            }
        })
        .boxed()
}

fn an_input() -> BoxedStrategy<Input> {
    (
        word(),
        0_usize..5,
        domain(),
        proptest::collection::vec(value(), 0..3),
    )
        .prop_map(|(name, slot, declared, values)| Input {
            name,
            slot,
            domain: declared,
            values,
        })
        .boxed()
}

fn action() -> BoxedStrategy<Action> {
    (
        word(),
        proptest::collection::vec(an_input(), 0..3),
        expr(),
        proptest::collection::vec(proptest::option::of(expr()), 0..5),
    )
        .prop_map(|(id, inputs, guard, updates)| Action {
            id,
            label: String::new(),
            line: 1,
            inputs,
            guard: guard.into(),
            updates: updates
                .into_iter()
                .map(|update| update.map(Code::new))
                .collect(),
            slots: 5,
        })
        .boxed()
}

fn model() -> BoxedStrategy<Model> {
    (
        word(),
        proptest::collection::vec(field(), 0..4),
        proptest::collection::vec(action(), 0..3),
        proptest::collection::vec((word(), expr()), 0..3),
        proptest::collection::vec((word(), expr()), 0..3),
    )
        .prop_map(|(name, fields, actions, rules, ends)| Model {
            name,
            file: String::new(),
            line: 1,
            symbols: words().0,
            fields,
            actions,
            rules: rules
                .into_iter()
                .map(|(id, holds)| Rule {
                    id,
                    says: String::new(),
                    line: 1,
                    text: String::new(),
                    holds: holds.into(),
                    slots: 5,
                })
                .collect(),
            ends: ends
                .into_iter()
                .map(|(id, when)| End {
                    id,
                    says: String::new(),
                    when: when.into(),
                    slots: 5,
                })
                .collect(),
            view: View::default(),
        })
        .boxed()
}

proptest! {
    #[test]
    fn any_model_exports_as_well_formed_text(model in model()) {
        let files = write(&model);
        assert_well_formed(&files, model.rules.len());
    }

    #[test]
    fn exporting_the_same_model_twice_gives_the_same_text(model in model()) {
        assert_eq!(write(&model), write(&model));
    }
}

#[test]
fn a_generated_value_is_printed_from_the_words_it_was_built_with() {
    let (symbols, syms) = words();
    let entries: BTreeMap<Value, Value> =
        [(Value::Str(syms[3]), Value::Null)].into_iter().collect();
    assert_eq!(symbols.show(&Value::map(entries)), "{\"IF\" -> null}");
}
