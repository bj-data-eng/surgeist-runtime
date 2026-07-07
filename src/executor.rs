use std::{error::Error, fmt};

use super::{AppScope, CoalescingKey, StartTaskEffect, TaskAttemptId, TaskHandle, TaskId, TaskKey};

pub trait RuntimeExecutor<Input> {
    fn spawn_task(
        &mut self,
        request: SpawnRequest<Input>,
    ) -> Result<ExecutorTaskHandle, ExecutorError>;

    fn spawn_blocking_task(
        &mut self,
        request: SpawnRequest<Input>,
    ) -> Result<ExecutorTaskHandle, ExecutorError>;

    fn cancel(&mut self, handle: TaskHandle) -> Result<(), ExecutorError>;

    fn name(&self) -> &'static str;
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BlockingPolicy {
    #[default]
    Abortable,
    Blocking,
    NonAbortableReportCancelling,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpawnRequest<Input = ()> {
    task_id: TaskId,
    attempt_id: TaskAttemptId,
    key: TaskKey,
    scope: AppScope,
    blocking: BlockingPolicy,
    input: Option<Input>,
}

impl SpawnRequest {
    #[must_use]
    pub fn new(task_id: TaskId, attempt_id: TaskAttemptId, key: TaskKey, scope: AppScope) -> Self {
        Self {
            task_id,
            attempt_id,
            key,
            scope,
            blocking: BlockingPolicy::Abortable,
            input: None,
        }
    }

    #[must_use]
    pub fn with_input<Input>(self, input: Input) -> SpawnRequest<Input> {
        SpawnRequest {
            task_id: self.task_id,
            attempt_id: self.attempt_id,
            key: self.key,
            scope: self.scope,
            blocking: self.blocking,
            input: Some(input),
        }
    }
}

impl<Input> SpawnRequest<Input> {
    #[must_use]
    pub fn from_start_effect(
        task_id: TaskId,
        attempt_id: TaskAttemptId,
        effect: &StartTaskEffect,
    ) -> Self {
        Self {
            task_id,
            attempt_id,
            key: TaskKey::new(effect.key().as_str()),
            scope: effect.scope().clone(),
            blocking: BlockingPolicy::Abortable,
            input: None,
        }
    }

    #[must_use]
    pub fn with_blocking_policy(mut self, blocking: BlockingPolicy) -> Self {
        self.blocking = blocking;
        self
    }

    #[must_use]
    pub const fn task_id(&self) -> TaskId {
        self.task_id
    }

    #[must_use]
    pub const fn attempt_id(&self) -> TaskAttemptId {
        self.attempt_id
    }

    #[must_use]
    pub fn key(&self) -> &TaskKey {
        &self.key
    }

    #[must_use]
    pub const fn scope(&self) -> &AppScope {
        &self.scope
    }

    #[must_use]
    pub const fn blocking_policy(&self) -> BlockingPolicy {
        self.blocking
    }

    #[must_use]
    pub const fn input(&self) -> Option<&Input> {
        self.input.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutorEvent<Output = ()> {
    task_id: TaskId,
    attempt_id: TaskAttemptId,
    payload: ExecutorEventPayload<Output>,
}

impl<Output> ExecutorEvent<Output> {
    #[must_use]
    pub const fn new(
        task_id: TaskId,
        attempt_id: TaskAttemptId,
        payload: ExecutorEventPayload<Output>,
    ) -> Self {
        Self {
            task_id,
            attempt_id,
            payload,
        }
    }

    #[must_use]
    pub fn progress(
        task_id: TaskId,
        attempt_id: TaskAttemptId,
        key: CoalescingKey,
        payload: impl Into<String>,
    ) -> Self {
        Self::new(
            task_id,
            attempt_id,
            ExecutorEventPayload::Progress {
                key,
                payload: payload.into(),
            },
        )
    }

    #[must_use]
    pub const fn completed(task_id: TaskId, attempt_id: TaskAttemptId, output: Output) -> Self {
        Self::new(task_id, attempt_id, ExecutorEventPayload::Completed(output))
    }

    #[must_use]
    pub fn failed(task_id: TaskId, attempt_id: TaskAttemptId, message: impl Into<String>) -> Self {
        Self::new(
            task_id,
            attempt_id,
            ExecutorEventPayload::Failed(message.into()),
        )
    }

    #[must_use]
    pub const fn cancelled(task_id: TaskId, attempt_id: TaskAttemptId) -> Self {
        Self::new(task_id, attempt_id, ExecutorEventPayload::Cancelled)
    }

    #[must_use]
    pub const fn task_id(&self) -> TaskId {
        self.task_id
    }

    #[must_use]
    pub const fn attempt_id(&self) -> TaskAttemptId {
        self.attempt_id
    }

    #[must_use]
    pub const fn payload(&self) -> &ExecutorEventPayload<Output> {
        &self.payload
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutorEventPayload<Output = ()> {
    Progress { key: CoalescingKey, payload: String },
    Completed(Output),
    Failed(String),
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FakeExecutor<Input = ()> {
    spawned: Vec<SpawnRequest<Input>>,
    cancelled: Vec<TaskHandle>,
}

impl<Input> Default for FakeExecutor<Input> {
    fn default() -> Self {
        Self {
            spawned: Vec::new(),
            cancelled: Vec::new(),
        }
    }
}

impl<Input> FakeExecutor<Input> {
    pub fn spawn_task(
        &mut self,
        request: SpawnRequest<Input>,
    ) -> Result<ExecutorTaskHandle, ExecutorError> {
        <Self as RuntimeExecutor<Input>>::spawn_task(self, request)
    }

    pub fn spawn_blocking_task(
        &mut self,
        request: SpawnRequest<Input>,
    ) -> Result<ExecutorTaskHandle, ExecutorError> {
        <Self as RuntimeExecutor<Input>>::spawn_blocking_task(self, request)
    }

    pub fn cancel(&mut self, handle: TaskHandle) -> Result<(), ExecutorError> {
        <Self as RuntimeExecutor<Input>>::cancel(self, handle)
    }

    #[must_use]
    pub fn spawned(&self) -> &[SpawnRequest<Input>] {
        &self.spawned
    }

    #[must_use]
    pub fn cancelled(&self) -> &[TaskHandle] {
        &self.cancelled
    }

    fn record_spawn(&mut self, request: SpawnRequest<Input>) -> ExecutorTaskHandle {
        let handle = ExecutorTaskHandle::new(request.task_id(), request.attempt_id());
        self.spawned.push(request);
        handle
    }
}

impl<Input> RuntimeExecutor<Input> for FakeExecutor<Input> {
    fn spawn_task(
        &mut self,
        request: SpawnRequest<Input>,
    ) -> Result<ExecutorTaskHandle, ExecutorError> {
        Ok(self.record_spawn(request))
    }

    fn spawn_blocking_task(
        &mut self,
        request: SpawnRequest<Input>,
    ) -> Result<ExecutorTaskHandle, ExecutorError> {
        let request = if request.blocking_policy() == BlockingPolicy::Abortable {
            request.with_blocking_policy(BlockingPolicy::Blocking)
        } else {
            request
        };
        Ok(self.record_spawn(request))
    }

    fn cancel(&mut self, handle: TaskHandle) -> Result<(), ExecutorError> {
        self.cancelled.push(handle);
        Ok(())
    }

    fn name(&self) -> &'static str {
        "fake"
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutorTaskHandle {
    task_id: TaskId,
    attempt_id: TaskAttemptId,
}

impl ExecutorTaskHandle {
    #[must_use]
    pub const fn new(task_id: TaskId, attempt_id: TaskAttemptId) -> Self {
        Self {
            task_id,
            attempt_id,
        }
    }

    #[must_use]
    pub const fn task_id(&self) -> TaskId {
        self.task_id
    }

    #[must_use]
    pub const fn attempt_id(&self) -> TaskAttemptId {
        self.attempt_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutorError {
    message: String,
}

impl ExecutorError {
    #[must_use]
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ExecutorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ExecutorError {}
