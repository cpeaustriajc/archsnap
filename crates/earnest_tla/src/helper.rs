use crate::names::Names;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Helper {
    Fault,
    Remainder,
    EmptyMap,
    MapSet,
    MapDelete,
    MapGet,
    MapGetOptional,
    SeqGet,
    SeqGetOptional,
    SeqFirst,
    SeqFirstOptional,
    SeqLast,
    SeqLastOptional,
    SeqShift,
    SeqPop,
    SeqIncludes,
    SeqItems,
}

pub(crate) const EVERY_HELPER: [Helper; 17] = [
    Helper::Fault,
    Helper::Remainder,
    Helper::EmptyMap,
    Helper::MapSet,
    Helper::MapDelete,
    Helper::MapGet,
    Helper::MapGetOptional,
    Helper::SeqGet,
    Helper::SeqGetOptional,
    Helper::SeqFirst,
    Helper::SeqFirstOptional,
    Helper::SeqLast,
    Helper::SeqLastOptional,
    Helper::SeqShift,
    Helper::SeqPop,
    Helper::SeqIncludes,
    Helper::SeqItems,
];

pub(crate) const REMAINDER_FAULT: &str = "a remainder with a negative number or a divisor below 1";

impl Helper {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Fault => "Fault",
            Self::Remainder => "Remainder",
            Self::EmptyMap => "EmptyMap",
            Self::MapSet => "MapSet",
            Self::MapDelete => "MapDelete",
            Self::MapGet => "MapGet",
            Self::MapGetOptional => "MapGetOptional",
            Self::SeqGet => "SeqGet",
            Self::SeqGetOptional => "SeqGetOptional",
            Self::SeqFirst => "SeqFirst",
            Self::SeqFirstOptional => "SeqFirstOptional",
            Self::SeqLast => "SeqLast",
            Self::SeqLastOptional => "SeqLastOptional",
            Self::SeqShift => "SeqShift",
            Self::SeqPop => "SeqPop",
            Self::SeqIncludes => "SeqIncludes",
            Self::SeqItems => "SeqItems",
        }
    }

    pub(crate) fn needs(self) -> Option<Self> {
        match self {
            Self::Remainder => Some(Self::Fault),
            _ => None,
        }
    }

    pub(crate) fn definition(self, module: &Names) -> String {
        let mut names = module.clone();
        let name = self.name();
        match self {
            Self::Fault => {
                let message = names.claim("message");
                format!("{name}({message}) == Assert(FALSE, {message})")
            }
            Self::Remainder => {
                let left = names.claim("left");
                let right = names.claim("right");
                format!(
                    "{name}({left}, {right}) == IF ({left} >= 0) /\\ ({right} >= 1) \
                     THEN {left} % {right} ELSE Fault(\"{REMAINDER_FAULT}\")"
                )
            }
            Self::EmptyMap => {
                let key = names.claim("key");
                format!("{name} == [{key} \\in {{}} |-> {key}]")
            }
            Self::MapSet | Self::MapDelete | Self::MapGet | Self::MapGetOptional => {
                on_a_map(self, &mut names)
            }
            Self::SeqGet | Self::SeqGetOptional => {
                let seq = names.claim("seq");
                let index = names.claim("index");
                let found = wrapped(self, &format!("{seq}[{index} + 1]"));
                format!(
                    "{name}({seq}, {index}) == IF ({index} >= 0) /\\ ({index} < Len({seq})) \
                     THEN {found} ELSE <<>>"
                )
            }
            _ => on_a_seq(self, &mut names),
        }
    }
}

fn wrapped(helper: Helper, found: &str) -> String {
    match helper {
        Helper::MapGetOptional
        | Helper::SeqGetOptional
        | Helper::SeqFirstOptional
        | Helper::SeqLastOptional => found.to_owned(),
        _ => format!("<<{found}>>"),
    }
}

fn on_a_map(helper: Helper, names: &mut Names) -> String {
    let name = helper.name();
    let map = names.claim("map");
    let key = names.claim("key");
    match helper {
        Helper::MapSet => {
            let value = names.claim("value");
            let other = names.claim("other");
            format!(
                "{name}({map}, {key}, {value}) == [{other} \\in (DOMAIN {map}) \\cup {{{key}}} \
                 |-> IF {other} = {key} THEN {value} ELSE {map}[{other}]]"
            )
        }
        Helper::MapDelete => {
            let other = names.claim("other");
            format!(
                "{name}({map}, {key}) == [{other} \\in (DOMAIN {map}) \\ {{{key}}} \
                 |-> {map}[{other}]]"
            )
        }
        _ => {
            let found = wrapped(helper, &format!("{map}[{key}]"));
            format!("{name}({map}, {key}) == IF {key} \\in DOMAIN {map} THEN {found} ELSE <<>>")
        }
    }
}

