use std::{borrow::Cow, collections::HashMap, error::Error, fmt};

use super::{CustomScopeId, ResourceId, ServiceId, SurfaceId, SurfaceRef, TaskIntentKey, WindowId};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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
    pub fn window(id: WindowId) -> Self {
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
    pub fn window_id(&self) -> Option<WindowId> {
        self.last_value("window")
            .and_then(|value| value.parse().ok())
            .map(WindowId::from_u64)
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
    pub fn task(key: TaskIntentKey) -> Self {
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

/// The relative importance of a subscription when aggregates are queried.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SubscriptionPriority {
    Low,
    #[default]
    Normal,
    High,
}

/// The complete identity of one subscription and its replay refcount.
///
/// Target, scope, generation-qualified observer, and priority all participate
/// in identity. Changing any one creates a distinct subscription key.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SubscriptionKey {
    target: SubscriptionTarget,
    scope: AppScope,
    observer: SurfaceRef,
    priority: SubscriptionPriority,
}

impl SubscriptionKey {
    #[must_use]
    pub fn new(
        target: SubscriptionTarget,
        scope: AppScope,
        observer: SurfaceRef,
        priority: SubscriptionPriority,
    ) -> Self {
        Self {
            target,
            scope,
            observer,
            priority,
        }
    }

    #[must_use]
    pub const fn target(&self) -> &SubscriptionTarget {
        &self.target
    }

    #[must_use]
    pub const fn scope(&self) -> &AppScope {
        &self.scope
    }

    #[must_use]
    pub const fn observer(&self) -> SurfaceRef {
        self.observer
    }

    #[must_use]
    pub const fn priority(&self) -> SubscriptionPriority {
        self.priority
    }
}

/// A request to add a reference to one complete [`SubscriptionKey`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Subscription {
    key: SubscriptionKey,
}

impl Subscription {
    /// Creates a subscription for a complete key.
    #[must_use]
    pub fn new(key: SubscriptionKey) -> Self {
        Self { key }
    }

    /// Creates a task subscription without supplying defaults for identity fields.
    #[must_use]
    pub fn task(
        key: TaskIntentKey,
        scope: AppScope,
        observer: SurfaceRef,
        priority: SubscriptionPriority,
    ) -> Self {
        Self::new(SubscriptionKey::new(
            SubscriptionTarget::task(key),
            scope,
            observer,
            priority,
        ))
    }

    /// Creates a resource subscription without supplying defaults for identity fields.
    #[must_use]
    pub fn resource(
        id: ResourceId,
        scope: AppScope,
        observer: SurfaceRef,
        priority: SubscriptionPriority,
    ) -> Self {
        Self::new(SubscriptionKey::new(
            SubscriptionTarget::resource(id),
            scope,
            observer,
            priority,
        ))
    }

    /// Creates a service subscription without supplying defaults for identity fields.
    #[must_use]
    pub fn service(
        id: ServiceId,
        scope: AppScope,
        observer: SurfaceRef,
        priority: SubscriptionPriority,
    ) -> Self {
        Self::new(SubscriptionKey::new(
            SubscriptionTarget::service(id),
            scope,
            observer,
            priority,
        ))
    }

    /// Returns the complete subscription identity.
    #[must_use]
    pub const fn key(&self) -> &SubscriptionKey {
        &self.key
    }
}

/// The exact result of adding or removing one subscription reference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SubscriptionChange {
    Added {
        key: SubscriptionKey,
        ref_count: usize,
    },
    Replayed {
        key: SubscriptionKey,
        ref_count: usize,
    },
    Decremented {
        key: SubscriptionKey,
        ref_count: usize,
    },
    Removed {
        key: SubscriptionKey,
    },
    NotFound {
        key: SubscriptionKey,
    },
}

impl SubscriptionChange {
    /// Returns the complete key whose reference count was observed or changed.
    #[must_use]
    pub const fn key(&self) -> &SubscriptionKey {
        match self {
            Self::Added { key, .. }
            | Self::Replayed { key, .. }
            | Self::Decremented { key, .. }
            | Self::Removed { key }
            | Self::NotFound { key } => key,
        }
    }

