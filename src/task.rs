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
