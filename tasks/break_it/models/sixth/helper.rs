use earnest::model::{Behavior, Model};

pub fn count<M: Model + Behavior>() -> usize {
    M::starts().len()
}
