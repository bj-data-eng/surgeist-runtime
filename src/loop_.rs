use super::{Reducer, Runtime, RuntimeBudget, RuntimeDrainError, RuntimeDrainReport};

/// A host-independent owner for deterministic runtime drain steps.
///
/// Native event-loop callbacks and wake handling stay in the root adapter; this
/// wrapper only exposes the contained [`Runtime`] and delegates one step at a time.
pub struct AppLoop<State = (), R = (), Input = ()> {
    runtime: Runtime<State, R, Input>,
}

impl<State, R, Input> AppLoop<State, R, Input> {
    #[must_use]
    /// Wraps an existing runtime without changing its state or queued inputs.
    pub fn new(runtime: Runtime<State, R, Input>) -> Self {
        Self { runtime }
    }

    #[must_use]
    /// Borrows the contained runtime for read-only inspection.
    pub const fn runtime(&self) -> &Runtime<State, R, Input> {
        &self.runtime
    }

    /// Mutably borrows the contained runtime for direct queue or state updates.
    pub fn runtime_mut(&mut self) -> &mut Runtime<State, R, Input> {
        &mut self.runtime
    }

    #[must_use]
    /// Consumes this loop wrapper and returns its runtime unchanged.
    pub fn into_runtime(self) -> Runtime<State, R, Input> {
        self.runtime
    }
}

impl<State, R, Input> AppLoop<State, R, Input>
where
    R: Reducer<State, Input>,
{
    /// Delegates exactly one bounded drain step to the contained runtime.
    ///
    /// The report or error has the same transaction and recovery semantics as
    /// [`Runtime::drain_once`].
    pub fn step(&mut self, budget: RuntimeBudget) -> Result<RuntimeDrainReport, RuntimeDrainError> {
        self.runtime.drain_once(budget)
    }
}
