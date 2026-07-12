use std::{borrow::Cow, error::Error, fmt};

use super::{CorrelationId, ServiceId, SurfaceRef, TaskIntentAttemptId, TaskIntentId};

static INPUT_SOURCE_UI: InputSourceId = InputSourceId::from_static("ui");
static INPUT_SOURCE_ADAPTER: InputSourceId = InputSourceId::from_static("adapter");
static INPUT_SOURCE_TASK: InputSourceId = InputSourceId::from_static("task");
static INPUT_SOURCE_SERVICE: InputSourceId = InputSourceId::from_static("service");
static INPUT_SOURCE_WINDOW: InputSourceId = InputSourceId::from_static("window");
static INPUT_SOURCE_SYSTEM: InputSourceId = InputSourceId::from_static("system");

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// A stable identifier for the source category recorded in input provenance.
pub struct InputSourceId(Cow<'static, str>);

impl InputSourceId {
    /// Identifies input produced by a user-interface surface.
    pub const UI: Self = Self::from_static("ui");
    /// Identifies input supplied by a root-owned adapter.
    pub const ADAPTER: Self = Self::from_static("adapter");
    /// Identifies input reported by an abstract task attempt.
    pub const TASK: Self = Self::from_static("task");
    /// Identifies input reported by a service.
    pub const SERVICE: Self = Self::from_static("service");
    /// Identifies input originating from window integration.
    pub const WINDOW: Self = Self::from_static("window");
    /// Identifies runtime-generated system input.
    pub const SYSTEM: Self = Self::from_static("system");

    #[must_use]
    /// Stores a custom source category for adapter-defined provenance.
    pub fn new(value: impl Into<Cow<'static, str>>) -> Self {
        Self(value.into())
    }

    #[must_use]
    /// Borrows the stable source-category text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    const fn from_static(value: &'static str) -> Self {
        Self(Cow::Borrowed(value))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Describes an input's source and explicitly recorded causal context.
///
/// Current and parent correlations are independent: constructors leave both
/// absent, and changing one never invents or alters the other.
pub struct InputProvenance {
    origin: InputOrigin,
    correlation: Correlation,
    parent_correlation: Correlation,
    sequence: Option<u64>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
/// The explicit presence or absence of one causal correlation.
///
/// [`Self::Absent`] is the default and carries no synthetic correlation ID.
pub enum Correlation {
    #[default]
    /// Records that no correlation identity is attached; this is the default.
    Absent,
    /// Records a validated nonzero correlation identity.
    Present(CorrelationId),
}

impl Correlation {
    #[must_use]
    /// Returns whether this field records no correlation identity.
    pub const fn is_absent(self) -> bool {
        matches!(self, Self::Absent)
    }

    #[must_use]
    /// Returns the correlation identity by value when this field is present.
    pub const fn id(self) -> Option<CorrelationId> {
        match self {
            Self::Absent => None,
            Self::Present(id) => Some(id),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// The semantic source category and source-specific identifying data for input.
pub enum InputOrigin {
    /// Input created by runtime itself, with no source-specific identity.
    System,
    /// Input from a generation-qualified user-interface surface.
    Ui(SurfaceProvenance),
    /// Input from a generation-qualified root-owned adapter surface.
    Adapter(SurfaceProvenance),
    /// Input reported by an abstract task intent attempt.
    Task(TaskProvenance),
    /// Input reported by a registered service.
    Service(ServiceProvenance),
    /// Input from a generation-qualified window-integrated surface.
    Window(SurfaceProvenance),
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Surface-origin data bound to a generation-qualified [`SurfaceRef`].
pub struct SurfaceProvenance {
    surface: SurfaceRef,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Task-origin data, with an optional generation-qualified surface attachment.
pub struct TaskProvenance {
    task_id: TaskIntentId,
    task_attempt_id: TaskIntentAttemptId,
    surface: Option<SurfaceRef>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Service-origin data identifying the reporting service.
pub struct ServiceProvenance {
    service_id: ServiceId,
}

impl InputProvenance {
    #[must_use]
    /// Creates system provenance with no correlations or sequence number.
    pub fn system() -> Self {
        Self::from_origin(InputOrigin::System)
    }

    #[must_use]
    /// Creates UI provenance for the supplied generation-qualified surface.
    pub fn ui(surface: SurfaceRef) -> Self {
        Self::from_origin(InputOrigin::Ui(SurfaceProvenance { surface }))
    }

    #[must_use]
    /// Creates adapter provenance for the supplied generation-qualified surface.
    pub fn adapter(surface: SurfaceRef) -> Self {
        Self::from_origin(InputOrigin::Adapter(SurfaceProvenance { surface }))
    }

    #[must_use]
    /// Creates task provenance for one exact intent attempt with no surface attachment.
    pub fn task(task_id: TaskIntentId, attempt_id: TaskIntentAttemptId) -> Self {
        Self::from_origin(InputOrigin::Task(TaskProvenance {
            task_id,
            task_attempt_id: attempt_id,
            surface: None,
        }))
    }

    #[must_use]
    /// Creates service provenance for the supplied service identity.
    pub fn service(service_id: ServiceId) -> Self {
        Self::from_origin(InputOrigin::Service(ServiceProvenance { service_id }))
    }

    #[must_use]
    /// Creates window provenance for the supplied generation-qualified surface.
    pub fn window(surface: SurfaceRef) -> Self {
        Self::from_origin(InputOrigin::Window(SurfaceProvenance { surface }))
    }

    /// Attaches a generation-qualified surface when this origin permits it.
    ///
    /// Task origins accept one surface; repeating the same attachment is
    /// idempotent, while a different surface is rejected. UI, adapter, and
    /// window origins likewise accept only their existing surface. System and
    /// service origins do not support surface attachment. On rejection, the
    /// returned [`ProvenanceError`] preserves the origin plus existing and
    /// attempted surface values; this value is unchanged because it is consumed.
    pub fn try_with_surface(mut self, surface: SurfaceRef) -> Result<Self, ProvenanceError> {
        let origin = self.origin.clone();
        match &mut self.origin {
            InputOrigin::Task(task) => match task.surface {
                Some(existing) if existing == surface => Ok(self),
                Some(existing) => Err(ProvenanceError::new(
                    ProvenanceErrorCode::SurfaceAlreadyAttached,
                    origin,
                    Some(existing),
                    surface,
                )),
                None => {
                    task.surface = Some(surface);
                    Ok(self)
                }
            },
            InputOrigin::Ui(existing)
            | InputOrigin::Adapter(existing)
            | InputOrigin::Window(existing) => {
                if existing.surface == surface {
                    Ok(self)
                } else {
                    Err(ProvenanceError::new(
                        ProvenanceErrorCode::SurfaceOverwriteUnsupported,
                        origin,
                        Some(existing.surface),
                        surface,
                    ))
                }
            }
            InputOrigin::System | InputOrigin::Service(_) => Err(ProvenanceError::new(
                ProvenanceErrorCode::SurfaceUnsupportedOrigin,
                origin,
                None,
                surface,
            )),
        }
    }

    #[must_use]
    /// Consumes this provenance and replaces its current correlation identity.
    pub fn with_correlation(mut self, id: CorrelationId) -> Self {
        self.correlation = Correlation::Present(id);
        self
    }

    #[must_use]
    /// Consumes this provenance and clears only its current correlation identity.
    pub fn without_correlation(mut self) -> Self {
        self.correlation = Correlation::Absent;
        self
    }

    #[must_use]
    /// Consumes this provenance and replaces its parent correlation identity.
    pub fn with_parent_correlation(mut self, id: CorrelationId) -> Self {
        self.parent_correlation = Correlation::Present(id);
        self
    }

    #[must_use]
    /// Consumes this provenance and clears only its parent correlation identity.
    pub fn without_parent_correlation(mut self) -> Self {
        self.parent_correlation = Correlation::Absent;
        self
    }

    #[must_use]
    /// Consumes this provenance and records a sequence number.
    pub fn with_sequence(mut self, sequence: u64) -> Self {
        self.sequence = Some(sequence);
        self
    }

    #[must_use]
    /// Consumes this provenance and clears its optional sequence number.
    pub fn without_sequence(mut self) -> Self {
        self.sequence = None;
        self
    }

    #[must_use]
    /// Borrows the stable source category derived from this origin.
    pub fn source(&self) -> &InputSourceId {
        match &self.origin {
            InputOrigin::System => &INPUT_SOURCE_SYSTEM,
            InputOrigin::Ui(_) => &INPUT_SOURCE_UI,
            InputOrigin::Adapter(_) => &INPUT_SOURCE_ADAPTER,
            InputOrigin::Task(_) => &INPUT_SOURCE_TASK,
            InputOrigin::Service(_) => &INPUT_SOURCE_SERVICE,
            InputOrigin::Window(_) => &INPUT_SOURCE_WINDOW,
        }
    }

    #[must_use]
    /// Borrows the full source category and source-specific identifying data.
    pub fn origin(&self) -> &InputOrigin {
        &self.origin
    }

    #[must_use]
    /// Returns the attached generation-qualified surface, when this origin carries one.
    pub fn surface(&self) -> Option<SurfaceRef> {
        match &self.origin {
            InputOrigin::Ui(value) | InputOrigin::Adapter(value) | InputOrigin::Window(value) => {
                Some(value.surface)
            }
            InputOrigin::Task(value) => value.surface,
            InputOrigin::System | InputOrigin::Service(_) => None,
        }
    }

    #[must_use]
    /// Returns the task intent identity only for task provenance.
    pub fn task_id(&self) -> Option<TaskIntentId> {
        match &self.origin {
            InputOrigin::Task(value) => Some(value.task_id),
            _ => None,
        }
    }

    #[must_use]
    /// Returns the task attempt identity only for task provenance.
    pub fn task_attempt_id(&self) -> Option<TaskIntentAttemptId> {
        match &self.origin {
            InputOrigin::Task(value) => Some(value.task_attempt_id),
            _ => None,
        }
    }

    #[must_use]
    /// Returns a cloned service identity only for service provenance.
    pub fn service_id(&self) -> Option<ServiceId> {
        match &self.origin {
            InputOrigin::Service(value) => Some(value.service_id.clone()),
            _ => None,
        }
    }

    #[must_use]
    /// Returns the current correlation field by value.
    pub const fn correlation(&self) -> Correlation {
        self.correlation
    }

    #[must_use]
    /// Returns the parent correlation field by value.
    pub const fn parent_correlation(&self) -> Correlation {
        self.parent_correlation
    }

    #[must_use]
    /// Returns the current correlation identity, when present.
    pub const fn correlation_id(&self) -> Option<CorrelationId> {
        self.correlation.id()
    }

    #[must_use]
    /// Returns the parent correlation identity, when present.
    pub const fn parent_correlation_id(&self) -> Option<CorrelationId> {
        self.parent_correlation.id()
    }

    #[must_use]
    /// Returns the optional sequence number by value.
    pub const fn sequence(&self) -> Option<u64> {
        self.sequence
    }

    fn from_origin(origin: InputOrigin) -> Self {
        Self {
            origin,
            correlation: Correlation::Absent,
            parent_correlation: Correlation::Absent,
            sequence: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
/// Classifies a rejected provenance surface attachment.
pub enum ProvenanceErrorCode {
    /// A task origin already has a different surface attached.
    SurfaceAlreadyAttached,
    /// A UI, adapter, or window origin cannot be attached to another surface.
    SurfaceOverwriteUnsupported,
    /// A system or service origin cannot carry a surface attachment.
    SurfaceUnsupportedOrigin,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Typed details for a rejected surface attachment.
///
/// The error retains the source origin, any existing generation-qualified
/// surface, and the attempted surface so callers can handle rejection without
/// parsing display text.
pub struct ProvenanceError {
    code: ProvenanceErrorCode,
    origin: InputOrigin,
    existing_surface: Option<SurfaceRef>,
    attempted_surface: SurfaceRef,
}

impl ProvenanceError {
    fn new(
        code: ProvenanceErrorCode,
        origin: InputOrigin,
        existing_surface: Option<SurfaceRef>,
        attempted_surface: SurfaceRef,
    ) -> Self {
        Self {
            code,
            origin,
            existing_surface,
            attempted_surface,
        }
    }

    #[must_use]
    /// Returns the typed reason the attachment was rejected.
    pub const fn code(&self) -> ProvenanceErrorCode {
        self.code
    }

    #[must_use]
    /// Returns the origin whose surface attachment was rejected.
    pub const fn origin(&self) -> &InputOrigin {
        &self.origin
    }

    #[must_use]
    /// Returns the existing generation-qualified surface, when one was present.
    pub const fn existing_surface(&self) -> Option<SurfaceRef> {
        self.existing_surface
    }

    #[must_use]
    /// Returns the generation-qualified surface that was requested.
    pub const fn attempted_surface(&self) -> SurfaceRef {
        self.attempted_surface
    }
}

impl fmt::Display for ProvenanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "surface attachment {:?} rejected for {:?}: existing {:?}, attempted {:?}",
            self.code, self.origin, self.existing_surface, self.attempted_surface
        )
    }
}

impl Error for ProvenanceError {}
