use crate::Guest::{self, Ann, Bob, Cy};
use earnest::model::{Map, Queue, Seq, Set, Start};

type Trail = Seq<Guest, 2>;
type Inbox = Queue<Guest, 2>;

#[test]
fn a_set_holds_each_item_once_whatever_the_order() {
    let pair = Set::of([Bob, Ann, Bob]);
    assert_eq!(pair.size(), 2, "Bob listed twice is held once");
    assert_eq!(pair, Set::of([Ann, Bob]), "order does not matter");
    assert_eq!(format!("{pair:?}"), "{Ann, Bob}", "printed in order");
    assert!(!pair.is_empty(), "two items");
    let empty = Set::<Guest>::of([]);
    assert!(empty.is_empty(), "nothing listed");
    assert_eq!(empty.size(), 0, "nothing listed");
    assert_eq!(Set::<Guest>::starts(), [empty], "a set field starts empty");
}

#[test]
fn a_set_answers_has_by_value_and_by_reference() {
    let pair = Set::of([Ann, Bob]);
    assert!(pair.has(Ann), "Ann by value");
    assert!(pair.has(Bob), "Bob by reference");
    assert!(!pair.has(Cy), "Cy is absent");
}

#[test]
fn add_and_delete_give_a_new_set_and_leave_the_old_one() {
    let one = Set::of([Ann]);
    let two = one.add(Bob);
    assert_eq!(two, Set::of([Ann, Bob]), "Bob added");
    assert_eq!(one, Set::of([Ann]), "the receiver of add is unchanged");
    assert_eq!(one.add(Ann), one, "adding a held item changes nothing");
    let back = two.delete(Bob);
    assert_eq!(back, one, "Bob deleted");
    assert_eq!(
        two,
        Set::of([Ann, Bob]),
        "the receiver of delete is unchanged"
    );
    assert_eq!(
        one.delete(Cy),
        one,
        "deleting an absent item changes nothing"
    );
}

#[test]
fn union_intersect_and_subtract_combine_two_sets() {
    let left = Set::of([Ann, Bob]);
    let right = Set::of([Bob, Cy]);
    assert_eq!(left.union(&right), Set::of([Ann, Bob, Cy]), "in either");
    assert_eq!(left.intersect(&right), Set::of([Bob]), "in both");
    assert_eq!(left.subtract(&right), Set::of([Ann]), "in the left only");
    assert_eq!(right.subtract(left.clone()), Set::of([Cy]), "by value");
    assert_eq!(left, Set::of([Ann, Bob]), "the receiver is unchanged");
    assert_eq!(right, Set::of([Bob, Cy]), "the argument is unchanged");
}

#[test]
fn is_subset_compares_item_by_item() {
    let small = Set::of([Ann]);
    let large = Set::of([Ann, Bob]);
    assert!(small.is_subset(&large), "Ann is in the larger set");
    assert!(!large.is_subset(&small), "Bob is not in the smaller set");
    assert!(large.is_subset(&large), "a set is a subset of itself");
    assert!(Set::of([]).is_subset(small), "the empty set fits anywhere");
}

#[test]
fn every_some_and_filter_test_each_item_of_a_set() {
    let pair = Set::of([Ann, Bob]);
    assert!(pair.every(|guest| guest != Cy), "neither is Cy");
    assert!(!pair.every(|guest| guest == Ann), "Bob is not Ann");
    assert!(pair.some(|guest| guest == Bob), "Bob is there");
    assert!(!pair.some(|guest| guest == Cy), "Cy is not");
    assert_eq!(pair.filter(|guest| guest != Ann), Set::of([Bob]), "kept");
    assert_eq!(pair, Set::of([Ann, Bob]), "the receiver is unchanged");
    let empty = Set::<Guest>::of([]);
    assert!(empty.every(|_| false), "every is true of nothing");
    assert!(!empty.some(|_| true), "some is false of nothing");
}

#[test]
fn a_map_keeps_one_value_for_a_key() {
    let stock = Map::of([(Ann, 1_u8), (Bob, 2), (Ann, 3)]);
    assert_eq!(stock.size(), 2, "Ann listed twice is one key");
    assert_eq!(stock.get(Ann), Some(3), "the last value listed wins");
    assert_eq!(stock.get(Bob), Some(2), "another guest");
    assert_eq!(stock.get(Cy), None, "no value for Cy");
    assert!(stock.has(Bob), "Bob is a key");
    assert!(!stock.has(Cy), "Cy is not");
    assert_eq!(format!("{stock:?}"), "{Ann: 3, Bob: 2}", "printed in order");
    let empty = Map::<Guest, u8>::of([]);
    assert!(empty.is_empty(), "nothing listed");
    assert!(!stock.is_empty(), "two keys");
    assert_eq!(
        Map::<Guest, u8>::starts(),
        [empty],
        "a map field starts empty"
    );
}

#[test]
fn set_and_delete_give_a_new_map_and_leave_the_old_one() {
    let one = Map::of([(Ann, 1_u8)]);
    let two = one.set(Bob, 2);
    assert_eq!(two, Map::of([(Ann, 1), (Bob, 2)]), "Bob set");
    assert_eq!(one, Map::of([(Ann, 1)]), "the receiver of set is unchanged");
    assert_eq!(two.set(Ann, 5).get(Ann), Some(5), "a value is replaced");
    assert_eq!(two.get(Ann), Some(1), "and the old map keeps the old value");
    assert_eq!(two.delete(Bob), one, "Bob deleted");
    assert_eq!(two.size(), 2, "the receiver of delete is unchanged");
    assert_eq!(
        one.delete(Cy),
        one,
        "deleting an absent key changes nothing"
    );
    assert_eq!(two.keys(), Set::of([Ann, Bob]), "the keys as a set");
}

