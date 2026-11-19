//! The Rust model library of `earnest`, a model checker for models written in the app's own
//! language.
//!
//! A model file imports everything from [`model`]: the derives and the attribute that mark a
//! model, [`require!`] for an action's guard, and the collections and parts a state is made
//! of. `earnest verify` reads the file and visits every reachable state. The same file is
//! ordinary Rust, so a test can list the start states, take steps and ask the rules:
//!
//! ```
//! use earnest::model::{Model, model, require};
//!
//! #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
//! pub struct Counter {
//!     #[range(0, 3)]
//!     #[initial(0)]
//!     count: u8,
//! }
//!
//! #[model]
//! impl Counter {
//!     #[action("the counter goes up by one")]
//!     fn increment(&self) -> Option<Self> {
//!         require!(self.count < 3);
//!         Some(Self { count: self.count + 1, ..self.clone() })
//!     }
//!
//!     #[rule("The count is never above three")]
//!     fn bounded(&self) -> bool {
//!         self.count <= 3
//!     }
//! }
//!
//! let start = Counter::starts().remove(0);
//! let next = start.increment().expect("the count is below three");
//! assert_eq!(next, Counter { count: 1 });
//! assert!(next.bounded());
//! ```

pub mod model;

/// Stops an action that can't happen in the state it was called on.
///
/// Write `require!(condition);` at the top level of an action's body. When the condition is
/// false the action returns `None` there. The checker reads every `require!` of an action as
/// one part of the condition for the action to happen.
///
/// The conformance model `tasks/conformance/models/counter.rs` counts 0, 1, 2, 3 and its
/// `require!(self.count < 3)` stops it there:
///
/// ```
/// include!("../../../tasks/conformance/models/counter.rs");
///
/// fn main() {
///     assert_eq!(Counter::starts(), [Counter { count: 0 }]);
///     let mut state = Counter { count: 0 };
///     assert!(state.bounded() && state.even() && !state.full());
///     for count in 1..=3 {
///         state = state.increment().expect("the count is below three");
///         assert_eq!(state, Counter { count });
///         assert!(state.bounded() && state.even());
///         assert_eq!(state.full(), count == 3);
///     }
///     assert_eq!(state.increment(), None);
/// }
/// ```
#[macro_export]
macro_rules! require {
    ($condition:expr $(,)?) => {
        if !$condition {
            return ::core::option::Option::None;
        }
    };
}
