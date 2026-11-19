use earnest_check::{Change, Edge, Named, Node, Outcome, Path, RuleOutcome, Shown, Status, Step};
use earnest_check::{Graph, Verdict};
use serde_json::json;
use std::collections::BTreeSet;

const READ_BY_MERMAID: &str = "\":;#<>[]{}%`|\\&*~@$^";

pub(crate) fn node(label: &str) -> Node {
    Node {
        label: label.to_owned(),
        states: 1,
        ..Node::default()
    }
}

pub(crate) fn start(label: &str) -> Node {
    Node {
        start: true,
        ..node(label)
    }
}

pub(crate) fn end(label: &str) -> Node {
    Node {
        end: true,
        ..node(label)
    }
}

pub(crate) fn broken(label: &str, rules: &[&str]) -> Node {
    Node {
        broken: rules.iter().map(|rule| (*rule).to_owned()).collect(),
        ..node(label)
    }
}

pub(crate) fn inside(group: &str, node: Node) -> Node {
    Node {
        group: Some(group.to_owned()),
        ..node
    }
}

pub(crate) fn edge(from: usize, to: usize, labels: &[&str]) -> Edge {
    Edge {
        from,
        to,
        labels: labels.iter().map(|label| (*label).to_owned()).collect(),
    }
}

pub(crate) fn graph(nodes: Vec<Node>, edges: Vec<Edge>) -> Graph {
    Graph {
        nodes,
        edges,
        ..Graph::default()
    }
}

pub(crate) fn text(value: &str) -> Shown {
    Shown {
        text: format!("\"{value}\""),
        json: json!(value),
    }
}

pub(crate) fn named(name: &str, value: Shown) -> Named {
    Named {
        name: name.to_owned(),
        value,
    }
}

pub(crate) fn change(field: &str, before: Shown, after: Shown) -> Change {
    Change {
        field: field.to_owned(),
        before,
        after,
    }
}

pub(crate) fn step(action: &str, input: Vec<Named>, changes: Vec<Change>) -> Step {
    Step {
        action: action.to_owned(),
        label: String::new(),
        input,
        changes,
        state: Vec::new(),
    }
}

pub(crate) fn rule(id: &str, says: &str, path: Option<Path>) -> RuleOutcome {
    RuleOutcome {
        id: id.to_owned(),
        says: says.to_owned(),
        verdict: if path.is_some() {
            Verdict::Broken
        } else {
            Verdict::Holds
        },
        path,
    }
}

pub(crate) fn outcome(status: Status, visited: usize) -> Outcome {
    Outcome {
        status,
        states: visited,
        rules: Vec::new(),
        findings: Vec::new(),
        reason: None,
        graph: None,
    }
}

pub(crate) fn decoded(label: &str) -> String {
    let mut plain = String::new();
    let mut rest = label;
    while let Some(at) = rest.find('#') {
        plain.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let close = after
            .find(';')
            .unwrap_or_else(|| panic!("`#` without `;` in `{label}`"));
        let code: u32 = after[..close]
            .parse()
            .unwrap_or_else(|_| panic!("`#` is not followed by a number in `{label}`"));
        plain.push(char::from_u32(code).unwrap_or_else(|| panic!("no character {code}")));
        rest = &after[close + 1..];
    }
    plain.push_str(rest);
    plain
}

fn assert_safe(label: &str, line: &str) {
    let mut rest = label;
    while let Some(character) = rest.chars().next() {
        if character == '#' {
            let close = rest
                .find(';')
                .unwrap_or_else(|| panic!("`#` without `;` in `{line}`"));
            assert!(
                close > 1 && rest[1..close].bytes().all(|byte| byte.is_ascii_digit()),
                "`#` is not the start of a numbered entity in `{line}`"
            );
            rest = &rest[close + 1..];
        } else {
            assert!(
                !READ_BY_MERMAID.contains(character) && !character.is_control(),
                "`{character}` is outside an entity in `{line}`"
            );
            rest = &rest[character.len_utf8()..];
        }
    }
}

fn is_id(word: &str, letter: char) -> bool {
    word.strip_prefix(letter).is_some_and(|digits| {
        !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
    })
}

#[derive(Debug, Default)]
pub(crate) struct Read {
    pub(crate) title: Option<String>,
    pub(crate) states: Vec<(String, String)>,
    pub(crate) groups: Vec<(String, String)>,
    pub(crate) starts: Vec<String>,
    pub(crate) ends: Vec<String>,
    pub(crate) arrows: Vec<(String, String, String)>,
    pub(crate) classes: Vec<(String, Vec<String>)>,
    pub(crate) styles: Vec<(String, String)>,
    pub(crate) note: Option<String>,
}

impl Read {
    pub(crate) fn label_of(&self, id: &str) -> Option<String> {
        self.states
            .iter()
            .chain(&self.groups)
            .find(|(known, _)| known == id)
            .map(|(_, label)| decoded(label))
    }

    pub(crate) fn members(&self, class: &str) -> Vec<String> {
        self.classes
            .iter()
            .filter(|(name, _)| name == class)
            .flat_map(|(_, members)| members.clone())
            .collect()
    }

