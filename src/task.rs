#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// A public runtime value with a private representation.
pub struct TaskIntentName(String);

impl TaskIntentName {
    #[must_use]
    /// Constructs this runtime value.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    /// Performs the associated runtime operation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// A public runtime value with a private representation.
pub struct TaskIntentKey(String);

impl TaskIntentKey {
    #[must_use]
    /// Constructs this runtime value.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    /// Performs the associated runtime operation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// A public runtime value with a private representation.
pub struct TaskIntentId(u64);

impl TaskIntentId {
    #[must_use]
    /// Constructs this runtime value.
    pub const fn from_u64(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    /// Performs the associated runtime operation.
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// A public runtime value with a private representation.
pub struct TaskIntentAttemptId(u64);

impl TaskIntentAttemptId {
    #[must_use]
    /// Constructs this runtime value.
    pub const fn from_u64(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    /// Performs the associated runtime operation.
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// A public runtime value with a private representation.
pub struct TaskIntentHandle {
    id: TaskIntentId,
    attempt_id: TaskIntentAttemptId,
}

impl TaskIntentHandle {
    #[must_use]
    /// Constructs this runtime value.
    pub const fn new(id: TaskIntentId, attempt_id: TaskIntentAttemptId) -> Self {
        Self { id, attempt_id }
    }

    #[must_use]
    /// Performs the associated runtime operation.
    pub const fn id(self) -> TaskIntentId {
        self.id
    }

    #[must_use]
    /// Performs the associated runtime operation.
    pub const fn attempt_id(self) -> TaskIntentAttemptId {
        self.attempt_id
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
/// Classifies a public runtime state or outcome.
pub enum TaskPriorityHint {
    /// One case of this public runtime contract.
    Low,
    /// One case of this public runtime contract.
    Normal,
    /// One case of this public runtime contract.
    High,
}
