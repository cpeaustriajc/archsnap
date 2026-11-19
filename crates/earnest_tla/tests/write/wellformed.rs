use earnest_tla::Files;
use std::collections::BTreeSet;

const WORDS_OF_THE_LANGUAGE: [&str; 57] = [
    "ACTION",
    "ASSUME",
    "ASSUMPTION",
    "AXIOM",
    "BOOLEAN",
    "BY",
    "CASE",
    "CHOOSE",
    "CONSTANT",
    "CONSTANTS",
    "COROLLARY",
    "DEF",
    "DEFINE",
    "DEFS",
    "DOMAIN",
    "ELSE",
    "ENABLED",
    "EXCEPT",
    "EXTENDS",
    "FALSE",
    "HAVE",
    "HIDE",
    "IF",
    "IN",
    "INSTANCE",
    "LAMBDA",
    "LEMMA",
    "LET",
    "LOCAL",
    "MODULE",
    "NEW",
    "OBVIOUS",
    "OMITTED",
    "ONLY",
    "OTHER",
    "PICK",
    "PROOF",
    "PROPOSITION",
    "PROVE",
    "QED",
    "RECURSIVE",
    "STATE",
    "STRING",
    "SUBSET",
    "SUFFICES",
    "TAKE",
    "TEMPORAL",
    "THEN",
    "THEOREM",
    "TRUE",
    "UNCHANGED",
    "UNION",
    "USE",
    "VARIABLE",
    "VARIABLES",
    "WITH",
    "WITNESS",
];

const WORDS_THE_EXPORT_WRITES: [&str; 11] = [
    "IF",
    "THEN",
    "ELSE",
    "LET",
    "IN",
    "DOMAIN",
    "UNCHANGED",
    "EXCEPT",
    "TRUE",
    "FALSE",
    "BOOLEAN",
];

const OPERATORS_OF_THE_STANDARD_MODULES: [&str; 8] = [
    "Len",
    "Head",
    "Tail",
    "Append",
    "SubSeq",
    "SelectSeq",
    "Cardinality",
    "Assert",
];

const SIGNS: [&str; 37] = [
    "(",
    ")",
    "[",
    "]",
    "{",
    "}",
    "<<",
    ">>",
    ",",
    ":",
    "'",
    "!",
    "#",
    "~",
    "+",
    "-",
    "*",
    "%",
    "<",
    "<=",
    ">",
    ">=",
    "=",
    "==",
    "=>",
    "|->",
    ".",
    "..",
    "/\\",
    "\\/",
    "\\",
    "\\in",
    "\\E",
    "\\A",
    "\\cup",
    "\\cap",
    "\\subseteq",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Token {
    Name(String),
    Number,
    Text,
    Sign(String),
}

pub(crate) fn sign(text: &str) -> Token {
    Token::Sign(text.to_owned())
}

fn name(text: &str) -> Token {
    Token::Name(text.to_owned())
}

pub(crate) fn tokens(text: &str) -> Vec<Token> {
    let letters: Vec<char> = text.chars().collect();
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(&letter) = letters.get(at) {
        let next = letters.get(at + 1).copied();
        let run = |from: usize, keep: fn(char) -> bool| -> usize {
            letters[from..]
                .iter()
                .take_while(|each| keep(**each))
                .count()
        };
        if letter.is_whitespace() {
            at += 1;
        } else if letter == '"' {
            at += 1;
            while letters[at] != '"' {
                at += if letters[at] == '\\' { 2 } else { 1 };
            }
            at += 1;
            found.push(Token::Text);
        } else if letter.is_ascii_digit() {
            at += run(at, |each| each.is_ascii_digit());
            found.push(Token::Number);
        } else if letter.is_ascii_alphabetic() || letter == '_' {
            let length = run(at, |each| each.is_ascii_alphanumeric() || each == '_');
            found.push(Token::Name(letters[at..at + length].iter().collect()));
            at += length;
        } else if letter == '\\' && next.is_some_and(|each| each.is_ascii_alphabetic()) {
            let length = 1 + run(at + 1, |each| each.is_ascii_alphabetic());
            found.push(Token::Sign(letters[at..at + length].iter().collect()));
            at += length;
        } else {
            let two: String = [Some(letter), next].into_iter().flatten().collect();
            let three: String = letters[at..letters.len().min(at + 3)].iter().collect();
            let length = if three == "|->" {
                3
            } else if ["<<", ">>", "<=", ">=", "==", "=>", "..", "/\\", "\\/"]
                .contains(&two.as_str())
            {
                2
            } else {
                1
            };
            found.push(Token::Sign(letters[at..at + length].iter().collect()));
            at += length;
        }
    }
    found
}

fn is_a_name_of_the_language(name: &str) -> bool {
    let mut letters = name.chars();
    letters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && letters.all(|each| each.is_ascii_alphanumeric() || each == '_')
        && !name.starts_with("WF_")
        && !name.starts_with("SF_")
        && !WORDS_OF_THE_LANGUAGE.contains(&name)
}

fn opens(token: &Token) -> bool {
    ["(", "[", "{", "<<"]
        .iter()
        .any(|each| *token == sign(each))
}

fn closes(token: &Token) -> bool {
    [")", "]", "}", ">>"]
        .iter()
        .any(|each| *token == sign(each))
}

pub(crate) fn assert_delimiters_balance(found: &[Token]) {
    let mut open: Vec<&str> = Vec::new();
    for token in found {
        let Token::Sign(text) = token else { continue };
        match text.as_str() {
            "(" | "[" | "{" | "<<" => open.push(text),
            ")" => assert_eq!(open.pop(), Some("("), "a `)` closes a `(`"),
            "]" => assert_eq!(open.pop(), Some("["), "a `]` closes a `[`"),
            "}" => assert_eq!(open.pop(), Some("{"), "a closing brace closes a brace"),
            ">>" => assert_eq!(open.pop(), Some("<<"), "a `>>` closes a `<<`"),
            _ => {}
        }
    }
    assert_eq!(open, Vec::<&str>::new(), "every delimiter is closed");
}

fn is_a_label(found: &[Token], at: usize) -> bool {
    let before = at.checked_sub(1).and_then(|index| found.get(index));
    before == Some(&sign(".")) || found.get(at + 1) == Some(&sign("|->"))
}

fn bound_after_the_colon(body: &[Token], brace: usize) -> Option<usize> {
    let mut depth = 0_usize;
    for (at, token) in body.iter().enumerate().skip(brace + 1) {
        if opens(token) {
            depth += 1;
        } else if closes(token) {
            if depth == 0 {
                return None;
            }
            depth -= 1;
        } else if depth == 0
            && *token == sign(":")
            && matches!(body.get(at + 1), Some(Token::Name(_)))
            && body.get(at + 2) == Some(&sign("\\in"))
        {
            return Some(at + 1);
        }
    }
    None
}

pub(crate) struct Names<'a> {
    pub(crate) defined: &'a str,
    pub(crate) known: &'a BTreeSet<String>,
    pub(crate) variables: &'a BTreeSet<String>,
}