fn on_a_seq(helper: Helper, names: &mut Names) -> String {
    let name = helper.name();
    let seq = names.claim("seq");
    match helper {
        Helper::SeqFirst | Helper::SeqFirstOptional => {
            let found = wrapped(helper, &format!("Head({seq})"));
            format!("{name}({seq}) == IF Len({seq}) = 0 THEN <<>> ELSE {found}")
        }
        Helper::SeqLast | Helper::SeqLastOptional => {
            let found = wrapped(helper, &format!("{seq}[Len({seq})]"));
            format!("{name}({seq}) == IF Len({seq}) = 0 THEN <<>> ELSE {found}")
        }
        Helper::SeqShift => {
            format!("{name}({seq}) == IF Len({seq}) = 0 THEN {seq} ELSE Tail({seq})")
        }
        Helper::SeqPop => format!(
            "{name}({seq}) == IF Len({seq}) = 0 THEN {seq} ELSE SubSeq({seq}, 1, Len({seq}) - 1)"
        ),
        Helper::SeqIncludes => {
            let item = names.claim("item");
            let index = names.claim("index");
            format!(
                "{name}({seq}, {item}) == \\E {index} \\in DOMAIN {seq} : {seq}[{index}] = {item}"
            )
        }
        _ => {
            let index = names.claim("index");
            format!("{name}({seq}) == {{{seq}[{index}] : {index} \\in DOMAIN {seq}}}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{EVERY_HELPER, Helper};
    use crate::names::Names;

    fn module() -> Names {
        Names::of_a_module(EVERY_HELPER.iter().map(|helper| helper.name()))
    }

    fn defined(helper: Helper) -> String {
        helper.definition(&module())
    }

    #[test]
    fn a_fault_stops_the_checker_with_its_message() {
        assert_eq!(
            defined(Helper::Fault),
            "Fault(message) == Assert(FALSE, message)"
        );
    }

    #[test]
    fn a_remainder_is_a_fault_where_the_engine_has_one() {
        assert_eq!(
            defined(Helper::Remainder),
            "Remainder(left, right) == IF (left >= 0) /\\ (right >= 1) THEN left % right \
             ELSE Fault(\"a remainder with a negative number or a divisor below 1\")"
        );
    }

    #[test]
    fn the_empty_map_is_the_function_of_no_key() {
        assert_eq!(
            defined(Helper::EmptyMap),
            "EmptyMap == [key \\in {} |-> key]"
        );
    }

    #[test]
    fn setting_a_key_adds_it_to_the_keys_and_keeps_the_other_values() {
        assert_eq!(
            defined(Helper::MapSet),
            "MapSet(map, key, value) == [other \\in (DOMAIN map) \\cup {key} |-> \
             IF other = key THEN value ELSE map[other]]"
        );
    }

    #[test]
    fn deleting_a_key_takes_it_from_the_keys() {
        assert_eq!(
            defined(Helper::MapDelete),
            "MapDelete(map, key) == [other \\in (DOMAIN map) \\ {key} |-> map[other]]"
        );
    }

    #[test]
    fn a_map_read_is_null_for_a_missing_key_and_wraps_a_plain_value() {
        assert_eq!(
            defined(Helper::MapGet),
            "MapGet(map, key) == IF key \\in DOMAIN map THEN <<map[key]>> ELSE <<>>"
        );
        assert_eq!(
            defined(Helper::MapGetOptional),
            "MapGetOptional(map, key) == IF key \\in DOMAIN map THEN map[key] ELSE <<>>"
        );
    }

    #[test]
    fn a_position_counts_from_zero_and_is_null_outside_the_sequence() {
        assert_eq!(
            defined(Helper::SeqGet),
            "SeqGet(seq, index) == IF (index >= 0) /\\ (index < Len(seq)) \
             THEN <<seq[index + 1]>> ELSE <<>>"
        );
        assert_eq!(
            defined(Helper::SeqGetOptional),
            "SeqGetOptional(seq, index) == IF (index >= 0) /\\ (index < Len(seq)) \
             THEN seq[index + 1] ELSE <<>>"
        );
    }

    #[test]
    fn the_first_and_last_items_are_null_when_there_is_none() {
        assert_eq!(
            defined(Helper::SeqFirst),
            "SeqFirst(seq) == IF Len(seq) = 0 THEN <<>> ELSE <<Head(seq)>>"
        );
        assert_eq!(
            defined(Helper::SeqFirstOptional),
            "SeqFirstOptional(seq) == IF Len(seq) = 0 THEN <<>> ELSE Head(seq)"
        );
        assert_eq!(
            defined(Helper::SeqLast),
            "SeqLast(seq) == IF Len(seq) = 0 THEN <<>> ELSE <<seq[Len(seq)]>>"
        );
        assert_eq!(
            defined(Helper::SeqLastOptional),
            "SeqLastOptional(seq) == IF Len(seq) = 0 THEN <<>> ELSE seq[Len(seq)]"
        );
    }

    #[test]
    fn taking_an_item_from_an_empty_sequence_leaves_it_empty() {
        assert_eq!(
            defined(Helper::SeqShift),
            "SeqShift(seq) == IF Len(seq) = 0 THEN seq ELSE Tail(seq)"
        );
        assert_eq!(
            defined(Helper::SeqPop),
            "SeqPop(seq) == IF Len(seq) = 0 THEN seq ELSE SubSeq(seq, 1, Len(seq) - 1)"
        );
    }

    #[test]
    fn a_sequence_includes_an_item_at_some_position() {
        assert_eq!(
            defined(Helper::SeqIncludes),
            "SeqIncludes(seq, item) == \\E index \\in DOMAIN seq : seq[index] = item"
        );
        assert_eq!(
            defined(Helper::SeqItems),
            "SeqItems(seq) == {seq[index] : index \\in DOMAIN seq}"
        );
    }

    #[test]
    fn a_parameter_steps_aside_for_a_name_the_module_has_taken() {
        let mut names = module();
        assert_eq!(names.claim("seq"), "seq");
        assert_eq!(
            Helper::SeqShift.definition(&names),
            "SeqShift(seq_2) == IF Len(seq_2) = 0 THEN seq_2 ELSE Tail(seq_2)"
        );
    }

    #[test]
    fn a_helper_that_uses_another_names_it() {
        assert_eq!(Helper::Remainder.needs(), Some(Helper::Fault));
        assert_eq!(Helper::SeqPop.needs(), None);
    }

    #[test]
    fn every_helper_is_listed_once_in_the_order_it_is_written() {
        let mut sorted = EVERY_HELPER.to_vec();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted, EVERY_HELPER.to_vec());
    }
}
