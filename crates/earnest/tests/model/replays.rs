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
        refused.reason.contains("`arrive[guest = Bob]`")
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
