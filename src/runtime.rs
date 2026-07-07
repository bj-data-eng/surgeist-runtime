use std::collections::VecDeque;

use super::{
    AppEffect, AppEffectPayload, AppInput, Diagnostic, DiagnosticCode, DiagnosticLog,
    InputProvenance, QueueDiagnostic, RedrawTarget, Reducer, StateVersion, SurfaceId, UiSurface,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeLane {
    Ui,
    Task,
    Service,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiInput<Input> {
    input: AppInput<Input>,
}

impl<Input> UiInput<Input> {
    pub fn new(payload: Input, provenance: InputProvenance) -> Result<Self, RuntimeInputError> {
        if provenance.task_id().is_some() || provenance.service_id().is_some() {
            return Err(RuntimeInputError::wrong_lane(RuntimeLane::Ui, provenance));
        }

        Ok(Self {
            input: AppInput::new(payload, provenance),
        })
    }

    #[must_use]
    pub fn into_app_input(self) -> AppInput<Input> {
        self.input
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskInput<Input> {
    input: AppInput<Input>,
}

impl<Input> TaskInput<Input> {
    pub fn new(payload: Input, provenance: InputProvenance) -> Result<Self, RuntimeInputError> {
        if provenance.task_id().is_none() || provenance.task_attempt_id().is_none() {
            return Err(RuntimeInputError::wrong_lane(RuntimeLane::Task, provenance));
        }

        Ok(Self {
            input: AppInput::new(payload, provenance),
        })
    }

    #[must_use]
    pub fn into_app_input(self) -> AppInput<Input> {
        self.input
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceInput<Input> {
    input: AppInput<Input>,
}

impl<Input> ServiceInput<Input> {
    pub fn new(payload: Input, provenance: InputProvenance) -> Result<Self, RuntimeInputError> {
        if provenance.service_id().is_none() {
            return Err(RuntimeInputError::wrong_lane(
                RuntimeLane::Service,
                provenance,
            ));
        }

        Ok(Self {
            input: AppInput::new(payload, provenance),
        })
    }

    #[must_use]
    pub fn into_app_input(self) -> AppInput<Input> {
        self.input
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeInputError {
    lane: RuntimeLane,
    provenance: InputProvenance,
}

impl RuntimeInputError {
    fn wrong_lane(lane: RuntimeLane, provenance: InputProvenance) -> Self {
        Self { lane, provenance }
    }

    #[must_use]
    pub const fn lane(&self) -> RuntimeLane {
        self.lane
    }

    #[must_use]
    pub const fn provenance(&self) -> &InputProvenance {
        &self.provenance
    }
}

const DEFAULT_TASK_QUEUE_CAPACITY: usize = 65_536;
const DEFAULT_SERVICE_QUEUE_CAPACITY: usize = 65_536;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeQueuePolicy {
    max_task_inputs: usize,
    max_service_inputs: usize,
}

impl RuntimeQueuePolicy {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            max_task_inputs: DEFAULT_TASK_QUEUE_CAPACITY,
            max_service_inputs: DEFAULT_SERVICE_QUEUE_CAPACITY,
        }
    }

    #[must_use]
    pub const fn max_task_inputs(mut self, capacity: usize) -> Self {
        self.max_task_inputs = capacity;
        self
    }

    #[must_use]
    pub const fn max_service_inputs(mut self, capacity: usize) -> Self {
        self.max_service_inputs = capacity;
        self
    }

    #[must_use]
    pub const fn task_capacity(&self) -> usize {
        self.max_task_inputs
    }

    #[must_use]
    pub const fn service_capacity(&self) -> usize {
        self.max_service_inputs
    }
}

impl Default for RuntimeQueuePolicy {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Runtime<State = (), R = (), Input = ()> {
    state: State,
    reducer: R,
    state_version: StateVersion,
    surfaces: BTreeMap<SurfaceId, UiSurface>,
    diagnostics: DiagnosticLog,
    ui_queue: VecDeque<UiInput<Input>>,
    task_queue: VecDeque<TaskInput<Input>>,
    service_queue: VecDeque<ServiceInput<Input>>,
    queue_policy: RuntimeQueuePolicy,
}

impl<State, R, Input> Runtime<State, R, Input> {
    #[must_use]
    pub fn new(state: State, reducer: R) -> Self {
        Self {
            state,
            reducer,
            state_version: StateVersion::from_u64(0),
            surfaces: BTreeMap::new(),
            diagnostics: DiagnosticLog::with_capacity(256),
            ui_queue: VecDeque::new(),
            task_queue: VecDeque::new(),
            service_queue: VecDeque::new(),
            queue_policy: RuntimeQueuePolicy::default(),
        }
    }

    #[must_use]
    pub const fn with_queue_policy(mut self, policy: RuntimeQueuePolicy) -> Self {
        self.queue_policy = policy;
        self
    }

    #[must_use]
    pub const fn queue_policy(&self) -> RuntimeQueuePolicy {
        self.queue_policy
    }

    #[must_use]
    pub const fn state(&self) -> &State {
        &self.state
    }

    #[must_use]
    pub const fn state_version(&self) -> StateVersion {
        self.state_version
    }

    #[must_use]
    pub const fn diagnostics(&self) -> &DiagnosticLog {
        &self.diagnostics
    }

    pub fn add_surface(&mut self, surface: UiSurface) {
        self.surfaces.insert(surface.id(), surface);
    }

    pub fn enqueue_ui(&mut self, input: UiInput<Input>) {
        self.ui_queue.push_back(input);
    }

    pub fn enqueue_task(&mut self, input: TaskInput<Input>) {
        if self.task_queue.len() >= self.queue_policy.task_capacity() {
            self.record_queue_overflow(
                RuntimeLane::Task,
                self.queue_policy.task_capacity(),
                input.input.provenance().clone(),
            );
            return;
        }
        self.task_queue.push_back(input);
    }

    pub fn enqueue_service(&mut self, input: ServiceInput<Input>) {
        if self.service_queue.len() >= self.queue_policy.service_capacity() {
            self.record_queue_overflow(
                RuntimeLane::Service,
                self.queue_policy.service_capacity(),
                input.input.provenance().clone(),
            );
            return;
        }
        self.service_queue.push_back(input);
    }

    fn record_queue_overflow(
        &mut self,
        lane: RuntimeLane,
        capacity: usize,
        provenance: InputProvenance,
    ) {
        let task = provenance.task_id().zip(provenance.task_attempt_id());
        let service = provenance.service_id();
        let mut diagnostic = Diagnostic::warning(
            DiagnosticCode::QUEUE_OVERFLOW,
            format!(
                "{} overflow at capacity {capacity}; dropped newest input",
                lane.queue_display_name()
            ),
            provenance,
        )
        .with_queue(QueueDiagnostic::new(lane.queue_name(), capacity).with_dropped(1));

        if let Some((task_id, attempt_id)) = task {
            diagnostic = diagnostic.with_task(task_id, attempt_id);
        }
        if let Some(service_id) = service {
            diagnostic = diagnostic.with_service(service_id);
        }

        self.diagnostics.push(diagnostic);
    }
}

impl RuntimeLane {
    const fn queue_name(self) -> &'static str {
        match self {
            Self::Ui => "runtime.ui",
            Self::Task => "runtime.task",
            Self::Service => "runtime.service",
        }
    }

    const fn queue_display_name(self) -> &'static str {
        match self {
            Self::Ui => "runtime UI queue",
            Self::Task => "runtime task queue",
            Self::Service => "runtime service queue",
        }
    }
}

impl<State, R, Input> Runtime<State, R, Input>
where
    R: Reducer<State, Input>,
{
    pub fn drain_once(&mut self, budget: RuntimeBudget) -> RuntimeDrainReport {
        let mut report = RuntimeDrainReport {
            remaining_task_inputs: self.task_queue.len(),
            ..RuntimeDrainReport::default()
        };

        while report.drained_inputs < budget.max_inputs && !self.ui_queue.is_empty() {
            let input = self
                .ui_queue
                .pop_front()
                .expect("queue was checked before pop");
            self.drain_input(RuntimeLane::Ui, input.into_app_input(), &mut report);
        }

        let mut drained_task_events = 0;
        while report.drained_inputs < budget.max_inputs
            && drained_task_events < budget.max_task_events
            && !self.task_queue.is_empty()
        {
            let input = self
                .task_queue
                .pop_front()
                .expect("queue was checked before pop");
            drained_task_events += 1;
            self.drain_input(RuntimeLane::Task, input.into_app_input(), &mut report);
        }

        let mut drained_service_events = 0;
        while report.drained_inputs < budget.max_inputs
            && drained_service_events < budget.max_service_events
            && !self.service_queue.is_empty()
        {
            let input = self
                .service_queue
                .pop_front()
                .expect("queue was checked before pop");
            drained_service_events += 1;
            self.drain_input(RuntimeLane::Service, input.into_app_input(), &mut report);
        }

        report.remaining_task_inputs = self.task_queue.len();
        report
    }

    fn drain_input(
        &mut self,
        lane: RuntimeLane,
        input: AppInput<Input>,
        report: &mut RuntimeDrainReport,
    ) {
        Self::record_drained_input(lane, report);
        self.reduce_input(input, report);
    }

    fn record_drained_input(lane: RuntimeLane, report: &mut RuntimeDrainReport) {
        if report.first_drained_lane.is_none() {
            report.first_drained_lane = Some(lane);
        }
        report.drained_inputs += 1;
    }

    fn reduce_input(&mut self, input: AppInput<Input>, report: &mut RuntimeDrainReport) {
        let provenance = input.provenance().clone();
        let result = self.reducer.reduce(&mut self.state, input);
        if let Some(error) = result.recoverable_error() {
            report.reducer_errors += 1;
            self.diagnostics.push(Diagnostic::error(
                DiagnosticCode::REDUCER_ERROR,
                error,
                result.provenance().cloned().unwrap_or(provenance),
            ));
            return;
        }

        if result.is_changed() {
            self.state_version = StateVersion::from_u64(self.state_version.as_u64() + 1);
        }

        for effect in result.effects() {
            self.execute_effect(effect, report);
        }
    }

    fn execute_effect(&mut self, app_effect: &AppEffect, report: &mut RuntimeDrainReport) {
        match app_effect.payload() {
            AppEffectPayload::RequestRedraw(effect) => {
                report.executed_effects += 1;
                match effect.target() {
                    RedrawTarget::All => {
                        report.redraw_requests.extend(self.surfaces.keys().copied());
                    }
                    RedrawTarget::Surface(id) => report.redraw_requests.push(*id),
                    RedrawTarget::Window(window_id) => {
                        report
                            .redraw_requests
                            .extend(self.surfaces.iter().filter_map(|(surface_id, surface)| {
                                (surface.window_id() == *window_id).then_some(*surface_id)
                            }));
                    }
                }
            }
            AppEffectPayload::Diagnostic(effect) => {
                report.executed_effects += 1;
                self.diagnostics.push(effect.diagnostic().clone());
            }
            AppEffectPayload::StartTask(_) => {
                report.executed_effects += 1;
                report.task_intents.push(app_effect.clone());
            }
            AppEffectPayload::CancelTask(_) => {
                report.executed_effects += 1;
                report.task_intents.push(app_effect.clone());
            }
            AppEffectPayload::LoadResource(effect) => {
                report.executed_effects += 1;
                self.diagnostics.push(
                    effect_failed("resource registry is not available")
                        .with_resource(effect.id().clone())
                        .with_scope(effect.scope().clone())
                        .with_effect("runtime.load_resource"),
                );
            }
            AppEffectPayload::InvalidateResource(effect) => {
                report.executed_effects += 1;
                self.diagnostics.push(
                    effect_failed("resource registry is not available")
                        .with_resource(effect.id().clone())
                        .with_effect("runtime.invalidate_resource"),
                );
            }
            AppEffectPayload::Persist(effect) => {
                report.executed_effects += 1;
                self.diagnostics.push(
                    effect_failed("persistence registry is not available")
                        .with_scope(effect.scope().clone())
                        .with_effect("runtime.persist"),
                );
            }
            AppEffectPayload::ReprioritizeTask(_) => {
                report.executed_effects += 1;
                report.task_intents.push(app_effect.clone());
            }
            AppEffectPayload::StartService(effect) => {
                report.executed_effects += 1;
                self.diagnostics.push(
                    effect_failed("service registry is not available")
                        .with_service(effect.id().clone())
                        .with_effect("runtime.start_service"),
                );
            }
            AppEffectPayload::StopService(effect) => {
                report.executed_effects += 1;
                self.diagnostics.push(
                    effect_failed("service registry is not available")
                        .with_service(effect.id().clone())
                        .with_effect("runtime.stop_service"),
                );
            }
            AppEffectPayload::CallService(effect) => {
                report.executed_effects += 1;
                self.diagnostics.push(
                    effect_failed("service registry is not available")
                        .with_service(effect.id().clone())
                        .with_effect("runtime.call_service"),
                );
            }
            AppEffectPayload::ServiceDiagnostic(effect) => {
                report.executed_effects += 1;
                self.diagnostics.push(
                    effect
                        .diagnostic()
                        .clone()
                        .with_service(effect.id().clone())
                        .with_effect("runtime.service_diagnostic"),
                );
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeBudget {
    max_inputs: usize,
    max_task_events: usize,
    max_service_events: usize,
}

impl RuntimeBudget {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            max_inputs: 64,
            max_task_events: 64,
            max_service_events: 32,
        }
    }

    #[must_use]
    pub const fn max_inputs(mut self, max_inputs: usize) -> Self {
        self.max_inputs = max_inputs;
        self
    }

    #[must_use]
    pub const fn max_task_events(mut self, max_task_events: usize) -> Self {
        self.max_task_events = max_task_events;
        self
    }

    #[must_use]
    pub const fn max_service_events(mut self, max_service_events: usize) -> Self {
        self.max_service_events = max_service_events;
        self
    }
}

impl Default for RuntimeBudget {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeDrainReport {
    drained_inputs: usize,
    executed_effects: usize,
    reducer_errors: usize,
    remaining_task_inputs: usize,
    first_drained_lane: Option<RuntimeLane>,
    redraw_requests: Vec<SurfaceId>,
    task_intents: Vec<AppEffect>,
}

impl RuntimeDrainReport {
    #[must_use]
    pub const fn drained_inputs(&self) -> usize {
        self.drained_inputs
    }

    #[must_use]
    pub const fn executed_effects(&self) -> usize {
        self.executed_effects
    }

    #[must_use]
    pub const fn reducer_errors(&self) -> usize {
        self.reducer_errors
    }

    #[must_use]
    pub const fn remaining_task_inputs(&self) -> usize {
        self.remaining_task_inputs
    }

    #[must_use]
    pub const fn first_drained_lane(&self) -> Option<RuntimeLane> {
        self.first_drained_lane
    }

    #[must_use]
    pub fn redraw_requests(&self) -> &[SurfaceId] {
        &self.redraw_requests
    }

    #[must_use]
    pub fn task_intents(&self) -> &[AppEffect] {
        &self.task_intents
    }
}

fn effect_failed(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(
        DiagnosticCode::EFFECT_FAILED,
        message,
        InputProvenance::system(),
    )
}

impl Default for Runtime<(), (), ()> {
    fn default() -> Self {
        Self::new((), ())
    }
}