    /// Returns the resulting reference count, including zero after removal or absence.
    #[must_use]
    pub const fn ref_count(&self) -> usize {
        match self {
            Self::Added { ref_count, .. }
            | Self::Replayed { ref_count, .. }
            | Self::Decremented { ref_count, .. } => *ref_count,
            Self::Removed { .. } | Self::NotFound { .. } => 0,
        }
    }
}

/// Reasons runtime-owned observer validation or refcount changes can fail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SubscriptionErrorCode {
    UnknownObserver,
    StaleObserver,
    TerminalObserver,
    RefCountOverflow,
}

/// An error for one complete subscription key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubscriptionError {
    code: SubscriptionErrorCode,
    key: SubscriptionKey,
}

impl SubscriptionError {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn new(code: SubscriptionErrorCode, key: SubscriptionKey) -> Self {
        Self { code, key }
    }

    /// Returns the semantic reason for this subscription failure.
    #[must_use]
    pub const fn code(&self) -> SubscriptionErrorCode {
        self.code
    }

    /// Returns the exact key whose operation failed.
    #[must_use]
    pub const fn key(&self) -> &SubscriptionKey {
        &self.key
    }
}

impl fmt::Display for SubscriptionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.code {
            SubscriptionErrorCode::UnknownObserver => "subscription observer is unknown",
            SubscriptionErrorCode::StaleObserver => "subscription observer is stale",
            SubscriptionErrorCode::TerminalObserver => "subscription observer is terminal",
            SubscriptionErrorCode::RefCountOverflow => "subscription reference count overflow",
        };
        formatter.write_str(message)
    }
}

impl Error for SubscriptionError {}

/// A deterministic view of all active subscription keys for one target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubscriptionAggregate {
    target: SubscriptionTarget,
    active_keys: usize,
    observers: Vec<SurfaceRef>,
    scopes: Vec<AppScope>,
    highest_priority: SubscriptionPriority,
}

impl SubscriptionAggregate {
    /// Returns the target shared by this aggregate.
    #[must_use]
    pub const fn target(&self) -> &SubscriptionTarget {
        &self.target
    }

    /// Returns the number of distinct active full keys.
    #[must_use]
    pub const fn active_keys(&self) -> usize {
        self.active_keys
    }

    /// Returns unique observer registrations ordered by surface ID then generation.
    #[must_use]
    pub fn observers(&self) -> &[SurfaceRef] {
        &self.observers
    }

    /// Returns unique scopes in deterministic structural order.
    #[must_use]
    pub fn scopes(&self) -> &[AppScope] {
        &self.scopes
    }

    /// Returns the highest priority among active keys.
    #[must_use]
    pub const fn highest_priority(&self) -> SubscriptionPriority {
        self.highest_priority
    }
}

/// Stores subscription references while runtime owns observer validation.
#[derive(Clone, Debug, Default)]
pub struct CoordinationState {
    ref_counts: HashMap<SubscriptionKey, usize>,
}

