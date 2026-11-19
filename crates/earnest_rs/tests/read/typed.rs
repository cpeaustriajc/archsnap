use crate::{actions, drafts, only_draft, search};
use earnest_ast::{HelperKind, ShapeKind, TermKind};
use earnest_check::{FindingKind, Outcome, Status, Verdict};
use earnest_rs::read;
use earnest_semantic::lower;
use earnest_value::IntType;

const SHELF: &str = include_str!("models/shelf.rs");

fn verdict(outcome: &Outcome, id: &str) -> Verdict {
    outcome
        .rules
        .iter()
        .find(|rule| rule.id == id)
        .unwrap_or_else(|| panic!("the rule `{id}` is in the outcome"))
        .verdict
}

fn steps_to(outcome: &Outcome, id: &str) -> Vec<String> {
    let rule = outcome
        .rules
        .iter()
        .find(|rule| rule.id == id)
        .unwrap_or_else(|| panic!("the rule `{id}` is in the outcome"));
    let path = rule
        .path
        .as_ref()
        .unwrap_or_else(|| panic!("the rule `{id}` has a path"));
    actions(path).into_iter().map(str::to_owned).collect()
}

fn the_fault(outcome: &Outcome) -> (&str, usize) {
    let faults: Vec<_> = outcome
        .findings
        .iter()
        .filter(|finding| finding.kind == FindingKind::Fault)
        .collect();
    assert_eq!(faults.len(), 1, "one fault in {:?}", outcome.findings);
    (faults[0].message.as_str(), faults[0].path.steps.len())
}

fn refused_places(source: &str) -> Vec<(u32, u32, String)> {
    let reading = read("src/case.rs", source);
    assert_eq!(reading.drafts, [], "an unreadable file gives no model");
    reading
        .problems
        .into_iter()
        .map(|problem| (problem.line, problem.column, problem.feature))
        .collect()
}

fn refused_messages(source: &str) -> Vec<String> {
    read("src/case.rs", source)
        .problems
        .into_iter()
        .map(|problem| problem.message)
        .collect()
}

fn lowering_refusals(source: &str) -> Vec<String> {
    let draft = only_draft("src/case.rs", source);
    match lower(&draft) {
        Ok(_) => panic!("the model should not lower"),
        Err(problems) => problems
            .into_iter()
            .map(|problem| problem.feature)
            .collect(),
    }
}

fn counter(ty: &str, range: &str, start: &str, methods: &str) -> String {
    format!(
        "use earnest::model::{{Model, Seq, model, require}};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Count {{
    #[range({range})]
    #[initial({start})]
    n: {ty},
    items: Seq<bool, 2>,
}}

#[model]
impl Count {{
{methods}
    #[action(\"the count goes up\")]
    fn up(&self) -> Option<Self> {{
        require!(self.n < 3);
        Some(Self {{ n: self.n + 1, ..self.clone() }})
    }}

    #[end(\"The count is three\")]
    fn top(&self) -> bool {{
        self.n == 3
    }}
}}
"
    )
}

fn small(methods: &str) -> String {
    counter("u8", "0, 3", "0", methods)
}

#[test]
fn the_shelf_model_gives_the_result_worked_out_by_hand() {
    let outcome = search(&only_draft("shelf.rs", SHELF));
    assert_eq!(
        outcome.states, 7,
        "no book, one book of two kinds, two books of four kinds"
    );
    assert_eq!(outcome.status, Status::Fail, "three rules are broken");
    assert_eq!(
        verdict(&outcome, "outside"),
        Verdict::Holds,
        "`pick(Cover::Hard, Cover::Soft)` is the function, which gives its second argument"
    );
    assert_eq!(
        verdict(&outcome, "inside"),
        Verdict::Broken,
        "`self.pick(Cover::Soft)` is the method, which gives `Hard` once a book is shelved"
    );
    assert_eq!(steps_to(&outcome, "inside"), ["shelve"], "the first book");
    assert_eq!(
        verdict(&outcome, "low"),
        Verdict::Broken,
        "the count of two is not below `double(1)`"
    );
    assert_eq!(steps_to(&outcome, "low"), ["shelve", "shelve"], "two books");
    assert_eq!(
        verdict(&outcome, "spare"),
        Verdict::Broken,
        "two books less one is not none"
    );
    assert_eq!(steps_to(&outcome, "spare"), ["shelve", "shelve"]);
    assert_eq!(
        the_fault(&outcome),
        (
            "the rule `spare` computes a number outside `usize` in this state",
            0
        ),
        "with no book, the size less one is below zero, where the file's own test panics"
    );
}