struct Walk<'a> {
    names: &'a Names<'a>,
    levels: Vec<Vec<String>>,
}

impl Walk<'_> {
    fn sees(&self, name: &str) -> bool {
        self.names.known.contains(name)
            || self
                .levels
                .iter()
                .any(|level| level.iter().any(|bound| bound == name))
    }

    fn bind(&mut self, name: &str) {
        let defined = self.names.defined;
        assert!(
            is_a_name_of_the_language(name),
            "`{name}` in `{defined}` is a name TLA+ can spell"
        );
        assert!(
            !self.sees(name) && name != defined,
            "`{name}` in `{defined}` does not hide a name that is already in scope"
        );
        self.levels
            .last_mut()
            .expect("the definition's own level is never closed")
            .push(name.to_owned());
    }
}

pub(crate) fn assert_names_are_in_scope(names: &Names<'_>, parameters: &[String], body: &[Token]) {
    let defined = names.defined;
    let mut walk = Walk {
        names,
        levels: vec![Vec::new()],
    };
    for parameter in parameters {
        walk.bind(parameter);
    }
    let mut bound_early = BTreeSet::new();
    let mut in_a_let_heading = false;
    for (at, token) in body.iter().enumerate() {
        let before = at.checked_sub(1).and_then(|index| body.get(index));
        let after = body.get(at + 1);
        match token {
            found if opens(found) && !in_a_let_heading => {
                walk.levels.push(Vec::new());
                let early = bound_after_the_colon(body, at).filter(|_| *found == sign("{"));
                if let Some(index) = early
                    && let Some(Token::Name(bound)) = body.get(index)
                {
                    walk.bind(bound);
                    bound_early.insert(index);
                }
            }
            found if closes(found) && !in_a_let_heading => {
                walk.levels.pop();
                assert!(
                    !walk.levels.is_empty(),
                    "`{defined}` closes only what it opened"
                );
            }
            Token::Sign(text) if text == "==" => in_a_let_heading = false,
            Token::Sign(text) if text == "'" => {
                let primed = matches!(before, Some(Token::Name(primed))
                    if names.variables.contains(primed));
                assert!(primed, "a prime in `{defined}` follows a declared variable");
            }
            Token::Name(found) if is_a_label(body, at) => assert!(
                is_a_name_of_the_language(found),
                "`{found}` is a record field TLA+ can spell, in `{defined}`"
            ),
            Token::Name(_) if bound_early.contains(&at) => {}
            Token::Name(found) => {
                let chosen = after == Some(&sign("\\in"))
                    && ["\\E", "\\A", "{", "[", ","]
                        .iter()
                        .any(|opener| before == Some(&sign(opener)));
                if before == Some(&name("LET")) {
                    in_a_let_heading = true;
                }
                if chosen || in_a_let_heading {
                    walk.bind(found);
                } else {
                    assert!(
                        walk.sees(found),
                        "`{found}` is in scope where `{defined}` uses it"
                    );
                }
            }
            _ => {}
        }
    }
}

