use crate::Expr;
use crate::eval::NO_UNIT;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_UNIT: AtomicU64 = AtomicU64::new(NO_UNIT + 1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Unbound,
    Steady,
    Varied,
}

#[derive(Debug)]
pub(super) struct Shared {
    unit: u64,
    kinds: Vec<Kind>,
    repeated: Vec<bool>,
}

impl Shared {
    pub(super) fn of(exprs: &[Expr]) -> Self {
        let mut survey = Survey::default();
        for expr in exprs {
            survey.walk(expr);
        }
        let mut shared = Self {
            unit: NEXT_UNIT.fetch_add(1, Ordering::Relaxed),
            kinds: survey.kinds(),
            repeated: survey.repeated(),
        };
        let mut scope = Scope::default();
        while exprs.iter().any(|expr| shared.narrowed(expr, &mut scope)) {}
        shared
    }

    pub(super) fn unit(&self) -> u64 {
        self.unit
    }

    pub(super) fn keeps(&self, slot: usize) -> bool {
        self.steady(slot) && self.repeated.get(slot).copied().unwrap_or(false)
    }

    fn steady(&self, slot: usize) -> bool {
        self.kinds.get(slot) == Some(&Kind::Steady)
    }

    fn bound(&self, slot: usize) -> bool {
        self.kinds
            .get(slot)
            .is_some_and(|kind| *kind != Kind::Unbound)
    }

    fn narrowed(&mut self, expr: &Expr, scope: &mut Scope) -> bool {
        match expr {
            Expr::Bind { slot, value, body } => {
                let mut changed = false;
                if self.steady(*slot) && !self.settles(value, scope) {
                    if let Some(kind) = self.kinds.get_mut(*slot) {
                        *kind = Kind::Varied;
                    }
                    changed = true;
                }
                changed |= self.narrowed(value, scope);
                scope.open(*slot);
                changed |= self.narrowed(body, scope);
                scope.close();
                changed
            }
            Expr::Sweep {
                slot, source, body, ..
            } => {
                let mut changed = self.narrowed(source, scope);
                scope.open(*slot);
                changed |= self.narrowed(body, scope);
                scope.close();
                changed
            }
            _ => {
                let mut changed = false;
                expr.each_part(&mut |part| changed |= self.narrowed(part, scope));
                changed
            }
        }
    }

    fn settles(&self, expr: &Expr, scope: &mut Scope) -> bool {
        match expr {
            Expr::Local(slot) => !self.bound(*slot) || (self.steady(*slot) && scope.holds(*slot)),
            Expr::Bind { slot, value, body } => {
                if !self.steady(*slot) || !self.settles(value, scope) {
                    return false;
                }
                scope.open(*slot);
                let settles = self.settles(body, scope);
                scope.close();
                settles
            }
            Expr::Sweep { .. } => false,
            _ => {
                let mut settles = true;
                expr.each_part(&mut |part| settles &= self.settles(part, scope));
                settles
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Binder<'e> {
    Unbound,
    Same { value: &'e Expr, repeated: bool },
    Mixed,
}

#[derive(Default)]
struct Survey<'e> {
    binders: Vec<Binder<'e>>,
    sweeping: usize,
}

impl<'e> Survey<'e> {
    fn binds(&mut self, slot: usize, value: Option<&'e Expr>) {
        if self.binders.len() <= slot {
            self.binders.resize(slot.saturating_add(1), Binder::Unbound);
        }
        let in_a_sweep = self.sweeping > 0;
        if let Some(binder) = self.binders.get_mut(slot) {
            *binder = match (*binder, value) {
                (Binder::Unbound, Some(first)) if !holds_a_sweep(first) => Binder::Same {
                    value: first,
                    repeated: in_a_sweep,
                },
                (Binder::Same { value: first, .. }, Some(again)) if first == again => {
                    Binder::Same {
                        value: first,
                        repeated: true,
                    }
                }
                _ => Binder::Mixed,
            };
        }
    }

    fn walk(&mut self, expr: &'e Expr) {
        match expr {
            Expr::Bind { slot, value, .. } => self.binds(*slot, Some(value)),
            Expr::Sweep {
                slot, source, body, ..
            } => {
                self.binds(*slot, None);
                self.walk(source);
                self.sweeping = self.sweeping.saturating_add(1);
                self.walk(body);
                self.sweeping = self.sweeping.saturating_sub(1);
                return;
            }
            _ => {}
        }
        expr.each_part(&mut |part| self.walk(part));
    }

    fn kinds(&self) -> Vec<Kind> {
        self.binders
            .iter()
            .map(|binder| match binder {
                Binder::Unbound => Kind::Unbound,
                Binder::Same { .. } => Kind::Steady,
                Binder::Mixed => Kind::Varied,
            })
            .collect()
    }

    fn repeated(&self) -> Vec<bool> {
        self.binders
            .iter()
            .map(|binder| matches!(binder, Binder::Same { repeated: true, .. }))
            .collect()
    }
}

#[derive(Default)]
struct Scope {
    open: Vec<usize>,
}

impl Scope {
    fn open(&mut self, slot: usize) {
        self.open.push(slot);
    }

    fn close(&mut self) {
        self.open.pop();
    }

    fn holds(&self, slot: usize) -> bool {
        self.open.contains(&slot)
    }
}

fn holds_a_sweep(expr: &Expr) -> bool {
    if matches!(expr, Expr::Sweep { .. }) {
        return true;
    }
    let mut found = false;
    expr.each_part(&mut |part| found |= holds_a_sweep(part));
    found
}