#[test]
fn a_method_and_a_function_of_one_name_are_two_helpers() {
    let draft = only_draft("shelf.rs", SHELF);
    let named: Vec<(HelperKind, &str, usize)> = draft
        .helpers
        .iter()
        .map(|helper| (helper.kind, helper.name.as_str(), helper.parameters.len()))
        .collect();
    assert_eq!(
        named,
        [
            (HelperKind::Function, "pick", 2),
            (HelperKind::Function, "double", 1),
            (HelperKind::Method, "pick", 2),
            (HelperKind::Method, "below", 2),
        ],
        "the functions of the file, then the methods of the model, each with `self` counted"
    );
}

#[test]
fn a_helper_method_takes_a_number_without_a_range_and_an_action_still_needs_one() {
    let helper = small(
        "    fn below(&self, limit: u8) -> bool {
        self.n < limit
    }

    #[rule(\"The count is below two\")]
    fn low(&self) -> bool {
        self.below(2)
    }
",
    );
    let outcome = search(&only_draft("count.rs", &helper));
    assert_eq!(outcome.states, 4, "the count goes 0, 1, 2, 3");
    assert_eq!(
        steps_to(&outcome, "low"),
        ["up", "up"],
        "two is not below two"
    );
    let action = small(
        "    #[action(\"the count is set\")]
    fn set(&self, to: u8) -> Option<Self> {
        Some(Self { n: to, ..self.clone() })
    }
",
    );
    assert_eq!(
        refused_places(&action),
        [(14, 23, "`u8`".to_owned())],
        "an action is tried with every value of its input, so the input needs a range"
    );
}

#[test]
fn a_number_a_helper_is_given_must_fit_the_type_of_its_parameter() {
    let source = small(
        "    fn twice(&self, number: u8) -> u8 {
        number + number
    }

    #[rule(\"Twice two hundred is large\")]
    fn large(&self) -> bool {
        self.twice(200) > 100
    }
",
    );
    let outcome = search(&only_draft("count.rs", &source));
    assert_eq!(verdict(&outcome, "large"), Verdict::Unknown);
    assert_eq!(
        the_fault(&outcome),
        (
            "the rule `large` computes a number outside `u8` in this state",
            0
        ),
        "200 + 200 is no `u8`"
    );
}

#[test]
fn each_model_calls_its_own_method_of_a_shared_name() {
    let source = "use earnest::model::{Model, model};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Porch {
    #[initial(false)]
    lit: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Hall {
    #[initial(false)]
    lit: bool,
}

#[model]
impl Porch {
    fn fine(&self) -> bool {
        self.lit
    }

    #[rule(\"The porch is fine\")]
    fn kept(&self) -> bool {
        self.fine()
    }
}

#[model]
impl Hall {
    fn fine(&self) -> bool {
        !self.lit
    }

    #[rule(\"The hall is fine\")]
    fn kept(&self) -> bool {
        self.fine()
    }
}
";
    let found = drafts("lights.rs", source);
    assert_eq!(found.len(), 2, "two models");
    assert_eq!(
        verdict(&search(&found[0]), "kept"),
        Verdict::Broken,
        "the porch is dark, and its `fine` wants it lit"
    );
    assert_eq!(
        verdict(&search(&found[1]), "kept"),
        Verdict::Holds,
        "the hall is dark, and its `fine` wants it dark"
    );
}

