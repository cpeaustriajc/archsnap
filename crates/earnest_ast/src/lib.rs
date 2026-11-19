//! The language-neutral tree both readers produce.
//!
//! A reader turns one model file into a [`Reading`]: a [`Draft`] for each model it found and a
//! `Problem` for each thing it could not read. A draft holds untyped [`Term`]s and [`Shape`]s.
//! Names of methods and properties are always `snake_case` here, so a TypeScript reader turns
//! `isEmpty` into `is_empty` and a property read into a [`TermKind::Member`].

use earnest_diagnostics::{Place, Problem};

/// An operator with one operand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Unary {
    /// `!x` on a boolean.
    Not,
    /// `-x` on a number.
    Negate,
}

/// An operator with two operands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Binary {
    /// `&&`. The right side is not evaluated when the left is false.
    And,
    /// `||`. The right side is not evaluated when the left is true.
    Or,
    /// `===` in TypeScript, `==` in Rust.
    Equal,
    /// `!==` in TypeScript, `!=` in Rust.
    Differ,
    /// `<` on numbers.
    Less,
    /// `<=` on numbers.
    LessOrEqual,
    /// `>` on numbers.
    Greater,
    /// `>=` on numbers.
    GreaterOrEqual,
    /// `+` on numbers.
    Add,
    /// `-` on numbers.
    Subtract,
    /// `*` on numbers.
    Multiply,
    /// `%` on numbers. A negative left side or a right side below 1 is a fault.
    Remainder,
}

/// How many values of a declaration must pass the test of a [`TermKind::Across`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Reach {
    /// `forall`: all of them.
    Every,
    /// `exists`: at least one.
    Some,
}

/// One expression of a model file, with where it starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Term {
    /// Where the expression starts. A problem with this term is reported here.
    pub place: Place,
    /// What the expression is.
    pub kind: TermKind,
}

/// One part of a template string.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TemplatePiece {
    /// Literal text, copied to the label as it is.
    Text(String),
    /// A `${...}` hole. Its value is printed without quotes.
    Value(Term),
}

/// What a [`Term`] is.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TermKind {
    /// `null`, or `None` in Rust.
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// A whole number.
    Int(i64),
    /// Text in quotes, or an enum variant by its name: `Gate::Locked` is `Text("Locked")`.
    Text(String),
    /// A name: a parameter, a `const` or `let` binding, an input, or a top-level constant.
    Name(String),
    /// A read of a field or a property without arguments: `s.cart`, `queue.is_empty`.
    Member(Box<Term>, String),
    /// A primitive called on a value: `target.method(arguments)`.
    Call {
        /// The value the primitive is called on.
        target: Box<Term>,
        /// The primitive's name in `snake_case`.
        method: String,
        /// The arguments in order. A test function is a [`TermKind::Lambda`].
        arguments: Vec<Term>,
    },
    /// A call to a helper function the model file defines.
    Helper {
        /// The helper's name, matching a [`HelperDraft`].
        name: String,
        /// The arguments in order. Passing the whole state is allowed here.
        arguments: Vec<Term>,
    },
    /// An operator with one operand.
    Unary(Unary, Box<Term>),
    /// An operator with two operands, left then right.
    Binary(Binary, Box<Term>, Box<Term>),
    /// `test ? yes : no`, or an `if` that returns early followed by the rest of the body.
    Choose(Box<Term>, Box<Term>, Box<Term>),
    /// A record literal. As the result of an action it lists the fields that change.
    Record {
        /// The record that is spread in: `...r` in TypeScript, `..r.clone()` in Rust.
        base: Option<Box<Term>>,
        /// The fields written out, in source order.
        fields: Vec<(String, Term)>,
    },
    /// `m.setOf(a, b)` or `Set::of([a, b])`.
    SetOf(Vec<Term>),
    /// `m.seqOf(a, b)` or `Seq::of([a, b])`.
    SeqOf(Vec<Term>),
    /// `m.mapOf([k, v])` or `Map::of([(k, v)])`, as key and value pairs.
    MapOf(Vec<(Term, Term)>),
    /// A function passed to `every`, `some`, `filter` or `is_some_and`. Anywhere else it is
    /// refused.
    Lambda {
        /// The parameter names. The primitives take exactly one.
        parameters: Vec<String>,
        /// The expression the function gives back.
        body: Box<Term>,
    },
    /// A `const` or `let` binding followed by the rest of the function body.
    Bind {
        /// The bound name, visible in `body` only.
        name: String,
        /// The bound value.
        value: Box<Term>,
        /// The rest of the body.
        body: Box<Term>,
    },
    /// `Some(x)` in Rust. An action's `Some(Self { .. })` is also read as this.
    Wrap(Box<Term>),
    /// `m.forall(T, f)` or `m.exists(T, f)` over every value of a declaration.
    Across {
        /// Whether all values or at least one must pass.
        reach: Reach,
        /// The declaration whose values are tried.
        shape: Box<Shape>,
        /// The name of the function's parameter.
        parameter: String,
        /// The test, which gives true or false.
        body: Box<Term>,
    },
    /// A template string. It is allowed only as a view's state or group label.
    Template(Vec<TemplatePiece>),
}

