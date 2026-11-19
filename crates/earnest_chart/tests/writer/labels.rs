use crate::support::{edge, graph, inside, node, read, start};
use earnest_chart::chart;
use earnest_check::Graph;

const HOSTILE: [(&str, &str); 9] = [
    (
        "say \"hi\": [*] --> end; #1 <b>",
        "say #34;hi#34;#58; #91;#42;#93; --#62; end#59; #35;1 #60;b#62;",
    ),
    (
        "line one\nline two\ttab\r\nthree",
        "line one line two tab  three",
    ),
    (
        "`code` *bold* _it_ ~s~ {x} |p| \\ % & @ $ ^",
        "#96;code#96; #42;bold#42; _it_ #126;s#126; #123;x#125; #124;p#124; #92; #37; #38; #64; #36; #94;",
    ),
    ("é 模型 🙂 ¶ß ﬂ°", "é 模型 🙂 #182;ß #64258;°"),
    (
        "%%{init: {'theme':'dark'}}%%",
        "#37;#37;#123;init#58; #123;'theme'#58;'dark'#125;#125;#37;#37;",
    ),
    ("```", "#96;#96;#96;"),
    ("classDef x fill:#f00;", "classDef x fill#58;#35;f00#59;"),
    ("", " "),
    (
        "a (b), c = d + e / f - g_h. Yes! No?",
        "a (b), c = d + e / f - g_h. Yes! No?",
    ),
];

fn plain(label: &str) -> String {
    label
        .chars()
        .map(|character| {
            if character.is_whitespace() {
                ' '
            } else {
                character
            }
        })
        .collect()
}

#[test]
fn a_hostile_state_label_is_written_as_entities() {
    for (label, written) in HOSTILE {
        let drawn = chart(&graph(vec![start(label)], Vec::new()));
        let expected = format!(
            "```mermaid\nstateDiagram-v2\n    state \"{written}\" as s0\n    [*] --> s0\n```"
        );
        assert_eq!(drawn, expected, "the label `{label}`");
        let back = read(&drawn);
        if !label.is_empty() {
            assert_eq!(
                back.label_of("s0"),
                Some(plain(label)),
                "`{label}` survives"
            );
        }
    }
}

#[test]
fn a_hostile_arrow_label_is_written_as_entities() {
    for (label, written) in HOSTILE {
        let drawn = chart(&graph(
            vec![start("a"), node("b")],
            vec![edge(0, 1, &[label, "plain"])],
        ));
        let arrow = if label.is_empty() {
            "plain".to_owned()
        } else {
            format!("{written} / plain")
        };
        let expected = format!(
            "```mermaid\nstateDiagram-v2\n    state \"a\" as s0\n    state \"b\" as s1\n    [*] --> s0\n    s0 --> s1 : {arrow}\n```"
        );
        assert_eq!(drawn, expected, "the label `{label}`");
        read(&drawn);
    }
}

#[test]
fn a_hostile_group_label_is_written_as_entities() {
    for (label, written) in HOSTILE {
        let drawn = chart(&graph(vec![inside(label, start("a"))], Vec::new()));
        let expected = format!(
            "```mermaid\nstateDiagram-v2\n    state \"{written}\" as g0 {{\n        state \"a\" as s0\n        [*] --> s0\n    }}\n```"
        );
        assert_eq!(drawn, expected, "the group `{label}`");
        read(&drawn);
    }
}

#[test]
fn a_hostile_title_is_one_escaped_yaml_string() {
    let titled = Graph {
        title: Some("50% \"off\" {now} #1 \\ back\n---\nstateDiagram-v2 ﬂ° ¶ß é".to_owned()),
        nodes: vec![start("a")],
        ..Graph::default()
    };
    let expected = "```mermaid\n---\ntitle: \"50\\x25 \\x22off\\x22 \\x7Bnow\\x7D \\x231 \\x5C back --- stateDiagram-v2 ﬂ\u{200b}° ¶\u{200b}ß é\"\n---\nstateDiagram-v2\n    state \"a\" as s0\n    [*] --> s0\n```";
    let drawn = chart(&titled);
    assert_eq!(drawn, expected);
    read(&drawn);
}

#[test]
fn two_states_with_the_same_label_keep_their_own_ids() {
    let twins = graph(vec![start("same"), node("same")], vec![edge(0, 1, &["go"])]);
    let drawn = read(&chart(&twins));
    assert_eq!(
        drawn.states,
        [
            ("s0".to_owned(), "same".to_owned()),
            ("s1".to_owned(), "same".to_owned())
        ]
    );
    assert_eq!(
        drawn.arrows,
        [("s0".to_owned(), "s1".to_owned(), "go".to_owned())]
    );
}
