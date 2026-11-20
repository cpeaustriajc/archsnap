use crate::only_draft;
use earnest_rs::read;

const HEAD: &str = "\
use earnest::model::{Model, model, require};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Pile {
    #[range(0, 3)]
    #[initial(0)]
    height: u8,
    #[initial(false)]
    sealed: bool,
}

#[model]
impl Pile {
";

const CLOCK: &str = "the system clock is outside the library: model time with a `Timer`, or as a number that actions step up";
const LOOP: &str = "a loop is outside the library: use `every`, `some`, `forall` or `exists`";

fn pile(methods: &str) -> String {
    format!("{HEAD}{methods}}}\n")
}

fn refused(source: &str) -> Vec<(u32, u32, String, String)> {
    let reading = read("src/pile.rs", source);
    assert_eq!(
        reading.drafts,
        [],
        "no model of an unreadable file is given"
    );
    for problem in &reading.problems {
        assert_eq!(problem.file, "src/pile.rs", "the file as given");
    }
    reading
        .problems
        .into_iter()
        .map(|problem| {
            (
                problem.line,
                problem.column,
                problem.feature,
                problem.message,
            )
        })
        .collect()
}

fn places(source: &str) -> Vec<(u32, u32, String)> {
    refused(source)
        .into_iter()
        .map(|(line, column, feature, _)| (line, column, feature))
        .collect()
}

fn at(line: u32, column: u32, feature: &str) -> (u32, u32, String) {
    (line, column, feature.to_owned())
}

#[test]
fn a_loop_and_what_it_holds_are_refused() {
    let source = pile(
        "    #[rule(\"The pile is short\")]
    fn short(&self) -> bool {
        for step in 0..3 {
            println!(\"{step}\");
        }
        self.height < 3
    }
",
    );
    let problems = refused(&source);
    assert_eq!(problems.len(), 2, "the loop and the printing: {problems:?}");
    assert_eq!(
        problems[0],
        (16, 9, "`for`".to_owned(), LOOP.to_owned()),
        "the loop"
    );
    assert_eq!(
        (problems[1].0, problems[1].1, problems[1].2.as_str()),
        (17, 13, "`println!`"),
        "the printing inside it"
    );
    assert_eq!(
        problems[1].3, "printing is input and output, which is outside the library",
        "why printing is refused"
    );
}

#[test]
fn while_and_loop_are_refused_too() {
    let source = pile(
        "    #[rule(\"The pile is short\")]
    fn short(&self) -> bool {
        while self.sealed {}
        loop {}
    }
",
    );
    assert_eq!(
        places(&source),
        [at(16, 9, "`while`"), at(17, 9, "`loop`")],
        "both loops"
    );
}

#[test]
fn the_clock_is_refused_where_it_is_read_and_where_it_is_imported() {
    let source = pile(
        "    #[action(\"a block is added\")]
    fn add(&self) -> Option<Self> {
        let now = std::time::Instant::now();
        require!(self.height < 3);
        Some(Self { height: self.height + 1, ..self.clone() })
    }
",
    );
    assert_eq!(
        refused(&source),
        [(
            16,
            19,
            "`std::time::Instant::now`".to_owned(),
            CLOCK.to_owned()
        )],
        "one problem, though the guard reads the binding too"
    );
    let imported = "use earnest::model::Model;\nuse std::time::Instant;\n";
    assert_eq!(
        refused(imported),
        [(2, 5, "`std::time::Instant`".to_owned(), CLOCK.to_owned())],
        "the import"
    );
}

#[test]
fn random_values_and_files_are_refused() {
    let source = pile(
        "    #[action(\"a block is added\")]
    fn add(&self) -> Option<Self> {
        let roll = rand::random::<bool>();
        let saved = std::fs::read_to_string(\"pile.txt\");
        println!(\"added\");
        Some(Self { sealed: roll, ..self.clone() })
    }
",
    );
    assert_eq!(
        refused(&source),
        [
            (
                16,
                20,
                "`rand::random`".to_owned(),
                "random values are outside the library: model each possible value as an action input, with `#[one_of(..)]` or an enum that derives `Domain`, so every one is tried".to_owned()
            ),
            (
                17,
                21,
                "`std::fs::read_to_string`".to_owned(),
                "input and output are outside the library: model the answer as an action input, or the call as a `Request`".to_owned()
            ),
            (
                18,
                9,
                "`println!`".to_owned(),
                "printing is input and output, which is outside the library".to_owned()
            ),
        ],
        "each on its own line"
    );
}

#[test]
fn unsafe_and_an_unknown_macro_are_refused() {
    let source = pile(
        "    #[rule(\"The pile is short\")]
    fn short(&self) -> bool {
        let top = unsafe { peek() };
        let all = vec![self.height];
        top < 3
    }
",
    );
    let problems = refused(&source);
    assert_eq!(
        (problems[0].0, problems[0].1, problems[0].2.as_str()),
        (16, 19, "`unsafe`"),
        "the unsafe block"
    );
    assert_eq!(
        problems[0].3, "unsafe code is outside the library",
        "why it is refused"
    );
    assert_eq!(
        (problems[1].0, problems[1].1, problems[1].2.as_str()),
        (16, 28, "`peek`"),
        "the unknown function inside it"
    );
    assert_eq!(
        (problems[2].0, problems[2].1, problems[2].2.as_str()),
        (17, 19, "`vec!`"),
        "the macro"
    );
    assert_eq!(
        problems[2].3,
        "the checker does not know this macro: only `require!` is part of the library",
        "why it is refused"
    );
    assert_eq!(problems.len(), 3, "nothing else: {problems:?}");
}

#[test]
fn every_change_in_place_is_refused() {
    let source = pile(
        "    #[action(\"a block is added\")]
    fn add(&mut self) -> Option<Self> {
        let mut next = self.clone();
        next.height += 1;
        self.sealed = true;
        Some(next)
    }
",
    );
    assert_eq!(
        places(&source),
        [
            at(15, 12, "`&mut self`"),
            at(16, 9, "`let mut`"),
            at(17, 21, "`+=`"),
            at(18, 21, "`=`"),
        ],
        "four changes in place"
    );
    let messages: Vec<String> = refused(&source)
        .into_iter()
        .map(|problem| problem.3)
        .collect();
    assert!(
        messages[1].contains("a binding can't change"),
        "{messages:?}"
    );
    assert!(
        messages[2].contains("nothing changes in place"),
        "{messages:?}"
    );
}

#[test]
fn a_method_outside_the_library_is_refused_at_its_name() {
    let source = pile(
        "    #[rule(\"The pile is short\")]
    fn short(&self) -> bool {
        self.height.pow(2) < 9 && Some(self.sealed).unwrap()
    }
",
    );
    assert_eq!(
        places(&source),
        [at(16, 21, "`.pow()`"), at(16, 53, "`.unwrap()`")],
        "both calls"
    );
}

#[test]
fn constructs_outside_the_allowed_forms_are_each_refused() {
    let source = pile(
        "    #[rule(\"One\")]
    fn one(&self) -> bool {
        match self.height {
            0 => true,
            _ => false,
        }
    }

    #[rule(\"Two\")]
    fn two(&self) -> bool {
        self.height as u16 == 0
    }

    #[rule(\"Three\")]
    fn three(&self) -> bool {
        if let Some(height) = &Some(self.height) { height == &0 } else { false }
    }

    #[rule(\"Four\")]
    fn four(&self) -> bool {
        self.label() == 1.5
    }
",
    );
    assert_eq!(
        places(&source),
        [
            at(16, 15, "`self.height`"),
            at(24, 21, "`as`"),
            at(29, 31, "`&Some(self.height)`"),
            at(34, 14, "`self.label()`"),
            at(34, 25, "`1.5`"),
        ],
        "every problem of the file, in order"
    );
}

#[test]
fn methods_that_are_not_an_action_or_a_rule_in_form_are_refused() {
    let source = pile(
        "    #[action(\"a block is added\")]
    fn add(&self) -> bool {
        true
    }

    #[rule]
    fn short(&self, limit: bool) -> bool {
        require!(limit);
        self.height < 3
    }

    #[action(\"the pile is sealed\")]
    fn seal(&self) -> Option<Self> {
        self.sealed.clone();
        if self.sealed {
            self.height;
        }
        None
    }
",
    );
    assert_eq!(
        places(&source),
        [
            at(15, 22, "`fn add`"),
            at(19, 5, "`#[rule]`"),
            at(20, 21, "`short`"),
            at(21, 9, "`require!`"),
            at(27, 9, "`self.sealed.clone()`"),
            at(28, 9, "`if`"),
            at(31, 9, "`None`"),
        ],
        "every problem of the file, in order"
    );
}

#[test]
fn declarations_outside_a_model_are_refused() {
    let source = "\
use earnest::model::{Model, model};

pub enum Mode {
    On,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Lamp {
    level: u8,
    name: String,
    mode: Mode,
}

impl Lamp {
    fn lit(&self) -> bool {
        true
    }
}

static LIMIT: u8 = 3;
";
    let problems = refused(source);
    let found: Vec<(u32, u32, &str)> = problems
        .iter()
        .map(|(line, column, feature, _)| (*line, *column, feature.as_str()))
        .collect();
    assert_eq!(
        found,
        [
            (3, 5, "`enum Mode`"),
            (9, 12, "`u8`"),
            (10, 11, "`String`"),
            (11, 11, "`Mode`"),
            (14, 1, "`impl`"),
            (20, 1, "`static`"),
        ],
        "every problem of the file, in order"
    );
    assert_eq!(
        problems[1].3,
        "a number needs `#[range(lo, hi)]`, so the checker knows every value it can take",
        "what to write for a number"
    );
}

#[test]
fn text_that_is_not_rust_gives_one_problem() {
    let problems = refused("pub struct {\n");
    assert_eq!(problems.len(), 1, "one problem: {problems:?}");
    assert_eq!(problems[0].0, 1, "on the line the parser stopped at");
    assert!(
        problems[0]
            .3
            .starts_with("this is not Rust the reader can parse"),
        "{}",
        problems[0].3
    );
}

#[test]
fn a_column_counts_characters_and_a_rule_text_survives_them() {
    let unreadable =
        pile("    #[rule(\"Café ☕\")] fn brewed(&self) -> bool { self.height.pow(2) < 9 }\n");
    assert_eq!(
        places(&unreadable),
        [at(14, 62, "`.pow()`")],
        "61 characters come before the method name"
    );
    let readable = pile("    #[rule(\"Café ☕\")] fn brewed(&self) -> bool { self.height < 9 }\n");
    let draft = only_draft("src/pile.rs", &readable);
    assert_eq!(draft.rules[0].says, "Café ☕", "the sentence");
    assert_eq!(draft.rules[0].text, "self.height < 9", "the body's text");
}

const SHORT: &str = "    #[rule(\"The pile is short\")]
    fn short(&self) -> bool {
        self.height < 3
    }
";

#[test]
fn a_rule_kept_or_left_out_by_cfg_is_refused() {
    let source = pile(&format!("    #[cfg(feature = \"strict\")]\n{SHORT}"));
    assert_eq!(
        places(&source),
        [at(14, 5, "`#[cfg]`")],
        "the rule would be checked in one build and missing in another"
    );
    let renamed = pile(&SHORT.replace(
        "#[rule(",
        "#[cfg_attr(feature = \"strict\", rule(\"The pile is short\"))]\n    #[doc(alias = ",
    ));
    assert_eq!(
        places(&renamed),
        [at(14, 5, "`#[cfg_attr]`")],
        "an attribute that is there in some builds only"
    );
}

#[test]
fn a_model_block_or_a_state_only_in_test_builds_is_refused_and_not_skipped() {
    let block = pile(SHORT).replace("#[model]", "#[cfg(test)]\n#[model]");
    assert_eq!(
        places(&block),
        [at(12, 1, "`#[cfg]`")],
        "the block holds the model's rules"
    );
    let state = pile(SHORT).replace("#[derive", "#[cfg(test)]\n#[derive");
    assert_eq!(
        places(&state),
        [at(3, 1, "`#[cfg]`")],
        "the struct is the model"
    );
}

#[test]
fn a_statement_a_field_or_an_input_under_cfg_is_refused() {
    let guarded = pile(
        "    #[action(\"a box is added\")]
    fn add(&self, #[cfg(test)] #[range(1, 2)] boxes: u8) -> Option<Self> {
        #[cfg(debug_assertions)]
        require!(self.height < 3);
        #[cfg(test)]
        let next = self.height + boxes;
        Some(Self { #[cfg(test)] height: next, ..self.clone() })
    }
",
    );
    assert_eq!(
        places(&guarded),
        [
            at(15, 19, "`#[cfg]`"),
            at(16, 9, "`#[cfg]`"),
            at(18, 9, "`#[cfg]`"),
            at(20, 21, "`#[cfg]`")
        ],
        "an input, a `require!`, a `let` and a field of the result"
    );
    let field = pile(SHORT).replace(
        "    #[initial(false)]",
        "    #[cfg(test)]\n    #[initial(false)]",
    );
    assert_eq!(
        places(&field),
        [at(8, 5, "`#[cfg]`")],
        "a field of the state"
    );
}

#[test]
fn a_role_written_with_a_path_is_refused_because_the_macro_would_not_see_it() {
    let source = pile(&SHORT.replace("#[rule(", "#[earnest::model::rule("));
    assert_eq!(
        places(&source),
        [at(14, 5, "`#[earnest::model::rule]`")],
        "`#[model]` would leave the method a helper, and the rule unchecked"
    );
}

#[test]
fn tests_and_their_helpers_under_cfg_test_are_still_skipped() {
    let source = format!(
        "{}\n#[cfg(test)]\nfn loud() {{ println!(\"x\"); }}\n\n#[cfg(test)]\nimpl Pile {{ fn poke(&mut self) {{ self.height += 1; }} }}\n\n#[cfg(test)]\nmod tests {{ use std::time::Instant; }}\n",
        pile(SHORT)
    );
    let draft = only_draft("src/pile.rs", &source);
    assert_eq!(draft.rules.len(), 1, "the rule of the model is read");
}
