//! Everything a model file imports: `use earnest::model::{Domain, Model, model, require};`.
//!
//! [`Domain`] and [`Model`] are each a trait and the derive that implements it. The value
//! types ([`Set`], [`Map`], [`Seq`], [`Queue`], [`Lock`], [`Timer`], [`Request`], [`Retry`])
//! never change in place: every operation gives back a new value and leaves the one it was
//! called on as it was. An operation that only looks at an argument takes it by value or by
//! reference, as you like; the checker reads `&x` and `x.clone()` as `x`.

mod lock;
mod map;
mod queue;
mod replay;
mod report;
mod request;
mod retry;
mod seq;
mod set;
mod timer;
mod view;
mod walk;

pub use crate::require;
pub use earnest_macros::{Domain, Model, model, view};
pub use lock::{Lock, Unowned};
pub use map::Map;
pub use queue::Queue;
pub use replay::{Mismatch, Move, replay};
pub use report::Report;
pub use request::Request;
pub use retry::Retry;
pub use seq::Seq;
pub use set::Set;
pub use timer::Timer;
pub use view::View;
pub use walk::Walk;

/// A type whose every value can be listed, so the checker can try each one.
///
/// Derive it on an enum of plain variants or on a struct whose fields have a domain. It is
/// what an action input, an `Option`'s inner type, and the type given to [`forall`] and
/// [`exists`] must be. `bool` and `Option<T>` have it already.
pub trait Domain: Sized {
    /// Every value of the type, each once, in a fixed order: an enum's variants in source
    /// order, `false` before `true`, `None` before each `Some`.
    fn values() -> Vec<Self>;
}

impl Domain for bool {
    fn values() -> Vec<Self> {
        vec![false, true]
    }
}

impl<T: Domain> Domain for Option<T> {
    fn values() -> Vec<Self> {
        std::iter::once(None)
            .chain(T::values().into_iter().map(Some))
            .collect()
    }
}

/// Where a model field of this type starts when it has no `#[initial(value)]`.
///
/// A type with a [`Domain`] starts at every one of its values. The library's collections and
/// parts start at one value: empty, free, idle or at zero. `#[derive(Model)]` calls this; a
/// model file never needs to.
pub trait Start: Sized {
    /// The values the field takes across the model's start states.
    fn starts() -> Vec<Self>;
}

impl<T: Domain> Start for T {
    fn starts() -> Vec<Self> {
        T::values()
    }
}

/// A model's state: implemented by `#[derive(Model)]` on the struct that holds the fields.
///
/// This replays the path `earnest verify` reports for the conformance model
/// `tasks/conformance/models/turnstile.rs`, whose rule `locked` breaks after one `push`:
///
/// ```
/// include!("../../../tasks/conformance/models/turnstile.rs");
///
/// fn main() {
///     let start = Turnstile { gate: Gate::Locked, paid: false };
///     assert_eq!(Turnstile::starts(), [start.clone()]);
///     assert!(start.locked());
///
///     let pushed = start.push().expect("a locked gate can be pushed");
///     assert_eq!(pushed, Turnstile { gate: Gate::Open, paid: false });
///     assert!(!pushed.locked());
///
///     let paid = start.pay().expect("a visitor at a locked gate can pay");
///     assert_eq!(paid, Turnstile { gate: Gate::Locked, paid: true });
///     assert_eq!(paid.pay(), None);
///     let open = paid.push().expect("a locked gate can be pushed");
///     assert_eq!(open, Turnstile { gate: Gate::Open, paid: true });
///     assert!(paid.locked() && open.locked());
///     assert_eq!(open.enter().expect("an open gate lets the visitor through"), start);
///     assert_eq!(pushed.enter().expect("an open gate lets the visitor through"), start);
/// }
/// ```
pub trait Model: Sized {
    /// Every state the model can start in: one for each combination of its fields' start
    /// values, with the first field changing slowest. A model whose fields all have
    /// `#[initial(value)]` gives exactly one.
    ///
    /// To replay a path from `earnest verify`, pick the state that equals the path's `start`,
    /// then call the action method of each step in order.
    fn starts() -> Vec<Self>;
}

/// One action that can happen in a state, with the input it was tried with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step<State> {
    /// The action's id: the name of its method.
    pub action: &'static str,
    /// The action's sentence from `#[action("...")]`.
    pub label: &'static str,
    /// Each input's name and its value as [`Report`] prints it, in parameter order.
    pub input: Vec<(&'static str, String)>,
    /// The state after the action.
    pub state: State,
}

/// A rule or an end, judged in one state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Claim {
    /// The id: the name of the method.
    pub id: &'static str,
    /// The sentence from `#[rule("...")]` or `#[end("...")]`.
    pub says: &'static str,
    /// What the method returned in the state.
    pub holds: bool,
}

