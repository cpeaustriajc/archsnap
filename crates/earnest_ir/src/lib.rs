//! The typed expression language, its evaluator and the checked model.
//!
//! A [`Model`] is what `earnest_semantic` makes of a draft: fields with finite domains, and
//! actions, rules and ends whose conditions are [`Expr`]s. An expression reads state fields by
//! position and local values by slot, and [`eval`] gives its value in one state. The model
//! keeps each expression the search evaluates as a [`Code`], which gives the same answers faster.

mod code;
mod eval;
mod parts;
mod told;

pub use code::Code;
pub use eval::{Fault, Frame, eval, truth};

use earnest_value::{Domain, IntType, Sym, Symbols, Value};

/// A comparison of two numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Order {
    /// The first is below the second.
    Less,
    /// The first is below or equal to the second.
    LessOrEqual,
    /// The first is above the second.
    Greater,
    /// The first is above or equal to the second.
    GreaterOrEqual,
}

/// Arithmetic on two numbers. A result that does not fit in 64 bits is [`Fault::Overflow`], and
/// so is one past the safe integers when it is wrapped in [`Op::Safe`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Math {
    /// The sum.
    Add,
    /// The first minus the second.
    Subtract,
    /// The product.
    Multiply,
    /// What is left of the first after dividing by the second. A negative first number or a
    /// second number below 1 is [`Fault::Remainder`].
    Remainder,
    /// The whole number of times the second goes into the first. A negative first number or a
    /// second number below 1 is [`Fault::Division`].
    Divide,
}

/// A primitive on a set. The set is the first argument.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SetOp {
    /// Whether the second argument is an item.
    Has,
    /// The set with the second argument as an item.
    Add,
    /// The set without the second argument.
    Delete,
    /// The items of either set.
    Union,
    /// The items of both sets.
    Intersect,
    /// The items of the first set that the second lacks.
    Subtract,
    /// Whether every item of the first set is in the second.
    IsSubset,
    /// How many items it holds.
    Size,
}

/// A primitive on a map. The map is the first argument and the key the second.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MapOp {
    /// Whether the key has a value.
    Has,
    /// The key's value, or `null` without one.
    Get,
    /// The map with the key set to the third argument.
    Set,
    /// The map without the key.
    Delete,
    /// How many keys it holds.
    Size,
    /// The set of its keys.
    Keys,
}

/// A primitive on a sequence or a queue. The sequence is the first argument.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SeqOp {
    /// How many items it holds.
    Size,
    /// The item at the position in the second argument, counted from 0, or `null` when there
    /// is none.
    Get,
    /// The first item, or `null` when empty.
    First,
    /// The last item, or `null` when empty.
    Last,
    /// The sequence with the second argument added at the end.
    Push,
    /// The sequence without its first item. An empty sequence stays empty.
    Shift,
    /// The sequence without its last item. An empty sequence stays empty.
    Pop,
    /// Whether the second argument is an item.
    Includes,
}

/// An operation on evaluated arguments, used in [`Expr::Apply`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Op {
    /// The opposite of a boolean.
    Not,
    /// A number with its sign flipped.
    Negate,
    /// A present optional value. It gives its argument back unchanged, because an optional
    /// value has no wrapper.
    SomeOf,
    /// The value of an optional that must be present. `null` is [`Fault::NullRead`].
    Unwrap,
    /// A number that must be one a JavaScript number holds exactly. It gives its argument back
    /// unchanged, and a number beyond `-MOST_SAFE_INTEGER` or `MOST_SAFE_INTEGER` is
    /// [`Fault::Overflow`].
    Safe,
    /// A number that must be a value of this integer type. It gives its argument back
    /// unchanged, and a number outside the type is [`Fault::Outside`].
    Fit(IntType),
    /// Whether two values are the same.
    Equal,
    /// Whether two values are not the same.
    Differ,
    /// A comparison of two numbers.
    Order(Order),
    /// Arithmetic on two numbers.
    Math(Math),
    /// A primitive on a set.
    Set(SetOp),
    /// A primitive on a map.
    Map(MapOp),
    /// A primitive on a sequence or a queue.
    Seq(SeqOp),
}

/// What an [`Expr::Sweep`] does with the test's answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Sweep {
    /// True when every item passes. True for no items.
    Every,
    /// True when at least one item passes. False for no items.
    Some,
    /// The items that pass, in their order.
    Filter,
}

/// The kind of collection an [`Sweep::Filter`] gives back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Shelf {
    /// A set.
    Set,
    /// A sequence or a queue.
    Seq,
}

