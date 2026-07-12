use super::{Reducer, Runtime, RuntimeBudget, RuntimeDrainError, RuntimeDrainReport};

pub struct AppLoop<State = (), R = (), Input = ()> {
    runtime: Runtime<State, R, Input>,
}

impl<State, R, Input> AppLoop<State, R, Input> {
    #[must_use]
    pub fn new(runtime: Runtime<State, R, Input>) -> Self {
        Self { runtime }
    }

    #[must_use]
    pub const fn runtime(&self) -> &Runtime<State, R, Input> {
        &self.runtime
    }

    pub fn runtime_mut(&mut self) -> &mut Runtime<State, R, Input> {
        &mut self.runtime
    }

    #[must_use]
    pub fn into_runtime(self) -> Runtime<State, R, Input> {
        self.runtime
    }
}

impl<State, R, Input> AppLoop<State, R, Input>
where
    R: Reducer<State, Input>,
{
    pub fn step(&mut self, budget: RuntimeBudget) -> Result<RuntimeDrainReport, RuntimeDrainError> {
        Ok(self.runtime.drain_once(budget))
    }
}
