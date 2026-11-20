use earnest::model::{Domain, Map, Queue, Report, Request, Retry, Seq, Set, Timer, Walk};
use std::cell::RefCell;
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Fruit {
    Pear,
    Apple,
    Fig,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Mood {
    Calm,
    Bold,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
struct Spot {
    #[range(0, 2)]
    zebra: u8,
    #[range(0, 2)]
    apple: u8,
}

#[test]
fn an_enum_is_walked_by_the_names_of_its_variants_not_the_order_declared() {
    let fruit = Set::of([Fruit::Pear, Fruit::Fig, Fruit::Apple]);
    assert_eq!(fruit.report(), "{\"Apple\", \"Fig\", \"Pear\"}");
    assert_eq!(format!("{fruit:?}"), "{Apple, Fig, Pear}");
    let seen = RefCell::new(Vec::new());
    assert!(fruit.every(|each| {
        seen.borrow_mut().push(each);
        true
    }));
    assert_eq!(seen.into_inner(), [Fruit::Apple, Fruit::Fig, Fruit::Pear]);
    assert_eq!(Fruit::values(), [Fruit::Pear, Fruit::Apple, Fruit::Fig]);
    assert_eq!(Fruit::Pear.walk_cmp(&Fruit::Apple), Ordering::Greater);
    assert_eq!(Mood::Calm.walk_cmp(&Mood::Bold), Ordering::Greater);
}

#[test]
fn a_quantifier_stops_at_the_first_item_in_the_order_of_the_names() {
    let fruit = Set::of([Fruit::Pear, Fruit::Fig, Fruit::Apple]);
    let tried = RefCell::new(Vec::new());
    assert!(fruit.some(|each| {
        tried.borrow_mut().push(each);
        each == Fruit::Fig
    }));
    assert_eq!(tried.into_inner(), [Fruit::Apple, Fruit::Fig]);
}

#[test]
fn a_text_is_walked_by_code_point_which_is_the_order_of_its_utf_8_bytes() {
    let texts = Set::of(["b", "a", "B", "\u{ff5e}", "\u{1f600}", "z", "\u{e9}"]);
    assert_eq!(
        texts.report(),
        "{\"B\", \"a\", \"b\", \"z\", \"\u{e9}\", \"\u{ff5e}\", \"\u{1f600}\"}"
    );
}

#[test]
fn numbers_ascend_and_false_comes_before_true() {
    assert_eq!(Set::of([3_u8, 1, 2]).report(), "{1, 2, 3}");
    assert_eq!(Set::of([-1_i8, -5, 0]).report(), "{-5, -1, 0}");
    assert_eq!(Set::of([true, false]).report(), "{false, true}");
}

#[test]
fn none_comes_before_every_present_value() {
    assert_eq!(
        Set::of([Some("b"), None, Some("a")]).report(),
        "{null, \"a\", \"b\"}"
    );
    assert_eq!(
        Set::of([Some(Fruit::Pear), Some(Fruit::Apple), None]).report(),
        "{null, \"Apple\", \"Pear\"}"
    );
}

#[test]
fn a_record_is_walked_field_by_field_in_the_order_of_the_field_names() {
    let spots = Set::of([
        Spot { zebra: 0, apple: 2 },
        Spot { zebra: 2, apple: 1 },
        Spot { zebra: 1, apple: 1 },
    ]);
    assert_eq!(
        spots.report(),
        "{{apple: 1, zebra: 1}, {apple: 1, zebra: 2}, {apple: 2, zebra: 0}}"
    );
    assert_eq!(
        Spot::values().first().map(Report::report),
        Some("{apple: 0, zebra: 0}".to_owned()),
        "a declared type is still walked in the order declared"
    );
}

#[test]
fn a_collection_is_walked_item_by_item_and_a_prefix_comes_first() {
    let nested = Set::of([
        Set::of([2_u8]),
        Set::of([1, 3]),
        Set::of([1, 2, 3]),
        Set::of([1, 2]),
        Set::of([]),
    ]);
    assert_eq!(nested.report(), "{{}, {1, 2}, {1, 2, 3}, {1, 3}, {2}}");
    let lines: Set<Seq<u8, 3>> =
        Set::of([Seq::of([2]), Seq::of([1, 1]), Seq::of([1]), Seq::of([])]);
    assert_eq!(lines.report(), "{[], [1], [1, 1], [2]}");
    let queues: Set<Queue<Fruit, 2>> = Set::of([
        Queue::default().send(Fruit::Pear),
        Queue::default().send(Fruit::Fig).send(Fruit::Pear),
        Queue::default().send(Fruit::Fig),
    ]);
    assert_eq!(
        queues.report(),
        "{[\"Fig\"], [\"Fig\", \"Pear\"], [\"Pear\"]}"
    );
}

#[test]
fn a_map_is_walked_by_its_keys_and_a_map_of_maps_entry_by_entry() {
    let stock = Map::of([(Fruit::Pear, 1_u8), (Fruit::Fig, 2), (Fruit::Apple, 3)]);
    assert_eq!(
        stock.report(),
        "{\"Apple\" -> 3, \"Fig\" -> 2, \"Pear\" -> 1}"
    );
    assert_eq!(format!("{stock:?}"), "{Apple: 3, Fig: 2, Pear: 1}");
    assert_eq!(stock.keys().report(), "{\"Apple\", \"Fig\", \"Pear\"}");
    let maps = Set::of([
        Map::of([(Fruit::Pear, 1_u8)]),
        Map::of([(Fruit::Apple, 2)]),
        Map::of([(Fruit::Apple, 1), (Fruit::Fig, 1)]),
        Map::of([(Fruit::Apple, 1)]),
    ]);
    assert_eq!(
        maps.report(),
        "{{\"Apple\" -> 1}, {\"Apple\" -> 1, \"Fig\" -> 1}, {\"Apple\" -> 2}, {\"Pear\" -> 1}}"
    );
}

#[test]
fn a_timer_a_request_and_a_retry_are_walked_by_the_text_the_checker_gives_them() {
    let timers = Set::of([
        Timer::default().start(),
        Timer::default(),
        Timer::default().fire(),
    ]);
    assert_eq!(timers.report(), "{\"fired\", \"idle\", \"running\"}");
    let requests = Set::of([
        Request::default().succeed(),
        Request::default().send(),
        Request::default().fail(),
        Request::default(),
    ]);
    assert_eq!(
        requests.report(),
        "{\"failed\", \"idle\", \"pending\", \"succeeded\"}"
    );
    let retries: Set<Retry<3>> = Set::of([Retry::default().again(), Retry::default()]);
    assert_eq!(retries.report(), "{0, 1}");
}

#[test]
fn a_set_made_by_adding_items_is_in_the_same_order_as_one_made_by_listing_them() {
    let added = Set::default()
        .add(Fruit::Pear)
        .add(Fruit::Apple)
        .add(Fruit::Fig);
    assert_eq!(added, Set::of([Fruit::Fig, Fruit::Pear, Fruit::Apple]));
    assert_eq!(added.report(), "{\"Apple\", \"Fig\", \"Pear\"}");
    assert_eq!(
        added.delete(Fruit::Fig).report(),
        "{\"Apple\", \"Pear\"}",
        "an item is deleted by its value"
    );
    assert!(added.has(Fruit::Fig) && !added.delete(Fruit::Fig).has(Fruit::Fig));
}