/// An expression over one state.
///
/// Evaluate it with [`eval`] in a [`Frame`] that holds the state and enough local slots for
/// every `slot` the expression names. To evaluate it in many states, prepare it once as a
/// [`Code`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Expr {
    /// A fixed value.
    Const(Value),
    /// The state field at this position in [`Model::fields`].
    Field(usize),
    /// The local value in this slot: an input, a binding or a test's item.
    Local(usize),
    /// An operation on the values of the arguments, all evaluated first, in order.
    Apply(Op, Vec<Self>),
    /// True when both are true. The second is not evaluated when the first is false.
    And(Box<Self>, Box<Self>),
    /// True when either is true. The second is not evaluated when the first is true.
    Or(Box<Self>, Box<Self>),
    /// The second expression when the first is true, the third otherwise. Only the chosen one
    /// is evaluated.
    Choose(Box<Self>, Box<Self>, Box<Self>),
    /// `body`, evaluated with `value` in `slot`.
    Bind {
        /// The slot `body` reads the value from.
        slot: usize,
        /// The value to bind.
        value: Box<Self>,
        /// The expression that gives the result.
        body: Box<Self>,
    },
    /// A record of these fields.
    Record(Vec<(Sym, Self)>),
    /// The named field of a record.
    Get(Box<Self>, Sym),
    /// A record with some fields replaced or added.
    With(Box<Self>, Vec<(Sym, Self)>),
    /// A set of these items.
    SetOf(Vec<Self>),
    /// A sequence of these items, in order.
    SeqOf(Vec<Self>),
    /// A map of these key and value pairs. A later pair replaces an earlier one with the same
    /// key.
    MapOf(Vec<(Self, Self)>),
    /// A test run on each item of a set or a sequence.
    Sweep {
        /// What to do with the answers.
        sweep: Sweep,
        /// The kind of collection a filter gives back.
        shelf: Shelf,
        /// The slot `body` reads the current item from.
        slot: usize,
        /// The collection whose items are tried.
        source: Box<Self>,
        /// The test, which gives true or false.
        body: Box<Self>,
    },
}

/// One field of a model's state.
#[derive(Clone, Debug)]
pub struct Field {
    /// The name printed in paths and charts.
    pub name: String,
    /// The same name, interned in the model's [`Symbols`].
    pub sym: Sym,
    /// Every value the field declares. A state that holds another value is out of range.
    pub domain: Domain,
    /// The values the field can start at. The start states are every combination across the
    /// fields.
    pub starts: Vec<Value>,
}

/// One input of an action.
#[derive(Clone, Debug)]
pub struct Input {
    /// The name printed in a path step.
    pub name: String,
    /// The local slot the action's expressions read the input from.
    pub slot: usize,
    /// What the input declares.
    pub domain: Domain,
    /// Every value of `domain`, in the order the search tries them.
    pub values: Vec<Value>,
}

/// One action: for each combination of input values, a guard and the fields it changes.
#[derive(Clone, Debug)]
pub struct Action {
    /// The name of the exported constant or the method.
    pub id: String,
    /// The action's sentence, the default arrow label.
    pub label: String,
    /// The line it is declared on, counted from 1.
    pub line: u32,
    /// The inputs, each in its own slot.
    pub inputs: Vec<Input>,
    /// Whether the action can happen in a state with given inputs. It must give a boolean.
    pub guard: Code,
    /// The new value of each state field, by position in [`Model::fields`]. `None` keeps the
    /// field as it is, and so does a list that ends before the field's position.
    pub updates: Vec<Option<Code>>,
    /// How many local slots `guard` and `updates` need.
    pub slots: usize,
}

/// One rule: a condition that must be true in every reachable state.
#[derive(Clone, Debug)]
pub struct Rule {
    /// The name of the exported constant or the method.
    pub id: String,
    /// The rule's sentence.
    pub says: String,
    /// The line it is declared on, counted from 1.
    pub line: u32,
    /// The condition's source text, kept to show a rule change in the report.
    pub text: String,
    /// The condition. It must give a boolean.
    pub holds: Code,
    /// How many local slots `holds` needs.
    pub slots: usize,
}

/// One end: a condition that marks states where stopping is correct.
#[derive(Clone, Debug)]
pub struct End {
    /// The name of the exported constant or the method.
    pub id: String,
    /// The end's sentence.
    pub says: String,
    /// The condition. It must give a boolean.
    pub when: Code,
    /// How many local slots `when` needs.
    pub slots: usize,
}

/// One part of a chart label.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Piece {
    /// Text copied as it is.
    Text(String),
    /// An expression whose value is printed without quotes.
    Shown(Code),
}

/// A chart label computed from a state.
#[derive(Clone, Debug)]
pub struct Label {
    /// The parts, joined without a separator.
    pub pieces: Vec<Piece>,
    /// How many local slots the parts need.
    pub slots: usize,
}

/// How a model wants its chart drawn. Every part is optional.
#[derive(Clone, Debug, Default)]
pub struct View {
    /// The chart's title.
    pub title: Option<String>,
    /// The label of a state. States with the same label and group are drawn as one.
    pub state: Option<Label>,
    /// The label of the box a state is drawn in.
    pub group: Option<Label>,
    /// Arrow labels as action id and label pairs. An action without one uses its sentence.
    pub steps: Vec<(String, String)>,
    /// Colors as pairs of `broken`, `start` or `end` and a CSS color.
    pub colors: Vec<(String, String)>,
}

/// A checked model, ready to search.
#[derive(Clone, Debug)]
pub struct Model {
    /// The model's name.
    pub name: String,
    /// The path of the file it was read from.
    pub file: String,
    /// The line it is declared on, counted from 1.
    pub line: u32,
    /// The names behind every [`Sym`] in the model. Print the model's values with it.
    pub symbols: Symbols,
    /// The state fields. A state is one value per field, in this order.
    pub fields: Vec<Field>,
    /// The actions, in the order the search tries them.
    pub actions: Vec<Action>,
    /// The rules.
    pub rules: Vec<Rule>,
    /// The ends.
    pub ends: Vec<End>,
    /// The view, empty when the model declares none.
    pub view: View,
}
