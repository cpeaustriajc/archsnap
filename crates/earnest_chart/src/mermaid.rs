use crate::select::{Selection, marked, select};
use earnest_check::{Graph, Node};
use std::fmt::Write as _;

const INDENT: &str = "    ";
const DEFAULT_BROKEN: &str = "fill:#c62828,color:#ffffff";
const KEPT_PUNCTUATION: &str = ",.'()=+/!?-_";
const DECODED_BY_MERMAID: [char; 2] = ['\u{fb02}', '\u{b6}'];
const ZERO_WIDTH_SPACE: char = '\u{200b}';
const LONGEST_COLOR_NAME: usize = 32;
const DARK_TEXT_FROM: u32 = 128_000;

struct Class {
    name: &'static str,
    color: &'static str,
    fallback: Option<&'static str>,
    applies: fn(&Node) -> bool,
}

const CLASSES: [Class; 3] = [
    Class {
        name: "broken",
        color: "broken",
        fallback: Some(DEFAULT_BROKEN),
        applies: marked,
    },
    Class {
        name: "ending",
        color: "end",
        fallback: None,
        applies: |node| node.end,
    },
    Class {
        name: "starting",
        color: "start",
        fallback: None,
        applies: |node| node.start,
    },
];

fn label(text: &str) -> String {
    let mut safe = String::new();
    for character in text.chars() {
        if character.is_whitespace() || character.is_control() {
            safe.push(' ');
        } else if DECODED_BY_MERMAID.contains(&character)
            || (character.is_ascii()
                && !character.is_ascii_alphanumeric()
                && !KEPT_PUNCTUATION.contains(character))
        {
            let _ = write!(safe, "#{};", u32::from(character));
        } else {
            safe.push(character);
        }
    }
    if safe.is_empty() {
        safe.push(' ');
    }
    safe
}

fn title(text: &str) -> String {
    let mut safe = String::new();
    for character in text.chars() {
        if character.is_whitespace() || character.is_control() {
            safe.push(' ');
        } else if DECODED_BY_MERMAID.contains(&character) {
            safe.push(character);
            safe.push(ZERO_WIDTH_SPACE);
        } else if matches!(character, '"' | '\\' | '%' | '#' | '{' | '}') {
            let _ = write!(safe, "\\x{:02X}", u32::from(character));
        } else {
            safe.push(character);
        }
    }
    safe
}

fn channel(digits: &str) -> Option<u32> {
    let value = u32::from_str_radix(digits, 16).ok()?;
    Some(if digits.len() == 1 {
        value.saturating_mul(17)
    } else {
        value
    })
}

fn text_on(hex: &str) -> Option<&'static str> {
    let width = match hex.len() {
        3 | 4 => 1,
        6 | 8 => 2,
        _ => return None,
    };
    if !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let red = channel(hex.get(..width)?)?;
    let green = channel(hex.get(width..width.saturating_mul(2))?)?;
    let blue = channel(hex.get(width.saturating_mul(2)..width.saturating_mul(3))?)?;
    let brightness = red
        .saturating_mul(299)
        .saturating_add(green.saturating_mul(587))
        .saturating_add(blue.saturating_mul(114));
    Some(if brightness >= DARK_TEXT_FROM {
        "#000000"
    } else {
        "#ffffff"
    })
}

fn style(color: &str) -> Option<String> {
    let color = color.trim();
    if let Some(hex) = color.strip_prefix('#') {
        return text_on(hex).map(|text| format!("fill:{color},color:{text}"));
    }
    let named = !color.is_empty()
        && color.len() <= LONGEST_COLOR_NAME
        && color.bytes().all(|byte| byte.is_ascii_alphabetic());
    named.then(|| format!("fill:{color}"))
}

fn style_of(graph: &Graph, class: &Class) -> Option<String> {
    graph
        .colors
        .iter()
        .filter(|(name, _)| name == class.color)
        .find_map(|(_, color)| style(color))
        .or_else(|| class.fallback.map(str::to_owned))
}

struct Drawing<'a> {
    graph: &'a Graph,
    selection: Selection,
    groups: Vec<&'a str>,
    group_of: Vec<Option<usize>>,
}

impl<'a> Drawing<'a> {
    fn new(graph: &'a Graph) -> Self {
        let selection = select(graph);
        let mut groups: Vec<&'a str> = Vec::new();
        let mut group_of = vec![None; graph.nodes.len()];
        for state in &selection.states {
            let Some(group) = graph.nodes[*state].group.as_deref() else {
                continue;
            };
            let known = groups.iter().position(|known| *known == group);
            group_of[*state] = Some(known.unwrap_or(groups.len()));
            if known.is_none() {
                groups.push(group);
            }
        }
        Self {
            graph,
            selection,
            groups,
            group_of,
        }
    }

    fn states(&self) -> impl Iterator<Item = (usize, &'a Node)> {
        self.selection
            .states
            .iter()
            .map(|state| (*state, &self.graph.nodes[*state]))
    }

    fn within(&self, group: Option<usize>) -> impl Iterator<Item = (usize, &'a Node)> {
        self.states()
            .filter(move |(state, _)| self.group_of[*state] == group)
    }

