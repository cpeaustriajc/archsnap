use crate::Guest::{self, Ann, Bob};
use earnest::model::{Lock, Request, Retry, Start, Timer, exists, forall};

#[test]
fn a_plain_lock_is_free_until_acquired() {
    let free: Lock = Lock::starts().remove(0);
    assert!(free.is_free(), "a lock field starts free");
    assert!(!free.is_held(), "nobody holds it");
    let held = free.acquire();
    assert!(held.is_held(), "acquired");
    assert!(!held.is_free(), "acquired");
    assert!(free.is_free(), "the receiver of acquire is unchanged");
    assert_eq!(held.acquire(), held, "acquiring a held lock is not refused");
    assert_eq!(held.release(), free, "released");
    assert!(held.is_held(), "the receiver of release is unchanged");
    assert_eq!(free.release(), free, "releasing a free lock keeps it free");
}

#[test]
fn an_owned_lock_knows_its_holder() {
    let free = Lock::<Guest>::starts().remove(0);
    assert!(free.is_free(), "a lock field starts free");
    assert!(!free.is_held(), "nobody holds it");
    assert_eq!(free.holder(), None, "no holder");
    assert!(!free.is_held_by(Ann), "not held by Ann");
    let held = free.acquire(Ann);
    assert!(held.is_held(), "acquired");
    assert!(!held.is_free(), "acquired");
    assert_eq!(held.holder(), Some(Ann), "held by Ann");
    assert!(held.is_held_by(Ann), "by value");
    assert!(!held.is_held_by(Bob), "another guest");
    assert!(free.is_free(), "the receiver of acquire is unchanged");
    assert_eq!(held.acquire(Bob).holder(), Some(Bob), "the last to acquire");
    assert_eq!(held.release(), free, "released");
    assert_eq!(
        held.holder(),
        Some(Ann),
        "the receiver of release is unchanged"
    );
}

#[test]
fn a_timer_is_idle_running_or_fired() {
    let idle = Timer::starts().remove(0);
    assert!(idle.is_idle(), "a timer field starts idle");
    assert!(!idle.is_running() && !idle.has_fired(), "only idle");
    let running = idle.start();
    assert!(running.is_running(), "started");
    assert!(!running.is_idle() && !running.has_fired(), "only running");
    assert!(idle.is_idle(), "the receiver of start is unchanged");
    let fired = running.fire();
    assert!(fired.has_fired(), "fired");
    assert!(!fired.is_idle() && !fired.is_running(), "only fired");
    assert!(running.is_running(), "the receiver of fire is unchanged");
    assert_eq!(running.cancel(), idle, "cancelled");
    assert!(running.is_running(), "the receiver of cancel is unchanged");
    assert_eq!(fired.reset(), idle, "reset");
    assert!(fired.has_fired(), "the receiver of reset is unchanged");
    assert_eq!(idle.fire(), fired, "firing does not check the stage");
}

#[test]
fn a_request_is_idle_pending_succeeded_or_failed() {
    let idle = Request::starts().remove(0);
    assert!(idle.is_idle(), "a request field starts idle");
    assert!(!idle.is_pending() && !idle.is_settled(), "only idle");
    let pending = idle.send();
    assert!(pending.is_pending(), "sent");
    assert!(!pending.is_idle() && !pending.is_settled(), "only pending");
    assert!(idle.is_idle(), "the receiver of send is unchanged");
    let succeeded = pending.succeed();
    assert!(succeeded.has_succeeded(), "succeeded");
    assert!(succeeded.is_settled(), "a success is settled");
    assert!(
        !succeeded.has_failed() && !succeeded.is_pending(),
        "only so"
    );
    let failed = pending.fail();
    assert!(failed.has_failed(), "failed");
    assert!(failed.is_settled(), "a failure is settled");
    assert!(
        !failed.has_succeeded() && !failed.is_pending(),
        "only failed"
    );
    assert!(pending.is_pending(), "the receiver is unchanged");
    assert_eq!(failed.reset(), idle, "reset");
    assert!(failed.has_failed(), "the receiver of reset is unchanged");
    assert_eq!(succeeded.reset(), idle, "reset");
}

#[test]
fn a_retry_counts_attempts_up_to_its_max() {
    let fresh = Retry::<2>::starts().remove(0);
    assert_eq!(fresh.attempts(), 0, "a retry field starts at zero");
    assert!(fresh.can_retry(), "two attempts left");
    assert!(!fresh.is_exhausted(), "two attempts left");
    let once = fresh.again();
    assert_eq!(once.attempts(), 1, "one counted");
    assert_eq!(fresh.attempts(), 0, "the receiver of again is unchanged");
    assert!(once.can_retry(), "one attempt left");
    let twice = once.again();
    assert_eq!(twice.attempts(), 2, "two counted");
    assert!(!twice.can_retry(), "none left");
    assert!(twice.is_exhausted(), "none left");
    assert_eq!(
        twice.again().attempts(),
        3,
        "counting past the max is not refused"
    );
    assert!(
        twice.again().is_exhausted(),
        "past the max is still exhausted"
    );
    assert_eq!(twice.reset(), fresh, "reset");
    assert_eq!(twice.attempts(), 2, "the receiver of reset is unchanged");
}

#[test]
fn forall_and_exists_try_every_value_of_a_type() {
    assert!(forall::<Guest>(|guest| guest >= Ann), "Ann is the least");
    assert!(!forall::<Guest>(|guest| guest == Ann), "Bob is not Ann");
    assert!(exists::<Guest>(|guest| guest == Bob), "Bob is a guest");
    assert!(!exists::<Guest>(|guest| guest < Ann), "nobody is below Ann");
    assert!(exists::<Option<bool>>(|flag| flag.is_none()), "None counts");
    assert!(!forall::<bool>(|flag| flag), "false is a bool");
}