fn chunks(module: &str) -> Vec<String> {
    let mut all: Vec<String> = Vec::new();
    for line in module.lines().filter(|line| !line.is_empty()) {
        match all.last_mut() {
            Some(last) if line.starts_with(' ') => {
                last.push('\n');
                last.push_str(line);
            }
            _ => all.push(line.to_owned()),
        }
    }
    all
}

fn check_a_definition(
    found: &[Token],
    variables: &BTreeSet<String>,
    definitions: &mut BTreeSet<String>,
) {
    let Some(Token::Name(defined)) = found.first() else {
        panic!("a definition starts with its name, not {found:?}");
    };
    let heading = found
        .iter()
        .position(|token| *token == sign("=="))
        .unwrap_or_else(|| panic!("`{defined}` is a definition"));
    let parameters: Vec<String> = found[1..heading]
        .iter()
        .filter_map(|token| match token {
            Token::Name(parameter) => Some(parameter.clone()),
            _ => None,
        })
        .collect();
    let mut known: BTreeSet<String> = WORDS_THE_EXPORT_WRITES
        .iter()
        .chain(&OPERATORS_OF_THE_STANDARD_MODULES)
        .map(|each| (*each).to_owned())
        .collect();
    known.extend(variables.iter().cloned());
    known.extend(definitions.iter().cloned());
    assert!(
        is_a_name_of_the_language(defined) && !known.contains(defined),
        "`{defined}` is a new name TLA+ can spell"
    );
    let names = Names {
        defined,
        known: &known,
        variables,
    };
    assert_names_are_in_scope(&names, &parameters, &found[heading + 1..]);
    definitions.insert(defined.clone());
}

pub(crate) fn assert_well_formed(files: &Files, rules: usize) {
    let header = format!("---- MODULE {} ----\n", files.name);
    assert!(
        is_a_name_of_the_language(&files.name),
        "`{}` is a module name TLA+ can spell",
        files.name
    );
    let rest = files
        .module
        .strip_prefix(&header)
        .unwrap_or_else(|| panic!("the module opens with its header: {}", files.module));
    let inside = rest
        .strip_suffix("\n====\n")
        .unwrap_or_else(|| panic!("the module closes with its footer: {}", files.module));
    let mut variables = BTreeSet::new();
    let mut definitions = BTreeSet::new();
    for chunk in chunks(inside) {
        let found = tokens(&chunk);
        for token in &found {
            if let Token::Sign(text) = token {
                assert!(SIGNS.contains(&text.as_str()), "`{text}` is a sign of TLA+");
            }
        }
        assert_delimiters_balance(&found);
        if chunk.starts_with("EXTENDS ") {
            continue;
        }
        if let Some(declared) = chunk.strip_prefix("VARIABLES ") {
            for variable in declared.split(", ") {
                assert!(
                    is_a_name_of_the_language(variable),
                    "`{variable}` is a variable TLA+ can spell"
                );
                assert!(
                    variables.insert(variable.to_owned()),
                    "`{variable}` is declared once"
                );
            }
            continue;
        }
        check_a_definition(&found, &variables, &mut definitions);
    }
    for needed in ["Init", "Next", "InRange", "Stop", "vars"] {
        assert!(definitions.contains(needed), "`{needed}` is defined");
    }
    let every_token = tokens(inside);
    for defined in &definitions {
        let written = every_token
            .iter()
            .filter(|token| **token == name(defined))
            .count();
        let configured = files.config.split_whitespace().any(|word| word == defined);
        assert!(
            written > 1 || configured,
            "`{defined}` is used by the module or named in the configuration"
        );
    }
    let lines: Vec<&str> = files.config.lines().collect();
    assert_eq!(
        lines[..3],
        ["INIT Init", "NEXT Next", "CHECK_DEADLOCK FALSE"],
        "the configuration opens with the start, the step and the deadlock setting"
    );
    let invariants: BTreeSet<&str> = lines[3..]
        .iter()
        .map(|line| {
            line.strip_prefix("INVARIANT ")
                .unwrap_or_else(|| panic!("`{line}` names an invariant"))
        })
        .collect();
    assert_eq!(invariants.len(), rules, "one invariant for each rule");
    for invariant in invariants {
        assert!(
            definitions.contains(invariant),
            "`{invariant}` is defined in the module"
        );
    }
}

