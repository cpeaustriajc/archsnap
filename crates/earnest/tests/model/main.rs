//! The model library as a model file uses it: every operation of every value type, the
//! derives, and the block attribute. Expected values are worked out by hand.

mod behavior;
mod collections;
mod derives;
mod listed;
mod parts;
mod replays;
mod views;

use earnest::model::Domain;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Guest {
    Ann,
    Bob,
    Cy,
}
