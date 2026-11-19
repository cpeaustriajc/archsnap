use crate::domain::Domain;
use crate::value::{Sym, Symbols, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Math {
    Add,
    Subtract,
    Multiply,
    Remainder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetOp {
    Has,
    Add,
    Delete,
    Union,
    Intersect,
    Subtract,
    IsSubset,
    Size,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapOp {
    Has,
    Get,
    Set,
    Delete,
    Size,
    Keys,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeqOp {
    Size,
    Get,
    First,
    Last,
    Push,
    Shift,
    Pop,
    Includes,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Not,
    Negate,
    SomeOf,
    Unwrap,
    Equal,
    Differ,
    Order(Order),
    Math(Math),
    Set(SetOp),
    Map(MapOp),
    Seq(SeqOp),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sweep {
    Every,
    Some,
    Filter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shelf {
    Set,
    Seq,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    Const(Value),
    Field(usize),
    Local(usize),
    Apply(Op, Vec<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Choose(Box<Expr>, Box<Expr>, Box<Expr>),
    Bind {
        slot: usize,
        value: Box<Expr>,
        body: Box<Expr>,
    },
    Record(Vec<(Sym, Expr)>),
    Get(Box<Expr>, Sym),
    With(Box<Expr>, Vec<(Sym, Expr)>),
    SetOf(Vec<Expr>),
    SeqOf(Vec<Expr>),
    MapOf(Vec<(Expr, Expr)>),
    Sweep {
        sweep: Sweep,
        shelf: Shelf,
        slot: usize,
        source: Box<Expr>,
        body: Box<Expr>,
    },
}

#[derive(Clone, Debug)]
pub struct Field {
    pub name: String,
    pub sym: Sym,
    pub domain: Domain,
    pub starts: Vec<Value>,
}

#[derive(Clone, Debug)]
pub struct Input {
    pub name: String,
    pub slot: usize,
    pub domain: Domain,
    pub values: Vec<Value>,
}

#[derive(Clone, Debug)]
pub struct Action {
    pub id: String,
    pub label: String,
    pub line: u32,
    pub inputs: Vec<Input>,
    pub guard: Expr,
    pub updates: Vec<Option<Expr>>,
    pub slots: usize,
}

#[derive(Clone, Debug)]
pub struct Rule {
    pub id: String,
    pub says: String,
    pub line: u32,
    pub text: String,
    pub holds: Expr,
    pub slots: usize,
}

#[derive(Clone, Debug)]
pub struct End {
    pub id: String,
    pub says: String,
    pub when: Expr,
    pub slots: usize,
}

#[derive(Clone, Debug)]
pub enum Piece {
    Text(String),
    Shown(Expr),
}

#[derive(Clone, Debug)]
pub struct Label {
    pub pieces: Vec<Piece>,
    pub slots: usize,
}

#[derive(Clone, Debug, Default)]
pub struct View {
    pub title: Option<String>,
    pub state: Option<Label>,
    pub group: Option<Label>,
    pub steps: Vec<(String, String)>,
    pub colors: Vec<(String, String)>,
}

#[derive(Clone, Debug)]
pub struct Model {
    pub name: String,
    pub file: String,
    pub line: u32,
    pub symbols: Symbols,
    pub fields: Vec<Field>,
    pub actions: Vec<Action>,
    pub rules: Vec<Rule>,
    pub ends: Vec<End>,
    pub view: View,
}
