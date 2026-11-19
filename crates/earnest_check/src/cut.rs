use vstd::prelude::*;

verus! {

pub(crate) fn cut_points(total: usize, size: usize) -> (cuts: Vec<usize>)
    ensures
        cuts.len() >= 1,
        cuts[0] == 0,
        cuts[cuts.len() - 1] == total,
        forall|i: int|
            0 <= i < cuts.len() - 1 ==> cuts[i] < cuts[i + 1] && (size > 0 ==> cuts[i + 1] - cuts[i]
                <= size),
{
    let step: usize = if size == 0 {
        1
    } else {
        size
    };
    let mut cuts: Vec<usize> = Vec::new();
    cuts.push(0);
    let mut at: usize = 0;
    while at < total
        invariant
            cuts.len() >= 1,
            cuts[0] == 0,
            cuts[cuts.len() - 1] == at,
            at <= total,
            step > 0,
            size > 0 ==> step == size,
            forall|i: int|
                0 <= i < cuts.len() - 1 ==> cuts[i] < cuts[i + 1] && (size > 0 ==> cuts[i + 1]
                    - cuts[i] <= size),
        decreases total - at,
    {
        at =
        match at.checked_add(step) {
            Some(next) if next < total => next,
            Some(_) | None => total,
        };
        cuts.push(at);
    }
    cuts
}

}
