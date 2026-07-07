use std::borrow::Cow;

use super::{CorrelationId, ServiceId, SurfaceId, TaskIntentAttemptId, TaskIntentId};

static INPUT_SOURCE_UI: InputSourceId = InputSourceId::from_static("ui");
static INPUT_SOURCE_RETAINED: InputSourceId = InputSourceId::from_static("retained");
static INPUT_SOURCE_TASK: InputSourceId = InputSourceId::from_static("task");
static INPUT_SOURCE_SERVICE: InputSourceId = InputSourceId::from_static("service");
static INPUT_SOURCE_WINDOW: InputSourceId = InputSourceId::from_static("window");
static INPUT_SOURCE_SYSTEM: InputSourceId = InputSourceId::from_static("system");

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InputSourceId(Cow<'static, str>);

impl InputSourceId {
    pub const UI: Self = Self::from_static("ui");
    pub const RETAINED: Self = Self::from_static("retained");
    pub const TASK: Self = Self::from_static("task");
    pub const SERVICE: Self = Self::from_static("service");
    pub const WINDOW: Self = Self::from_static("window");
    pub const SYSTEM: Self = Self::from_static("system");

    #[must_use]
    pub fn new(value: impl Into<Cow<'static, str>>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    const fn from_static(value: &'static str) -> Self {
        Self(Cow::Borrowed(value))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputProvenance {
    origin: InputOrigin,
    correlation_id: CorrelationId,
    parent_correlation_id: Option<CorrelationId>,
    sequence: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InputOrigin {
    System,
    Ui(SurfaceProvenance),
    Retained(SurfaceProvenance),
    Task(TaskProvenance),
    Service(ServiceProvenance),
    Window(SurfaceProvenance),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurfaceProvenance {
    surface_id: SurfaceId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskProvenance {
    task_id: TaskIntentId,
    task_attempt_id: TaskIntentAttemptId,
    surface_id: Option<SurfaceId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceProvenance {
    service_id: ServiceId,
}

impl InputProvenance {
    #[must_use]
    pub fn system() -> Self {
        Self::from_origin(InputOrigin::System)
    }

    #[must_use]
    pub fn ui(surface_id: SurfaceId) -> Self {
        Self::from_origin(InputOrigin::Ui(SurfaceProvenance { surface_id }))
    }

    #[must_use]
    pub fn retained(surface_id: SurfaceId) -> Self {
        Self::from_origin(InputOrigin::Retained(SurfaceProvenance { surface_id }))
    }

    #[must_use]
    pub fn task(task_id: TaskIntentId, attempt_id: TaskIntentAttemptId) -> Self {
        Self::from_origin(InputOrigin::Task(TaskProvenance {
            task_id,
            task_attempt_id: attempt_id,
            surface_id: None,
        }))
    }

    #[must_use]
    pub fn service(service_id: ServiceId) -> Self {
        Self::from_origin(InputOrigin::Service(ServiceProvenance { service_id }))
    }

    #[must_use]
    pub fn window(surface_id: SurfaceId) -> Self {
        Self::from_origin(InputOrigin::Window(SurfaceProvenance { surface_id }))
    }

    #[must_use]
    pub fn with_surface(mut self, id: SurfaceId) -> Self {
        if let InputOrigin::Task(task) = &mut self.origin {
            task.surface_id = Some(id);
        }
        self
    }

    #[must_use]
    pub fn with_correlation(mut self, id: CorrelationId) -> Self {
        self.correlation_id = id;
        self
    }

    #[must_use]
    pub fn with_parent(mut self, id: CorrelationId) -> Self {
        self.parent_correlation_id = Some(id);
        self
    }

    #[must_use]
    pub fn with_sequence(mut self, sequence: u64) -> Self {
        self.sequence = Some(sequence);
        self
    }

    #[must_use]
    pub fn source(&self) -> &InputSourceId {
        match &self.origin {
            InputOrigin::System => &INPUT_SOURCE_SYSTEM,
            InputOrigin::Ui(_) => &INPUT_SOURCE_UI,
            InputOrigin::Retained(_) => &INPUT_SOURCE_RETAINED,
            InputOrigin::Task(_) => &INPUT_SOURCE_TASK,
            InputOrigin::Service(_) => &INPUT_SOURCE_SERVICE,
            InputOrigin::Window(_) => &INPUT_SOURCE_WINDOW,
        }
    }

    #[must_use]
    pub fn origin(&self) -> &InputOrigin {
        &self.origin
    }

    #[must_use]
    pub fn surface_id(&self) -> Option<SurfaceId> {
        match &self.origin {
            InputOrigin::Ui(value) | InputOrigin::Retained(value) | InputOrigin::Window(value) => {
                Some(value.surface_id)
            }
            InputOrigin::Task(value) => value.surface_id,
            InputOrigin::System | InputOrigin::Service(_) => None,
        }
    }

    #[must_use]
    pub fn task_id(&self) -> Option<TaskIntentId> {
        match &self.origin {
            InputOrigin::Task(value) => Some(value.task_id),
            _ => None,
        }
    }

    #[must_use]
    pub fn task_attempt_id(&self) -> Option<TaskIntentAttemptId> {
        match &self.origin {
            InputOrigin::Task(value) => Some(value.task_attempt_id),
            _ => None,
        }
    }

    #[must_use]
    pub fn service_id(&self) -> Option<ServiceId> {
        match &self.origin {
            InputOrigin::Service(value) => Some(value.service_id.clone()),
            _ => None,
        }
    }

    #[must_use]
    pub const fn correlation_id(&self) -> CorrelationId {
        self.correlation_id
    }

    #[must_use]
    pub const fn parent_correlation_id(&self) -> Option<CorrelationId> {
        self.parent_correlation_id
    }

    #[must_use]
    pub const fn sequence(&self) -> Option<u64> {
        self.sequence
    }

    fn from_origin(origin: InputOrigin) -> Self {
        Self {
            origin,
            correlation_id: CorrelationId::from_u64(0),
            parent_correlation_id: None,
            sequence: None,
        }
    }
}
