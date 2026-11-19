//! The language-neutral tree both readers produce.
//!
//! A reader turns one model file into a [`Reading`]: a [`Draft`] for each model it found and a
//! `Problem` for each thing it could not read. A draft holds untyped [`Term`]s and [`Shape`]s.
//! The name of a called method is always `snake_case` here, so a TypeScript reader turns
//! `isHeldBy(o)` into `is_held_by`. A read without arguments is a [`TermKind::Member`] with the
//! name as the file writes it, because a reader can't tell a record's field from a primitive's
//! property: the lowering decides by the kind of the value that is read.

use earnest_diagnostics::{Place, Problem};
use earnest_value::IntType;

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
    /// A read of a field or a property without arguments, by the name as the file writes it:
    /// `s.cart`, and `queue.isEmpty` from TypeScript.
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
        /// Whether a function of the file or a method of the model is called. The two may
        /// share a name.
        kind: HelperKind,
        /// The helper's name, matching a [`HelperDraft`] of the same kind.
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
    /// A `const` or `let` binding followed by the rest of the function body. In a draft read
    /// from Rust, a value that holds a number nothing gives an integer type is refused: wrap
    /// it in a [`TermKind::Declared`].
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
    /// A value where the file writes its type: in Rust a `let` with a type, a constant, a
    /// parameter or the result of a helper, and a number with a suffix such as `1u8`.
    ///
    /// The value must be of the shape's kind and each number in it gets the shape's integer
    /// type. The bounds of a number range in the shape are not checked.
    Declared {
        /// The type, as a declaration.
        shape: Box<Shape>,
        /// The value that has it.
        value: Box<Term>,
    },
}

/// What a helper is. A Rust file may have a function and a method with one name, and
/// `pick(x)` calls the first where `self.pick(x)` calls the second.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum HelperKind {
    /// A function at the top of the file.
    Function,
    /// A method of the model without `#[action]`, `#[rule]` or `#[end]`. Its first parameter
    /// is the state.
    Method,
}

/// The language a draft was read from, for the two things that mean something else in each.
///
/// Everything else in a draft means the same whichever reader made it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Language {
    /// A number is one kind of value held in 64 bits, and `null` is one value: an optional
    /// read from a collection of optional values is absent or present, never both.
    TypeScript,
    /// Every number has an integer type and a result outside its type is a fault. A number
    /// no declaration gives a type is an `i32`, a size is a `usize`, and a `let` must say the
    /// type of a number that nothing else does. `Some(None)` is another value than `None`,
    /// which the checker can't hold, so an optional read from a collection of optional values
    /// is refused.
    Rust,
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
        /// The integer type the file gives the number, which every result computed from it
        /// must stay inside. `None` in TypeScript.
        ty: Option<IntType>,
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

/// One function of an action, with the names that function itself brings into scope.
///
/// The `when` and the `then` of one action may take different names. A name in `term` that
/// none of these explains, and no binding inside `term` either, is a top-level constant, even
/// when the action declares an input with that name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionBody {
    /// The name this function gives the state (`s`, `self`), when it takes it.
    pub state_name: Option<String>,
    /// The name this function gives the whole input object, when it takes it without picking
    /// inputs out of it. An input is then read as a field of this name.
    pub input_name: Option<String>,
    /// The inputs this function takes by their own names. In `term` each is a
    /// [`TermKind::Name`] with the name the action declares the input by.
    pub taken: Vec<String>,
    /// The function's result.
    pub term: Term,
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
    /// The `when` condition, with every `require!` joined in. `None` means always.
    pub guard: Option<ActionBody>,
    /// The `then` result. Its term is a [`TermKind::Record`] of the fields that change,
    /// possibly inside [`TermKind::Bind`], [`TermKind::Choose`] or [`TermKind::Wrap`].
    pub change: ActionBody,
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
    /// The condition's source text in the normal form of [`condition_text`], kept to show a
    /// rule change in the report and to tell whether the condition was rewritten.
    pub text: String,
}

/// The normal form of a condition's source text, the same for every reader: each run of white
/// space is one space and there is none at either end.
///
/// A reader passes the source of a rule's condition through this before it stores it in
/// [`RuleDraft::text`], so an edit that only moves white space does not count as a change of
/// the rule. White space inside a text literal is folded too, so two texts can be equal when
/// the conditions are not.
#[must_use]
pub fn condition_text(source: &str) -> String {
    let words: Vec<&str> = source.split_whitespace().collect();
    words.join(" ")
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

/// A helper function a model file defines: a function at its top level, or in Rust a method
/// of the model that is no action, rule or end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelperDraft {
    /// Whether it is a function of the file or a method of the model. A call finds the helper
    /// with its own kind and name.
    pub kind: HelperKind,
    /// The name it is called by.
    pub name: String,
    /// The parameter names in order. A method's first parameter is the state.
    pub parameters: Vec<String>,
    /// The expression it gives back, with statements folded into [`TermKind::Bind`] and
    /// [`TermKind::Choose`]. Where the file writes the type of a parameter or of the result,
    /// the body binds the parameter again as a [`TermKind::Declared`] value and is itself one.
    pub body: Term,
}

/// One model as a reader found it, before any name or kind is checked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
    /// The model's name.
    pub name: String,
    /// The path of the file it was read from, used in every problem.
    pub file: String,
    /// The language the file is written in.
    pub language: Language,
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