    fn arrows(&self, group: Option<usize>, indent: &str) -> Vec<String> {
        let mut lines: Vec<String> = self
            .within(group)
            .filter(|(_, node)| node.start)
            .map(|(state, _)| format!("{indent}[*] --> s{state}"))
            .collect();
        for position in &self.selection.arrows {
            let edge = &self.graph.edges[*position];
            let home = self.group_of[edge.from]
                .filter(|_| self.group_of[edge.from] == self.group_of[edge.to]);
            if home != group {
                continue;
            }
            let labels: Vec<String> = edge
                .labels
                .iter()
                .filter(|text| !text.trim().is_empty())
                .map(|text| label(text))
                .collect();
            let mut line = format!("{indent}s{} --> s{}", edge.from, edge.to);
            if !labels.is_empty() {
                let _ = write!(line, " : {}", labels.join(" / "));
            }
            lines.push(line);
        }
        lines.extend(
            self.within(group)
                .filter(|(_, node)| node.end)
                .map(|(state, _)| format!("{indent}s{state} --> [*]")),
        );
        lines
    }

    fn declarations(&self) -> Vec<String> {
        let inner = INDENT.repeat(2);
        let mut opened = vec![false; self.groups.len()];
        let mut lines = Vec::new();
        for (state, node) in self.states() {
            let Some(group) = self.group_of[state] else {
                lines.push(format!(
                    "{INDENT}state \"{}\" as s{state}",
                    label(&node.label)
                ));
                continue;
            };
            if opened[group] {
                continue;
            }
            opened[group] = true;
            lines.push(format!(
                "{INDENT}state \"{}\" as g{group} {{",
                label(self.groups[group])
            ));
            lines.extend(self.within(Some(group)).map(|(member, node)| {
                format!("{inner}state \"{}\" as s{member}", label(&node.label))
            }));
            lines.extend(self.arrows(Some(group), &inner));
            lines.push(format!("{INDENT}}}"));
        }
        lines
    }

    fn classes(&self) -> Vec<String> {
        let styles: Vec<Option<String>> = CLASSES
            .iter()
            .map(|class| style_of(self.graph, class))
            .collect();
        let mut members = vec![Vec::new(); CLASSES.len()];
        for (state, node) in self.states() {
            let class = CLASSES
                .iter()
                .zip(&styles)
                .position(|(class, style)| style.is_some() && (class.applies)(node));
            if let Some(class) = class {
                members[class].push(format!("s{state}"));
            }
        }
        let mut lines = Vec::new();
        for ((class, style), members) in CLASSES.iter().zip(styles).zip(members) {
            let Some(style) = style.filter(|_| !members.is_empty()) else {
                continue;
            };
            lines.push(format!("{INDENT}classDef {} {style}", class.name));
            lines.push(format!(
                "{INDENT}class {} {}",
                members.join(","),
                class.name
            ));
        }
        lines
    }

    fn left_out(&self) -> Option<String> {
        let states = self.graph.nodes.len();
        let arrows = self.graph.edges.len();
        let hidden_states = states.saturating_sub(self.selection.states.len());
        let hidden_arrows = arrows.saturating_sub(self.selection.arrows.len());
        let beyond = match self.selection.beyond {
            0 => String::new(),
            1 => " A broken state lies beyond what is shown.".to_owned(),
            many => format!(" {many} broken states lie beyond what is shown."),
        };
        (hidden_states > 0 || hidden_arrows > 0).then(|| {
            format!(
                "Not shown: {hidden_states} of {states} states and {hidden_arrows} of {arrows} arrows.{beyond}"
            )
        })
    }
}

/// The graph as one fenced `mermaid` block holding a `stateDiagram-v2` chart, ready to paste
/// into Markdown that GitHub renders. The text has no trailing line break.
///
/// A state is declared as `s<n>`, where `n` is its position in the graph's nodes, with its label
/// attached; a group is a composite state `g<n>`. Every character of a label that Mermaid could
/// read as syntax is written as a `#<code>;` entity, and a line break in a label becomes a
/// space. A state that breaks a rule or is out of range has the class `broken`. The view's
/// `broken` color replaces the default red; its `start` and `end` colors add the classes
/// `starting` and `ending`. A color that is not `#` with 3, 4, 6 or 8 hex digits or a plain
/// color name is ignored.
///
/// At most [`ARROW_LIMIT`](crate::ARROW_LIMIT) arrows are drawn. When the graph has more, or
/// has a state no start state leads to, the block is followed by a blank line and one line of
/// the form `Not shown: 16 of 36 states and 40 of 60 arrows.` When the route to a state that
/// breaks a rule or is out of range does not fit, its first arrows are drawn as far as the
/// limit allows, the state itself is not, and the line goes on with `A broken state lies
/// beyond what is shown.` or `2 broken states lie beyond what is shown.`
#[must_use]
pub fn chart(graph: &Graph) -> String {
    let drawing = Drawing::new(graph);
    let mut lines = vec!["```mermaid".to_owned()];
    if let Some(text) = &graph.title {
        lines.push("---".to_owned());
        lines.push(format!("title: \"{}\"", title(text)));
        lines.push("---".to_owned());
    }
    lines.push("stateDiagram-v2".to_owned());
    lines.extend(drawing.declarations());
    lines.extend(drawing.arrows(None, INDENT));
    lines.extend(drawing.classes());
    lines.push("```".to_owned());
    if let Some(note) = drawing.left_out() {
        lines.push(String::new());
        lines.push(note);
    }
    lines.join("\n")
}