#[test]
fn a_sequence_keeps_order_and_repeats() {
    let trail = Trail::of([Bob, Ann, Bob]);
    assert_eq!(
        trail.size(),
        3,
        "repeats are counted, and MAX is not enforced"
    );
    assert_eq!(trail.first(), Some(Bob), "the front");
    assert_eq!(trail.last(), Some(Bob), "the back");
    assert_eq!(trail.get(1), Some(Ann), "counted from 0");
    assert_eq!(trail.get(3), None, "past the back");
    assert!(trail.includes(Ann), "by value");
    assert!(!trail.includes(Cy), "another guest");
    assert_eq!(format!("{trail:?}"), "[Bob, Ann, Bob]", "printed in order");
    assert_ne!(trail, Trail::of([Ann, Bob, Bob]), "order matters");
    let empty = Trail::of([]);
    assert!(empty.is_empty(), "nothing listed");
    assert!(!trail.is_empty(), "three items");
    assert_eq!(empty.first(), None, "no front");
    assert_eq!(empty.last(), None, "no back");
    assert_eq!(Trail::starts(), [empty], "a sequence field starts empty");
}

#[test]
fn push_shift_and_pop_give_a_new_sequence_and_leave_the_old_one() {
    let one = Trail::of([Ann]);
    let two = one.push(Bob);
    assert_eq!(two, Trail::of([Ann, Bob]), "pushed at the back");
    assert_eq!(one, Trail::of([Ann]), "the receiver of push is unchanged");
    assert_eq!(two.shift(), Trail::of([Bob]), "the front is dropped");
    assert_eq!(two.pop(), Trail::of([Ann]), "the back is dropped");
    assert_eq!(two, Trail::of([Ann, Bob]), "the receiver is unchanged");
    let empty = Trail::of([]);
    assert_eq!(empty.shift(), empty, "an empty sequence stays empty");
    assert_eq!(empty.pop(), empty, "an empty sequence stays empty");
}

#[test]
fn every_some_and_filter_test_each_item_of_a_sequence() {
    let trail = Trail::of([Ann, Bob, Ann]);
    assert!(trail.every(|guest| guest != Cy), "none is Cy");
    assert!(!trail.every(|guest| guest == Ann), "Bob is not Ann");
    assert!(trail.some(|guest| guest == Bob), "Bob is there");
    assert!(!trail.some(|guest| guest == Cy), "Cy is not");
    let kept = trail.filter(|guest| guest == Ann);
    assert_eq!(kept, Trail::of([Ann, Ann]), "kept in order, repeats too");
    assert_eq!(trail.size(), 3, "the receiver is unchanged");
    assert!(Trail::of([]).every(|_| false), "every is true of nothing");
    assert!(!Trail::of([]).some(|_| true), "some is false of nothing");
}

#[test]
fn a_queue_hands_out_its_messages_in_the_order_sent() {
    let empty = Inbox::starts().remove(0);
    assert!(empty.is_empty(), "a queue field starts empty");
    assert_eq!(empty.size(), 0, "no message");
    assert_eq!(empty.head(), None, "no head");
    assert!(!empty.is_full(), "room for two");
    let one = empty.send(Bob);
    assert_eq!(one.size(), 1, "one sent");
    assert!(empty.is_empty(), "the receiver of send is unchanged");
    assert!(!one.is_full(), "room for one more");
    let two = one.send(Ann);
    assert!(two.is_full(), "at capacity");
    assert_eq!(two.head(), Some(Bob), "the first sent is the head");
    assert_eq!(format!("{two:?}"), "[Bob, Ann]", "printed head first");
    assert_eq!(two.receive(), empty.send(Ann), "the head is dropped");
    assert_eq!(two.size(), 2, "the receiver of receive is unchanged");
    assert_eq!(empty.receive(), empty, "an empty queue stays empty");
    let three = two.send(Cy);
    assert_eq!(three.size(), 3, "sending past the capacity is not refused");
    assert!(three.is_full(), "past capacity is still full");
}

#[test]
fn includes_clear_every_and_some_look_at_all_waiting_messages() {
    let empty = Inbox::default();
    let two = empty.send(Bob).send(Ann);
    assert!(two.includes(Ann), "by value");
    assert!(two.includes(Bob), "another guest");
    assert!(!two.includes(Cy), "never sent");
    assert_eq!(two.clear(), empty, "cleared");
    assert_eq!(two.size(), 2, "the receiver of clear is unchanged");
    assert!(two.every(|guest| guest != Cy), "neither is Cy");
    assert!(!two.every(|guest| guest == Ann), "Bob is not Ann");
    assert!(two.some(|guest| guest == Ann), "Ann is waiting");
    assert!(!two.some(|guest| guest == Cy), "Cy is not");
    assert!(empty.every(|_| false), "every is true of nothing");
    assert!(!empty.some(|_| true), "some is false of nothing");
}