impl CoordinationState {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn subscribe(
        &mut self,
        subscription: &Subscription,
    ) -> Result<SubscriptionChange, SubscriptionError> {
        let key = subscription.key().clone();
        match self.ref_counts.get_mut(&key) {
            Some(ref_count) => {
                let next_count = ref_count.checked_add(1).ok_or_else(|| {
                    SubscriptionError::new(SubscriptionErrorCode::RefCountOverflow, key.clone())
                })?;
                *ref_count = next_count;
                Ok(SubscriptionChange::Replayed {
                    key,
                    ref_count: next_count,
                })
            }
            None => {
                self.ref_counts.insert(key.clone(), 1);
                Ok(SubscriptionChange::Added { key, ref_count: 1 })
            }
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn unsubscribe(&mut self, key: &SubscriptionKey) -> SubscriptionChange {
        let Some(ref_count) = self.ref_counts.get_mut(key) else {
            return SubscriptionChange::NotFound { key: key.clone() };
        };

        if *ref_count > 1 {
            *ref_count -= 1;
            return SubscriptionChange::Decremented {
                key: key.clone(),
                ref_count: *ref_count,
            };
        }

        self.ref_counts.remove(key);
        SubscriptionChange::Removed { key: key.clone() }
    }

    /// Returns the current replay reference count for an exact key.
    #[must_use]
    pub fn ref_count(&self, key: &SubscriptionKey) -> usize {
        self.ref_counts.get(key).copied().unwrap_or(0)
    }

    /// Returns the deterministic aggregate for active keys sharing `target`.
    #[must_use]
    pub fn aggregate(&self, target: &SubscriptionTarget) -> Option<SubscriptionAggregate> {
        let keys = self
            .ref_counts
            .keys()
            .filter(|key| key.target() == target)
            .collect::<Vec<_>>();
        if keys.is_empty() {
            return None;
        }

        let mut observers = keys.iter().map(|key| key.observer()).collect::<Vec<_>>();
        observers.sort_by_key(|observer| {
            (
                observer.surface_id().as_u64(),
                observer.generation().as_u64(),
            )
        });
        observers.dedup();

        let mut scopes = keys
            .iter()
            .map(|key| key.scope().clone())
            .collect::<Vec<_>>();
        scopes.sort();
        scopes.dedup();

        let highest_priority = keys
            .iter()
            .fold(SubscriptionPriority::Low, |priority, key| {
                priority.max(key.priority())
            });

        Some(SubscriptionAggregate {
            target: target.clone(),
            active_keys: keys.len(),
            observers,
            scopes,
            highest_priority,
        })
    }

    /// Returns the unique generation-qualified observers for a resource target.
    #[must_use]
    pub fn resource_observer_count(&self, id: &ResourceId) -> usize {
        self.aggregate(&SubscriptionTarget::resource(id.clone()))
            .map_or(0, |aggregate| aggregate.observers().len())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn remove_observer(&mut self, observer: SurfaceRef) {
        self.ref_counts.retain(|key, _| key.observer() != observer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SurfaceGeneration, SurfaceRef};

    #[test]
    fn subscription_cleanup_removes_only_the_exact_observer_registration() {
        let target = SubscriptionTarget::resource(ResourceId::new("graph"));
        let old = SurfaceRef::new(SurfaceId::from_u64(3), SurfaceGeneration::initial());
        let replacement = SurfaceRef::new(SurfaceId::from_u64(3), SurfaceGeneration::from_u64(1));
        let old_subscription = Subscription::new(SubscriptionKey::new(
            target.clone(),
            AppScope::app(),
            old,
            SubscriptionPriority::Normal,
        ));
        let replacement_subscription = Subscription::new(SubscriptionKey::new(
            target.clone(),
            AppScope::app(),
            replacement,
            SubscriptionPriority::Normal,
        ));
        let mut coordination = CoordinationState::default();

        coordination.subscribe(&old_subscription).unwrap();
        coordination.subscribe(&old_subscription).unwrap();
        coordination.subscribe(&replacement_subscription).unwrap();
        coordination.remove_observer(old);

        assert_eq!(coordination.ref_count(old_subscription.key()), 0);
        assert_eq!(coordination.ref_count(replacement_subscription.key()), 1);
        assert_eq!(
            coordination.resource_observer_count(&ResourceId::new("graph")),
            1
        );
    }

    #[test]
    fn subscription_refcount_overflow_is_atomic() {
        let subscription = Subscription::resource(
            ResourceId::new("graph"),
            AppScope::app(),
            SurfaceRef::new(SurfaceId::from_u64(3), SurfaceGeneration::initial()),
            SubscriptionPriority::Normal,
        );
        let key = subscription.key().clone();
        let mut coordination = CoordinationState::default();
        coordination.ref_counts.insert(key.clone(), usize::MAX);

        let error = coordination.subscribe(&subscription).unwrap_err();
        assert_eq!(error.code(), SubscriptionErrorCode::RefCountOverflow);
        assert_eq!(error.key(), &key);
        assert_eq!(error.to_string(), "subscription reference count overflow");
        assert!(std::error::Error::source(&error).is_none());
        assert_eq!(coordination.ref_count(&key), usize::MAX);
        assert_eq!(
            coordination.aggregate(key.target()).unwrap().active_keys(),
            1
        );
    }
}