impl Term {
    /// A term of `kind` that starts at `place`.
    #[must_use]
    pub fn new(place: Place, kind: TermKind) -> Self {
        Self { place, kind }
    }
}

/// One declaration: what values a field, an input or a collection item can take.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shape {
    /// Where the declaration starts.
    pub place: Place,
    /// What is declared.
    pub kind: ShapeKind,
    /// The value given with `.initial(value)` or `#[initial(value)]`. It has a meaning only on
    /// the shape of a state field.
    pub initial: Option<Term>,
}

/// What a [`Shape`] declares.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ShapeKind {
    /// `m.boolean()` or `bool`.
    Bool,
    /// `m.int(lo, hi)` or `#[range(lo, hi)]`: the whole numbers from `lo` to `hi`, both
    /// included.
    Int {
        /// The smallest number.
        lo: i64,
        /// The largest number.
        hi: i64,
    },
    /// `m.enum([...])`, `m.oneOf([...])` or a Rust enum. Each choice is a [`TermKind::Text`], a
    /// [`TermKind::Int`] or a [`TermKind::Bool`], all of one kind.
    OneOf(Vec<Term>),
    /// `.optional()` or `Option<T>`.
    Optional(Box<Shape>),
    /// `m.record({...})` or a Rust struct, as its fields in source order.
    Record(Vec<(String, Shape)>),
    /// `m.set(T)` or `Set<T>`.
    Set(Box<Shape>),
    /// `m.map(K, V)` or `Map<K, V>`, as the key's shape then the value's.
    Map(Box<Shape>, Box<Shape>),
    /// `m.seq(T, { max })` or `Seq<T, MAX>`.
    Seq {
        /// The shape of each item.
        item: Box<Shape>,
        /// The most items the sequence holds before the state is out of range.
        max: usize,
    },
    /// `m.queue(T, { capacity })` or `Queue<T, CAPACITY>`.
    Queue {
        /// The shape of each message.
        item: Box<Shape>,
        /// The most messages the queue holds before the state is out of range.
        capacity: usize,
    },
    /// `m.lock()` or `Lock` without an owner, `m.lock(Owner)` or `Lock<Owner>` with one.
    Lock(Option<Box<Shape>>),
    /// `m.timer()` or `Timer`.
    Timer,
    /// `m.request()` or `Request`.
    Request,
    /// `m.retry({ max })` or `Retry<MAX>`.
    Retry {
        /// The most attempts before the state is out of range.
        max: i64,
    },
}

/// One field of a model's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldDraft {
    /// The field's name, as rules and actions read it.
    pub name: String,
    /// What the field can hold and, with `initial`, where it starts.
    pub shape: Shape,
}

