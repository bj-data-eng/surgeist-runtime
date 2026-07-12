use std::{error::Error, fmt, num::NonZeroU64};

use super::{ResourceGeneration, ResourceId, ResourceOperationId, VersionError};
use crate::ids::CheckedNext;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResourceStatus {
    Idle,
    Loading,
    Ready,
    Refreshing,
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

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceOperation {
    resource_id: ResourceId,
    id: ResourceOperationId,
    generation: ResourceGeneration,
}

impl ResourceOperation {
    #[must_use]
    pub fn resource_id(&self) -> &ResourceId {
        &self.resource_id
    }

    #[must_use]
    pub const fn id(&self) -> ResourceOperationId {
        self.id
    }

    #[must_use]
    pub const fn generation(&self) -> ResourceGeneration {
        self.generation
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum ResourceStateErrorCode {
    InvalidTransition,
    OperationMismatch,
    OperationOverlap,
    CancellationAlreadyRequested,
    AlreadyCancelled,
    VersionOverflow,
}

#[derive(Clone, Debug, Eq, PartialEq)]
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

    #[must_use]
    pub const fn code(&self) -> ResourceStateErrorCode {
        self.code
    }

    #[must_use]
    pub fn resource_id(&self) -> &ResourceId {
        &self.resource_id
    }

    #[must_use]
    pub fn expected_operation(&self) -> Option<&ResourceOperation> {
        self.expected_operation.as_ref()
    }

    #[must_use]
    pub fn actual_operation(&self) -> Option<&ResourceOperation> {
        self.actual_operation.as_ref()
    }

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

    pub fn begin_load(&mut self) -> Result<ResourceOperation, ResourceStateError> {
        self.begin(ResourceStatus::Loading)
    }

    pub fn begin_refresh(&mut self) -> Result<ResourceOperation, ResourceStateError> {
        self.begin(ResourceStatus::Refreshing)
    }

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
    pub const fn is_renderable(&self) -> bool {
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
    pub const fn generation(&self) -> ResourceGeneration {
        self.generation
    }

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
    pub const fn is_renderable(&self) -> bool {
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
    pub const fn generation(&self) -> ResourceGeneration {
        self.generation
    }

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
