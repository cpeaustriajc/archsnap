use crate::{only_draft, search};
use earnest_ast::{TemplatePiece, TermKind};
use earnest_rs::read;
use earnest_semantic::lower;

const CART: &str = include_str!("../../../../tasks/conformance/twins/cart.rs");
const DIAL: &str = include_str!("../../../../tasks/conformance/twins/dial.rs");

fn refused(source: &str) -> Vec<(String, String)> {
    let reading = read("src/cart.rs", source);
    assert_eq!(
        reading.drafts,
        [],
        "no model of an unreadable file is given"
    );
    reading
        .problems
        .into_iter()
        .map(|problem| {
            (
                problem.feature.trim_matches('`').to_owned(),
                problem.message,
            )
        })
        .collect()
}

fn features(source: &str) -> Vec<String> {
    refused(source)
        .into_iter()
        .map(|(feature, _)| feature)
        .collect()
}

fn with(from: &str, to: &str) -> String {
    assert!(CART.contains(from), "{from} is in the cart twin");
    CART.replacen(from, to, 1)
}

#[test]
fn a_view_block_gives_the_title_the_steps_the_colors_and_both_labels() {
    let draft = only_draft("cart.rs", CART);
    let view = &draft.view;
    assert_eq!(
        view.title.as_deref(),
        Some("Add to cart focus"),
        "the title"
    );
    assert_eq!(
        view.steps,
        [("press".to_owned(), "press Add".to_owned())],
        "the step labels"
    );
    assert_eq!(
        view.colors,
        [("broken".to_owned(), "#c62828".to_owned())],
        "the colors"
    );
    let state = view.state.as_ref().expect("a state label");
    assert_eq!(
        state.state_name.as_deref(),
        Some("self"),
        "the state is `self`"
    );
    let TermKind::Template(pieces) = &state.term.kind else {
        panic!("`format!` is a template: {:?}", state.term.kind);
    };
    let kinds: Vec<&str> = pieces
        .iter()
        .map(|piece| match piece {
            TemplatePiece::Text(text) if text == ", focus on " => "the text between",
            TemplatePiece::Value(_) => "a value",
            _ => "something else",
        })
        .collect();
    assert_eq!(
        kinds,
        ["a value", "the text between", "a value"],
        "the pieces"
    );
    let group = view.group.as_ref().expect("a group label");
    assert!(
        matches!(&group.term.kind, TermKind::Member(_, field) if field == "surface"),
        "`self.surface.to_owned()` shows the field: {:?}",
        group.term.kind
    );
}

#[test]
fn a_model_without_a_view_block_has_an_empty_view() {
    let source = CART.split("#[view(").next().expect("the model comes first");
    let draft = only_draft("cart.rs", source);
    assert_eq!(draft.view, earnest_ast::ViewDraft::default(), "no view");
}

#[test]
fn braces_are_escaped_by_doubling_and_a_label_may_be_a_single_to_string() {
    let source = with(
        "format!(\"{:?}, focus on {:?}\", self.cta, self.focus)",
        "format!(\"{{{:?}}} {}\", self.cta, self.focus == Focus::Lost)",
    );
    let draft = only_draft("cart.rs", &source);
    let state = draft.view.state.expect("a state label");
    let TermKind::Template(pieces) = state.term.kind else {
        panic!("a template");
    };
    assert_eq!(pieces.len(), 4, "`{{`, a value, `}} `, a value: {pieces:?}");
    assert_eq!(
        pieces[0],
        TemplatePiece::Text("{".to_owned()),
        "the escaped brace"
    );
    let dial = DIAL.replace("format!(\"x{} y{}\", self.x, self.y)", "self.x.to_string()");
    let draft = only_draft("dial.rs", &dial);
    let state = draft.view.state.expect("a state label");
    assert!(
        matches!(&state.term.kind, TermKind::Member(_, field) if field == "x"),
        "`.to_string()` shows the value: {:?}",
        state.term.kind
    );
}

#[test]
fn a_step_for_an_action_the_model_does_not_have_is_refused() {
    let source = with("step(press = \"press Add\")", "step(prez = \"press Add\")");
    let found = refused(&source);
    assert_eq!(found.len(), 1, "one problem: {found:?}");
    assert_eq!(found[0].0, "prez", "the unknown action");
    assert!(found[0].1.contains("no action with this id"), "{found:?}");
}

#[test]
fn a_color_that_is_not_one_and_a_part_that_is_not_colored_are_refused() {
    let bad = with("#c62828", "#c6282");
    let found = refused(&bad);
    assert_eq!(found.len(), 1, "one problem: {found:?}");
    assert_eq!(found[0].0, "\"#c6282\"", "the color");
    assert!(found[0].1.contains("hexadecimal digits"), "{found:?}");
    let part = with("broken = ", "lost = ");
    let found = refused(&part);
    assert_eq!(found.len(), 1, "one problem: {found:?}");
    assert_eq!(found[0].0, "lost", "the part");
    assert!(
        found[0].1.contains("`broken`, `start` and `end`"),
        "{found:?}"
    );
    for good in ["#fff", "#ffff", "#c62828ff", "seagreen"] {
        let source = with("#c62828", good);
        assert_eq!(
            only_draft("cart.rs", &source).view.colors[0].1,
            good,
            "{good}"
        );
    }
}

