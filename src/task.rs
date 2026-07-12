#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// An opaque, caller-defined name for the kind of work a runtime asks root to start.
pub struct TaskIntentName(String);

impl TaskIntentName {
    #[must_use]
    /// Stores task-kind text without parsing or executing it.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    /// Borrows the task-kind text for root-owned intent lowering.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// An opaque caller-defined key used to correlate related task intents.
pub struct TaskIntentKey(String);

impl TaskIntentKey {
    #[must_use]
    /// Stores key text without imposing task-execution policy.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    /// Borrows the correlation key text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// An opaque runtime-assigned numeric identity for a task intent.
pub struct TaskIntentId(u64);

impl TaskIntentId {
    #[must_use]
    /// Preserves an exact numeric task-intent identity supplied by an adapter.
    pub const fn from_u64(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    /// Returns the exact numeric identity for adapter transport.
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// An opaque runtime-assigned numeric identity for one attempt of a task intent.
pub struct TaskIntentAttemptId(u64);

impl TaskIntentAttemptId {
    #[must_use]
    /// Preserves an exact numeric attempt identity supplied by an adapter.
    pub const fn from_u64(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    /// Returns the exact numeric attempt identity for adapter transport.
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// The pair that uniquely identifies one task-intent attempt.
pub struct TaskIntentHandle {
    id: TaskIntentId,
    attempt_id: TaskIntentAttemptId,
}

impl TaskIntentHandle {
    #[must_use]
    /// Couples an intent identity with the attempt identity selected for it.
    pub const fn new(id: TaskIntentId, attempt_id: TaskIntentAttemptId) -> Self {
        Self { id, attempt_id }
    }

    #[must_use]
    /// Returns the intent identity by value.
    pub const fn id(self) -> TaskIntentId {
        self.id
    }

    #[must_use]
    /// Returns the attempt identity by value.
    pub const fn attempt_id(self) -> TaskIntentAttemptId {
        self.attempt_id
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
/// A scheduling preference carried by an abstract task intent.
///
/// Root adapters may interpret this hint when lowering the intent, while this crate
/// neither executes tasks nor defines concrete scheduler policy.
pub enum TaskPriorityHint {
    /// Prefer this work after normal and high-priority intents.
    Low,
    /// Request ordinary scheduling without elevation or demotion.
    Normal,
    /// Prefer this work before normal and low-priority intents.
    High,
}
