use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use super::{
    AppScope, Diagnostic, DiagnosticCode, InputProvenance, TaskAttemptId, TaskId, TaskKey, TaskName,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskIntentName(String);

impl TaskIntentName {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskIntentKey(String);

impl TaskIntentKey {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskIntentId(u64);

impl TaskIntentId {
    #[must_use]
    pub const fn from_u64(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskIntentAttemptId(u64);

impl TaskIntentAttemptId {
    #[must_use]
    pub const fn from_u64(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskIntentHandle {
    id: TaskIntentId,
    attempt_id: TaskIntentAttemptId,
}

impl TaskIntentHandle {
    #[must_use]
    pub const fn new(id: TaskIntentId, attempt_id: TaskIntentAttemptId) -> Self {
        Self { id, attempt_id }
    }

    #[must_use]
    pub const fn id(self) -> TaskIntentId {
        self.id
    }

    #[must_use]
    pub const fn attempt_id(self) -> TaskIntentAttemptId {
        self.attempt_id
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TaskPriorityHint {
    Low,
    Normal,
    High,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TaskStatus {
    Queued,
    Running,
    Waiting,
    Blocked,
    Completed,
    Failed,
    Cancelling,
    Cancelled,
    FinishedAfterCancel,
    FailedToCancel,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum UnobservedPolicy {
    Continue,
    LowerPriority,
    Pause,
    Cancel,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TaskPriority {
    Low,
    Normal,
    High,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskPolicy {
    dedupe_by_key: bool,
    unobserved: UnobservedPolicy,
    priority: TaskPriority,
    retry_limit: Option<u32>,
}

impl TaskPolicy {
    #[must_use]
    pub const fn continue_when_unobserved() -> Self {
        Self {
            dedupe_by_key: false,
            unobserved: UnobservedPolicy::Continue,
            priority: TaskPriority::Normal,
            retry_limit: None,
        }
    }

    #[must_use]
    pub const fn cancel_when_unobserved() -> Self {
        Self {
            dedupe_by_key: false,
            unobserved: UnobservedPolicy::Cancel,
            priority: TaskPriority::Normal,
            retry_limit: None,
        }
    }

    #[must_use]
    pub const fn dedupe_by_key(mut self) -> Self {
        self.dedupe_by_key = true;
        self
    }

    #[must_use]
    pub const fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }

    #[must_use]
    pub const fn with_retry_limit(mut self, retry_limit: u32) -> Self {
        self.retry_limit = Some(retry_limit);
        self
    }

    #[must_use]
    pub const fn dedupes_by_key(&self) -> bool {
        self.dedupe_by_key
    }

    #[must_use]
    pub const fn unobserved(&self) -> UnobservedPolicy {
        self.unobserved
    }

    #[must_use]
    pub const fn priority(&self) -> TaskPriority {
        self.priority
    }

    #[must_use]
    pub const fn retry_limit(&self) -> Option<u32> {
        self.retry_limit
    }
}

pub struct TaskRegistration<Input> {
    id: TaskName,
    scope: Arc<dyn Fn(&Input) -> AppScope + Send + Sync>,
    key: Arc<dyn Fn(&Input) -> TaskKey + Send + Sync>,
    policy: TaskPolicy,
}

impl<Input> Clone for TaskRegistration<Input> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            scope: Arc::clone(&self.scope),
            key: Arc::clone(&self.key),
            policy: self.policy.clone(),
        }
    }
}

impl<Input> fmt::Debug for TaskRegistration<Input> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TaskRegistration")
            .field("id", &self.id)
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

impl<Input> TaskRegistration<Input> {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        let id = TaskName::new(id);
        let key_id = id.clone();

        Self {
            id,
            scope: Arc::new(|_| AppScope::app()),
            key: Arc::new(move |_| TaskKey::new(key_id.as_str())),
            policy: TaskPolicy::continue_when_unobserved(),
        }
    }

    #[must_use]
    pub fn scope(mut self, scope: impl Fn(&Input) -> AppScope + Send + Sync + 'static) -> Self {
        self.scope = Arc::new(scope);
        self
    }

    #[must_use]
    pub fn key(mut self, key: impl Fn(&Input) -> TaskKey + Send + Sync + 'static) -> Self {
        self.key = Arc::new(key);
        self
    }

    #[must_use]
    pub fn with_policy(mut self, policy: TaskPolicy) -> Self {
        self.policy = policy;
        self
    }

    #[must_use]
    pub fn id(&self) -> &TaskName {
        &self.id
    }

    #[must_use]
    pub fn scope_for(&self, input: &Input) -> AppScope {
        (self.scope)(input)
    }

    #[must_use]
    pub fn key_for(&self, input: &Input) -> TaskKey {
        (self.key)(input)
    }

    #[must_use]
    pub const fn policy(&self) -> &TaskPolicy {
        &self.policy
    }
}

#[derive(Clone, Debug)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    #[must_use]
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskHandle {
    task_id: TaskId,
    attempt_id: TaskAttemptId,
}

impl TaskHandle {
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

#[derive(Clone, Debug)]
pub struct TaskRecord {
    id: TaskId,
    key: TaskKey,
    scope: AppScope,
    policy: TaskPolicy,
    status: TaskStatus,
    attempt_id: Option<TaskAttemptId>,
    cancellation: CancellationToken,
    observers: usize,
}

impl TaskRecord {
    #[must_use]
    pub fn queued(id: TaskId, key: TaskKey, scope: AppScope, policy: TaskPolicy) -> Self {
        Self {
            id,
            key,
            scope,
            policy,
            status: TaskStatus::Queued,
            attempt_id: None,
            cancellation: CancellationToken::new(),
            observers: 0,
        }
    }

    #[must_use]
    pub fn running_for_test(
        id: TaskId,
        key: TaskKey,
        scope: AppScope,
        policy: TaskPolicy,
        attempt_id: TaskAttemptId,
    ) -> Self {
        let mut record = Self::queued(id, key, scope, policy);
        record.start_attempt(attempt_id);
        record.mark_running();
        record
    }

    pub fn start_attempt(&mut self, attempt_id: TaskAttemptId) -> TaskAttemptId {
        self.attempt_id = Some(attempt_id);
        self.cancellation = CancellationToken::new();
        attempt_id
    }

    pub fn mark_running(&mut self) {
        self.status = TaskStatus::Running;
    }

    #[must_use]
    pub fn request_cancel(&mut self) -> CancellationToken {
        self.cancellation.cancel();
        self.status = TaskStatus::Cancelling;
        self.cancellation.clone()
    }

    pub fn mark_finished_after_cancel(&mut self) {
        self.status = TaskStatus::FinishedAfterCancel;
    }

    #[must_use]
    pub const fn id(&self) -> TaskId {
        self.id
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
    pub const fn policy(&self) -> &TaskPolicy {
        &self.policy
    }

    #[must_use]
    pub const fn status(&self) -> TaskStatus {
        self.status
    }

    #[must_use]
    pub const fn attempt_id(&self) -> Option<TaskAttemptId> {
        self.attempt_id
    }

    #[must_use]
    pub const fn cancellation(&self) -> &CancellationToken {
        &self.cancellation
    }

    #[must_use]
    pub const fn observers(&self) -> usize {
        self.observers
    }

    #[must_use]
    pub fn accepts_attempt(&self, attempt_id: TaskAttemptId) -> bool {
        self.attempt_id == Some(attempt_id)
    }

    #[must_use]
    pub fn reject_stale(&self, attempt_id: TaskAttemptId) -> Diagnostic {
        Diagnostic::warning(
            DiagnosticCode::STALE_TASK_EVENT,
            "task event came from a stale attempt",
            InputProvenance::task(self.id, attempt_id),
        )
        .with_scope(self.scope.clone())
        .with_task(self.id, attempt_id)
    }
}
