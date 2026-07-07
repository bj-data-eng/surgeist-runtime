use std::{
    borrow::Cow,
    collections::{HashMap, HashSet, VecDeque},
};

use super::{CustomScopeId, ResourceId, ServiceId, SurfaceId, TaskAttemptId, TaskId, TaskKey};
use surgeist_window::Id;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ScopePathSegment {
    namespace: String,
    value: String,
}

impl ScopePathSegment {
    #[must_use]
    pub fn new(namespace: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            value: value.into(),
        }
    }

    #[must_use]
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppScope {
    segments: Vec<ScopePathSegment>,
}

impl AppScope {
    #[must_use]
    pub fn app() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    #[must_use]
    pub fn window(id: Id) -> Self {
        Self::app().then(ScopePathSegment::new("window", id.as_u64().to_string()))
    }

    #[must_use]
    pub fn surface(id: SurfaceId) -> Self {
        Self::app().then(ScopePathSegment::new("surface", id.as_u64().to_string()))
    }

    #[must_use]
    pub fn resource(id: ResourceId) -> Self {
        Self::app().then(ScopePathSegment::new("resource", id.as_str()))
    }

    #[must_use]
    pub fn custom(id: impl Into<CustomScopeId>) -> Self {
        let id = id.into();
        Self::app().then(ScopePathSegment::new("custom", id.as_str()))
    }

    #[must_use]
    pub fn workspace(value: impl Into<String>) -> Self {
        Self::app().then(ScopePathSegment::new("workspace", value))
    }

    #[must_use]
    pub fn document(value: impl Into<String>) -> Self {
        Self::app().then(ScopePathSegment::new("document", value))
    }

    #[must_use]
    pub fn widget(value: impl Into<String>) -> Self {
        Self::app().then(ScopePathSegment::new("widget", value))
    }

    #[must_use]
    pub fn then(mut self, segment: ScopePathSegment) -> Self {
        self.segments.push(segment);
        self
    }

    #[must_use]
    pub fn segments(&self) -> &[ScopePathSegment] {
        &self.segments
    }

    #[must_use]
    pub fn is_app(&self) -> bool {
        self.segments.is_empty()
    }

    #[must_use]
    pub fn resource_id(&self) -> Option<ResourceId> {
        self.last_value("resource").map(ResourceId::new)
    }

    #[must_use]
    pub fn window_id(&self) -> Option<Id> {
        self.last_value("window")
            .and_then(|value| value.parse().ok())
            .map(Id::from_u64)
    }

    #[must_use]
    pub fn surface_id(&self) -> Option<SurfaceId> {
        self.last_value("surface")
            .and_then(|value| value.parse().ok())
            .map(SurfaceId::from_u64)
    }

    fn last_value(&self, namespace: &str) -> Option<&str> {
        self.segments
            .iter()
            .rev()
            .find(|segment| segment.namespace() == namespace)
            .map(ScopePathSegment::value)
    }
}

