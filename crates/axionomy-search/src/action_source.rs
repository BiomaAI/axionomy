//! Lazy proposal generation for search algorithms.

use std::ops::ControlFlow;

/// Emits concrete action proposals for one derived search state.
///
/// An action source is deliberately non-authoritative. It may derive proposals
/// lazily, but the receiving algorithm must still validate every emitted
/// exchange against the Axionomy economy it intends to traverse.
pub trait ActionSource<State, Action> {
    fn for_each_action(&mut self, state: &State, emit: &mut dyn FnMut(Action));

    /// Visits proposals until the source is exhausted or the receiver breaks.
    ///
    /// Existing sources receive a compatibility implementation. Sources that
    /// can stop generation eagerly should override this method.
    fn for_each_action_until(
        &mut self,
        state: &State,
        emit: &mut dyn FnMut(Action) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        let mut flow = ControlFlow::Continue(());
        self.for_each_action(state, &mut |action| {
            if flow.is_continue() {
                flow = emit(action);
            }
        });
        flow
    }
}

/// Adapts a visitor-style closure into a lazy [`ActionSource`].
pub struct LazyActionSource<F> {
    generate: F,
}

pub const fn lazy_actions<F>(generate: F) -> LazyActionSource<F> {
    LazyActionSource { generate }
}

impl<State, Action, F> ActionSource<State, Action> for LazyActionSource<F>
where
    F: FnMut(&State, &mut dyn FnMut(Action)),
{
    fn for_each_action(&mut self, state: &State, emit: &mut dyn FnMut(Action)) {
        (self.generate)(state, emit);
    }
}

/// Adapts the original vector-returning candidate API into an action source.
pub struct EagerActionSource<F> {
    generate: F,
}

pub const fn eager_actions<F>(generate: F) -> EagerActionSource<F> {
    EagerActionSource { generate }
}

impl<State, Action, F> ActionSource<State, Action> for EagerActionSource<F>
where
    F: FnMut(&State) -> Vec<Action>,
{
    fn for_each_action(&mut self, state: &State, emit: &mut dyn FnMut(Action)) {
        for action in (self.generate)(state) {
            emit(action);
        }
    }

    fn for_each_action_until(
        &mut self,
        state: &State,
        emit: &mut dyn FnMut(Action) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        for action in (self.generate)(state) {
            emit(action)?;
        }
        ControlFlow::Continue(())
    }
}

/// Materializes proposals at the algorithm boundary.
///
/// This is primarily useful to algorithms that must compare or randomly select
/// the current alternatives. Proposal generation itself remains visitor-based,
/// so domains need not allocate or register an eager action catalog.
pub fn collect_actions<State, Action>(
    source: &mut impl ActionSource<State, Action>,
    state: &State,
) -> Vec<Action> {
    let mut actions = Vec::new();
    source.for_each_action(state, &mut |action| actions.push(action));
    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn lazy_sources_emit_only_when_visited() {
        let calls = Cell::new(0);
        let mut source = lazy_actions(|limit: &u8, emit: &mut dyn FnMut(u8)| {
            calls.set(calls.get() + 1);
            for action in 0..*limit {
                emit(action);
            }
        });

        assert_eq!(calls.get(), 0);
        assert_eq!(collect_actions(&mut source, &3), vec![0, 1, 2]);
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn eager_candidate_closures_remain_compatible() {
        let mut source = eager_actions(|limit: &u8| (0..*limit).collect::<Vec<_>>());

        assert_eq!(collect_actions(&mut source, &2), vec![0, 1]);
    }

    #[test]
    fn eager_sources_stop_generating_when_the_receiver_breaks() {
        let mut source = eager_actions(|limit: &u8| (0..*limit).collect::<Vec<_>>());
        let mut visited = Vec::new();
        let flow = source.for_each_action_until(&5, &mut |action| {
            visited.push(action);
            if visited.len() == 2 {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        });

        assert!(flow.is_break());
        assert_eq!(visited, vec![0, 1]);
    }
}
