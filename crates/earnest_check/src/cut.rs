use vstd::prelude::*;

verus! {

pub(crate) fn cut_points(total: usize, size: usize) -> (cuts: Vec<usize>)
    ensures
        cuts.len() >= 1,
        cuts[0] == 0,
        cuts[cuts.len() - 1] == total,
        forall|i: int, j: int|
            0 <= i && j == i + 1 && j < cuts.len() ==> cuts[i] < cuts[j] && (size > 0 ==> cuts[j]
                - cuts[i] <= size),
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
            forall|i: int, j: int|
                0 <= i && j == i + 1 && j < cuts.len() ==> cuts[i] < cuts[j] && (size > 0
                    ==> cuts[j] - cuts[i] <= size),
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

#[cfg(test)]
mod tests {
    use super::cut_points;

    #[test]
    fn a_level_is_cut_into_slices_of_the_given_size_and_a_shorter_last_one() {
        assert_eq!(cut_points(10, 4), vec![0, 4, 8, 10]);
    }

    #[test]
    fn a_level_that_divides_evenly_has_no_shorter_slice() {
        assert_eq!(cut_points(8, 4), vec![0, 4, 8]);
    }

    #[test]
    fn a_level_smaller_than_the_slice_size_is_one_slice() {
        assert_eq!(cut_points(3, 10), vec![0, 3]);
    }

    #[test]
    fn an_empty_level_has_no_slice() {
        assert_eq!(cut_points(0, 5), vec![0]);
    }

    #[test]
    fn a_slice_size_of_zero_cuts_one_state_at_a_time() {
        assert_eq!(cut_points(3, 0), vec![0, 1, 2, 3]);
    }

    #[test]
    fn a_cut_past_the_largest_number_ends_at_the_total() {
        let most = usize::MAX;
        assert_eq!(cut_points(most, most - 1), vec![0, most - 1, most]);
    }
}
