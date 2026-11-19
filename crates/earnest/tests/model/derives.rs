use crate::Guest::{self, Ann, Bob, Cy};
use earnest::model::{Domain, Lock, Map, Model, Queue, Request, Retry, Seq, Set, Timer};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
struct Seat {
    #[range(1, 2)]
    row: u8,
    aisle: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Lamp {
    #[initial(Bob)]
    owner: Guest,
    power: bool,
    #[range(0, 2)]
    level: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Hall {
    usher: Guest,
    reserved: Option<Guest>,
    seat: Seat,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Workshop {
    inside: Set<Guest>,
    stock: Map<Guest, bool>,
    trail: Seq<Guest, 2>,
    inbox: Queue<Guest, 2>,
    tray: Lock,
    job: Lock<Guest>,
    timer: Timer,
    request: Request,
    retry: Retry<2>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Settled {
    #[initial(true)]
    open: bool,
    #[range(0, 9)]
    #[initial(4)]
    level: u8,
    #[initial(None)]
    reserved: Option<Guest>,
    #[initial(Seat { row: 2, aisle: true })]
    seat: Seat,
    #[initial(Set::of([Ann, Cy]))]
    inside: Set<Guest>,
    #[initial(Seq::of([Bob]))]
    trail: Seq<Guest, 2>,
    #[initial(Map::of([(Ann, true)]))]
    stock: Map<Guest, bool>,
    #[initial(Timer::default().start())]
    timer: Timer,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Ledger {
    #[range(1, 2)]
    tip: Option<u8>,
    #[range(0, 9)]
    tab: Map<Guest, u8>,
}

#[test]
fn a_range_reaches_the_number_inside_an_option_and_leaves_a_collection_empty() {
    let ledger = |tip| Ledger {
        tip,
        tab: Map::of([]),
    };
    assert_eq!(
        Ledger::starts(),
        [ledger(None), ledger(Some(1)), ledger(Some(2))],
        "no tip, then each tip in the range; the map starts empty"
    );
}

#[test]
fn an_enum_lists_its_variants_in_source_order() {
    assert_eq!(Guest::values(), [Ann, Bob, Cy], "the three guests");
}

#[test]
fn bool_and_option_have_a_domain_already() {
    assert_eq!(bool::values(), [false, true], "false first");
    assert_eq!(
        Option::<Guest>::values(),
        [None, Some(Ann), Some(Bob), Some(Cy)],
        "None first, then each guest"
    );
}

#[test]
fn a_struct_lists_every_combination_with_the_first_field_slowest() {
    let seat = |row, aisle| Seat { row, aisle };
    assert_eq!(
        Seat::values(),
        [seat(1, false), seat(1, true), seat(2, false), seat(2, true)],
        "two rows times two sides"
    );
}

#[test]
fn a_field_with_an_initial_value_starts_there_and_the_others_at_every_value() {
    let lamp = |power, level| Lamp {
        owner: Bob,
        power,
        level,
    };
    assert_eq!(
        Lamp::starts(),
        [
            lamp(false, 0),
            lamp(false, 1),
            lamp(false, 2),
            lamp(true, 0),
            lamp(true, 1),
            lamp(true, 2),
        ],
        "one owner, two powers, three levels"
    );
}

#[test]
fn an_enum_an_option_and_a_record_without_an_initial_value_start_at_every_value() {
    let starts = Hall::starts();
    assert_eq!(starts.len(), 48, "3 ushers x 4 reservations x 4 seats");
    let first = Hall {
        usher: Ann,
        reserved: None,
        seat: Seat {
            row: 1,
            aisle: false,
        },
    };
    let last = Hall {
        usher: Cy,
        reserved: Some(Cy),
        seat: Seat {
            row: 2,
            aisle: true,
        },
    };
    assert_eq!(
        starts.first(),
        Some(&first),
        "every field at its first value"
    );
    assert_eq!(starts.last(), Some(&last), "every field at its last value");
    let mut distinct = starts.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(distinct.len(), 48, "no start state is listed twice");
    let middle = Hall {
        usher: Bob,
        reserved: Some(Ann),
        seat: Seat {
            row: 2,
            aisle: false,
        },
    };
    assert!(starts.contains(&middle), "a combination from the middle");
}

#[test]
fn collections_and_parts_start_empty_free_idle_or_at_zero() {
    let starts = Workshop::starts();
    assert_eq!(starts.len(), 1, "one start state");
    let start = &starts[0];
    assert!(start.inside.is_empty(), "the set is empty");
    assert!(start.stock.is_empty(), "the map is empty");
    assert!(start.trail.is_empty(), "the sequence is empty");
    assert!(start.inbox.is_empty(), "the queue is empty");
    assert!(start.tray.is_free(), "the plain lock is free");
    assert!(start.job.is_free(), "the owned lock is free");
    assert!(start.timer.is_idle(), "the timer is idle");
    assert!(start.request.is_idle(), "the request is idle");
    assert_eq!(start.retry.attempts(), 0, "the retry counter is at zero");
}

#[test]
fn every_kind_of_field_takes_an_initial_value() {
    let wanted = Settled {
        open: true,
        level: 4,
        reserved: None,
        seat: Seat {
            row: 2,
            aisle: true,
        },
        inside: Set::of([Ann, Cy]),
        trail: Seq::of([Bob]),
        stock: Map::of([(Ann, true)]),
        timer: Timer::default().start(),
    };
    assert_eq!(Settled::starts(), [wanted], "the one start state");
}
