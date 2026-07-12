use super::{Reducer, Runtime, RuntimeBudget, RuntimeDrainError, RuntimeDrainReport};

/// A public runtime value with a private representation.
pub struct AppLoop<State = (), R = (), Input = ()> {
    runtime: Runtime<State, R, Input>,
}

impl<State, R, Input> AppLoop<State, R, Input> {
    #[must_use]
    /// Constructs this runtime value.
    pub fn new(runtime: Runtime<State, R, Input>) -> Self {
        Self { runtime }
    }

    #[must_use]
    /// Performs the associated runtime operation.
    pub const fn runtime(&self) -> &Runtime<State, R, Input> {
        &self.runtime
    }

    /// Performs the associated runtime operation.
    pub fn runtime_mut(&mut self) -> &mut Runtime<State, R, Input> {
        &mut self.runtime
    }

    #[must_use]
    /// Performs the associated runtime operation.
    pub fn into_runtime(self) -> Runtime<State, R, Input> {
        self.runtime
    }
}

impl<State, R, Input> AppLoop<State, R, Input>
where
    R: Reducer<State, Input>,
{
    /// Performs the associated runtime operation.
    pub fn step(&mut self, budget: RuntimeBudget) -> Result<RuntimeDrainReport, RuntimeDrainError> {
        self.runtime.drain_once(budget)
    }
}