#[test]
fn a_part_or_a_step_given_twice_is_refused() {
    let steps = with(
        "step(press = \"press Add\")",
        "step(press = \"press Add\", press = \"again\")",
    );
    assert_eq!(features(&steps), ["press"], "the second step");
    let colors = with(
        "colors(broken = \"#c62828\")",
        "colors(broken = \"#c62828\", broken = \"red\")",
    );
    assert_eq!(features(&colors), ["broken"], "the second color");
    let title = with(
        "title = \"Add to cart focus\",",
        "title = \"a\", title = \"b\",",
    );
    assert_eq!(features(&title).len(), 1, "the second title");
    let unknown = with("title = \"Add to cart focus\",", "heading = \"a\",");
    assert_eq!(
        features(&unknown).len(),
        1,
        "a setting the view does not have"
    );
}

#[test]
fn a_second_view_and_a_view_of_no_model_are_refused() {
    let twice = format!(
        "{CART}\n#[view(title = \"Again\")]\nimpl Cart {{\n    #[state]\n    fn other(&self) -> String {{\n        self.surface.to_owned()\n    }}\n}}\n"
    );
    assert_eq!(features(&twice), ["#[view]"], "the second block");
    let stray = format!("{CART}\n#[view]\nimpl Missing {{}}\n");
    assert_eq!(features(&stray), ["impl Missing"], "no such model");
}

#[test]
fn a_view_block_holds_only_labels_that_read_the_state_and_give_a_string() {
    let helper = with(
        "    #[group]",
        "    fn helper(&self) -> String {\n        self.surface.to_owned()\n    }\n\n    #[group]",
    );
    assert_eq!(features(&helper), ["fn helper"], "a method without a label");
    let wrong = with("fn group(&self) -> String", "fn group(&self) -> bool");
    assert_eq!(features(&wrong), ["fn group"], "a label gives a `String`");
    let extra = with(
        "fn group(&self) -> String",
        "fn group(&self, extra: bool) -> String",
    );
    assert_eq!(features(&extra), ["group"], "a label takes only the state");
    let both = with("    #[group]", "    #[state]");
    assert_eq!(features(&both), ["#[state]"], "two state labels");
}

#[test]
fn a_label_is_a_format_or_one_value_with_to_string_or_to_owned() {
    let other = with("self.surface.to_owned()", "self.surface");
    assert_eq!(features(&other), ["self.surface"], "a bare value");
    let two = with(
        "self.surface.to_owned()",
        "let shown = self.surface.to_owned();\n        shown",
    );
    assert_eq!(features(&two), ["{"], "a label is one expression");
}

#[test]
fn a_hole_is_empty_or_a_debug_hole_filled_from_the_values_after_the_text() {
    for (written, feature) in [
        ("format!(\"{:>3}\", self.cta)", "{:>3}"),
        ("format!(\"{0}\", self.cta)", "{0}"),
        ("format!(\"{cta}\", self.cta)", "{cta}"),
        ("format!(\"{} {}\", self.cta)", "{}"),
        ("format!(\"}\", self.cta)", "}"),
        ("format!(\"no hole\", self.cta)", "format!"),
    ] {
        let source = with(
            "format!(\"{:?}, focus on {:?}\", self.cta, self.focus)",
            written,
        );
        assert_eq!(features(&source), [feature], "{written}");
    }
    let text = with("self.surface.to_owned()", "format!(\"{:?}\", self.surface)");
    let found = refused(&text);
    assert_eq!(found[0].0, "self.surface", "a text printed with `Debug`");
    assert!(found[0].1.contains("write `{}`"), "{found:?}");
}

#[test]
fn format_is_refused_outside_a_label() {
    let source = with(
        "self.cta == Cta::Add);",
        "format!(\"{}\", self.surface) == self.surface);",
    );
    let found = refused(&source);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].1.contains("only in the `#[state]`"), "{found:?}");
}

#[test]
fn a_label_that_shows_more_than_a_scalar_is_refused_when_the_model_is_lowered() {
    let source = with(
        "format!(\"{:?}, focus on {:?}\", self.cta, self.focus)",
        "format!(\"{}\", self.surface == \"results\")",
    );
    let draft = only_draft("cart.rs", &source);
    let model = lower(&draft).expect("a boolean can be shown");
    assert!(model.view.state.is_some(), "the label is kept");
    let optional = r#"
use earnest::model::{Model, model, view};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Cart {
    #[one_of("a", "b")]
    kind: Option<&'static str>,
}

#[model]
impl Cart {
    #[rule("A cart is a cart")]
    fn cart(&self) -> bool {
        true
    }
}

#[view]
impl Cart {
    #[state]
    fn state(&self) -> String {
        format!("{}", self.kind.is_some())
    }

    #[group]
    fn group(&self) -> String {
        format!("{}", self.kind)
    }
}
"#;
    let draft = only_draft("cart.rs", optional);
    let problems = lower(&draft).expect_err("an optional value is no label");
    let features: Vec<&str> = problems
        .iter()
        .map(|problem| problem.feature.as_str())
        .collect();
    assert_eq!(features, ["a value in a label"], "the group label");
    let counted = search(&only_draft("cart.rs", &with("fn group", "fn group")));
    assert_eq!(counted.states, 6, "the view does not change the search");
}
