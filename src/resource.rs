use std::{error::Error, fmt, num::NonZeroU64};

use super::{ResourceGeneration, ResourceId, ResourceOperationId, VersionError};
use crate::ids::CheckedNext;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// The lifecycle status of a resource-owned value.
///
/// `ResourceState` starts [`Idle`](Self::Idle). A load moves it through
/// [`Loading`](Self::Loading), while a refresh of a retained value moves it
/// through [`Refreshing`](Self::Refreshing). Completion, failure, cancellation,
/// and explicit invalidation select the remaining states.
pub enum ResourceStatus {
    /// No operation has started yet.
    Idle,
    /// A load operation is active.
    Loading,
    /// A current value is available.
    Ready,
    /// A refresh operation is active for a retained value.
    Refreshing,
    /// An active operation completed with an error.
    Failed,
    /// Cancellation has been requested for the active operation.
    Cancelling,
    /// The active operation acknowledged cancellation.
    Cancelled,
    /// A retained value was explicitly invalidated.
    Stale,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// Whether a retained resource value is current or stale.
///
/// This is meaningful only when [`ResourceState::value`] is present; states
/// without a value always report [`Fresh`](Self::Fresh).
pub enum Freshness {
    /// The retained value is current.
    Fresh,
    /// The retained value may be displayed but requires refresh.
    Stale,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// Controls whether a failed operation retains its previous value.
pub enum FailureVisibility {
    /// Clear the value, stale reason, and stale freshness when recording failure.
    ClearValue,
    /// Retain a previous value as stale when one exists.
    KeepStaleValue,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// An opaque identity for one operation issued by a [`ResourceState`].
///
/// Callers may carry this token to completion or cancellation methods, but can
/// only obtain it from [`ResourceState::begin_load`] or
/// [`ResourceState::begin_refresh`]. Its resource ID, operation ID, and
/// generation must all match the active operation.
pub struct ResourceOperation {
    resource_id: ResourceId,
    id: ResourceOperationId,
    generation: ResourceGeneration,
}

impl ResourceOperation {
    /// Returns the resource this operation belongs to.
    #[must_use]
    pub fn resource_id(&self) -> &ResourceId {
        &self.resource_id
    }

    /// Returns this operation's opaque ID.
    #[must_use]
    pub const fn id(&self) -> ResourceOperationId {
        self.id
    }