impl From<&str> for CustomScopeId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for CustomScopeId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SubscriptionTargetKindId(Cow<'static, str>);

impl SubscriptionTargetKindId {
    pub const TASK: Self = Self(Cow::Borrowed("task"));
    pub const RESOURCE: Self = Self(Cow::Borrowed("resource"));
    pub const SERVICE: Self = Self(Cow::Borrowed("service"));

    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(Cow::Owned(value.into()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SubscriptionTarget {
    kind: SubscriptionTargetKindId,
    key: String,
}

impl SubscriptionTarget {
    #[must_use]
    pub fn new(kind: SubscriptionTargetKindId, key: impl Into<String>) -> Self {
        Self {
            kind,
            key: key.into(),
        }
    }

    #[must_use]
    pub fn task(key: TaskKey) -> Self {
        Self::new(SubscriptionTargetKindId::TASK, key.as_str())
    }

    #[must_use]
    pub fn resource(id: ResourceId) -> Self {
        Self::new(SubscriptionTargetKindId::RESOURCE, id.as_str())
    }

    #[must_use]
    pub fn service(id: ServiceId) -> Self {
        Self::new(SubscriptionTargetKindId::SERVICE, id.as_str())
    }

    #[must_use]
    pub fn kind(&self) -> &SubscriptionTargetKindId {
        &self.kind
    }

    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum SubscriptionPriority {
    Low,
    #[default]
    Normal,
    High,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Subscription {
    target: SubscriptionTarget,
    scope: AppScope,
    observer: SurfaceId,
    priority: SubscriptionPriority,
}

impl Subscription {
    #[must_use]
    pub fn new(target: SubscriptionTarget) -> Self {
        Self {
            target,
            scope: AppScope::app(),
            observer: SurfaceId::from_u64(0),
            priority: SubscriptionPriority::Normal,
        }
    }

    #[must_use]
    pub fn task(key: TaskKey) -> Self {
        Self::new(SubscriptionTarget::task(key))
    }

    #[must_use]
    pub fn resource(id: ResourceId) -> Self {
        Self::new(SubscriptionTarget::resource(id))
    }

    #[must_use]
    pub fn service(id: ServiceId) -> Self {
        Self::new(SubscriptionTarget::service(id))
    }

    #[must_use]
    pub fn scope(mut self, scope: AppScope) -> Self {
        self.scope = scope;
        self
    }

    #[must_use]
    pub const fn observer(mut self, observer: SurfaceId) -> Self {
        self.observer = observer;
        self
    }

    #[must_use]
    pub const fn with_priority(mut self, priority: SubscriptionPriority) -> Self {
        self.priority = priority;
        self
    }

    #[must_use]
    pub fn target(&self) -> SubscriptionTarget {
        self.target.clone()
    }

    #[must_use]
    pub const fn scope_ref(&self) -> &AppScope {
        &self.scope
    }

    #[must_use]
    pub const fn observer_id(&self) -> SurfaceId {
        self.observer
    }

    #[must_use]
    pub const fn priority(&self) -> SubscriptionPriority {
        self.priority
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CoalescingKey(String);

impl CoalescingKey {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgressEvent {
    task_id: TaskId,
    attempt_id: TaskAttemptId,
    key: CoalescingKey,
    payload: String,
}

impl ProgressEvent {
    #[must_use]
    pub fn new(
        task_id: TaskId,
        attempt_id: TaskAttemptId,
        key: CoalescingKey,
        payload: impl Into<String>,
    ) -> Self {
        Self {
            task_id,
            attempt_id,
            key,
            payload: payload.into(),
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

    #[must_use]
    pub const fn key(&self) -> &CoalescingKey {
        &self.key
    }

    #[must_use]
    pub fn payload(&self) -> &str {
        &self.payload
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ProgressSlot {
    task_id: TaskId,
    attempt_id: TaskAttemptId,
    key: CoalescingKey,
}

impl From<&ProgressEvent> for ProgressSlot {
    fn from(event: &ProgressEvent) -> Self {
        Self {
            task_id: event.task_id,
            attempt_id: event.attempt_id,
            key: event.key.clone(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct CoordinationState {
    observers: HashMap<SubscriptionTarget, HashSet<SurfaceId>>,
    progress: HashMap<ProgressSlot, ProgressEvent>,
    progress_order: VecDeque<ProgressSlot>,
    coalesced_progress_count: usize,
}

impl CoordinationState {
    pub fn subscribe(&mut self, subscription: Subscription) {
        self.observers
            .entry(subscription.target)
            .or_default()
            .insert(subscription.observer);
    }

    pub fn unsubscribe(&mut self, subscription: &Subscription) {
        let target = subscription.target();
        let should_remove = if let Some(observers) = self.observers.get_mut(&target) {
            observers.remove(&subscription.observer);
            observers.is_empty()
        } else {
            false
        };

        if should_remove {
            self.observers.remove(&target);
        }
    }

    #[must_use]
    pub fn observer_count(&self, target: &SubscriptionTarget) -> usize {
        self.observers.get(target).map_or(0, HashSet::len)
    }

    #[must_use]
    pub fn is_observed(&self, target: &SubscriptionTarget) -> bool {
        self.observer_count(target) > 0
    }

    pub fn record_progress(&mut self, event: ProgressEvent) {
        let slot = ProgressSlot::from(&event);
        if self.progress.insert(slot.clone(), event).is_some() {
            self.coalesced_progress_count += 1;
        } else {
            self.progress_order.push_back(slot);
        }
    }

    #[must_use]
    pub fn drain_progress_budgeted(&mut self, budget: usize) -> Vec<ProgressEvent> {
        let mut drained = Vec::new();

        while drained.len() < budget {
            let Some(slot) = self.progress_order.pop_front() else {
                break;
            };

            if let Some(event) = self.progress.remove(&slot) {
                drained.push(event);
            }
        }

        drained
    }

    #[must_use]
    pub const fn coalesced_progress_count(&self) -> usize {
        self.coalesced_progress_count
    }
}