/// What a model can do from one state: implemented by `#[model]` on the `impl` block.
///
/// It lets a test walk a model without naming its methods, for example to find the step with
/// the action id and input a report names.
///
/// The conformance model `tasks/conformance/models/download.rs` is stuck after `start`,
/// `fail`: no action can happen there and no end marks it.
///
/// ```
/// include!("../../../tasks/conformance/models/download.rs");
///
/// use earnest::model::Behavior;
///
/// fn main() {
///     let idle = Download { phase: Phase::Idle, saved: false };
///     assert_eq!(Download::starts(), [idle.clone()]);
///     let ids = |state: &Download| -> Vec<&str> {
///         state.steps().iter().map(|step| step.action).collect()
///     };
///     assert_eq!(ids(&idle), ["start"]);
///
///     let loading = idle.start().expect("an idle download can start");
///     assert_eq!(loading, Download { phase: Phase::Loading, saved: false });
///     assert_eq!(ids(&loading), ["finish", "fail"]);
///
///     let failed = loading.fail().expect("a loading download can fail");
///     assert_eq!(failed, Download { phase: Phase::Failed, saved: false });
///     assert!(failed.steps().is_empty());
///     assert!(failed.ends().iter().all(|end| !end.holds));
///
///     let done = loading.finish().expect("a loading download can finish");
///     assert_eq!(done, Download { phase: Phase::Done, saved: true });
///     assert!(done.steps().is_empty());
///     assert!(done.done());
///     assert_eq!(done.ends()[0].id, "done");
///     assert!(done.ends()[0].holds);
///
///     for state in [idle, loading, failed, done] {
///         assert!(state.whole());
///     }
/// }
/// ```
pub trait Behavior: Sized {
    /// Every step that can happen in this state: each action in source order, tried with
    /// every combination of its input values, kept where the action returned `Some`. Empty
    /// when no action can happen.
    fn steps(&self) -> Vec<Step<Self>>;

    /// Every rule in source order, with whether it holds in this state.
    ///
    /// The conformance model `tasks/conformance/models/oven.rs` breaks `closed` after `open`
    /// and `safe` after `start`, `open`:
    ///
    /// ```
    /// include!("../../../tasks/conformance/models/oven.rs");
    ///
    /// use earnest::model::Behavior;
    ///
    /// fn main() {
    ///     let cold = Oven { door: Door::Closed, heat: Heat::Off };
    ///     assert_eq!(Oven::starts(), [cold.clone()]);
    ///     let broken = |state: &Oven| -> Vec<&str> {
    ///         let rules = state.rules();
    ///         rules.iter().filter(|rule| !rule.holds).map(|rule| rule.id).collect()
    ///     };
    ///     assert!(broken(&cold).is_empty());
    ///
    ///     let ajar = cold.open().expect("a closed door opens");
    ///     assert_eq!(ajar, Oven { door: Door::Open, heat: Heat::Off });
    ///     assert_eq!(broken(&ajar), ["closed"]);
    ///     assert!(ajar.safe() && !ajar.closed());
    ///     assert_eq!(ajar.start(), None);
    ///
    ///     let hot = cold.start().expect("the heat starts with the door closed");
    ///     assert_eq!(hot, Oven { door: Door::Closed, heat: Heat::On });
    ///     assert!(broken(&hot).is_empty());
    ///
    ///     let heating_open = hot.open().expect("a closed door opens");
    ///     assert_eq!(heating_open, Oven { door: Door::Open, heat: Heat::On });
    ///     assert_eq!(broken(&heating_open), ["safe"]);
    ///     assert!(!heating_open.safe() && heating_open.closed());
    ///
    ///     assert_eq!(heating_open.close().expect("an open door closes"), hot);
    ///     assert_eq!(heating_open.stop().expect("the heat stops"), ajar);
    ///     assert_eq!(cold.rules()[0].says, "The oven never heats with the door open");
    /// }
    /// ```
    fn rules(&self) -> Vec<Claim>;

    /// Every end in source order, with whether it marks this state as a correct place to
    /// stop.
    fn ends(&self) -> Vec<Claim>;
}

/// Whether `test` is true for every value of `T`. Name the type: `forall::<Guest>(|g| ...)`.
pub fn forall<T: Domain>(test: impl Fn(T) -> bool) -> bool {
    T::values().into_iter().all(test)
}

/// Whether `test` is true for at least one value of `T`. Name the type:
/// `exists::<Guest>(|g| ...)`.
pub fn exists<T: Domain>(test: impl Fn(T) -> bool) -> bool {
    T::values().into_iter().any(test)
}