    /// Returns the resource generation issued with this operation.
    #[must_use]
    pub const fn generation(&self) -> ResourceGeneration {
        self.generation
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
/// Classifies a rejected resource-state transition.
///
/// Token mismatch takes precedence over transition-specific checks. A matching
/// token then distinguishes cancellation replay from an invalid transition.
pub enum ResourceStateErrorCode {
    /// The current status does not allow the requested transition.
    InvalidTransition,
    /// The supplied token does not match the active operation.
    OperationMismatch,
    /// A begin request was made while an operation is already active.
    OperationOverlap,
    /// Cancellation was already requested for the matching active operation.
    CancellationAlreadyRequested,
    /// Cancellation was replayed after the matching operation was cancelled.
    AlreadyCancelled,
    /// The next resource generation or operation ID cannot be issued.
    VersionOverflow,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Details of a rejected [`ResourceState`] transition.
///
/// The error always identifies the affected resource. Operation mismatch and
/// cancellation replay errors carry the expected and supplied tokens where
/// applicable. [`ResourceStateErrorCode::VersionOverflow`] carries
/// [`VersionError`] as its source; other error codes have no source.
pub struct ResourceStateError {
    code: ResourceStateErrorCode,
    resource_id: ResourceId,
    expected_operation: Option<ResourceOperation>,
    actual_operation: Option<ResourceOperation>,
    source: Option<VersionError>,
}

impl ResourceStateError {
    fn new(
        code: ResourceStateErrorCode,
        resource_id: &ResourceId,
        expected_operation: Option<ResourceOperation>,
        actual_operation: Option<ResourceOperation>,
    ) -> Self {
        Self {
            code,
            resource_id: resource_id.clone(),
            expected_operation,
            actual_operation,
            source: None,
        }
    }

    fn overflow(resource_id: &ResourceId) -> Self {
        Self {
            code: ResourceStateErrorCode::VersionOverflow,
            resource_id: resource_id.clone(),
            expected_operation: None,
            actual_operation: None,
            source: Some(VersionError::Overflow),
        }
    }

    /// Returns the semantic reason the transition was rejected.
    #[must_use]
    pub const fn code(&self) -> ResourceStateErrorCode {
        self.code
    }

    /// Returns the resource whose transition was rejected.
    #[must_use]
    pub fn resource_id(&self) -> &ResourceId {
        &self.resource_id
    }

    /// Returns the active or replay token the transition expected, when relevant.
    #[must_use]
    pub fn expected_operation(&self) -> Option<&ResourceOperation> {
        self.expected_operation.as_ref()
    }

    /// Returns the token supplied to the rejected transition, when relevant.
    #[must_use]
    pub fn actual_operation(&self) -> Option<&ResourceOperation> {
        self.actual_operation.as_ref()
    }

    /// Returns the checked-version failure for overflow errors.
    ///
    /// This is `Some` only for [`ResourceStateErrorCode::VersionOverflow`].
    #[must_use]
    pub const fn source(&self) -> Option<&VersionError> {
        self.source.as_ref()
    }
}

impl fmt::Display for ResourceStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.code {
            ResourceStateErrorCode::InvalidTransition => "invalid resource state transition",
            ResourceStateErrorCode::OperationMismatch => "resource operation does not match",
            ResourceStateErrorCode::OperationOverlap => "resource operation is already active",
            ResourceStateErrorCode::CancellationAlreadyRequested => {
                "resource cancellation was already requested"
            }
            ResourceStateErrorCode::AlreadyCancelled => "resource operation was already cancelled",
            ResourceStateErrorCode::VersionOverflow => "resource operation version overflow",
        };
        formatter.write_str(message)
    }
}

impl Error for ResourceStateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source.as_ref().map(|source| source as &dyn Error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Resource value state, its current operation, and its checked generation.
///
/// The state starts idle with generation zero. [`Self::begin_load`] is allowed
/// from idle, failed, and cancelled; [`Self::begin_refresh`] is allowed from
/// ready and stale. A matching active token may complete, fail, or request
/// cancellation; only cancellation acknowledgement from cancelling is valid.
/// [`Self::mark_stale`] is valid from ready or stale, and repeating the same
/// stale reason is a no-op.
///
/// Every successful non-idempotent transition advances the generation exactly
/// once. Rejected transitions and checked ID or generation overflow are atomic:
/// they leave every observable field and internal counter unchanged.
pub struct ResourceState<T, E> {
    id: ResourceId,
    status: ResourceStatus,
    value: Option<T>,
    error: Option<E>,
    freshness: Freshness,
    stale_reason: Option<String>,
    generation: ResourceGeneration,
    next_operation_id: u64,
    active_operation: Option<ResourceOperation>,
    last_cancelled_operation: Option<ResourceOperation>,
}

impl<T, E> ResourceState<T, E> {
    /// Creates an idle resource with no value, error, or active operation.
    #[must_use]
    pub fn new(id: ResourceId) -> Self {
        Self {
            id,
            status: ResourceStatus::Idle,
            value: None,
            error: None,
            freshness: Freshness::Fresh,
            stale_reason: None,
            generation: ResourceGeneration::initial(),
            next_operation_id: 1,
            active_operation: None,
            last_cancelled_operation: None,
        }
    }

    /// Starts a load and returns the operation token required for its outcome.
    ///
    /// Valid from idle, failed, and cancelled. An active operation instead
    /// returns [`ResourceStateErrorCode::OperationOverlap`].
    pub fn begin_load(&mut self) -> Result<ResourceOperation, ResourceStateError> {
        self.begin(ResourceStatus::Loading)
    }

    /// Starts a refresh of a ready or stale resource and returns its token.
    ///
    /// A refresh preserves the retained value, freshness, and stale reason while
    /// clearing any prior error.
    pub fn begin_refresh(&mut self) -> Result<ResourceOperation, ResourceStateError> {
        self.begin(ResourceStatus::Refreshing)
    }

    /// Completes the matching load or refresh with a fresh replacement value.
    ///
    /// This clears the error, stale reason, and active token, then enters
    /// [`ResourceStatus::Ready`].
    pub fn ready(
        &mut self,
        operation: &ResourceOperation,
        value: T,
    ) -> Result<(), ResourceStateError> {
        self.require_active(operation, false)?;
        if !matches!(
            self.status,
            ResourceStatus::Loading | ResourceStatus::Refreshing
        ) {
            return Err(self.invalid_transition());
        }
        let generation = self.next_generation()?;

        self.status = ResourceStatus::Ready;
        self.value = Some(value);
        self.error = None;
        self.freshness = Freshness::Fresh;
        self.stale_reason = None;
        self.generation = generation;
        self.active_operation = None;
        Ok(())
    }

    /// Completes the matching load or refresh with an error.
    ///
    /// [`FailureVisibility`] decides whether a retained value is cleared or
    /// remains visible as stale. In either case the active token is cleared and
    /// the state enters [`ResourceStatus::Failed`].
    pub fn failed(
        &mut self,
        operation: &ResourceOperation,
        error: E,
        visibility: FailureVisibility,
    ) -> Result<(), ResourceStateError> {
        self.require_active(operation, false)?;
        if !matches!(
            self.status,
            ResourceStatus::Loading | ResourceStatus::Refreshing
        ) {
            return Err(self.invalid_transition());
        }
        let generation = self.next_generation()?;

        self.status = ResourceStatus::Failed;
        self.error = Some(error);
        if visibility == FailureVisibility::ClearValue {
            self.value = None;
            self.freshness = Freshness::Fresh;
            self.stale_reason = None;
        } else if self.value.is_some() {
            self.freshness = Freshness::Stale;
        } else {
            self.freshness = Freshness::Fresh;
            self.stale_reason = None;
        }
        self.generation = generation;
        self.active_operation = None;
        Ok(())
    }

    /// Requests cancellation of the matching active load or refresh.
    ///
    /// The operation remains active in [`ResourceStatus::Cancelling`] until
    /// [`Self::cancelled`] acknowledges it. Repeating this request with the
    /// matching token returns [`ResourceStateErrorCode::CancellationAlreadyRequested`].
    pub fn cancel(&mut self, operation: &ResourceOperation) -> Result<(), ResourceStateError> {
        self.require_active(operation, true)?;
        match self.status {
            ResourceStatus::Cancelling => {
                return Err(ResourceStateError::new(
                    ResourceStateErrorCode::CancellationAlreadyRequested,
                    &self.id,
                    self.active_operation.clone(),
                    Some(operation.clone()),
                ));
            }
            ResourceStatus::Loading | ResourceStatus::Refreshing => {}
            _ => return Err(self.invalid_transition()),
        }
        let generation = self.next_generation()?;

        self.status = ResourceStatus::Cancelling;
        self.error = None;
        self.generation = generation;
        Ok(())
    }

    /// Acknowledges cancellation of the matching cancelling operation.
    ///
    /// This clears the active token, retains any value as stale, and records the
    /// token only to classify later cancellation replays as
    /// [`ResourceStateErrorCode::AlreadyCancelled`].
    pub fn cancelled(&mut self, operation: &ResourceOperation) -> Result<(), ResourceStateError> {
        self.require_active(operation, true)?;
        if self.status != ResourceStatus::Cancelling {
            return Err(self.invalid_transition());
        }
        let generation = self.next_generation()?;

        self.status = ResourceStatus::Cancelled;
        self.error = None;
        if self.value.is_some() {
            self.freshness = Freshness::Stale;
        } else {
            self.freshness = Freshness::Fresh;
            self.stale_reason = None;
        }
        self.generation = generation;
        self.active_operation = None;
        self.last_cancelled_operation = Some(operation.clone());
        Ok(())
    }

    /// Marks a ready or stale resource value as stale for `reason`.
    ///
    /// Repeating the same reason while already stale succeeds without changing
    /// the generation; a new reason is a normal state transition.
    pub fn mark_stale(&mut self, reason: impl Into<String>) -> Result<(), ResourceStateError> {
        let reason = reason.into();
        match self.status {
            ResourceStatus::Stale if self.stale_reason.as_deref() == Some(reason.as_str()) => {
                return Ok(());
            }
            ResourceStatus::Ready | ResourceStatus::Stale => {}
            _ => return Err(self.invalid_transition()),
        }
        let generation = self.next_generation()?;

        self.status = ResourceStatus::Stale;
        self.error = None;
        self.freshness = Freshness::Stale;
        self.stale_reason = Some(reason);
        self.generation = generation;
        Ok(())
    }

    /// Returns this resource's stable identity.
    #[must_use]
    pub fn id(&self) -> &ResourceId {
        &self.id
    }

    /// Returns the current lifecycle status.
    #[must_use]
    pub const fn status(&self) -> ResourceStatus {
        self.status
    }

    /// Returns the retained value, if any.
    #[must_use]
    pub fn value(&self) -> Option<&T> {
        self.value.as_ref()
    }

    /// Returns the most recently recorded failure, if any.
    #[must_use]
    pub fn error(&self) -> Option<&E> {
        self.error.as_ref()
    }

    /// Returns whether a value is available for rendering.
    ///
    /// This is exactly equivalent to `value().is_some()` regardless of status.
    #[must_use]
    pub const fn is_renderable(&self) -> bool {
        self.value.is_some()
    }

    /// Returns freshness for the retained value.
    #[must_use]
    pub const fn freshness(&self) -> Freshness {
        self.freshness
    }

    /// Returns explicit invalidation context, not a load or refresh error.
    #[must_use]
    pub fn stale_reason(&self) -> Option<&str> {
        self.stale_reason.as_deref()
    }

    /// Returns the checked generation of the current observable state.
    #[must_use]
    pub const fn generation(&self) -> ResourceGeneration {
        self.generation
    }

    /// Returns the token for the in-flight operation, if one exists.
    #[must_use]
    pub fn active_operation(&self) -> Option<&ResourceOperation> {
        self.active_operation.as_ref()
    }

    fn begin(
        &mut self,
        next_status: ResourceStatus,
    ) -> Result<ResourceOperation, ResourceStateError> {
        if matches!(
            self.status,
            ResourceStatus::Loading | ResourceStatus::Refreshing | ResourceStatus::Cancelling
        ) {
            return Err(ResourceStateError::new(
                ResourceStateErrorCode::OperationOverlap,
                &self.id,
                self.active_operation.clone(),
                None,
            ));
        }
        let allowed = match next_status {
            ResourceStatus::Loading => matches!(
                self.status,
                ResourceStatus::Idle | ResourceStatus::Failed | ResourceStatus::Cancelled
            ),
            ResourceStatus::Refreshing => {
                matches!(self.status, ResourceStatus::Ready | ResourceStatus::Stale)
            }
            _ => false,
        };
        if !allowed {
            return Err(self.invalid_transition());
        }

        let next_operation_id = self
            .next_operation_id
            .checked_add(1)
            .ok_or_else(|| ResourceStateError::overflow(&self.id))?;
        let operation_id = NonZeroU64::new(self.next_operation_id)
            .map(ResourceOperationId::new)
            .ok_or_else(|| ResourceStateError::overflow(&self.id))?;
        let generation = self.next_generation()?;
        let operation = ResourceOperation {
            resource_id: self.id.clone(),
            id: operation_id,
            generation,
        };

        self.status = next_status;
        self.error = None;
        if next_status == ResourceStatus::Loading {
            if self.value.is_some() {
                self.freshness = Freshness::Stale;
            } else {
                self.freshness = Freshness::Fresh;
                self.stale_reason = None;
            }
        }
        self.generation = generation;
        self.next_operation_id = next_operation_id;
        self.active_operation = Some(operation.clone());
        self.last_cancelled_operation = None;
        Ok(operation)
    }

    fn require_active(
        &self,
        operation: &ResourceOperation,
        cancellation: bool,
    ) -> Result<(), ResourceStateError> {
        if self.active_operation.as_ref() == Some(operation) {
            return Ok(());
        }
        if cancellation && self.last_cancelled_operation.as_ref() == Some(operation) {
            return Err(ResourceStateError::new(
                ResourceStateErrorCode::AlreadyCancelled,
                &self.id,
                self.last_cancelled_operation.clone(),
                Some(operation.clone()),
            ));
        }
        Err(ResourceStateError::new(
            ResourceStateErrorCode::OperationMismatch,
            &self.id,
            self.active_operation.clone(),
            Some(operation.clone()),
        ))
    }

    fn invalid_transition(&self) -> ResourceStateError {
        ResourceStateError::new(
            ResourceStateErrorCode::InvalidTransition,
            &self.id,
            None,
            None,
        )
    }

    fn next_generation(&self) -> Result<ResourceGeneration, ResourceStateError> {
        self.generation
            .checked_next()
            .map_err(|_| ResourceStateError::overflow(&self.id))
    }
}

impl<T: Clone, E: Clone> ResourceState<T, E> {
    /// Clones the resource's observable state into an observer-free snapshot.
    #[must_use]
    pub fn snapshot(&self) -> ResourceSnapshot<T, E> {
        ResourceSnapshot {
            id: self.id.clone(),
            status: self.status,
            value: self.value.clone(),
            error: self.error.clone(),
            freshness: self.freshness,
            stale_reason: self.stale_reason.clone(),
            generation: self.generation,
            active_operation: self.active_operation.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// An immutable clone of a resource's observable state at one generation.
///
/// A snapshot includes the resource identity, status, value, error, freshness,
/// stale reason, generation, and active operation. It does not include internal
/// replay state or observer counts.
pub struct ResourceSnapshot<T, E> {
    id: ResourceId,
    status: ResourceStatus,
    value: Option<T>,
    error: Option<E>,
    freshness: Freshness,
    stale_reason: Option<String>,
    generation: ResourceGeneration,
    active_operation: Option<ResourceOperation>,
}

impl<T, E> ResourceSnapshot<T, E> {
    /// Returns the snapshot's resource identity.
    #[must_use]
    pub fn id(&self) -> &ResourceId {
        &self.id
    }

    /// Returns the captured lifecycle status.
    #[must_use]
    pub const fn status(&self) -> ResourceStatus {
        self.status
    }

    /// Returns the captured retained value, if any.
    #[must_use]
    pub fn value(&self) -> Option<&T> {
        self.value.as_ref()
    }

    /// Returns the captured failure, if any.
    #[must_use]
    pub fn error(&self) -> Option<&E> {
        self.error.as_ref()
    }

    /// Returns whether the snapshot captured a renderable value.
    ///
    /// This is exactly equivalent to `value().is_some()`.
    #[must_use]
    pub const fn is_renderable(&self) -> bool {
        self.value.is_some()
    }

    /// Returns freshness captured for the retained value.
    #[must_use]
    pub const fn freshness(&self) -> Freshness {
        self.freshness
    }

    /// Returns captured explicit invalidation context.
    #[must_use]
    pub fn stale_reason(&self) -> Option<&str> {
        self.stale_reason.as_deref()
    }

    /// Returns the generation captured by this snapshot.
    #[must_use]
    pub const fn generation(&self) -> ResourceGeneration {
        self.generation
    }

    /// Returns the operation token captured as active, if any.
    #[must_use]
    pub fn active_operation(&self) -> Option<&ResourceOperation> {
        self.active_operation.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VersionError;

    fn resource() -> ResourceState<u32, &'static str> {
        ResourceState::new(ResourceId::new("photos"))
    }

    struct Observable<'a> {
        status: ResourceStatus,
        value: Option<u32>,
        error: Option<&'static str>,
        freshness: Freshness,
        stale_reason: Option<&'static str>,
        generation: u64,
        active_operation: Option<&'a ResourceOperation>,
    }

    fn assert_observable(resource: &ResourceState<u32, &'static str>, expected: Observable<'_>) {
        assert_eq!(resource.id(), &ResourceId::new("photos"));
        assert_eq!(resource.status(), expected.status);
        assert_eq!(resource.value().copied(), expected.value);
        assert_eq!(resource.error().copied(), expected.error);
        assert_eq!(resource.is_renderable(), expected.value.is_some());
        assert_eq!(resource.freshness(), expected.freshness);
        assert_eq!(resource.stale_reason(), expected.stale_reason);
        assert_eq!(
            resource.generation(),
            ResourceGeneration::from_u64(expected.generation)
        );
        assert_eq!(resource.active_operation(), expected.active_operation);

        let snapshot = resource.snapshot();
        assert_eq!(snapshot.id(), &ResourceId::new("photos"));
        assert_eq!(snapshot.status(), expected.status);
        assert_eq!(snapshot.value().copied(), expected.value);
        assert_eq!(snapshot.error().copied(), expected.error);
        assert_eq!(snapshot.is_renderable(), expected.value.is_some());
        assert_eq!(snapshot.freshness(), expected.freshness);
        assert_eq!(snapshot.stale_reason(), expected.stale_reason);
        assert_eq!(
            snapshot.generation(),
            ResourceGeneration::from_u64(expected.generation)
        );
        assert_eq!(snapshot.active_operation(), expected.active_operation);
    }

    #[test]
    fn operations_are_issued_and_overlap_and_mismatch_are_rejected() {
        let mut resource = resource();
        let operation = resource.begin_load().unwrap();

        assert_eq!(operation.resource_id(), resource.id());
        assert_eq!(operation.id().get(), 1);
        assert_eq!(operation.generation(), ResourceGeneration::from_u64(1));
        assert_eq!(
            resource.begin_refresh().unwrap_err().code(),
            ResourceStateErrorCode::OperationOverlap
        );

        let mut other = ResourceState::<u32, &'static str>::new(ResourceId::new("other"));
        let other_operation = other.begin_load().unwrap();
        let error = resource.ready(&other_operation, 1).unwrap_err();
        assert_eq!(error.code(), ResourceStateErrorCode::OperationMismatch);
        assert_eq!(error.expected_operation(), Some(&operation));
        assert_eq!(error.actual_operation(), Some(&other_operation));
    }

    #[test]
    fn cancellation_replay_has_exact_classification() {
        let mut resource = resource();
        let operation = resource.begin_load().unwrap();
        resource.cancel(&operation).unwrap();

        assert_eq!(
            resource.cancel(&operation).unwrap_err().code(),
            ResourceStateErrorCode::CancellationAlreadyRequested
        );
        assert_eq!(
            resource.ready(&operation, 1).unwrap_err().code(),
            ResourceStateErrorCode::InvalidTransition
        );
        resource.cancelled(&operation).unwrap();

        assert_eq!(
            resource.cancel(&operation).unwrap_err().code(),
            ResourceStateErrorCode::AlreadyCancelled
        );
        assert_eq!(
            resource.cancelled(&operation).unwrap_err().code(),
            ResourceStateErrorCode::AlreadyCancelled
        );
    }

    #[test]
    fn cancellation_and_restart_preserve_a_retained_value() {
        let mut resource = resource();
        let load = resource.begin_load().unwrap();
        resource.ready(&load, 1).unwrap();
        let refresh = resource.begin_refresh().unwrap();
        resource.cancel(&refresh).unwrap();

        assert_eq!(resource.status(), ResourceStatus::Cancelling);
        assert_eq!(resource.value(), Some(&1));
        assert_eq!(resource.error(), None);
        assert_eq!(resource.freshness(), Freshness::Fresh);
        assert_eq!(resource.active_operation(), Some(&refresh));

        resource.cancelled(&refresh).unwrap();
        assert_eq!(resource.status(), ResourceStatus::Cancelled);
        assert_eq!(resource.value(), Some(&1));
        assert_eq!(resource.error(), None);
        assert_eq!(resource.freshness(), Freshness::Stale);
        assert_eq!(resource.active_operation(), None);

        let retry = resource.begin_load().unwrap();
        assert_eq!(resource.status(), ResourceStatus::Loading);
        assert_eq!(retry.id().get(), 3);
        assert_eq!(resource.value(), Some(&1));
        assert_eq!(resource.freshness(), Freshness::Stale);
    }

    #[test]
    fn invalid_transitions_are_failure_atomic() {
        let mut resource = resource();
        let snapshot = resource.snapshot();
        assert_eq!(
            resource.begin_refresh().unwrap_err().code(),
            ResourceStateErrorCode::InvalidTransition
        );
        assert_eq!(resource.snapshot(), snapshot);

        let load = resource.begin_load().unwrap();
        resource.ready(&load, 1).unwrap();
        let snapshot = resource.snapshot();
        assert_eq!(
            resource.begin_load().unwrap_err().code(),
            ResourceStateErrorCode::InvalidTransition
        );
        assert_eq!(resource.snapshot(), snapshot);
    }

    #[test]
    fn stale_non_active_operations_are_mismatches_and_failure_atomic() {
        let mut resource = resource();
        let stale = resource.begin_load().unwrap();
        resource.ready(&stale, 1).unwrap();
        let active = resource.begin_refresh().unwrap();
        let snapshot = resource.snapshot();

        let error = resource.ready(&stale, 2).unwrap_err();

        assert_eq!(error.code(), ResourceStateErrorCode::OperationMismatch);
        assert_eq!(error.resource_id(), resource.id());
        assert_eq!(error.expected_operation(), Some(&active));
        assert_eq!(error.actual_operation(), Some(&stale));
        assert_eq!(error.source(), None);
        assert_eq!(resource.snapshot(), snapshot);
        assert_observable(
            &resource,
            Observable {
                status: ResourceStatus::Refreshing,
                value: Some(1),
                error: None,
                freshness: Freshness::Fresh,
                stale_reason: None,
                generation: 3,
                active_operation: Some(&active),
            },
        );
    }

    #[test]
    fn failed_while_cancelling_is_invalid_and_failure_atomic() {
        let mut resource = resource();
        let operation = resource.begin_load().unwrap();
        resource.cancel(&operation).unwrap();
        let snapshot = resource.snapshot();

        let error = resource
            .failed(&operation, "timeout", FailureVisibility::ClearValue)
            .unwrap_err();

        assert_eq!(error.code(), ResourceStateErrorCode::InvalidTransition);
        assert_eq!(error.resource_id(), resource.id());
        assert_eq!(error.expected_operation(), None);
        assert_eq!(error.actual_operation(), None);
        assert_eq!(error.source(), None);
        assert_eq!(resource.snapshot(), snapshot);
        assert_observable(
            &resource,
            Observable {
                status: ResourceStatus::Cancelling,
                value: None,
                error: None,
                freshness: Freshness::Fresh,
                stale_reason: None,
                generation: 2,
                active_operation: Some(&operation),
            },
        );
    }

    #[test]
    fn later_begin_clears_last_cancelled_replay_classification() {
        let mut resource = resource();
        let cancelled = resource.begin_load().unwrap();
        resource.cancel(&cancelled).unwrap();
        resource.cancelled(&cancelled).unwrap();
        let active = resource.begin_load().unwrap();
        let snapshot = resource.snapshot();

        let error = resource.cancel(&cancelled).unwrap_err();

        assert_eq!(error.code(), ResourceStateErrorCode::OperationMismatch);
        assert_eq!(error.resource_id(), resource.id());
        assert_eq!(error.expected_operation(), Some(&active));
        assert_eq!(error.actual_operation(), Some(&cancelled));
        assert_eq!(error.source(), None);
        assert_eq!(resource.snapshot(), snapshot);
        assert_observable(
            &resource,
            Observable {
                status: ResourceStatus::Loading,
                value: None,
                error: None,
                freshness: Freshness::Fresh,
                stale_reason: None,
                generation: 4,
                active_operation: Some(&active),
            },
        );
    }

    #[test]
    fn transitions_preserve_and_clear_observable_fields_exactly() {
        let mut resource = resource();
        let load = resource.begin_load().unwrap();
        resource.ready(&load, 1).unwrap();
        resource.mark_stale("expired").unwrap();
        let refresh = resource.begin_refresh().unwrap();

        assert_eq!(resource.status(), ResourceStatus::Refreshing);
        assert_eq!(resource.value(), Some(&1));
        assert_eq!(resource.error(), None);
        assert_eq!(resource.freshness(), Freshness::Stale);
        assert_eq!(resource.stale_reason(), Some("expired"));

        resource
            .failed(&refresh, "timeout", FailureVisibility::KeepStaleValue)
            .unwrap();
        assert_eq!(resource.status(), ResourceStatus::Failed);
        assert_eq!(resource.value(), Some(&1));
        assert_eq!(resource.error(), Some(&"timeout"));
        assert_eq!(resource.freshness(), Freshness::Stale);
        assert_eq!(resource.stale_reason(), Some("expired"));
        assert_eq!(resource.active_operation(), None);

        let load = resource.begin_load().unwrap();
        assert_eq!(resource.status(), ResourceStatus::Loading);
        assert_eq!(resource.value(), Some(&1));
        assert_eq!(resource.error(), None);
        assert_eq!(resource.freshness(), Freshness::Stale);
        assert_eq!(resource.stale_reason(), Some("expired"));

        resource
            .failed(&load, "gone", FailureVisibility::ClearValue)
            .unwrap();
        assert_eq!(resource.status(), ResourceStatus::Failed);
        assert_eq!(resource.value(), None);
        assert_eq!(resource.error(), Some(&"gone"));
        assert_eq!(resource.freshness(), Freshness::Fresh);
        assert_eq!(resource.stale_reason(), None);
    }

    #[test]
    fn stale_with_the_same_reason_is_idempotent() {
        let mut resource = resource();
        let load = resource.begin_load().unwrap();
        resource.ready(&load, 1).unwrap();
        resource.mark_stale("expired").unwrap();
        let generation = resource.generation();
        let snapshot = resource.snapshot();

        resource.mark_stale("expired").unwrap();

        assert_eq!(resource.generation(), generation);
        assert_eq!(resource.snapshot(), snapshot);
    }

    #[test]
    fn stale_with_a_changed_reason_advances_generation_and_replaces_reason() {
        let mut resource = resource();
        let load = resource.begin_load().unwrap();
        resource.ready(&load, 1).unwrap();
        resource.mark_stale("expired").unwrap();

        resource.mark_stale("source changed").unwrap();

        assert_observable(
            &resource,
            Observable {
                status: ResourceStatus::Stale,
                value: Some(1),
                error: None,
                freshness: Freshness::Stale,
                stale_reason: Some("source changed"),
                generation: 4,
                active_operation: None,
            },
        );
    }

    #[test]
    fn no_value_cancellation_and_failure_visibility_have_exact_field_matrices() {
        let mut cancellation = resource();
        let operation = cancellation.begin_load().unwrap();
        cancellation.cancel(&operation).unwrap();
        assert_observable(
            &cancellation,
            Observable {
                status: ResourceStatus::Cancelling,
                value: None,
                error: None,
                freshness: Freshness::Fresh,
                stale_reason: None,
                generation: 2,
                active_operation: Some(&operation),
            },
        );
        cancellation.cancelled(&operation).unwrap();
        assert_observable(
            &cancellation,
            Observable {
                status: ResourceStatus::Cancelled,
                value: None,
                error: None,
                freshness: Freshness::Fresh,
                stale_reason: None,
                generation: 3,
                active_operation: None,
            },
        );

        for (visibility, error) in [
            (FailureVisibility::KeepStaleValue, "keep"),
            (FailureVisibility::ClearValue, "clear"),
        ] {
            let mut failure = resource();
            let operation = failure.begin_load().unwrap();

            failure.failed(&operation, error, visibility).unwrap();

            assert_observable(
                &failure,
                Observable {
                    status: ResourceStatus::Failed,
                    value: None,
                    error: Some(error),
                    freshness: Freshness::Fresh,
                    stale_reason: None,
                    generation: 2,
                    active_operation: None,
                },
            );
        }

        for (visibility, value, freshness, stale_reason, error) in [
            (
                FailureVisibility::KeepStaleValue,
                Some(1),
                Freshness::Stale,
                Some("expired"),
                "retain",
            ),
            (
                FailureVisibility::ClearValue,
                None,
                Freshness::Fresh,
                None,
                "discard",
            ),
        ] {
            let mut failure = resource();
            let load = failure.begin_load().unwrap();
            failure.ready(&load, 1).unwrap();
            failure.mark_stale("expired").unwrap();
            let refresh = failure.begin_refresh().unwrap();

            failure.failed(&refresh, error, visibility).unwrap();

            assert_observable(
                &failure,
                Observable {
                    status: ResourceStatus::Failed,
                    value,
                    error: Some(error),
                    freshness,
                    stale_reason,
                    generation: 5,
                    active_operation: None,
                },
            );
        }
    }

    #[test]
    fn resource_generation_overflow_is_failure_atomic() {
        let mut resource = resource();
        resource.generation = ResourceGeneration::from_u64(u64::MAX);
        let snapshot = resource.snapshot();
        let next_operation_id = resource.next_operation_id;

        let error = resource.begin_load().unwrap_err();

        assert_eq!(error.code(), ResourceStateErrorCode::VersionOverflow);
        assert_eq!(error.source(), Some(&VersionError::Overflow));
        assert_eq!(resource.snapshot(), snapshot);
        assert_eq!(resource.next_operation_id, next_operation_id);
    }

    #[test]
    fn resource_operation_id_overflow_is_failure_atomic() {
        let mut resource = resource();
        resource.next_operation_id = u64::MAX;
        let snapshot = resource.snapshot();

        let error = resource.begin_load().unwrap_err();

        assert_eq!(error.code(), ResourceStateErrorCode::VersionOverflow);
        assert_eq!(error.source(), Some(&VersionError::Overflow));
        assert_eq!(resource.snapshot(), snapshot);
        assert_eq!(resource.next_operation_id, u64::MAX);
    }
}
