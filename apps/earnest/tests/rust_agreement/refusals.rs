use crate::harness::{Crate, Sandbox};

struct Case {
    name: &'static str,
    method: &'static str,
    call: &'static str,
    says: &'static str,
    rustc: Option<&'static str>,
}

const BASE: &str = r#"use earnest::model::{Domain, Model, model, require};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Phase {
    Idle,
    Busy,
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Mode {
    Slow,
    Fast,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
struct Line {
    #[range(0, 2)]
    qty: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Shop {
    #[initial(Phase::Idle)]
    phase: Phase,
    #[initial(Mode::Slow)]
    mode: Mode,
    #[range(0, 3)]
    #[initial(0)]
    n: u8,
    #[range(0, 2)]
    #[initial(None)]
    last: Option<u8>,
    #[initial(None)]
    line: Option<Line>,
}

#[model]
impl Shop {
    fn with_phase(&self, phase: Phase) -> Self {
        Self { phase, ..self.clone() }
    }

    METHOD

    #[rule("fine")]
    fn fine(&self) -> bool {
        self.n <= 3 && CALL
    }
}
"#;

const CASES: [Case; 21] = [
    Case {
        call: "true",
        name: "a match that leaves a variant out",
        method: "fn a(&self) -> u8 { match self.phase { Phase::Idle => 1, Phase::Busy => 2 } }",
        says: "the arms do not cover `Done`",
        rustc: Some("E0004"),
    },
    Case {
        call: "true",
        name: "a match arm of another enum",
        method: "fn a(&self) -> u8 { match self.mode { Mode::Slow => 1, Phase::Idle => 2, Mode::Fast => 3 } }",
        says: "not a variant of the enum being matched",
        rustc: Some("E0308"),
    },
    Case {
        call: "true",
        name: "a match that leaves one pair out",
        method: "fn a(&self) -> u8 { match (self.phase, self.mode) { (Phase::Idle, _) => 1, (Phase::Busy, _) => 2, (Phase::Done, Mode::Slow) => 3 } }",
        says: "the arms do not cover `(Done, Fast)`",
        rustc: Some("E0004"),
    },
    Case {
        call: "true",
        name: "a guard on a match arm",
        method: "fn a(&self) -> u8 { match self.phase { Phase::Idle if self.n > 1 => 1, _ => 2 } }",
        says: "a guard is outside the library",
        rustc: None,
    },
    Case {
        call: "true",
        name: "a name that catches the rest of a match",
        method: "fn a(&self) -> u8 { match self.phase { Phase::Idle => 1, other => 2 } }",
        says: "a name catches every value and binds it",
        rustc: None,
    },
    Case {
        call: "true",
        name: "a match on a number",
        method: "fn a(&self) -> u8 { match self.n { 0 => 1, _ => 2 } }",
        says: "`match` is read on a value of an enum",
        rustc: None,
    },
    Case {
        call: "true",
        name: "unwrap",
        method: "fn a(&self) -> u8 { self.last.unwrap() }",
        says: "`unwrap` and `expect` panic on `None`",
        rustc: None,
    },
    Case {
        call: "true",
        name: "expect",
        method: "fn a(&self) -> u8 { self.last.expect(\"present\") }",
        says: "`unwrap` and `expect` panic on `None`",
        rustc: None,
    },
    Case {
        call: "true",
        name: "an if let on a reference to an option",
        method: "fn a(&self) -> u8 { if let Some(x) = &self.last { x + 1 } else { 0 } }",
        says: "binds the value inside by reference",
        rustc: None,
    },
    Case {
        call: "true",
        name: "an if let that moves a value out of the state",
        method: "fn a(&self) -> u8 { if let Some(l) = self.line { l.qty } else { 0 } }",
        says: "so the type must be `Copy`",
        rustc: Some("E0507"),
    },
    Case {
        call: "true",
        name: "a let Some without else",
        method: "fn a(&self) -> u8 { let Some(x) = self.last; x }",
        says: "can fail, and Rust refuses it without an `else`",
        rustc: Some("E0005"),
    },
    Case {
        call: "true",
        name: "a let else that gives something else",
        method: "fn a(&self) -> Option<Self> { let Some(x) = self.last else { return Some(self.clone()); }; Some(Self { n: x, ..self.clone() }) }",
        says: "read only as `else { return None; }`",
        rustc: None,
    },
    Case {
        call: "true",
        name: "a question mark in a rule",
        method: "fn a(&self) -> bool { let x = self.last?; x == 1 }",
        says: "read only in an action that returns `Option<Self>`",
        rustc: Some("E0277"),
    },
    Case {
        call: "true",
        name: "a question mark in an action that returns Self",
        method: "#[action(\"go\")] fn a(&self) -> Self { let x = self.last?; Self { n: x, ..self.clone() } }",
        says: "read only in an action that returns `Option<Self>`",
        rustc: Some("E0277"),
    },
    Case {
        call: "self.a() < 9",
        name: "a conversion that could lose a number",
        method: "fn a(&self) -> u8 { let wide: i32 = 3; u8::from(wide) }",
        says: "Rust has no `From<i32>` for `u8`",
        rustc: Some("E0277"),
    },
    Case {
        call: "self.a() < 9",
        name: "a conversion to a narrower unsigned type",
        method: "fn a(&self) -> u16 { let wide: u32 = 3; u16::from(wide) }",
        says: "Rust has no `From<u32>` for `u16`",
        rustc: Some("E0277"),
    },
    Case {
        call: "self.a()",
        name: "a state-returning helper read as a value",
        method: "fn a(&self) -> bool { self.with_phase(Phase::Busy).n == 0 }",
        says: "a helper that returns `Self` can be given back by an action",
        rustc: None,
    },
    Case {
        call: "true",
        name: "a dereference",
        method: "fn a(&self, last: &Option<u8>) -> bool { *last == Some(1) }",
        says: "Take the parameter by value",
        rustc: None,
    },
    Case {
        call: "true",
        name: "a helper that returns a type the checker does not know",
        method: "fn a(&self) -> Vec<u8> { Vec::new() }",
        says: "the checker knows `bool`, the integer types",
        rustc: None,
    },
    Case {
        call: "true",
        name: "an option method the checker does not read",
        method: "fn a(&self) -> u8 { self.last.map(|x| x + 1).unwrap_or(2) }",
        says: "write `value.map_or(default, |x| ..)`",
        rustc: None,
    },
    Case {
        call: "true",
        name: "a cast",
        method: "fn a(&self) -> u16 { self.n as u16 }",
        says: "widen a number with `u16::from(small)`",
        rustc: None,
    },
];

fn file_of(case: &Case) -> String {
    BASE.replace("METHOD", case.method)
        .replace("CALL", case.call)
}

#[test]
fn each_form_the_checker_will_not_read_is_refused_with_a_sentence_that_says_what_to_write() {
    let sandbox = Sandbox::new();
    for case in &CASES {
        let text = file_of(case);
        sandbox.write("model.rs", &text);
        let run = sandbox.earnest(&["verify", "model.rs", "--no-cache"]);
        let shown = run.text();
        assert_eq!(run.code, Some(1), "{}: refused\n{shown}", case.name);
        assert!(
            shown.contains("unreadable"),
            "{}: unreadable\n{shown}",
            case.name
        );
        assert!(
            shown.contains(case.says),
            "{}: the sentence says `{}`\n{shown}",
            case.name,
            case.says
        );
    }
}

#[test]
fn a_form_that_rustc_rejects_is_one_the_checker_refuses() {
    for (index, case) in CASES
        .iter()
        .enumerate()
        .filter(|(_, case)| case.rustc.is_some())
    {
        let name = format!("refused{index}");
        let compiled = Crate::make(&name, &[("model".to_owned(), file_of(case))]);
        let refusal = compiled
            .built()
            .expect_err(&format!("rustc accepts {}", case.name));
        let code = case.rustc.expect("filtered to the cases with a code");
        assert!(
            refusal.contains(code),
            "{}: rustc says {code}\n{refusal}",
            case.name
        );
    }
}
