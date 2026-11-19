use serde::Serialize;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Place {
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Problem {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub feature: String,
    pub message: String,
}

impl Problem {
    #[must_use]
    pub fn new(file: &str, place: Place, feature: &str, message: &str) -> Self {
        Self {
            file: file.to_owned(),
            line: place.line,
            column: place.column,
            feature: feature.to_owned(),
            message: message.to_owned(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unary {
    Not,
    Negate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Binary {
    And,
    Or,
    Equal,
    Differ,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    Add,
    Subtract,
    Multiply,
    Remainder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reach {
    Every,
    Some,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Term {
    pub place: Place,
    pub kind: TermKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TemplatePiece {
    Text(String),
    Value(Term),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TermKind {
    Null,
    Bool(bool),
    Int(i64),
    Text(String),
    Name(String),
    Member(Box<Term>, String),
    Call {
        target: Box<Term>,
        method: String,
        arguments: Vec<Term>,
    },
    Helper {
        name: String,
        arguments: Vec<Term>,
    },
    Unary(Unary, Box<Term>),
    Binary(Binary, Box<Term>, Box<Term>),
    Choose(Box<Term>, Box<Term>, Box<Term>),
    Record {
        base: Option<Box<Term>>,
        fields: Vec<(String, Term)>,
    },
    SetOf(Vec<Term>),
    SeqOf(Vec<Term>),
    MapOf(Vec<(Term, Term)>),
    Lambda {
        parameters: Vec<String>,
        body: Box<Term>,
    },
    Bind {
        name: String,
        value: Box<Term>,
        body: Box<Term>,
    },
    Wrap(Box<Term>),
    Across {
        reach: Reach,
        shape: Box<Shape>,
        parameter: String,
        body: Box<Term>,
    },
    Template(Vec<TemplatePiece>),
}

impl Term {
    #[must_use]
    pub fn new(place: Place, kind: TermKind) -> Self {
        Self { place, kind }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shape {
    pub place: Place,
    pub kind: ShapeKind,
    pub initial: Option<Term>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Bool,
    Int { lo: i64, hi: i64 },
    OneOf(Vec<Term>),
    Optional(Box<Shape>),
    Record(Vec<(String, Shape)>),
    Set(Box<Shape>),
    Map(Box<Shape>, Box<Shape>),
    Seq { item: Box<Shape>, max: usize },
    Queue { item: Box<Shape>, capacity: usize },
    Lock(Option<Box<Shape>>),
    Timer,
    Request,
    Retry { max: i64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldDraft {
    pub name: String,
    pub shape: Shape,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionDraft {
    pub id: String,
    pub label: String,
    pub place: Place,
    pub inputs: Vec<(String, Shape)>,
    pub state_name: Option<String>,
    pub input_name: Option<String>,
    pub guard: Option<Term>,
    pub change: Term,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleDraft {
    pub id: String,
    pub says: String,
    pub place: Place,
    pub state_name: Option<String>,
    pub holds: Term,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndDraft {
    pub id: String,
    pub says: String,
    pub place: Place,
    pub state_name: Option<String>,
    pub when: Term,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LabelDraft {
    pub state_name: Option<String>,
    pub term: Term,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ViewDraft {
    pub title: Option<String>,
    pub state: Option<LabelDraft>,
    pub group: Option<LabelDraft>,
    pub steps: Vec<(String, String)>,
    pub colors: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelperDraft {
    pub name: String,
    pub parameters: Vec<String>,
    pub body: Term,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
    pub name: String,
    pub file: String,
    pub place: Place,
    pub fields: Vec<FieldDraft>,
    pub actions: Vec<ActionDraft>,
    pub rules: Vec<RuleDraft>,
    pub ends: Vec<EndDraft>,
    pub view: ViewDraft,
    pub constants: Vec<(String, Term)>,
    pub helpers: Vec<HelperDraft>,
}

#[derive(Clone, Debug, Default)]
pub struct Reading {
    pub drafts: Vec<Draft>,
    pub problems: Vec<Problem>,
}