fn scoped(known: &[&str], variables: &[&str], parameters: &[&str], body: &str) -> bool {
    let known: BTreeSet<String> = known
        .iter()
        .chain(variables)
        .chain(&WORDS_THE_EXPORT_WRITES)
        .map(|each| (*each).to_owned())
        .collect();
    let variables: BTreeSet<String> = variables.iter().map(|each| (*each).to_owned()).collect();
    let parameters: Vec<String> = parameters.iter().map(|each| (*each).to_owned()).collect();
    let body = tokens(body);
    std::panic::catch_unwind(|| {
        let names = Names {
            defined: "Defined",
            known: &known,
            variables: &variables,
        };
        assert_names_are_in_scope(&names, &parameters, &body);
    })
    .is_ok()
}

#[test]
fn the_checks_notice_a_name_that_is_not_defined() {
    assert!(scoped(
        &["InRange"],
        &["count"],
        &[],
        "InRange => (count < 3)"
    ));
    assert!(!scoped(&["InRange"], &[], &[], "InRange => (count < 3)"));
}

#[test]
fn the_checks_notice_a_name_used_after_its_scope_has_closed() {
    assert!(scoped(&[], &["s"], &[], "(\\A item \\in s : (item < 2))"));
    assert!(!scoped(
        &[],
        &["s"],
        &[],
        "(\\A item \\in s : TRUE) /\\ (item < 2)"
    ));
    assert!(!scoped(&[], &["s"], &[], "{item \\in s : TRUE} = {item}"));
    assert!(!scoped(&[], &[], &[], "(LET local == 1 IN local) + local"));
}

#[test]
fn the_checks_notice_a_name_that_hides_another() {
    assert!(!scoped(&[], &["s"], &[], "(\\A s \\in {1} : TRUE)"));
    assert!(!scoped(
        &[],
        &["s"],
        &[],
        "(\\A item \\in s : (\\E item \\in s : TRUE))"
    ));
    assert!(!scoped(&[], &[], &["seq"], "(LET seq == 1 IN seq)"));
}

#[test]
fn the_checks_let_two_scopes_side_by_side_use_one_name() {
    assert!(scoped(
        &[],
        &["s"],
        &[],
        "(\\A item \\in s : (item < 2)) /\\ (\\E item \\in s : (item > 0))"
    ));
    assert!(scoped(
        &[],
        &[],
        &[],
        "(LET local == 1 IN local) + (LET local == 2 IN local)"
    ));
}

#[test]
fn the_checks_know_every_way_the_export_binds_a_name() {
    assert!(scoped(
        &["S", "T", "SelectSeq"],
        &[],
        &["seq"],
        "\\E a \\in S, b \\in T : (LET local == a IN (LET Keep(item) == (item = b) IN \
         SelectSeq({other \\in {seq[index] : index \\in DOMAIN seq} : TRUE}, Keep)) = \
         [key \\in S |-> local])"
    ));
}

#[test]
fn the_checks_notice_a_prime_on_something_that_is_not_a_variable() {
    assert!(scoped(&[], &["x"], &[], "x' = 1"));
    assert!(!scoped(&["Init"], &["x"], &[], "Init' = 1"));
}

#[test]
fn the_checks_notice_a_record_field_the_language_cannot_spell() {
    assert!(scoped(&[], &["r"], &[], "r.qty = [qty |-> 1].qty"));
    assert!(!scoped(&[], &["r"], &[], "r.IF"));
    assert!(!scoped(&[], &["r"], &[], "[r EXCEPT !.WF_x = 1]"));
}

#[test]
fn the_checks_notice_a_delimiter_left_open() {
    let caught = std::panic::catch_unwind(|| assert_delimiters_balance(&tokens("<<(1, 2>>")));
    assert!(caught.is_err(), "the `(` is closed by `>>`");
}

#[test]
fn the_checks_read_signs_texts_and_names_apart() {
    assert_eq!(
        tokens("x' = <<\"a \\\" b\">> /\\ y \\in 0..3"),
        [
            name("x"),
            sign("'"),
            sign("="),
            sign("<<"),
            Token::Text,
            sign(">>"),
            sign("/\\"),
            name("y"),
            sign("\\in"),
            Token::Number,
            sign(".."),
            Token::Number,
        ]
    );
}
