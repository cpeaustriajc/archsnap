use crate::store::Store;
use earnest_value::Value;

pub(super) fn with_updates(state: &[Value], updates: &[(usize, Value)]) -> Box<[Value]> {
    let mut next = state.to_vec();
    for (position, value) in updates {
        if let Some(place) = next.get_mut(*position) {
            *place = value.clone();
        }
    }
    next.into_boxed_slice()
}

pub(super) fn encode_next(
    store: &Store,
    base: Option<&[u64]>,
    row: &mut Vec<u64>,
    state: &[Value],
    updates: &[(usize, Value)],
) -> bool {
    match base {
        Some(base) => {
            if let ([into], [word]) = (row.as_mut_slice(), base) {
                *into = *word;
            } else {
                row.clear();
                row.extend_from_slice(base);
            }
            updates
                .iter()
                .all(|(position, value)| store.encode_update(row, *position, value))
        }
        None => store.encode(&with_updates(state, updates), row),
    }
}
