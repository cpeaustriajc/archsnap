use vstd::prelude::*;

verus! {

const DAYS_IN_WEEK: u32 = 7;

pub(crate) fn index_in_sunday_first_week(cron_day: u32) -> (index: u32)
    ensures
        index < DAYS_IN_WEEK,
        cron_day < DAYS_IN_WEEK ==> index == cron_day,
        cron_day == DAYS_IN_WEEK ==> index == 0,
{
    cron_day % DAYS_IN_WEEK
}

}
