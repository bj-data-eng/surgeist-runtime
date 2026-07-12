use std::{borrow::Cow, error::Error, fmt};

use super::{CorrelationId, ServiceId, SurfaceRef, TaskIntentAttemptId, TaskIntentId};

static INPUT_SOURCE_UI: InputSourceId = InputSourceId::from_static("ui");
static INPUT_SOURCE_ADAPTER: InputSourceId = InputSourceId::from_static("adapter");
static INPUT_SOURCE_TASK: InputSourceId = InputSourceId::from_static("task");
static INPUT_SOURCE_SERVICE: InputSourceId = InputSourceId::from_static("service");
static INPUT_SOURCE_WINDOW: InputSourceId = InputSourceId::from_static("window");
static INPUT_SOURCE_SYSTEM: InputSourceId = InputSourceId::from_static("system");

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// A public runtime value with a private representation.
pub struct InputSourceId(Cow<'static, str>);

impl InputSourceId {
    /// A predefined runtime contract identifier.
    pub const UI: Self = Self::from_static("ui");
    /// A predefined runtime contract identifier.
    pub const ADAPTER: Self = Self::from_static("adapter");
    /// A predefined runtime contract identifier.
    pub const TASK: Self = Self::from_static("task");
    /// A predefined runtime contract identifier.
    pub const SERVICE: Self = Self::from_static("service");
    /// A predefined runtime contract identifier.
    pub const WINDOW: Self = Self::from_static("window");
    /// A predefined runtime contract identifier.
    pub const SYSTEM: Self = Self::from_static("system");

    #[must_use]
    /// Constructs this runtime value.
    pub fn new(value: impl Into<Cow<'static, str>>) -> Self {
        Self(value.into())
    }

    #[must_use]
    /// Performs the associated runtime operation.
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
    /// One case of this public runtime contract.
    Absent,
    /// One case of this public runtime contract.
    Present(CorrelationId),
}

impl Correlation {
    #[must_use]
    /// Performs the associated runtime operation.
    pub const fn is_absent(self) -> bool {
        matches!(self, Self::Absent)
    }

    #[must_use]
    /// Performs the associated runtime operation.
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
    /// One case of this public runtime contract.
    System,
    /// One case of this public runtime contract.
    Ui(SurfaceProvenance),
    /// One case of this public runtime contract.
    Adapter(SurfaceProvenance),
    /// One case of this public runtime contract.
    Task(TaskProvenance),
    /// One case of this public runtime contract.
    Service(ServiceProvenance),
    /// One case of this public runtime contract.
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
/// A public runtime value with a private representation.
pub struct ServiceProvenance {
    service_id: ServiceId,
}

impl InputProvenance {
    #[must_use]
    /// Constructs this runtime value.
    pub fn system() -> Self {
        Self::from_origin(InputOrigin::System)
    }

    #[must_use]
    /// Constructs this runtime value.
    pub fn ui(surface: SurfaceRef) -> Self {
        Self::from_origin(InputOrigin::Ui(SurfaceProvenance { surface }))
    }

    #[must_use]
    /// Constructs this runtime value.
    pub fn adapter(surface: SurfaceRef) -> Self {
        Self::from_origin(InputOrigin::Adapter(SurfaceProvenance { surface }))
    }

    #[must_use]
    /// Constructs this runtime value.
    pub fn task(task_id: TaskIntentId, attempt_id: TaskIntentAttemptId) -> Self {
        Self::from_origin(InputOrigin::Task(TaskProvenance {
            task_id,
            task_attempt_id: attempt_id,
            surface: None,
        }))
    }

    #[must_use]
    /// Constructs this runtime value.
    pub fn service(service_id: ServiceId) -> Self {
        Self::from_origin(InputOrigin::Service(ServiceProvenance { service_id }))
    }

    #[must_use]
    /// Constructs this runtime value.
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
    /// Performs the associated runtime operation.
    pub fn with_correlation(mut self, id: CorrelationId) -> Self {
        self.correlation = Correlation::Present(id);
        self
    }

    #[must_use]
    /// Performs the associated runtime operation.
    pub fn without_correlation(mut self) -> Self {
        self.correlation = Correlation::Absent;
        self
    }

    #[must_use]
    /// Performs the associated runtime operation.
    pub fn with_parent_correlation(mut self, id: CorrelationId) -> Self {
        self.parent_correlation = Correlation::Present(id);
        self
    }

    #[must_use]
    /// Performs the associated runtime operation.
    pub fn without_parent_correlation(mut self) -> Self {
        self.parent_correlation = Correlation::Absent;
        self
    }

    #[must_use]
    /// Performs the associated runtime operation.
    pub fn with_sequence(mut self, sequence: u64) -> Self {
        self.sequence = Some(sequence);
        self
    }

    #[must_use]
    /// Performs the associated runtime operation.
    pub fn without_sequence(mut self) -> Self {
        self.sequence = None;
        self
    }

    #[must_use]
    /// Performs the associated runtime operation.
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
    /// Performs the associated runtime operation.
    pub fn origin(&self) -> &InputOrigin {
        &self.origin
    }

    #[must_use]
    /// Performs the associated runtime operation.
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
    /// Performs the associated runtime operation.
    pub fn task_id(&self) -> Option<TaskIntentId> {
        match &self.origin {
            InputOrigin::Task(value) => Some(value.task_id),
            _ => None,
        }
    }

    #[must_use]
    /// Performs the associated runtime operation.
    pub fn task_attempt_id(&self) -> Option<TaskIntentAttemptId> {
        match &self.origin {
            InputOrigin::Task(value) => Some(value.task_attempt_id),
            _ => None,
        }
    }

    #[must_use]
    /// Performs the associated runtime operation.
    pub fn service_id(&self) -> Option<ServiceId> {
        match &self.origin {
            InputOrigin::Service(value) => Some(value.service_id.clone()),
            _ => None,
        }
    }

    #[must_use]
    /// Performs the associated runtime operation.
    pub const fn correlation(&self) -> Correlation {
        self.correlation
    }

    #[must_use]
    /// Performs the associated runtime operation.
    pub const fn parent_correlation(&self) -> Correlation {
        self.parent_correlation
    }

    #[must_use]
    /// Performs the associated runtime operation.
    pub const fn correlation_id(&self) -> Option<CorrelationId> {
        self.correlation.id()
    }

    #[must_use]
    /// Performs the associated runtime operation.
    pub const fn parent_correlation_id(&self) -> Option<CorrelationId> {
        self.parent_correlation.id()
    }

    #[must_use]
    /// Performs the associated runtime operation.
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
