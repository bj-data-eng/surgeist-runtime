use std::collections::{BTreeMap, VecDeque};

use super::{
    AppEffect, AppEffectPayload, AppInput, CoordinationState, Diagnostic, DiagnosticCode,
    DiagnosticLog, InputProvenance, QueueDiagnostic, RedrawTarget, Reducer, StateVersion,
    Subscription, SubscriptionChange, SubscriptionError, SubscriptionErrorCode, SubscriptionKey,
    SurfaceError, SurfaceErrorCode, SurfaceGeneration, SurfaceId, SurfaceLifecycle, SurfaceRef,
    UiSurface,
};
use crate::ids::CheckedNext;

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
    retired_surface_generations: BTreeMap<SurfaceId, SurfaceGeneration>,
    coordination: CoordinationState,
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
            retired_surface_generations: BTreeMap::new(),
            coordination: CoordinationState::default(),
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

    /// Registers a newly created surface and returns its current identity.
    ///
    /// A removed surface ID receives a checked successor generation, so old
    /// references cannot target a replacement registration.
    pub fn register_surface(&mut self, mut surface: UiSurface) -> Result<SurfaceRef, SurfaceError> {
        let id = surface.id();
        if self.surfaces.contains_key(&id) {
            return Err(SurfaceError::new(
                SurfaceErrorCode::DuplicateSurface,
                "surface is already registered",
            ));
        }
        if surface.lifecycle() != SurfaceLifecycle::Created {
            return Err(SurfaceError::new(
                SurfaceErrorCode::InvalidLifecycleTransition,
                "surface registration requires the created lifecycle",
            ));
        }
        if surface.generation() != SurfaceGeneration::initial() {
            return Err(SurfaceError::new(
                SurfaceErrorCode::StaleSurfaceGeneration,
                "surface registration requires the initial generation",
            ));
        }

        if let Some(retired_generation) = self.retired_surface_generations.get(&id).copied() {
            let generation = retired_generation
                .checked_next()
                .map_err(|_| SurfaceError::version_overflow())?;
            surface.assign_registration_generation(generation);
        }

        let reference = surface.surface_ref();
        self.surfaces.insert(id, surface);
        Ok(reference)
    }

    /// Returns the current registered surface for `id`.
    #[must_use]
    pub fn surface(&self, id: SurfaceId) -> Option<&UiSurface> {
        self.surfaces.get(&id)
    }

    /// Returns the current generation-qualified reference for `id`.
    #[must_use]
    pub fn surface_ref(&self, id: SurfaceId) -> Option<SurfaceRef> {
        self.surface(id).map(UiSurface::surface_ref)
    }

    /// Stages a mutation against one current surface and commits it atomically.
    ///
    /// Failed updates leave both the surface registry and observer subscriptions
    /// unchanged. A generation change or first terminal transition removes the
    /// subscriptions for the prior current registration before the new state commits.
    pub fn update_surface(
        &mut self,
        surface: SurfaceRef,
        update: impl FnOnce(&mut UiSurface) -> Result<(), SurfaceError>,
    ) -> Result<(), SurfaceError> {
        let current = self.current_surface(surface)?;
        let was_terminal = is_terminal(current.lifecycle());
        let mut staged = current.staged_clone();
        update(&mut staged)?;

        let cleanup_observer = (staged.surface_ref() != surface
            || (!was_terminal && is_terminal(staged.lifecycle())))
        .then_some(surface);

        if let Some(observer) = cleanup_observer {
            self.coordination.remove_observer(observer);
        }
        self.surfaces.insert(surface.surface_id(), staged);
        Ok(())
    }

    /// Removes a current registration, records its generation tombstone, and
    /// clears all subscriptions observed by that exact registration.
    pub fn remove_surface(&mut self, surface: SurfaceRef) -> Result<UiSurface, SurfaceError> {
        self.current_surface(surface)?;

        self.coordination.remove_observer(surface);
        self.retired_surface_generations
            .insert(surface.surface_id(), surface.generation());
        Ok(self
            .surfaces
            .remove(&surface.surface_id())
            .expect("current surface was validated before removal"))
    }

    /// Iterates registered IDs in deterministic ascending order.
    pub fn surface_ids(&self) -> impl Iterator<Item = SurfaceId> + '_ {
        self.surfaces.keys().copied()
    }

    /// Returns read-only subscription coordination state owned by this runtime.
    #[must_use]
    pub const fn coordination(&self) -> &CoordinationState {
        &self.coordination
    }

    /// Adds a subscription after validating its observer against the registry.
    pub fn subscribe(
        &mut self,
        subscription: Subscription,
    ) -> Result<SubscriptionChange, SubscriptionError> {
        self.validate_subscription_observer(subscription.key())?;
        self.coordination.subscribe(&subscription)
    }

    /// Removes one subscription reference after validating its observer.
    pub fn unsubscribe(
        &mut self,
        key: &SubscriptionKey,
    ) -> Result<SubscriptionChange, SubscriptionError> {
        self.validate_subscription_observer(key)?;
        Ok(self.coordination.unsubscribe(key))
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

    fn current_surface(&self, surface: SurfaceRef) -> Result<&UiSurface, SurfaceError> {
        let Some(current) = self.surfaces.get(&surface.surface_id()) else {
            return Err(SurfaceError::new(
                SurfaceErrorCode::UnknownSurface,
                "surface is not registered",
            ));
        };
        if current.generation() != surface.generation() {
            return Err(SurfaceError::new(
                SurfaceErrorCode::StaleSurfaceGeneration,
                "surface reference uses a stale generation",
            ));
        }
        Ok(current)
    }

    fn validate_subscription_observer(
        &self,
        key: &SubscriptionKey,
    ) -> Result<(), SubscriptionError> {
        let observer = key.observer();
        let Some(current) = self.surfaces.get(&observer.surface_id()) else {
            return Err(SubscriptionError::new(
                SubscriptionErrorCode::UnknownObserver,
                key.clone(),
            ));
        };
        if current.generation() != observer.generation() {
            return Err(SubscriptionError::new(
                SubscriptionErrorCode::StaleObserver,
                key.clone(),
            ));
        }
        if is_terminal(current.lifecycle()) {
            return Err(SubscriptionError::new(
                SubscriptionErrorCode::TerminalObserver,
                key.clone(),
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn set_retired_generation_for_test(
        &mut self,
        id: SurfaceId,
        generation: SurfaceGeneration,
    ) {
        self.retired_surface_generations.insert(id, generation);
    }
}

const fn is_terminal(lifecycle: SurfaceLifecycle) -> bool {
    matches!(
        lifecycle,
        SurfaceLifecycle::Closing | SurfaceLifecycle::Closed | SurfaceLifecycle::Destroyed
    )
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
                    RedrawTarget::Surface(surface) => {
                        report.redraw_requests.push(surface.surface_id());
                    }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RuntimeDrainError {}

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
