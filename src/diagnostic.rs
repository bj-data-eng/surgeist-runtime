use std::{
    borrow::Cow,
    collections::{BTreeMap, VecDeque},
};

use super::{
    AppId, AppScope, InputProvenance, ResourceId, RootId, ServiceId, TaskAttemptId, TaskId,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DiagnosticCode(Cow<'static, str>);

impl DiagnosticCode {
    pub const UNKNOWN_RETAINED_COMMAND: Self = Self::from_static("unknown_retained_command");
    pub const INVALID_RETAINED_PAYLOAD: Self = Self::from_static("invalid_retained_payload");
    pub const STALE_ELEMENT: Self = Self::from_static("stale_element");
    pub const INELIGIBLE_RETAINED_TARGET: Self = Self::from_static("ineligible_retained_target");
    pub const STALE_TASK_EVENT: Self = Self::from_static("stale_task_event");
    pub const QUEUE_OVERFLOW: Self = Self::from_static("queue_overflow");
    pub const QUEUE_COALESCED: Self = Self::from_static("queue_coalesced");
    pub const REDUCER_ERROR: Self = Self::from_static("reducer_error");
    pub const EFFECT_FAILED: Self = Self::from_static("effect_failed");
    pub const SERVICE_MAILBOX_OVERFLOW: Self = Self::from_static("service_mailbox_overflow");
    pub const SURFACE_DEGRADED: Self = Self::from_static("surface_degraded");

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
pub struct QueueDiagnostic {
    name: String,
    capacity: usize,
    dropped: usize,
    age_ms: Option<u64>,
}

impl QueueDiagnostic {
    #[must_use]
    pub fn new(name: impl Into<String>, capacity: usize) -> Self {
        Self {
            name: name.into(),
            capacity,
            dropped: 0,
            age_ms: None,
        }
    }

    #[must_use]
    pub const fn with_dropped(mut self, dropped: usize) -> Self {
        self.dropped = dropped;
        self
    }

    #[must_use]
    pub fn with_age_ms(mut self, age_ms: u64) -> Self {
        self.age_ms = Some(age_ms);
        self
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    #[must_use]
    pub const fn dropped(&self) -> usize {
        self.dropped
    }

    #[must_use]
    pub const fn age_ms(&self) -> Option<u64> {
        self.age_ms
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    severity: DiagnosticSeverity,
    code: DiagnosticCode,
    message: String,
    provenance: InputProvenance,
    app_id: Option<AppId>,
    window_id: Option<surgeist_window::Id>,
    root_id: Option<RootId>,
    scope: Option<AppScope>,
    resource_id: Option<ResourceId>,
    task_id: Option<TaskId>,
    task_attempt_id: Option<TaskAttemptId>,
    service_id: Option<ServiceId>,
    emitted_effects: Vec<String>,
    queue: Option<QueueDiagnostic>,
}

impl Diagnostic {
    #[must_use]
    pub fn info(
        code: DiagnosticCode,
        message: impl Into<String>,
        provenance: InputProvenance,
    ) -> Self {
        Self::new(DiagnosticSeverity::Info, code, message, provenance)
    }

    #[must_use]
    pub fn warning(
        code: DiagnosticCode,
        message: impl Into<String>,
        provenance: InputProvenance,
    ) -> Self {
        Self::new(DiagnosticSeverity::Warning, code, message, provenance)
    }

    #[must_use]
    pub fn error(
        code: DiagnosticCode,
        message: impl Into<String>,
        provenance: InputProvenance,
    ) -> Self {
        Self::new(DiagnosticSeverity::Error, code, message, provenance)
    }

    #[must_use]
    pub fn with_app(mut self, id: AppId) -> Self {
        self.app_id = Some(id);
        self
    }

    #[must_use]
    pub fn with_window(mut self, id: surgeist_window::Id) -> Self {
        self.window_id = Some(id);
        self
    }

    #[must_use]
    pub fn with_root(mut self, id: RootId) -> Self {
        self.root_id = Some(id);
        self
    }

    #[must_use]
    pub fn with_scope(mut self, scope: AppScope) -> Self {
        self.scope = Some(scope);
        self
    }

    #[must_use]
    pub fn with_resource(mut self, id: ResourceId) -> Self {
        self.resource_id = Some(id);
        self
    }

    #[must_use]
    pub fn with_task(mut self, id: TaskId, attempt: TaskAttemptId) -> Self {
        self.task_id = Some(id);
        self.task_attempt_id = Some(attempt);
        self
    }

    #[must_use]
    pub fn with_service(mut self, id: ServiceId) -> Self {
        self.service_id = Some(id);
        self
    }

    #[must_use]
    pub fn with_effect(mut self, effect: impl Into<String>) -> Self {
        self.emitted_effects.push(effect.into());
        self
    }

    #[must_use]
    pub fn with_queue(mut self, queue: QueueDiagnostic) -> Self {
        self.queue = Some(queue);
        self
    }

    #[must_use]
    pub const fn severity(&self) -> DiagnosticSeverity {
        self.severity
    }

    #[must_use]
    pub fn code(&self) -> &DiagnosticCode {
        &self.code
    }

    #[must_use]
    pub const fn provenance(&self) -> &InputProvenance {
        &self.provenance
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub fn app_id(&self) -> Option<&AppId> {
        self.app_id.as_ref()
    }

    #[must_use]
    pub const fn window_id(&self) -> Option<surgeist_window::Id> {
        self.window_id
    }

    #[must_use]
    pub fn root_id(&self) -> Option<&RootId> {
        self.root_id.as_ref()
    }

    #[must_use]
    pub fn scope(&self) -> Option<&AppScope> {
        self.scope.as_ref()
    }

    #[must_use]
    pub fn resource_id(&self) -> Option<&ResourceId> {
        self.resource_id.as_ref()
    }

    #[must_use]
    pub const fn task_id(&self) -> Option<TaskId> {
        self.task_id
    }

    #[must_use]
    pub const fn task_attempt_id(&self) -> Option<TaskAttemptId> {
        self.task_attempt_id
    }

    #[must_use]
    pub fn service_id(&self) -> Option<&ServiceId> {
        self.service_id.as_ref()
    }

    #[must_use]
    pub fn emitted_effects(&self) -> &[String] {
        &self.emitted_effects
    }

    #[must_use]
    pub const fn queue(&self) -> Option<&QueueDiagnostic> {
        self.queue.as_ref()
    }

    fn new(
        severity: DiagnosticSeverity,
        code: DiagnosticCode,
        message: impl Into<String>,
        provenance: InputProvenance,
    ) -> Self {
        Self {
            severity,
            code,
            message: message.into(),
            provenance,
            app_id: None,
            window_id: None,
            root_id: None,
            scope: None,
            resource_id: None,
            task_id: None,
            task_attempt_id: None,
            service_id: None,
            emitted_effects: Vec::new(),
            queue: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DiagnosticLog {
    capacity: usize,
    entries: VecDeque<Diagnostic>,
    dropped_oldest: usize,
    counts: BTreeMap<DiagnosticCode, usize>,
}

impl DiagnosticLog {
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            entries: VecDeque::new(),
            dropped_oldest: 0,
            counts: BTreeMap::new(),
        }
    }

    pub fn push(&mut self, diagnostic: Diagnostic) {
        *self.counts.entry(diagnostic.code().clone()).or_default() += 1;
        if self.capacity == 0 {
            self.dropped_oldest += 1;
            return;
        }
        if self.entries.len() == self.capacity {
            self.entries.pop_front();
            self.dropped_oldest += 1;
        }
        self.entries.push_back(diagnostic);
    }

    #[must_use]
    pub fn entries(&self) -> Vec<Diagnostic> {
        self.entries.iter().cloned().collect()
    }

    #[must_use]
    pub const fn dropped_oldest(&self) -> usize {
        self.dropped_oldest
    }

    #[must_use]
    pub fn count(&self, code: &DiagnosticCode) -> usize {
        self.counts.get(code).copied().unwrap_or(0)
    }
}
