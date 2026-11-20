use crate::Guest::{self, Ann, Bob};
use earnest::model::{Model, Move, Set, model, replay, require};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Door {
    inside: Set<Guest>,
    #[one_of("front", "back")]
    #[initial("front")]
    way: &'static str,
}

#[model]
impl Door {
    #[action("a guest arrives")]
    fn arrive(&self, guest: Guest) -> Option<Self> {
        require!(!self.inside.has(guest));
        Some(Self {
            inside: self.inside.add(guest),
            ..self.clone()
        })
    }

    #[action("the door is used the other way")]
    fn turn(&self, #[one_of("front", "back")] to: &'static str) -> Option<Self> {
        require!(self.way != to);
        Some(Self {
            way: to,
            ..self.clone()
        })
    }
}

fn start() -> Door {
    Door::starts().remove(0)
}

#[test]
fn a_replay_gives_every_state_on_the_way_start_first() {
    let path = [
        Move::new("arrive").with("guest", "Bob"),
        Move::new("turn").with("to", "back"),
        Move::new("arrive").with("guest", "Ann"),
    ];
    let states = replay(&start(), &path).expect("the path can be taken");
    let sets: Vec<Vec<Guest>> = states
        .iter()
        .map(|state| {
            [Ann, Bob]
                .into_iter()
                .filter(|g| state.inside.has(*g))
                .collect()
        })
        .collect();
    assert_eq!(sets, [vec![], vec![Bob], vec![Bob], vec![Ann, Bob]]);
    assert_eq!(states[1].way, "front");
    assert_eq!(
        states[2].way, "back",
        "a text is written as the report prints it"
    );
}

#[test]
fn a_replay_of_no_steps_is_the_start_state() {
    assert_eq!(replay(&start(), &[]), Ok(vec![start()]));
}

#[test]
fn a_step_that_can_not_happen_is_refused_with_its_number_and_what_could_happen() {
    let path = [
        Move::new("arrive").with("guest", "Ann"),
        Move::new("arrive").with("guest", "Ann"),
    ];
    let refused = replay(&start(), &path).expect_err("Ann is inside already");
    assert_eq!(refused.step, 2, "the second step");
    assert!(
        refused
            .reason
            .contains("`arrive[guest = Ann]` can't happen"),
        "{refused}"
    );
    assert!(
        refused.reason.contains("`arrive[guest = \"Bob\"]`")
            && refused.reason.contains("`turn[to = \"back\"]`"),
        "what could happen: {refused}"
    );
    assert_eq!(
        refused.to_string(),
        format!("step 2: {}", refused.reason),
        "the message names the step"
    );
}

#[test]
fn an_unknown_action_a_missing_input_and_an_extra_input_are_refused() {
    for path in [
        vec![Move::new("jump")],
        vec![Move::new("arrive")],
        vec![
            Move::new("arrive")
                .with("guest", "Ann")
                .with("guest", "Bob"),
        ],
        vec![Move::new("arrive").with("guest", "Zed")],
        vec![Move::new("turn").with("to", "side")],
    ] {
        let refused = replay(&start(), &path).expect_err("no such step");
        assert_eq!(refused.step, 1, "{path:?}");
    }
}

#[test]
fn a_move_prints_as_the_report_writes_a_step() {
    let step = Move::new("deliver")
        .with("letter", "Bill")
        .with("copies", "2");
    assert_eq!(step.to_string(), "deliver[letter = Bill, copies = 2]");
    assert_eq!(Move::new("push").to_string(), "push");
}

mod every_kind {
    use crate::Guest::{self, Bob};
    use earnest::model::{Domain, Model, Move, Report, model, replay, require};

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
    struct Spot {
        #[range(0, 1)]
        row: u8,
        lit: bool,
        who: Guest,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
    struct Pair {
        #[range(-1, 0)]
        low: i8,
        spot: Spot,
        #[one_of("a b", "c\"d")]
        tag: &'static str,
    }

    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
    struct Desk {
        #[initial(0)]
        #[range(0, 9)]
        taken: u8,
    }

    #[model]
    impl Desk {
        #[action("a guest and a spot")]
        fn take(
            &self,
            guest: Option<Guest>,
            spot: Spot,
            flag: Option<bool>,
            #[range(-2, 0)] sign: i8,
        ) -> Option<Self> {
            require!(self.taken < 9);
            let _ = (guest, spot, flag, sign);
            Some(Self {
                taken: self.taken + 1,
            })
        }

