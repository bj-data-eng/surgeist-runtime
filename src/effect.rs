use std::borrow::Cow;

use super::{
    AppScope, CorrelationId, Diagnostic, InputProvenance, ResourceId, ResourceOperation,
    ServiceCommandName, ServiceCommandPayload, ServiceId, SurfaceRef, TaskIntentHandle,
    TaskIntentKey, TaskIntentName, TaskPriorityHint, WindowId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RedrawTarget {
    All,
    Surface(SurfaceRef),
    Window(WindowId),
}

impl RedrawTarget {
    #[must_use]
    pub const fn all() -> Self {
        Self::All
    }

    #[must_use]
    pub const fn surface(surface: SurfaceRef) -> Self {
        Self::Surface(surface)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// Identifies one effect kind with a defined runtime handling path.
///
/// The associated constants are the complete supported set. Effects themselves
/// obtain their kind through [`AppEffect::kind`].
pub struct EffectKindId(Cow<'static, str>);

static REQUEST_REDRAW_EFFECT_KIND: EffectKindId = EffectKindId::REQUEST_REDRAW;
static PERSIST_EFFECT_KIND: EffectKindId = EffectKindId::PERSIST;
static EMIT_DIAGNOSTIC_EFFECT_KIND: EffectKindId = EffectKindId::EMIT_DIAGNOSTIC;
static LOAD_RESOURCE_EFFECT_KIND: EffectKindId = EffectKindId::LOAD_RESOURCE;
static INVALIDATE_RESOURCE_EFFECT_KIND: EffectKindId = EffectKindId::INVALIDATE_RESOURCE;
static START_TASK_EFFECT_KIND: EffectKindId = EffectKindId::START_TASK;
static CANCEL_TASK_EFFECT_KIND: EffectKindId = EffectKindId::CANCEL_TASK;
static REPRIORITIZE_TASK_EFFECT_KIND: EffectKindId = EffectKindId::REPRIORITIZE_TASK;
static START_SERVICE_EFFECT_KIND: EffectKindId = EffectKindId::START_SERVICE;
static STOP_SERVICE_EFFECT_KIND: EffectKindId = EffectKindId::STOP_SERVICE;
static CALL_SERVICE_EFFECT_KIND: EffectKindId = EffectKindId::CALL_SERVICE;
static SERVICE_DIAGNOSTIC_EFFECT_KIND: EffectKindId = EffectKindId::SERVICE_DIAGNOSTIC;

impl EffectKindId {
    /// Identifies a request to redraw one or more surfaces.
    pub const REQUEST_REDRAW: Self = Self::from_static("runtime.request_redraw");
    /// Identifies an adapter-owned persistence request.
    pub const PERSIST: Self = Self::from_static("runtime.persist");
    /// Identifies an app diagnostic to apply locally.
    pub const EMIT_DIAGNOSTIC: Self = Self::from_static("runtime.emit_diagnostic");
    /// Identifies an adapter-owned resource load request.
    pub const LOAD_RESOURCE: Self = Self::from_static("runtime.load_resource");
    /// Identifies an adapter-owned resource invalidation request.
    pub const INVALIDATE_RESOURCE: Self = Self::from_static("runtime.invalidate_resource");
    /// Identifies an adapter-owned task start request.
    pub const START_TASK: Self = Self::from_static("runtime.start_task");
    /// Identifies an adapter-owned task cancellation request.
    pub const CANCEL_TASK: Self = Self::from_static("runtime.cancel_task");
    /// Identifies an adapter-owned task reprioritization request.
    pub const REPRIORITIZE_TASK: Self = Self::from_static("runtime.reprioritize_task");
    /// Identifies an adapter-owned service start request.
    pub const START_SERVICE: Self = Self::from_static("runtime.start_service");
    /// Identifies an adapter-owned service stop request.
    pub const STOP_SERVICE: Self = Self::from_static("runtime.stop_service");
    /// Identifies an adapter-owned service call request.
    pub const CALL_SERVICE: Self = Self::from_static("runtime.call_service");
    /// Identifies a service-scoped diagnostic to apply locally.
    pub const SERVICE_DIAGNOSTIC: Self = Self::from_static("runtime.service_diagnostic");

    /// Returns the stable runtime kind name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    const fn from_static(value: &'static str) -> Self {
        Self(Cow::Borrowed(value))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// An app-authored effect with a closed runtime payload.
pub struct AppEffect {
    payload: AppEffectPayload,
}

impl AppEffect {
    /// Requests redraw for the selected target.
    #[must_use]
    pub fn request_redraw(target: RedrawTarget) -> Self {
        Self {
            payload: AppEffectPayload::RequestRedraw(RequestRedrawEffect { target }),
        }
    }

    /// Requests adapter-owned persistence for `key` in `scope`.
    #[must_use]
    pub fn persist(key: impl Into<String>, scope: AppScope) -> Self {
        Self {
            payload: AppEffectPayload::Persist(PersistEffect {
                key: key.into(),
                scope,
            }),
        }
    }

    /// Applies an app diagnostic during runtime effect processing.
    #[must_use]
    pub fn diagnostic(diagnostic: Diagnostic) -> Self {
        Self {
            payload: AppEffectPayload::Diagnostic(DiagnosticEffect {
                diagnostic: Box::new(diagnostic),
            }),
        }
    }

    /// Forwards the complete resource operation token for loading in `scope`.
    ///
    /// The token is issued by [`ResourceState`](super::ResourceState) and is
    /// retained unchanged so an adapter can complete or cancel that exact
    /// operation, including its generation.
    #[must_use]
    pub fn load_resource(operation: ResourceOperation, scope: AppScope) -> Self {
        Self {
            payload: AppEffectPayload::LoadResource(LoadResourceEffect { operation, scope }),
        }
    }

    /// Forwards resource invalidation with its reason.
    #[must_use]
    pub fn invalidate_resource(id: ResourceId, reason: impl Into<String>) -> Self {
        Self {
            payload: AppEffectPayload::InvalidateResource(InvalidateResourceEffect {
                id,
                reason: reason.into(),
            }),
        }
    }

    /// Forwards a task-start request.
    #[must_use]
    pub fn start_task(name: TaskIntentName, key: TaskIntentKey, scope: AppScope) -> Self {
        Self {
            payload: AppEffectPayload::StartTask(StartTaskEffect { name, key, scope }),
        }
    }

    /// Forwards a task-cancellation request.
    #[must_use]
    pub fn cancel_task(handle: TaskIntentHandle) -> Self {
        Self {
            payload: AppEffectPayload::CancelTask(CancelTaskEffect { handle }),
        }
    }

    /// Forwards a task reprioritization request.
    #[must_use]
    pub fn reprioritize_task(handle: TaskIntentHandle, priority: TaskPriorityHint) -> Self {
        Self {
            payload: AppEffectPayload::ReprioritizeTask(ReprioritizeTaskEffect {
                handle,
                priority,
            }),
        }
    }

    /// Forwards a service-start request.
    #[must_use]
    pub fn start_service(id: ServiceId) -> Self {
        Self {
            payload: AppEffectPayload::StartService(StartServiceEffect { id }),
        }
    }

    /// Forwards a service-stop request.
    #[must_use]
    pub fn stop_service(id: ServiceId) -> Self {
        Self {
            payload: AppEffectPayload::StopService(StopServiceEffect { id }),
        }
    }

    /// Forwards a service command with its correlation identity.
    #[must_use]
    pub fn call_service(
        id: ServiceId,
        command: ServiceCommandName,
        payload: ServiceCommandPayload,
        correlation: CorrelationId,
    ) -> Self {
        Self {
            payload: AppEffectPayload::CallService(CallServiceEffect {
                id,
                command,
                payload,
                correlation,
            }),
        }
    }

    /// Applies a diagnostic associated with `id` during runtime processing.
    #[must_use]
    pub fn service_diagnostic(id: ServiceId, diagnostic: Diagnostic) -> Self {
        Self {
            payload: AppEffectPayload::ServiceDiagnostic(ServiceDiagnosticEffect {
                id,
                diagnostic: Box::new(diagnostic),
            }),
        }
    }

    /// Returns the supported runtime kind for this effect.
    #[must_use]
    pub fn kind(&self) -> &EffectKindId {
        match &self.payload {
            AppEffectPayload::RequestRedraw(_) => &REQUEST_REDRAW_EFFECT_KIND,
            AppEffectPayload::Persist(_) => &PERSIST_EFFECT_KIND,
            AppEffectPayload::Diagnostic(_) => &EMIT_DIAGNOSTIC_EFFECT_KIND,
            AppEffectPayload::LoadResource(_) => &LOAD_RESOURCE_EFFECT_KIND,
            AppEffectPayload::InvalidateResource(_) => &INVALIDATE_RESOURCE_EFFECT_KIND,
            AppEffectPayload::StartTask(_) => &START_TASK_EFFECT_KIND,
            AppEffectPayload::CancelTask(_) => &CANCEL_TASK_EFFECT_KIND,
            AppEffectPayload::ReprioritizeTask(_) => &REPRIORITIZE_TASK_EFFECT_KIND,
            AppEffectPayload::StartService(_) => &START_SERVICE_EFFECT_KIND,
            AppEffectPayload::StopService(_) => &STOP_SERVICE_EFFECT_KIND,
            AppEffectPayload::CallService(_) => &CALL_SERVICE_EFFECT_KIND,
            AppEffectPayload::ServiceDiagnostic(_) => &SERVICE_DIAGNOSTIC_EFFECT_KIND,
        }
    }

    /// Returns this effect's closed payload.
    #[must_use]
    pub const fn payload(&self) -> &AppEffectPayload {
        &self.payload
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// The closed payload carried by an [`AppEffect`].
pub enum AppEffectPayload {
    /// A redraw request.
    RequestRedraw(RequestRedrawEffect),
    /// A persistence request for an adapter.
    Persist(PersistEffect),
    /// An app diagnostic to apply locally.
    Diagnostic(DiagnosticEffect),
    /// A resource-operation request for an adapter.
    LoadResource(LoadResourceEffect),
    /// A resource invalidation request for an adapter.
    InvalidateResource(InvalidateResourceEffect),
    /// A task-start request for an adapter.
    StartTask(StartTaskEffect),
    /// A task-cancellation request for an adapter.
    CancelTask(CancelTaskEffect),
    /// A task-priority request for an adapter.
    ReprioritizeTask(ReprioritizeTaskEffect),
    /// A service-start request for an adapter.
    StartService(StartServiceEffect),
    /// A service-stop request for an adapter.
    StopService(StopServiceEffect),
    /// A service-call request for an adapter.
    CallService(CallServiceEffect),
    /// A service-scoped diagnostic to apply locally.
    ServiceDiagnostic(ServiceDiagnosticEffect),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestRedrawEffect {
    target: RedrawTarget,
}

impl RequestRedrawEffect {
    #[must_use]
    pub const fn target(&self) -> &RedrawTarget {
        &self.target
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistEffect {
    key: String,
    scope: AppScope,
}

impl PersistEffect {
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    #[must_use]
    pub const fn scope(&self) -> &AppScope {
        &self.scope
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticEffect {
    diagnostic: Box<Diagnostic>,
}

impl DiagnosticEffect {
    #[must_use]
    pub fn diagnostic(&self) -> &Diagnostic {
        self.diagnostic.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Carries one operation token issued by a resource state and its owning scope.
pub struct LoadResourceEffect {
    operation: ResourceOperation,
    scope: AppScope,
}

impl LoadResourceEffect {
    /// Returns the complete token that must be preserved by adapter handoff.
    #[must_use]
    pub const fn operation(&self) -> &ResourceOperation {
        &self.operation
    }

    /// Returns the resource ID carried by [`Self::operation`].
    #[must_use]
    pub fn id(&self) -> &ResourceId {
        self.operation.resource_id()
    }

    /// Returns the scope in which this resource work was requested.
    #[must_use]
    pub const fn scope(&self) -> &AppScope {
        &self.scope
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvalidateResourceEffect {
    id: ResourceId,
    reason: String,
}

impl InvalidateResourceEffect {
    #[must_use]
    pub fn id(&self) -> &ResourceId {
        &self.id
    }

    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartTaskEffect {
    name: TaskIntentName,
    key: TaskIntentKey,
    scope: AppScope,
}

impl StartTaskEffect {
    #[must_use]
    pub fn name(&self) -> &TaskIntentName {
        &self.name
    }

    #[must_use]
    pub fn key(&self) -> &TaskIntentKey {
        &self.key
    }

    #[must_use]
    pub const fn scope(&self) -> &AppScope {
        &self.scope
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CancelTaskEffect {
    handle: TaskIntentHandle,
}

impl CancelTaskEffect {
    #[must_use]
    pub const fn handle(&self) -> TaskIntentHandle {
        self.handle
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReprioritizeTaskEffect {
    handle: TaskIntentHandle,
    priority: TaskPriorityHint,
}

impl ReprioritizeTaskEffect {
    #[must_use]
    pub const fn handle(&self) -> TaskIntentHandle {
        self.handle
    }

    #[must_use]
    pub const fn priority(&self) -> TaskPriorityHint {
        self.priority
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartServiceEffect {
    id: ServiceId,
}

impl StartServiceEffect {
    #[must_use]
    pub const fn id(&self) -> &ServiceId {
        &self.id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StopServiceEffect {
    id: ServiceId,
}

impl StopServiceEffect {
    #[must_use]
    pub const fn id(&self) -> &ServiceId {
        &self.id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallServiceEffect {
    id: ServiceId,
    command: ServiceCommandName,
    payload: ServiceCommandPayload,
    correlation: CorrelationId,
}

impl CallServiceEffect {
    #[must_use]
    pub const fn id(&self) -> &ServiceId {
        &self.id
    }

    #[must_use]
    pub const fn command(&self) -> &ServiceCommandName {
        &self.command
    }

    #[must_use]
    pub const fn payload(&self) -> &ServiceCommandPayload {
        &self.payload
    }

    #[must_use]
    pub const fn correlation(&self) -> CorrelationId {
        self.correlation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceDiagnosticEffect {
    id: ServiceId,
    diagnostic: Box<Diagnostic>,
}

impl ServiceDiagnosticEffect {
    #[must_use]
    pub const fn id(&self) -> &ServiceId {
        &self.id
    }

    #[must_use]
    pub fn diagnostic(&self) -> &Diagnostic {
        self.diagnostic.as_ref()
    }
}

#[derive(Clone, Debug, Default)]
pub struct EffectBatch {
    effects: Vec<AppEffect>,
}

/// Records how runtime handled an effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectDisposition {
    /// Runtime applied the effect locally.
    Applied,
    /// Runtime preserved the effect as a typed adapter intent.
    Forwarded,
    /// Runtime rejected the effect and retained a diagnostic.
    Rejected,
}

/// Adapter-owned work forwarded unchanged by runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeIntent {
    /// Persists the exact effect payload.
    Persist(PersistEffect),
    /// Loads the exact resource operation token and scope.
    LoadResource(LoadResourceEffect),
    /// Invalidates the exact resource payload.
    InvalidateResource(InvalidateResourceEffect),
    /// Starts the exact task payload.
    StartTask(StartTaskEffect),
    /// Cancels the exact task payload.
    CancelTask(CancelTaskEffect),
    /// Reprioritizes the exact task payload.
    ReprioritizeTask(ReprioritizeTaskEffect),
    /// Starts the exact service payload.
    StartService(StartServiceEffect),
    /// Stops the exact service payload.
    StopService(StopServiceEffect),
    /// Calls the exact service payload.
    CallService(CallServiceEffect),
}

/// The invariant-preserving result of handling one app effect.
///
/// Runtime constructs outcomes through crate-private constructors, so public
/// callers can inspect but cannot create a contradictory disposition and payload
/// combination.
///
/// ```compile_fail
/// use surgeist_runtime::{EffectDisposition, EffectKindId, EffectOutcome, InputProvenance};
///
/// let _ = EffectOutcome {
///     kind: EffectKindId::REQUEST_REDRAW,
///     disposition: EffectDisposition::Applied,
///     provenance: InputProvenance::system(),
///     intent: None,
///     diagnostic: None,
/// };
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectOutcome {
    kind: EffectKindId,
    disposition: EffectDisposition,
    provenance: InputProvenance,
    intent: Option<RuntimeIntent>,
    diagnostic: Option<Diagnostic>,
}

impl EffectOutcome {
    pub(crate) fn applied(kind: EffectKindId, provenance: InputProvenance) -> Self {
        Self {
            kind,
            disposition: EffectDisposition::Applied,
            provenance,
            intent: None,
            diagnostic: None,
        }
    }

    pub(crate) fn forwarded(
        kind: EffectKindId,
        provenance: InputProvenance,
        intent: RuntimeIntent,
    ) -> Self {
        Self {
            kind,
            disposition: EffectDisposition::Forwarded,
            provenance,
            intent: Some(intent),
            diagnostic: None,
        }
    }

    pub(crate) fn rejected(
        kind: EffectKindId,
        provenance: InputProvenance,
        diagnostic: Diagnostic,
    ) -> Self {
        Self {
            kind,
            disposition: EffectDisposition::Rejected,
            provenance,
            intent: None,
            diagnostic: Some(diagnostic),
        }
    }

    /// Returns the kind of effect that produced this outcome.
    #[must_use]
    pub const fn kind(&self) -> &EffectKindId {
        &self.kind
    }

    /// Returns whether runtime applied, forwarded, or rejected the effect.
    #[must_use]
    pub const fn disposition(&self) -> EffectDisposition {
        self.disposition
    }

    /// Returns the effective provenance assigned to this handling result.
    #[must_use]
    pub const fn provenance(&self) -> &InputProvenance {
        &self.provenance
    }

    /// Returns the forwarded adapter intent only for [`EffectDisposition::Forwarded`].
    #[must_use]
    pub const fn intent(&self) -> Option<&RuntimeIntent> {
        self.intent.as_ref()
    }

    /// Returns the rejection diagnostic only for [`EffectDisposition::Rejected`].
    #[must_use]
    pub const fn diagnostic(&self) -> Option<&Diagnostic> {
        self.diagnostic.as_ref()
    }
}

impl EffectBatch {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn push(mut self, effect: AppEffect) -> Self {
        self.effects.push(effect);
        self
    }

    #[must_use]
    pub fn effects(&self) -> &[AppEffect] {
        &self.effects
    }
}
