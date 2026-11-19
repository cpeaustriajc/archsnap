use super::{Domain, Start};
use std::borrow::Borrow;

/// The owner of a [`Lock`] that does not say who holds it. A model never names it: write the
/// field type as `Lock`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Unowned;

/// A lock that is free or held. As a model field it starts free.
///
/// `Lock` only knows whether it is held. `Lock<Owner>`, where `Owner` has a [`Domain`], also
/// knows by whom, and its `acquire` takes the owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Lock<Owner = Unowned> {
    holder: Option<Owner>,
}

impl<Owner> Default for Lock<Owner> {
    fn default() -> Self {
        Self { holder: None }
    }
}

impl<Owner> Start for Lock<Owner> {
    fn starts() -> Vec<Self> {
        vec![Self::default()]
    }
}

impl<Owner> Lock<Owner> {
    /// Whether nobody holds the lock.
    pub fn is_free(&self) -> bool {
        self.holder.is_none()
    }

    /// Whether somebody holds the lock.
    pub fn is_held(&self) -> bool {
        self.holder.is_some()
    }

    /// The free lock, whoever held this one.
    #[must_use]
    pub fn release(&self) -> Self {
        Self::default()
    }
}

impl Lock<Unowned> {
    /// The held lock. It does not check that this one is free: guard the action with
    /// `is_free()` when a second holder must wait.
    #[must_use]
    pub fn acquire(&self) -> Self {
        Self {
            holder: Some(Unowned),
        }
    }
}

impl<Owner: Domain + Clone + PartialEq> Lock<Owner> {
    /// The lock held by `owner`. It does not check that this one is free: guard the action
    /// with `is_free()` when a second holder must wait.
    #[must_use]
    pub fn acquire(&self, owner: Owner) -> Self {
        Self {
            holder: Some(owner),
        }
    }

    /// Whether `owner` holds the lock.
    pub fn is_held_by(&self, owner: impl Borrow<Owner>) -> bool {
        self.holder.as_ref() == Some(owner.borrow())
    }

    /// Who holds the lock, or `None` when it is free.
    pub fn holder(&self) -> Option<Owner> {
        self.holder.clone()
    }
}