    pub(crate) fn reached(&self) -> BTreeSet<String> {
        let mut reached: BTreeSet<String> = self.starts.iter().cloned().collect();
        loop {
            let found: Vec<String> = self
                .arrows
                .iter()
                .filter(|(from, to, _)| reached.contains(from) && !reached.contains(to))
                .map(|(_, to, _)| to.clone())
                .collect();
            if found.is_empty() {
                return reached;
            }
            reached.extend(found);
        }
    }
}

fn read_state(read: &mut Read, line: &str, depth: &mut usize) {
    let rest = line
        .strip_prefix("state \"")
        .unwrap_or_else(|| panic!("not a state: `{line}`"));
    let (label, tail) = rest
        .split_once('"')
        .unwrap_or_else(|| panic!("the label is not closed in `{line}`"));
    assert_safe(label, line);
    let id = tail
        .strip_prefix(" as ")
        .unwrap_or_else(|| panic!("no id in `{line}`"));
    if let Some(group) = id.strip_suffix(" {") {
        assert!(is_id(group, 'g'), "`{group}` is not a group id in `{line}`");
        assert_eq!(*depth, 0, "a group inside a group in `{line}`");
        *depth += 1;
        read.groups.push((group.to_owned(), label.to_owned()));
    } else {
        assert!(is_id(id, 's'), "`{id}` is not a state id in `{line}`");
        read.states.push((id.to_owned(), label.to_owned()));
    }
}

fn read_arrow(read: &mut Read, line: &str) {
    let (ends, label) = line.split_once(" : ").unwrap_or((line, ""));
    assert_safe(label, line);
    let (from, to) = ends
        .split_once(" --> ")
        .unwrap_or_else(|| panic!("not an arrow: `{line}`"));
    match (from, to) {
        ("[*]", state) if is_id(state, 's') && label.is_empty() => {
            read.starts.push(state.to_owned());
        }
        (state, "[*]") if is_id(state, 's') && label.is_empty() => read.ends.push(state.to_owned()),
        _ => {
            assert!(
                is_id(from, 's') && is_id(to, 's'),
                "an arrow end is not a state id in `{line}`"
            );
            read.arrows
                .push((from.to_owned(), to.to_owned(), label.to_owned()));
        }
    }
}

fn read_body(read: &mut Read, line: &str, depth: &mut usize) {
    if line == "}" {
        assert_eq!(*depth, 1, "`}}` closes nothing");
        *depth -= 1;
    } else if line.starts_with("state ") {
        read_state(read, line, depth);
    } else if let Some(rest) = line.strip_prefix("classDef ") {
        let (name, style) = rest
            .split_once(' ')
            .unwrap_or_else(|| panic!("no style in `{line}`"));
        assert!(
            style
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'#' | b':' | b',')),
            "the style of `{line}` holds more than colors"
        );
        read.styles.push((name.to_owned(), style.to_owned()));
    } else if let Some(rest) = line.strip_prefix("class ") {
        let (members, name) = rest
            .split_once(' ')
            .unwrap_or_else(|| panic!("no class name in `{line}`"));
        let members: Vec<String> = members.split(',').map(str::to_owned).collect();
        assert!(
            members.iter().all(|member| is_id(member, 's')),
            "a member of `{line}` is not a state id"
        );
        read.classes.push((name.to_owned(), members));
    } else {
        read_arrow(read, line);
    }
}

pub(crate) fn read(chart: &str) -> Read {
    let mut read = Read::default();
    let mut lines = chart.split('\n');
    assert_eq!(lines.next(), Some("```mermaid"), "the block opens");
    let mut line = lines.next();
    if line == Some("---") {
        let title = lines
            .next()
            .and_then(|found| found.strip_prefix("title: \""))
            .and_then(|found| found.strip_suffix('"'))
            .unwrap_or_else(|| panic!("no title after `---` in\n{chart}"));
        assert!(
            !title.contains(['"', '%', '#', '{', '}']) && !title.replace("\\x", "").contains('\\'),
            "the title `{title}` is not a safe YAML string"
        );
        read.title = Some(title.to_owned());
        assert_eq!(lines.next(), Some("---"), "the front matter closes");
        line = lines.next();
    }
    assert_eq!(line, Some("stateDiagram-v2"), "the chart type");
    let mut depth = 0;
    let mut closed = false;
    for line in lines.by_ref() {
        if line == "```" {
            closed = true;
            break;
        }
        assert!(
            !line.contains("```") && !line.chars().any(char::is_control),
            "`{line}` could close the block"
        );
        let body = line.trim_start_matches(' ');
        assert_eq!(
            line.len() - body.len(),
            4 * (depth + 1) - if body == "}" { 4 } else { 0 },
            "the indent of `{line}`"
        );
        read_body(&mut read, body, &mut depth);
    }
    assert!(closed, "the block closes in\n{chart}");
    assert_eq!(depth, 0, "every group closes");
    let after: Vec<&str> = lines.collect();
    match after.as_slice() {
        [] => {}
        ["", note] => read.note = Some((*note).to_owned()),
        _ => panic!("more than a note after the block: {after:?}"),
    }
    for (from, to, _) in &read.arrows {
        assert!(
            read.label_of(from).is_some() && read.label_of(to).is_some(),
            "the arrow {from} --> {to} has an end that is not declared"
        );
    }
    read
}