/// One action of a model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionDraft {
    /// The name of the exported constant or the method. It names the action in every output.
    pub id: String,
    /// The action's sentence, such as "the visitor pays".
    pub label: String,
    /// Where the action is declared.
    pub place: Place,
    /// Each input's name and declaration. The checker tries every combination of their values.
    pub inputs: Vec<(String, Shape)>,
    /// The name the functions give the state (`s`, `self`), when they take it.
    pub state_name: Option<String>,
    /// The name the functions give the whole input object, when it is not destructured. A
    /// destructured or Rust input is read by its own name instead.
    pub input_name: Option<String>,
    /// The `when` condition, with every `require!` joined in. `None` means always.
    pub guard: Option<Term>,
    /// The `then` result: a [`TermKind::Record`] of the fields that change, possibly inside
    /// [`TermKind::Bind`], [`TermKind::Choose`] or [`TermKind::Wrap`].
    pub change: Term,
}

/// One rule of a model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleDraft {
    /// The name of the exported constant or the method.
    pub id: String,
    /// The rule's sentence.
    pub says: String,
    /// Where the rule is declared.
    pub place: Place,
    /// The name the condition gives the state, when it takes it.
    pub state_name: Option<String>,
    /// The condition that must be true in every reachable state.
    pub holds: Term,
    /// The condition's source text, kept to show a rule change in the report.
    pub text: String,
}

/// One end of a model: states where stopping is correct.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndDraft {
    /// The name of the exported constant or the method.
    pub id: String,
    /// The end's sentence.
    pub says: String,
    /// Where the end is declared.
    pub place: Place,
    /// The name the condition gives the state, when it takes it.
    pub state_name: Option<String>,
    /// The condition that marks a state as an end.
    pub when: Term,
}

/// A label function of a view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LabelDraft {
    /// The name the function gives the state, when it takes it.
    pub state_name: Option<String>,
    /// A [`TermKind::Template`], or any term whose value is printed as the whole label.
    pub term: Term,
}

/// How a model wants its chart drawn. Every part is optional.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ViewDraft {
    /// The chart's title.
    pub title: Option<String>,
    /// The label of a state. States with the same label and group are drawn as one.
    pub state: Option<LabelDraft>,
    /// The label of the box a state is drawn in.
    pub group: Option<LabelDraft>,
    /// Arrow labels as action id and label pairs. An action without one uses its sentence.
    pub steps: Vec<(String, String)>,
    /// Colors as pairs of `broken`, `start` or `end` and a CSS color.
    pub colors: Vec<(String, String)>,
}

/// A helper function a model file defines at its top level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelperDraft {
    /// The name it is called by.
    pub name: String,
    /// The parameter names in order.
    pub parameters: Vec<String>,
    /// The expression it gives back, with statements folded into [`TermKind::Bind`] and
    /// [`TermKind::Choose`].
    pub body: Term,
}

/// One model as a reader found it, before any name or kind is checked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
    /// The model's name.
    pub name: String,
    /// The path of the file it was read from, used in every problem.
    pub file: String,
    /// Where the model is declared.
    pub place: Place,
    /// The state fields in declaration order. That order is the order of every printed state.
    pub fields: Vec<FieldDraft>,
    /// The actions in file order.
    pub actions: Vec<ActionDraft>,
    /// The rules in file order.
    pub rules: Vec<RuleDraft>,
    /// The ends in file order.
    pub ends: Vec<EndDraft>,
    /// The view, empty when the file declares none.
    pub view: ViewDraft,
    /// The file's top-level constants by name. A [`TermKind::Name`] no binding explains is
    /// looked up here.
    pub constants: Vec<(String, Term)>,
    /// The file's helper functions.
    pub helpers: Vec<HelperDraft>,
}

/// Everything a reader got out of one model file.
///
/// When `problems` is not empty the file is unreadable and none of its drafts may be checked.
#[derive(Clone, Debug, Default)]
pub struct Reading {
    /// The models found, in file order.
    pub drafts: Vec<Draft>,
    /// What the reader refused.
    pub problems: Vec<Problem>,
}
