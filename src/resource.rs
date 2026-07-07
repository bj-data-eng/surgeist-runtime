use super::{ResourceId, StateVersion};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResourceStatus {
    Idle,
    Starting,
    Running,
    Refreshing,
    Ready,
    Failed,
    Cancelling,
    Cancelled,
    Stale,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Freshness {
    Fresh,
    Stale,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FailureVisibility {
    ClearValue,
    KeepStaleValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceState<T, E> {
    id: ResourceId,
    status: ResourceStatus,
    value: Option<T>,
    error: Option<E>,
    freshness: Freshness,
    stale_reason: Option<String>,
    version: StateVersion,
    observer_count: usize,
}

impl<T, E> ResourceState<T, E> {
    #[must_use]
    pub fn idle(id: ResourceId) -> Self {
        Self {
            id,
            status: ResourceStatus::Idle,
            value: None,
            error: None,
            freshness: Freshness::Fresh,
            stale_reason: None,
            version: StateVersion::initial(),
            observer_count: 0,
        }
    }

    #[must_use]
    pub fn ready(id: ResourceId, value: T, freshness: Freshness) -> Self {
        let mut resource = Self::idle(id);
        resource.set_ready(value, freshness);
        resource
    }

    pub fn starting(&mut self) {
        self.status = ResourceStatus::Starting;
        self.error = None;
        self.bump_version();
    }

    pub fn refreshing(&mut self) {
        self.status = ResourceStatus::Refreshing;
        self.error = None;
        self.bump_version();
    }

    pub fn failed(&mut self, error: E, visibility: FailureVisibility) {
        self.status = ResourceStatus::Failed;
        self.error = Some(error);
        if visibility == FailureVisibility::ClearValue {
            self.value = None;
        } else if self.value.is_some() {
            self.freshness = Freshness::Stale;
        }
        self.bump_version();
    }

    pub fn mark_stale(&mut self, reason: impl Into<String>) {
        self.freshness = Freshness::Stale;
        self.stale_reason = Some(reason.into());
        self.bump_version();
    }

    pub fn add_observer(&mut self) {
        self.observer_count += 1;
        self.bump_version();
    }

    pub fn remove_observer(&mut self) {
        if self.observer_count > 0 {
            self.observer_count -= 1;
            self.bump_version();
        }
    }

    #[must_use]
    pub fn id(&self) -> &ResourceId {
        &self.id
    }

    #[must_use]
    pub const fn status(&self) -> ResourceStatus {
        self.status
    }

    #[must_use]
    pub fn value(&self) -> Option<&T> {
        self.value.as_ref()
    }

    #[must_use]
    pub fn error(&self) -> Option<&E> {
        self.error.as_ref()
    }

    #[must_use]
    pub fn is_renderable(&self) -> bool {
        self.value.is_some()
    }

    #[must_use]
    pub const fn freshness(&self) -> Freshness {
        self.freshness
    }

    #[must_use]
    pub fn stale_reason(&self) -> Option<&str> {
        self.stale_reason.as_deref()
    }

    #[must_use]
    pub const fn version(&self) -> StateVersion {
        self.version
    }

    #[must_use]
    pub const fn observer_count(&self) -> usize {
        self.observer_count
    }

    fn set_ready(&mut self, value: T, freshness: Freshness) {
        self.status = ResourceStatus::Ready;
        self.value = Some(value);
        self.error = None;
        self.freshness = freshness;
        if freshness == Freshness::Fresh {
            self.stale_reason = None;
        }
        self.bump_version();
    }

    fn bump_version(&mut self) {
        self.version = self.version.next();
    }
}

impl<T: Clone, E: Clone> ResourceState<T, E> {
    #[must_use]
    pub fn snapshot(&self) -> ResourceSnapshot<T, E> {
        ResourceSnapshot {
            id: self.id.clone(),
            status: self.status,
            value: self.value.clone(),
            error: self.error.clone(),
            freshness: self.freshness,
            stale_reason: self.stale_reason.clone(),
            version: self.version,
            observer_count: self.observer_count,
        }
    }
}

pub trait ResourceStateReadyTransition<T> {
    fn ready(&mut self, value: T, freshness: Freshness);
}

impl<T, E> ResourceStateReadyTransition<T> for ResourceState<T, E> {
    fn ready(&mut self, value: T, freshness: Freshness) {
        self.set_ready(value, freshness);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceSnapshot<T, E> {
    id: ResourceId,
    status: ResourceStatus,
    value: Option<T>,
    error: Option<E>,
    freshness: Freshness,
    stale_reason: Option<String>,
    version: StateVersion,
    observer_count: usize,
}

impl<T, E> ResourceSnapshot<T, E> {
    #[must_use]
    pub fn id(&self) -> &ResourceId {
        &self.id
    }

    #[must_use]
    pub const fn status(&self) -> ResourceStatus {
        self.status
    }

    #[must_use]
    pub fn value(&self) -> Option<&T> {
        self.value.as_ref()
    }

    #[must_use]
    pub fn error(&self) -> Option<&E> {
        self.error.as_ref()
    }

    #[must_use]
    pub fn is_renderable(&self) -> bool {
        self.value.is_some()
    }

    #[must_use]
    pub const fn freshness(&self) -> Freshness {
        self.freshness
    }

    #[must_use]
    pub fn stale_reason(&self) -> Option<&str> {
        self.stale_reason.as_deref()
    }

    #[must_use]
    pub const fn version(&self) -> StateVersion {
        self.version
    }

    #[must_use]
    pub const fn observer_count(&self) -> usize {
        self.observer_count
    }
}