        #[action("words and a pair")]
        fn mark(
            &self,
            #[range(0, 2)] maybe: Option<u8>,
            #[one_of("y z", "q\"r")] word: &'static str,
            #[one_of("p", "q")] chosen: Option<&'static str>,
            pair: Option<Pair>,
        ) -> Option<Self> {
            require!(self.taken < 9);
            let _ = (maybe, word, chosen, pair);
            Some(Self {
                taken: self.taken + 1,
            })
        }
    }

    fn take(guest: &str, spot: &str, flag: &str, sign: &str) -> Move {
        Move::new("take")
            .with("guest", guest)
            .with("spot", spot)
            .with("flag", flag)
            .with("sign", sign)
    }

    fn mark(maybe: &str, word: &str, chosen: &str, pair: &str) -> Move {
        Move::new("mark")
            .with("maybe", maybe)
            .with("word", word)
            .with("chosen", chosen)
            .with("pair", pair)
    }

    fn can_be_replayed(path: &[Move]) -> bool {
        replay(&Desk::starts().remove(0), path).is_ok()
    }

    #[test]
    fn an_input_of_every_kind_is_written_as_the_json_and_the_text_report_print_it() {
        let json = [
            take("null", "{lit: true, row: 1, who: Bob}", "false", "-2"),
            mark(
                "2",
                "q\"r",
                "p",
                "{low: -1, spot: {lit: false, row: 0, who: Ann}, tag: \"c\\\"d\"}",
            ),
        ];
        let text = [
            take("null", "{row: 1, lit: true, who: \"Bob\"}", "false", "-2"),
            mark(
                "2",
                "\"q\\\"r\"",
                "\"p\"",
                "{low: -1, spot: {row: 0, lit: false, who: \"Ann\"}, tag: \"c\\\"d\"}",
            ),
        ];
        let some = [
            take("Bob", "{row: 0, lit: false, who: \"Cy\"}", "true", "0"),
            mark("null", "y z", "null", "null"),
        ];
        for path in [json, text, some] {
            assert!(can_be_replayed(&path), "{}", path[0]);
        }
    }

    #[test]
    fn a_value_the_checker_does_not_try_or_writes_differently_is_refused() {
        let taking = |changed: usize, value: &str| {
            let mut given = ["null", "{row: 1, lit: true, who: Bob}", "false", "-2"];
            given[changed] = value;
            take(given[0], given[1], given[2], given[3])
        };
        let marking = |changed: usize, value: &str| {
            let mut given = ["2", "y z", "p", "null"];
            given[changed] = value;
            mark(given[0], given[1], given[2], given[3])
        };
        assert!(can_be_replayed(&[taking(0, "null"), marking(0, "2")]));
        for (position, wrong) in [
            (0, "None"),
            (0, "Some(Bob)"),
            (0, "\"null\""),
            (1, "{row: 2, lit: true, who: Bob}"),
            (1, "{row: 1, lit: true}"),
            (1, "{row: 1, lit: true, who: Bob, extra: 1}"),
            (1, "Spot { row: 1, lit: true, who: Bob }"),
            (2, "\"false\""),
            (3, "1"),
            (3, "\"-2\""),
        ] {
            assert!(
                !can_be_replayed(&[taking(position, wrong)]),
                "`{wrong}` for input {position} of `take` is not a value the checker tries"
            );
        }
        for (position, wrong) in [
            (0, "3"),
            (1, "w"),
            (2, "r"),
            (3, "{low: 1}"),
            (3, "{low: 0}"),
        ] {
            assert!(
                !can_be_replayed(&[marking(position, wrong)]),
                "`{wrong}` for input {position} of `mark` is not a value the checker tries"
            );
        }
    }

    #[test]
    fn the_steps_hold_the_inputs_as_the_text_report_prints_them() {
        let start = Desk::starts().remove(0);
        let steps = earnest::model::Behavior::steps(&start);
        let found = steps
            .iter()
            .filter(|step| step.action == "take")
            .find(|step| {
                step.input
                    .iter()
                    .any(|(name, shown)| *name == "guest" && shown == "\"Bob\"")
                    && step.input.iter().any(|(name, shown)| {
                        *name == "spot" && shown == "{lit: true, row: 1, who: \"Bob\"}"
                    })
                    && step
                        .input
                        .iter()
                        .any(|(name, shown)| *name == "sign" && shown == "-2")
            })
            .expect("the report text of each kind");
        assert_eq!(found.label, "a guest and a spot");
        assert_eq!(Some(Bob).report(), "\"Bob\"");
        assert_eq!(
            Some(Spot {
                row: 0,
                lit: true,
                who: Bob
            })
            .report(),
            "{lit: true, row: 0, who: \"Bob\"}"
        );
    }
}
