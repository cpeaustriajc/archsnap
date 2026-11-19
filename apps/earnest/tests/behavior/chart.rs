use crate::support::{Run, Sandbox};

const ARROW: &str = "-->";
const ARROW_LIMIT: usize = 20;
const ESCAPE: char = '\u{1b}';

fn fence_lines(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim)
        .filter(|line| line.starts_with("```"))
        .collect()
}

fn mermaid_block(run: &Run) -> Vec<String> {
    assert_eq!(
        fence_lines(&run.stdout),
        ["```mermaid", "```"],
        "the chart is one fenced `mermaid` block\n{run}"
    );
    run.stdout
        .lines()
        .map(str::trim)
        .skip_while(|line| *line != "```mermaid")
        .skip(1)
        .take_while(|line| *line != "```")
        .map(str::to_owned)
        .collect()
}

fn arrows(block: &[String]) -> Vec<&String> {
    block
        .iter()
        .filter(|line| line.contains(ARROW) && !line.contains("[*]"))
        .collect()
}

fn gives_a_state_the_class_broken(block: &[String]) -> bool {
    block.iter().any(|line| {
        line.contains(":::broken") || (line.starts_with("class ") && line.ends_with(" broken"))
    })
}

fn assert_has_label(block: &[String], label: &str, run: &Run) {
    assert!(
        block.iter().any(|line| line.contains(label)),
        "the chart carries the label `{label}`\n{run}"
    );
}

#[test]
fn r09_the_chart_carries_the_labels_of_the_view_and_marks_broken_states() {
    let sandbox = Sandbox::with(&["cart.model.ts"]);
    let run = sandbox.earnest(&["chart", "models/cart.model.ts", "--model", "Cart"]);
    let block = mermaid_block(&run);
    assert!(
        block.iter().any(|line| line == "stateDiagram-v2"),
        "the block holds a `stateDiagram-v2` chart\n{run}"
    );
    for state in [
        "Add, focus on Add",
        "Adding, focus on Add",
        "InCart, focus on InCart",
        "InCart, focus on Lost",
    ] {
        assert_has_label(&block, state, &run);
    }
    for group in ["results", "shortlist"] {
        assert_has_label(&block, group, &run);
    }
    assert_has_label(&block, "Add to cart focus", &run);

    let arrows = arrows(&block);
    assert_eq!(
        arrows.len(),
        4,
        "one arrow for each step of the model\n{run}"
    );
    let pressed = arrows
        .iter()
        .filter(|line| line.contains("press Add"))
        .count();
    assert_eq!(
        pressed, 2,
        "an arrow carries the label the view gives its action\n{run}"
    );
    let landed = arrows
        .iter()
        .filter(|line| line.contains("the add lands"))
        .count();
    assert_eq!(
        landed, 2,
        "an arrow without a label in the view carries the sentence\n{run}"
    );
    assert!(
        !arrows
            .iter()
            .any(|line| line.contains("the buyer presses Add")),
        "the label of the view replaces the sentence of the action\n{run}"
    );

    assert!(
        gives_a_state_the_class_broken(&block),
        "the state that breaks the rule has the class `broken`\n{run}"
    );
    assert!(
        block.iter().any(|line| line.contains("#c62828")),
        "the class `broken` takes the colour the view gives it\n{run}"
    );
}

#[test]
fn r09_a_chart_never_has_more_than_twenty_arrows() {
    let sandbox = Sandbox::with(&["dial.model.ts"]);
    let run = sandbox.earnest(&["chart", "models/dial.model.ts", "--model", "Dial"]);
    let block = mermaid_block(&run);
    let arrows = arrows(&block);
    assert!(
        arrows.len() <= ARROW_LIMIT,
        "a model with 60 arrows is drawn with at most {ARROW_LIMIT}, not {}\n{run}",
        arrows.len()
    );
    assert!(
        arrows.len() >= 10,
        "the ten arrows of the path to the broken state are kept\n{run}"
    );
    assert_has_label(&block, "x0 y0", &run);
    assert_has_label(&block, "x5 y5", &run);
    assert!(
        gives_a_state_the_class_broken(&block),
        "the far corner has the class `broken`\n{run}"
    );
    assert!(
        run.stdout.lines().any(|line| {
            line.contains("states")
                && line.contains("arrows")
                && line.contains(|character: char| character.is_ascii_digit())
        }),
        "the chart says how many states and arrows it left out\n{run}"
    );
}

#[test]
fn r09_a_model_without_a_view_still_has_a_chart() {
    let sandbox = Sandbox::with(&["turnstile.model.ts"]);
    let run = sandbox.earnest(&["chart", "models/turnstile.model.ts", "--model", "Turnstile"]);
    let block = mermaid_block(&run);
    let arrows = arrows(&block);
    assert_eq!(
        arrows.len(),
        5,
        "one arrow for each step of the model\n{run}"
    );
    for sentence in [
        "the visitor pays",
        "the visitor pushes",
        "the visitor walks through",
    ] {
        assert!(
            arrows.iter().any(|line| line.contains(sentence)),
            "an arrow carries the sentence `{sentence}`\n{run}"
        );
    }
    assert!(
        gives_a_state_the_class_broken(&block),
        "the open, unpaid gate has the class `broken`\n{run}"
    );
}

#[test]
fn r10_the_chart_is_plain_text_for_a_pr_body() {
    let sandbox = Sandbox::with(&["cart.model.ts"]);
    let run = sandbox.earnest(&["chart", "models/cart.model.ts", "--model", "Cart"]);
    assert!(!run.stdout.is_empty(), "the chart is printed\n{run}");
    assert!(
        !run.stdout.contains(ESCAPE),
        "the chart has no terminal colour codes\n{run}"
    );
}

#[test]
fn r10_the_report_is_markdown_for_a_pr_body() {
    let sandbox = Sandbox::with(&["cart.model.ts", "counter.model.ts"]);
    let run = sandbox.earnest(&["report"]);
    assert!(
        !run.stdout.contains(ESCAPE),
        "the report has no terminal colour codes\n{run}"
    );
    let fences = fence_lines(&run.stdout);
    let opened = fences.iter().filter(|line| **line == "```mermaid").count();
    assert_eq!(opened, 2, "one chart for each model\n{run}");
    assert_eq!(fences.len() % 2, 0, "every fenced block is closed\n{run}");
    let charts = run.stdout.matches("stateDiagram-v2").count();
    assert_eq!(charts, 2, "each chart is a `stateDiagram-v2`\n{run}");
    for name in ["Cart", "Counter"] {
        assert!(
            run.stdout
                .lines()
                .any(|line| line.starts_with('#') && line.contains(name)),
            "the report has a section for {name}\n{run}"
        );
    }
    for tokens in [
        ["fail", "Cart", "6 states"],
        ["pass", "Counter", "4 states"],
    ] {
        assert!(
            run.stdout
                .lines()
                .any(|line| tokens.iter().all(|token| line.contains(token))),
            "the report has the result line with {tokens:?}\n{run}"
        );
    }
    for token in [
        "focusLands",
        "After an add lands, focus is on In cart",
        "surface = \"shortlist\"",
        "cta: \"Add\" -> \"Adding\"",
        "focus: \"Add\" -> \"Lost\"",
    ] {
        assert!(
            run.stdout.contains(token),
            "the report has the path to the broken rule, with `{token}`\n{run}"
        );
    }
    assert!(
        !run.stdout.contains("rule changes since"),
        "without `--base` the report lists no rule changes\n{run}"
    );
}
