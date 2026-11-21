use super::Search;
use crate::expand::{Asked, Scratch, Successors};
use earnest_value::Value;

struct Choice<'a> {
    target: &'a [Value],
    found: Option<(usize, usize)>,
}

impl Successors for Choice<'_> {
    fn offer(
        &mut self,
        action: usize,
        input: usize,
        state: &[Value],
        updates: &[(usize, Value)],
    ) -> bool {
        let mut changed = updates.iter().peekable();
        let same = state.len() == self.target.len()
            && state
                .iter()
                .zip(self.target)
                .enumerate()
                .all(|(position, (old, wanted))| {
                    let value = changed
                        .next_if(|(at, _)| *at == position)
                        .map_or(old, |(_, new)| new);
                    value == wanted
                });
        if same && self.found.is_none() {
            self.found = Some((action, input));
        }
        false
    }

    fn done(&self) -> bool {
        self.found.is_some()
    }
}

impl Search<'_> {
    pub(crate) fn choice(&self, from: usize, to: usize) -> Option<(usize, usize)> {
        let state = self.store.state(from);
        let target = self.store.state(to);
        let mut choice = Choice {
            target: &target,
            found: None,
        };
        let mut row = Vec::new();
        self.store.read_state(from, &mut row, &mut Vec::new());
        let outside = self.store.outside(from, &row, &self.model.fields);
        self.expand(None, Asked::Everything).state(
            &state,
            outside,
            &mut choice,
            &mut Scratch::default(),
        );
        choice.found
    }
}