#[test]
fn a_method_named_like_a_constant_and_a_closure_parameter_named_like_a_function_keep_apart() {
    let source = "use earnest::model::{Model, Set, model};

const two: u8 = 2;

fn same(_given: bool) -> bool {
    false
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Pair {
    #[range(0, 3)]
    #[initial(1)]
    n: u8,
    #[initial(Set::of([true]))]
    flags: Set<bool>,
}

#[model]
impl Pair {
    fn two(&self) -> u8 {
        self.n + 2
    }

    #[rule(\"The method is above the constant\")]
    fn over(&self) -> bool {
        self.two() > two
    }

    #[rule(\"Every flag is its own value\")]
    fn plain(&self) -> bool {
        self.flags.every(|same| same)
    }

    #[rule(\"The function gives false\")]
    fn called(&self) -> bool {
        !same(true)
    }
}
";
    let outcome = search(&only_draft("pair.rs", source));
    assert_eq!(outcome.states, 1, "nothing can happen");
    assert_eq!(
        verdict(&outcome, "over"),
        Verdict::Holds,
        "`self.two()` is the method, 3, and `two` is the constant, 2"
    );
    assert_eq!(
        verdict(&outcome, "plain"),
        Verdict::Holds,
        "`same` in the closure is the flag `true`, not the function that gives `false`"
    );
    assert_eq!(
        verdict(&outcome, "called"),
        Verdict::Holds,
        "`same(true)` is the function"
    );
}

#[test]
fn an_option_held_in_a_map_a_sequence_or_a_queue_is_refused_where_it_is_written() {
    let source = "use earnest::model::{Map, Model, Queue, Seq, Set, model};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Gaps {
    notes: Map<bool, Option<bool>>,
    items: Seq<Option<bool>, 1>,
    inbox: Queue<Option<bool>, 1>,
    seen: Set<Option<bool>>,
    keyed: Map<Option<bool>, bool>,
    last: Option<bool>,
}
";
    assert_eq!(
        refused_places(source),
        [
            (5, 22, "`Option<bool>`".to_owned()),
            (6, 16, "`Option<bool>`".to_owned()),
            (7, 18, "`Option<bool>`".to_owned()),
        ],
        "a set of options, a map keyed by an option and an optional field are read"
    );
    let messages = refused_messages(source);
    assert!(
        messages[0].starts_with("a `Map` can't hold an `Option`: `get()` would give `Some(None)`"),
        "{}",
        messages[0]
    );
    assert!(
        messages[1].starts_with("a `Seq` can't hold an `Option`: `first()` would give"),
        "{}",
        messages[1]
    );
    assert!(
        messages[2].starts_with("a `Queue` can't hold an `Option`: `head()` would give"),
        "{}",
        messages[2]
    );
}

