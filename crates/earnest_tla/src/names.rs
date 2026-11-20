use std::collections::BTreeSet;

const WORDS_OF_THE_LANGUAGE: &[&str] = &[
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

const NAMES_OF_THE_STANDARD_MODULES: &[&str] = &[
    "Naturals",
    "Integers",
    "Sequences",
    "FiniteSets",
    "TLC",
    "Nat",
    "Int",
    "Seq",
    "Len",
    "Head",
    "Tail",
    "Append",
    "SubSeq",
    "SelectSeq",
    "IsFiniteSet",
    "Cardinality",
    "Print",
    "PrintT",
    "Assert",
    "JavaTime",
    "TLCGet",
    "TLCSet",
    "Permutations",
    "SortSeq",
    "RandomElement",
    "Any",
    "ToString",
    "TLCEval",
];

pub(crate) const STARTS: &str = "Init";
pub(crate) const STEPS: &str = "Next";
pub(crate) const EVERY_VARIABLE: &str = "vars";
pub(crate) const IN_RANGE: &str = "InRange";
pub(crate) const STOP: &str = "Stop";

const NAMES_OF_THE_EXPORT: &[&str] = &[STARTS, STEPS, EVERY_VARIABLE, IN_RANGE, STOP];

const FAIRNESS_PREFIXES: &[&str] = &["WF_", "SF_"];

pub(crate) fn spelled(wanted: &str) -> String {
    let mut name: String = wanted
        .chars()
        .map(|letter| {
            if letter.is_ascii_alphanumeric() || letter == '_' {
                letter
            } else {
                '_'
            }
        })
        .collect();
    let starts_with_a_letter = name
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic());
    let reads_as_fairness = FAIRNESS_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix));
    if !starts_with_a_letter || reads_as_fairness {
        name.insert(0, 'v');
    }
    name
}

#[derive(Clone, Debug)]
pub(crate) struct Names {
    taken: BTreeSet<String>,
}

impl Names {
    pub(crate) fn of_the_language() -> Self {
        Self {
            taken: WORDS_OF_THE_LANGUAGE
                .iter()
                .chain(NAMES_OF_THE_STANDARD_MODULES)
                .map(|word| (*word).to_owned())
                .collect(),
        }
    }

    pub(crate) fn of_a_module(helpers: impl Iterator<Item = &'static str>) -> Self {
        let mut names = Self::of_the_language();
        names.taken.extend(
            NAMES_OF_THE_EXPORT
                .iter()
                .copied()
                .chain(helpers)
                .map(str::to_owned),
        );
        names
    }

    pub(crate) fn claim(&mut self, wanted: &str) -> String {
        let plain = spelled(wanted);
        let mut name = plain.clone();
        let mut copy = 2_u64;
        while self.taken.contains(&name) {
            name = format!("{plain}_{copy}");
            copy = copy.saturating_add(1);
        }
        self.taken.insert(name.clone());
        name
    }
}

#[cfg(test)]
mod tests {
    use super::{Names, spelled};

    #[test]
    fn a_name_the_language_can_spell_is_kept() {
        assert_eq!(spelled("pressBy"), "pressBy");
        assert_eq!(spelled("stays_locked"), "stays_locked");
        assert_eq!(spelled("x2"), "x2");
    }

    #[test]
    fn a_letter_the_language_lacks_becomes_an_underscore() {
        assert_eq!(spelled("price$"), "price_");
        assert_eq!(spelled("Add to cart"), "Add_to_cart");
        assert_eq!(spelled("café"), "caf_");
    }

    #[test]
    fn a_name_that_does_not_start_with_a_letter_gets_one() {
        assert_eq!(spelled("_hidden"), "v_hidden");
        assert_eq!(spelled("2nd"), "v2nd");
        assert_eq!(spelled(""), "v");
    }

    #[test]
    fn a_name_that_reads_as_a_fairness_operator_gets_a_letter_in_front() {
        assert_eq!(spelled("WF_vars"), "vWF_vars");
        assert_eq!(spelled("SF_x"), "vSF_x");
        assert_eq!(spelled("WFx"), "WFx");
    }

    #[test]
    fn a_free_name_is_claimed_as_it_is() {
        let mut names = Names::of_the_language();
        assert_eq!(names.claim("count"), "count");
    }

    #[test]
    fn a_taken_name_gets_the_first_free_number_from_two() {
        let mut names = Names::of_the_language();
        assert_eq!(names.claim("count"), "count");
        assert_eq!(names.claim("count"), "count_2");
        assert_eq!(names.claim("count"), "count_3");
        assert_eq!(names.claim("count_2"), "count_2_2");
    }

    #[test]
    fn a_word_of_the_language_is_never_handed_out() {
        let mut names = Names::of_the_language();
        assert_eq!(names.claim("IF"), "IF_2");
        assert_eq!(names.claim("Len"), "Len_2");
        assert_eq!(names.claim("if"), "if");
    }

    #[test]
    fn a_module_also_keeps_its_own_definitions_and_helpers() {
        let mut names = Names::of_a_module(["SeqShift"].into_iter());
        assert_eq!(names.claim("Init"), "Init_2");
        assert_eq!(names.claim("vars"), "vars_2");
        assert_eq!(names.claim("SeqShift"), "SeqShift_2");
    }

    #[test]
    fn two_names_that_spell_the_same_stay_apart() {
        let mut names = Names::of_the_language();
        assert_eq!(names.claim("a$"), "a_");
        assert_eq!(names.claim("a_"), "a__2");
    }
}
