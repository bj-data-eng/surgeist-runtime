#[cfg(feature = "runtime-tokio")]
use super::{ExecutorError, ExecutorTaskHandle, RuntimeExecutor, SpawnRequest, TaskHandle};

#[cfg(feature = "runtime-tokio")]
pub struct TokioExecutor {
    runtime: tokio::runtime::Runtime,
}

#[cfg(feature = "runtime-tokio")]
impl TokioExecutor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            runtime: tokio::runtime::Runtime::new()
                .expect("tokio runtime should initialize for app executor"),
        }
    }

    #[must_use]
    pub const fn name(&self) -> &'static str {
        "tokio"
    }
}

#[cfg(feature = "runtime-tokio")]
impl Default for TokioExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "runtime-tokio")]
impl<Input> RuntimeExecutor<Input> for TokioExecutor {
    fn spawn_task(
        &mut self,
        request: SpawnRequest<Input>,
    ) -> Result<ExecutorTaskHandle, ExecutorError> {
        let _handle = self.runtime.handle();
        Ok(ExecutorTaskHandle::new(
            request.task_id(),
            request.attempt_id(),
        ))
    }

    fn spawn_blocking_task(
        &mut self,
        request: SpawnRequest<Input>,
    ) -> Result<ExecutorTaskHandle, ExecutorError> {
        let _handle = self.runtime.handle();
        Ok(ExecutorTaskHandle::new(
            request.task_id(),
            request.attempt_id(),
        ))
    }

    fn cancel(&mut self, _handle: TaskHandle) -> Result<(), ExecutorError> {
        let _handle = self.runtime.handle();
        Ok(())
    }

    fn name(&self) -> &'static str {
        self.name()
    }
}