#[test]
fn a_lock_owned_by_an_option_and_a_lock_with_an_owner_in_a_map_are_refused() {
    let head = "use earnest::model::{Domain, Lock, Map, Model, model};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
pub enum Who {
    Ada,
    Bo,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Doors {
";
    let optional_owner = format!("{head}    door: Lock<Option<Who>>,\n}}\n");
    assert_eq!(
        lowering_refusals(&optional_owner),
        ["a lock owned by an optional value"],
        "`holder()` would give `Some(None)`"
    );
    let in_a_map = format!("{head}    doors: Map<Who, Lock<Who>>,\n}}\n");
    assert_eq!(
        lowering_refusals(&in_a_map),
        ["a lock with an owner as a map value"],
        "`get(who)` of a free lock could not be told from no lock"
    );
    let plain = format!("{head}    doors: Map<Who, Lock>,\n    door: Lock<Who>,\n}}\n");
    assert_eq!(
        search(&only_draft("doors.rs", &plain)).states,
        1,
        "a lock without an owner in a map, and a lock with one as a field, are read"
    );
}

#[test]
fn a_range_must_lie_inside_the_integer_type_of_its_number() {
    let above = counter("u8", "0, 300", "0", "");
    assert_eq!(
        refused_places(&above),
        [(5, 5, "`#[range(0, 300)]`".to_owned())],
        "300 is no `u8`"
    );
    assert_eq!(
        refused_messages(&above),
        ["a `u8` holds 0 to 255, so the range must lie inside that"]
    );
    let below = counter("u16", "-1, 3", "0", "");
    assert_eq!(
        refused_places(&below),
        [(5, 5, "`#[range(-1, 3)]`".to_owned())],
        "-1 is no `u16`"
    );
    let signed = counter("i8", "-128, 127", "0", "");
    let draft = only_draft("count.rs", &signed);
    assert_eq!(
        draft.fields[0].shape.kind,
        ShapeKind::Int {
            lo: -128,
            hi: 127,
            ty: Some(IntType::I8),
        },
        "the whole of an `i8`"
    );
}

#[test]
fn a_number_below_zero_of_a_type_without_a_sign_is_a_fault_where_it_is_computed() {
    let source = small(
        "    #[rule(\"One below the count is at most two\")]
    fn below(&self) -> bool {
        self.n - 1 <= 2
    }
",
    );
    let outcome = search(&only_draft("count.rs", &source));
    assert_eq!(outcome.states, 4, "the count goes 0, 1, 2, 3");
    assert_eq!(outcome.status, Status::Fail, "a fault fails the model");
    assert_eq!(
        verdict(&outcome, "below"),
        Verdict::Unknown,
        "true at 1, 2 and 3, and without a value at 0"
    );
    assert_eq!(
        the_fault(&outcome),
        (
            "the rule `below` computes a number outside `u8` in this state",
            0
        ),
        "0 - 1 in the start state"
    );
}

#[test]
fn a_number_above_the_largest_of_its_type_is_a_fault_also_for_a_type_with_a_sign() {
    let source = counter(
        "i8",
        "0, 127",
        "125",
        "    #[action(\"the count jumps\")]
    fn jump(&self) -> Option<Self> {
        require!(self.n < 127);
        Some(Self { n: self.n + 1, ..self.clone() })
    }

    #[rule(\"The next count is above zero\")]
    fn next(&self) -> bool {
        self.n + 1 > 0
    }
",
    );
    let outcome = search(&only_draft("count.rs", &source));
    assert_eq!(outcome.states, 3, "125, 126 and 127");
    assert_eq!(
        the_fault(&outcome),
        (
            "the rule `next` computes a number outside `i8` in this state",
            2
        ),
        "127 + 1 after two jumps"
    );
}

#[test]
fn a_let_must_state_the_type_of_a_number_nothing_else_types() {
    let untyped = small(
        "    #[rule(\"The count is at most three\")]
    fn capped(&self) -> bool {
        let most = 3;
        self.n <= most
    }
",
    );
    assert_eq!(
        lowering_refusals(&untyped),
        ["`let most` with a number of no stated type"],
        "Rust would make `most` a `u8` here, and an `i32` elsewhere"
    );
    let typed = untyped.replace("let most = 3;", "let most: u8 = 3;");
    assert_eq!(
        verdict(&search(&only_draft("count.rs", &typed)), "capped"),
        Verdict::Holds
    );
    let from_a_field = untyped.replace("let most = 3;", "let most = self.n + 3;");
    assert_eq!(
        verdict(&search(&only_draft("count.rs", &from_a_field)), "capped"),
        Verdict::Holds,
        "a number computed from a field has the field's type"
    );
}

#[test]
fn numbers_of_two_integer_types_do_not_mix() {
    let source = small(
        "    #[rule(\"The count is the size\")]
    fn sized(&self) -> bool {
        self.n == self.items.size()
    }
",
    );
    assert_eq!(
        lowering_refusals(&source),
        ["a `u8` number with a `usize` number"],
        "a size is a `usize`, and Rust does not compare it with a `u8`"
    );
}

#[test]
fn a_constant_a_typed_let_and_a_number_with_a_suffix_carry_their_type() {
    let source = small(
        "    #[rule(\"The count is at most three\")]
    fn capped(&self) -> bool {
        let most: u8 = 3;
        self.n <= most && self.n < 4_u8
    }
",
    )
    .replace(
        "use earnest::model::{Model, Seq, model, require};\n",
        "use earnest::model::{Model, Seq, model, require};\n\nconst TOP: u16 = 9;\n",
    );
    let draft = only_draft("count.rs", &source);
    let declared_type = |kind: &TermKind| -> Option<(Option<IntType>, TermKind)> {
        let TermKind::Declared { shape, value } = kind else {
            return None;
        };
        let ShapeKind::Int { ty, .. } = shape.kind else {
            return None;
        };
        Some((ty, value.kind.clone()))
    };
    assert_eq!(
        declared_type(&draft.constants[0].1.kind),
        Some((Some(IntType::U16), TermKind::Int(9))),
        "the constant has the type it is declared with"
    );
    let TermKind::Bind { name, value, body } = &draft.rules[0].holds.kind else {
        panic!("a `let` is a binding: {:?}", draft.rules[0].holds);
    };
    assert_eq!(name, "most", "the bound name");
    assert_eq!(
        declared_type(&value.kind),
        Some((Some(IntType::U8), TermKind::Int(3))),
        "the `let` has the type it states"
    );
    let TermKind::Binary(_, _, right) = &body.kind else {
        panic!("the body is the `&&`: {body:?}");
    };
    let TermKind::Binary(_, _, four) = &right.kind else {
        panic!("its right side is the `<`: {right:?}");
    };
    assert_eq!(
        declared_type(&four.kind),
        Some((Some(IntType::U8), TermKind::Int(4))),
        "`4_u8` has the type of its suffix"
    );
}

#[test]
fn a_function_named_like_a_call_the_library_owns_is_refused() {
    let source = "use earnest::model::{Model, model};

fn forall(given: u8) -> u8 {
    given
}

fn exists(given: u8) -> u8 {
    given
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Lamp {
    #[initial(false)]
    lit: bool,
}
";
    assert_eq!(
        refused_places(source),
        [
            (3, 4, "`fn forall`".to_owned()),
            (7, 4, "`fn exists`".to_owned()),
        ],
        "`forall(...)` in a body is always read as the library's"
    );
    assert_eq!(
        refused_messages(source)[0],
        "the checker reads a call of `forall` as the library's, so a function of the file \
         can't have this name"
    );
}
