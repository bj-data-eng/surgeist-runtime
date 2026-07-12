use std::{
    collections::{BTreeMap, VecDeque},
    error::Error,
    fmt,
};

use super::{
    AppEffect, AppEffectPayload, AppInput, CoordinationState, Diagnostic, DiagnosticCode,
    DiagnosticLog, EffectOutcome, ElementPhase, InputProvenance, QueueDiagnostic, RedrawTarget,
    Reducer, ReducerResult, RuntimeIntent, StateVersion, Subscription, SubscriptionChange,
    SubscriptionError, SubscriptionErrorCode, SubscriptionKey, SurfaceElementRef, SurfaceError,
    SurfaceErrorCode, SurfaceGeneration, SurfaceId, SurfaceLifecycle, SurfaceMutation,
    SurfacePoint, SurfaceRef, SurfaceRenderAck, SurfaceRenderFrame, SurfaceRenderState,
    SurfaceRoute, SurfaceSize, UiSurface, VersionError,
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
            state_version: StateVersion::initial(),
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

    /// Validates a targetable element against the current surface registration.
    ///
    /// Registry lookup precedes lifecycle, element, and phase checks so stale
    /// references cannot target a replacement surface.
    pub fn validate_element(
        &self,
        reference: SurfaceElementRef,
        phase: ElementPhase,
    ) -> Result<(), SurfaceError> {
        let surface = self.current_surface(reference.surface())?;
        ensure_targetable(surface.lifecycle())?;
        surface.validate_element(reference, phase)
    }

    /// Validates every route step against its current, targetable surface.
    ///
    /// Returns the route target only after all element-phase registrations pass.
    pub fn validate_route(&self, route: &SurfaceRoute) -> Result<SurfaceElementRef, SurfaceError> {
        let surface = self.current_surface(route.surface())?;
        ensure_targetable(surface.lifecycle())?;
        surface.validate_route(route)
    }

    /// Updates a renderable surface viewport and records its viewport invalidation.
    pub fn resize(
        &mut self,
        surface: SurfaceRef,
        viewport: SurfaceSize,
    ) -> Result<SurfaceMutation, SurfaceError> {
        self.current_surface_mut(surface)?.set_viewport(viewport)
    }

    /// Updates one registered surface's scroll offset and records the change.
    pub fn set_scroll_offset(
        &mut self,
        surface: SurfaceRef,
        offset: SurfacePoint,
    ) -> Result<SurfaceMutation, SurfaceError> {
        self.current_surface_mut(surface)?.set_scroll_offset(offset)
    }

    /// Sets or clears focus for one registered surface.
    ///
    /// A non-empty element reference must belong to the current registration;
    /// clearing focus does not require an element lookup.
    pub fn set_focus(
        &mut self,
        surface: SurfaceRef,
        element: Option<SurfaceElementRef>,
    ) -> Result<SurfaceMutation, SurfaceError> {
        self.current_surface_mut(surface)?.set_focus(element)
    }

    /// Sets or clears hover for one registered surface.
    ///
    /// Hover is independent from focus and validates non-empty references against
    /// the current registration.
    pub fn set_hover(
        &mut self,
        surface: SurfaceRef,
        element: Option<SurfaceElementRef>,
    ) -> Result<SurfaceMutation, SurfaceError> {
        self.current_surface_mut(surface)?.set_hover(element)
    }

    /// Borrows the current state with a frame for one renderable surface.
    ///
    /// The returned view prevents mutable Runtime operations until it is
    /// consumed with [`SurfaceRenderState::into_frame`].
    pub fn begin_render(
        &self,
        surface: SurfaceRef,
    ) -> Result<SurfaceRenderState<'_, State>, SurfaceError> {
        let frame = self
            .current_surface(surface)?
            .begin_render(self.state_version)?;
        Ok(SurfaceRenderState::new(&self.state, frame))
    }

    /// Acknowledges the work represented by a Runtime-issued render frame.
    ///
    /// The acknowledgement is registry-validated before its local lifecycle and
    /// monotonic state-version checks can consume invalidations.
    pub fn mark_rendered(
        &mut self,
        frame: SurfaceRenderFrame,
    ) -> Result<SurfaceRenderAck, SurfaceError> {
        self.current_surface_mut(frame.surface())?
            .acknowledge_render(frame)
    }

    /// Iterates invalidated renderable surfaces in ascending surface-ID order.
    pub fn renderable_invalidated_surfaces(&self) -> impl Iterator<Item = SurfaceRef> + '_ {
        self.surfaces.values().filter_map(|surface| {
            (is_renderable(surface.lifecycle()) && !surface.invalidations().is_empty())
                .then_some(surface.surface_ref())
        })
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

    fn current_surface_mut(&mut self, surface: SurfaceRef) -> Result<&mut UiSurface, SurfaceError> {
        let Some(current) = self.surfaces.get_mut(&surface.surface_id()) else {
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

    #[cfg(test)]
    pub(crate) fn set_state_version_for_test(&mut self, state_version: StateVersion) {
        self.state_version = state_version;
    }
}

const fn is_terminal(lifecycle: SurfaceLifecycle) -> bool {
    matches!(
        lifecycle,
        SurfaceLifecycle::Closing | SurfaceLifecycle::Closed | SurfaceLifecycle::Destroyed
    )
}

const fn is_renderable(lifecycle: SurfaceLifecycle) -> bool {
    matches!(
        lifecycle,
        SurfaceLifecycle::Ready | SurfaceLifecycle::Resized
    )
}

fn ensure_targetable(lifecycle: SurfaceLifecycle) -> Result<(), SurfaceError> {
    if is_terminal(lifecycle) {
        return Err(SurfaceError::new(
            SurfaceErrorCode::TerminalSurface,
            "surface is terminal",
        ));
    }
    if !is_renderable(lifecycle) {
        return Err(SurfaceError::new(
            SurfaceErrorCode::InvalidLifecycleTransition,
            "surface targeting requires a renderable lifecycle",
        ));
    }
    Ok(())
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
    /// Drains queued inputs in the current lane order until `budget` is exhausted.
    ///
    /// A checked changed-state transaction that cannot advance its state version
    /// or a required surface invalidation returns the partial work committed
    /// before that input and restores the exact input to the front of its lane.
    pub fn drain_once(
        &mut self,
        budget: RuntimeBudget,
    ) -> Result<RuntimeDrainReport, RuntimeDrainError> {
        let mut report = RuntimeDrainReport::default();

        while report.drained_inputs < budget.max_inputs && !self.ui_queue.is_empty() {
            let input = self
                .ui_queue
                .pop_front()
                .expect("queue was checked before pop");
            if let Err(overflow) = self.drain_input(RuntimeLane::Ui, &input.input, &mut report) {
                self.ui_queue.push_front(input);
                return Err(overflow.into_error(RuntimeLane::Ui, report));
            }
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
            if let Err(overflow) = self.drain_input(RuntimeLane::Task, &input.input, &mut report) {
                self.task_queue.push_front(input);
                return Err(overflow.into_error(RuntimeLane::Task, report));
            }
            drained_task_events += 1;
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
            if let Err(overflow) = self.drain_input(RuntimeLane::Service, &input.input, &mut report)
            {
                self.service_queue.push_front(input);
                return Err(overflow.into_error(RuntimeLane::Service, report));
            }
            drained_service_events += 1;
        }

        report.finish();
        Ok(report)
    }

    fn drain_input(
        &mut self,
        lane: RuntimeLane,
        input: &AppInput<Input>,
        report: &mut RuntimeDrainReport,
    ) -> Result<(), DrainOverflow> {
        self.reduce_input(input, report)?;
        Self::record_drained_input(lane, report);
        Ok(())
    }

    fn record_drained_input(lane: RuntimeLane, report: &mut RuntimeDrainReport) {
        if report.first_drained_lane.is_none() {
            report.first_drained_lane = Some(lane);
        }
        report.drained_inputs += 1;
    }

    fn reduce_input(
        &mut self,
        input: &AppInput<Input>,
        report: &mut RuntimeDrainReport,
    ) -> Result<(), DrainOverflow> {
        let provenance = input.provenance().clone();
        match self.reducer.reduce(&self.state, input) {
            ReducerResult::Unchanged(commit) => {
                self.execute_commit(&commit, provenance, report);
            }
            ReducerResult::Changed(change) => {
                let (state, commit) = change.into_parts();
                let next_version = self
                    .state_version
                    .checked_next()
                    .map_err(|source| DrainOverflow::state(provenance.clone(), source))?;
                self.preflight_snapshot_invalidations(&provenance)?;
                self.state = state;
                self.state_version = next_version;
                self.record_snapshot_invalidations(next_version, report);
                self.execute_commit(&commit, provenance, report);
            }
            ReducerResult::RecoverableFailure(failure) => {
                report.reducer_errors += 1;
                self.diagnostics.push(Diagnostic::error(
                    DiagnosticCode::REDUCER_ERROR,
                    failure.message(),
                    failure.provenance().cloned().unwrap_or(provenance),
                ));
            }
        }
        Ok(())
    }

    fn preflight_snapshot_invalidations(
        &self,
        provenance: &InputProvenance,
    ) -> Result<(), DrainOverflow> {
        for surface in self.surfaces.values() {
            if is_terminal(surface.lifecycle()) {
                continue;
            }
            surface
                .preflight_snapshot_invalidation()
                .map_err(|source| {
                    DrainOverflow::surface(provenance.clone(), surface.surface_ref(), source)
                })?;
        }
        Ok(())
    }

    fn record_snapshot_invalidations(
        &mut self,
        version: StateVersion,
        report: &mut RuntimeDrainReport,
    ) {
        for surface in self.surfaces.values_mut() {
            if is_terminal(surface.lifecycle()) {
                continue;
            }
            let mutation = surface
                .invalidate_snapshot(version)
                .expect("runtime preflighted every nonterminal surface invalidation");
            if mutation.redraw_required() {
                report.redraw_requests.push(surface.surface_ref());
            }
        }
    }

    fn execute_commit(
        &mut self,
        commit: &super::ReducerCommit,
        trigger_provenance: InputProvenance,
        report: &mut RuntimeDrainReport,
    ) {
        let provenance = commit.provenance().cloned().unwrap_or(trigger_provenance);
        for effect in commit.effects().effects() {
            self.execute_effect(effect, &provenance, report);
        }
    }

    fn execute_effect(
        &mut self,
        app_effect: &AppEffect,
        provenance: &InputProvenance,
        report: &mut RuntimeDrainReport,
    ) {
        let kind = app_effect.kind().clone();
        match app_effect.payload() {
            AppEffectPayload::RequestRedraw(effect) => {
                match self.resolve_redraw_target(effect.target()) {
                    Ok(surfaces) => {
                        report.redraw_requests.extend(surfaces);
                        report.record_applied(EffectOutcome::applied(kind, provenance.clone()));
                    }
                    Err(error) => {
                        self.reject_effect(kind, provenance.clone(), error, report);
                    }
                }
            }
            AppEffectPayload::Diagnostic(effect) => {
                self.diagnostics.push(effect.diagnostic().clone());
                report.record_applied(EffectOutcome::applied(kind, provenance.clone()));
            }
            AppEffectPayload::Persist(effect) => self.forward_effect(
                kind,
                provenance.clone(),
                RuntimeIntent::Persist(effect.clone()),
                report,
            ),
            AppEffectPayload::LoadResource(effect) => self.forward_effect(
                kind,
                provenance.clone(),
                RuntimeIntent::LoadResource(effect.clone()),
                report,
            ),
            AppEffectPayload::InvalidateResource(effect) => self.forward_effect(
                kind,
                provenance.clone(),
                RuntimeIntent::InvalidateResource(effect.clone()),
                report,
            ),
            AppEffectPayload::StartTask(effect) => self.forward_effect(
                kind,
                provenance.clone(),
                RuntimeIntent::StartTask(effect.clone()),
                report,
            ),
            AppEffectPayload::CancelTask(effect) => self.forward_effect(
                kind,
                provenance.clone(),
                RuntimeIntent::CancelTask(effect.clone()),
                report,
            ),
            AppEffectPayload::ReprioritizeTask(effect) => self.forward_effect(
                kind,
                provenance.clone(),
                RuntimeIntent::ReprioritizeTask(effect.clone()),
                report,
            ),
            AppEffectPayload::StartService(effect) => self.forward_effect(
                kind,
                provenance.clone(),
                RuntimeIntent::StartService(effect.clone()),
                report,
            ),
            AppEffectPayload::StopService(effect) => self.forward_effect(
                kind,
                provenance.clone(),
                RuntimeIntent::StopService(effect.clone()),
                report,
            ),
            AppEffectPayload::CallService(effect) => self.forward_effect(
                kind,
                provenance.clone(),
                RuntimeIntent::CallService(effect.clone()),
                report,
            ),
            AppEffectPayload::ServiceDiagnostic(effect) => {
                self.diagnostics.push(
                    effect
                        .diagnostic()
                        .clone()
                        .with_service(effect.id().clone())
                        .with_effect("runtime.service_diagnostic"),
                );
                report.record_applied(EffectOutcome::applied(kind, provenance.clone()));
            }
        }
    }

    fn resolve_redraw_target(
        &self,
        target: &RedrawTarget,
    ) -> Result<Vec<SurfaceRef>, SurfaceError> {
        match target {
            RedrawTarget::All => Ok(self
                .surfaces
                .values()
                .filter(|surface| is_renderable(surface.lifecycle()))
                .map(UiSurface::surface_ref)
                .collect()),
            RedrawTarget::Surface(reference) => {
                let surface = self.current_surface(*reference)?;
                ensure_targetable(surface.lifecycle())?;
                Ok(vec![surface.surface_ref()])
            }
            RedrawTarget::Window(window_id) => {
                let matching = self
                    .surfaces
                    .values()
                    .filter(|surface| surface.window_id() == *window_id)
                    .collect::<Vec<_>>();
                if matching.is_empty() {
                    return Err(SurfaceError::new(
                        SurfaceErrorCode::UnknownSurface,
                        "window has no registered surfaces",
                    ));
                }
                let eligible = matching
                    .iter()
                    .filter(|surface| is_renderable(surface.lifecycle()))
                    .map(|surface| surface.surface_ref())
                    .collect::<Vec<_>>();
                if !eligible.is_empty() {
                    return Ok(eligible);
                }
                let code = if matching
                    .iter()
                    .any(|surface| !is_terminal(surface.lifecycle()))
                {
                    SurfaceErrorCode::InvalidLifecycleTransition
                } else {
                    SurfaceErrorCode::TerminalSurface
                };
                Err(SurfaceError::new(code, "window has no eligible surfaces"))
            }
        }
    }

    fn forward_effect(
        &self,
        kind: super::EffectKindId,
        provenance: InputProvenance,
        intent: RuntimeIntent,
        report: &mut RuntimeDrainReport,
    ) {
        report.record_forwarded(EffectOutcome::forwarded(kind, provenance, intent));
    }

    fn reject_effect(
        &mut self,
        kind: super::EffectKindId,
        provenance: InputProvenance,
        error: SurfaceError,
        report: &mut RuntimeDrainReport,
    ) {
        let diagnostic = Diagnostic::error(
            DiagnosticCode::EFFECT_FAILED,
            format!("{}: {error}", kind.as_str()),
            provenance.clone(),
        )
        .with_effect(kind.as_str());
        self.diagnostics.push(diagnostic.clone());
        report.record_rejected(EffectOutcome::rejected(kind, provenance, diagnostic));
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
/// The committed work and effect disposition from one successful runtime drain.
pub struct RuntimeDrainReport {
    drained_inputs: usize,
    applied_effects: usize,
    forwarded_effects: usize,
    rejected_effects: usize,
    reducer_errors: usize,
    first_drained_lane: Option<RuntimeLane>,
    redraw_requests: Vec<SurfaceRef>,
    intents: Vec<RuntimeIntent>,
    effect_outcomes: Vec<EffectOutcome>,
}

/// Identifies the checked transaction step that stopped a runtime drain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RuntimeDrainErrorCode {
    /// Advancing the runtime state version would overflow.
    StateVersionOverflow,
    /// Advancing a nonterminal surface invalidation generation would overflow.
    SurfaceInvalidationOverflow,
}

/// A checked runtime transaction failure with the already committed drain work.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDrainError {
    code: RuntimeDrainErrorCode,
    lane: RuntimeLane,
    provenance: InputProvenance,
    surface: Option<SurfaceRef>,
    partial_report: Box<RuntimeDrainReport>,
    source: VersionError,
}

impl RuntimeDrainReport {
    /// Returns the number of inputs that committed or failed recoverably.
    #[must_use]
    pub const fn drained_inputs(&self) -> usize {
        self.drained_inputs
    }

    /// Returns the number of effects applied locally by runtime.
    #[must_use]
    pub const fn applied_effects(&self) -> usize {
        self.applied_effects
    }

    /// Returns the number of effects preserved as adapter intents.
    #[must_use]
    pub const fn forwarded_effects(&self) -> usize {
        self.forwarded_effects
    }

    /// Returns the number of effects rejected by runtime validation.
    #[must_use]
    pub const fn rejected_effects(&self) -> usize {
        self.rejected_effects
    }

    /// Returns the number of recoverable reducer failures recorded as diagnostics.
    #[must_use]
    pub const fn reducer_errors(&self) -> usize {
        self.reducer_errors
    }

    /// Returns the first lane from which an input committed during this drain.
    #[must_use]
    pub const fn first_drained_lane(&self) -> Option<RuntimeLane> {
        self.first_drained_lane
    }

    /// Returns deduplicated renderable surfaces requested for redraw.
    #[must_use]
    pub fn redraw_requests(&self) -> &[SurfaceRef] {
        &self.redraw_requests
    }

    /// Returns forwarded adapter intents in effect commit order.
    #[must_use]
    pub fn intents(&self) -> &[RuntimeIntent] {
        &self.intents
    }

    /// Returns one outcome for every effect in successful commit order.
    #[must_use]
    pub fn effect_outcomes(&self) -> &[EffectOutcome] {
        &self.effect_outcomes
    }

    fn record_applied(&mut self, outcome: EffectOutcome) {
        self.applied_effects += 1;
        self.effect_outcomes.push(outcome);
    }

    fn record_forwarded(&mut self, outcome: EffectOutcome) {
        let intent = outcome
            .intent()
            .expect("forwarded effect outcomes always contain an adapter intent")
            .clone();
        self.forwarded_effects += 1;
        self.intents.push(intent);
        self.effect_outcomes.push(outcome);
    }

    fn record_rejected(&mut self, outcome: EffectOutcome) {
        self.rejected_effects += 1;
        self.effect_outcomes.push(outcome);
    }

    fn finish(&mut self) {
        self.redraw_requests
            .sort_by_key(|surface| (surface.surface_id(), surface.generation()));
        self.redraw_requests.dedup();
    }
}

impl RuntimeDrainError {
    fn new(
        code: RuntimeDrainErrorCode,
        lane: RuntimeLane,
        provenance: InputProvenance,
        surface: Option<SurfaceRef>,
        mut partial_report: RuntimeDrainReport,
        source: VersionError,
    ) -> Self {
        partial_report.finish();
        Self {
            code,
            lane,
            provenance,
            surface,
            partial_report: Box::new(partial_report),
            source,
        }
    }

    /// Returns the checked transaction step that failed.
    #[must_use]
    pub const fn code(&self) -> RuntimeDrainErrorCode {
        self.code
    }

    /// Returns the lane containing the exact input restored after failure.
    #[must_use]
    pub const fn lane(&self) -> RuntimeLane {
        self.lane
    }

    /// Returns the provenance of the input that triggered the failure.
    #[must_use]
    pub const fn provenance(&self) -> &InputProvenance {
        &self.provenance
    }

    /// Returns the first affected surface for invalidation overflow only.
    #[must_use]
    pub const fn surface(&self) -> Option<SurfaceRef> {
        self.surface
    }

    /// Returns work committed before the input that failed preflight.
    #[must_use]
    pub const fn partial_report(&self) -> &RuntimeDrainReport {
        &self.partial_report
    }

    /// Returns the checked version source that caused this transaction to fail.
    #[must_use]
    pub const fn source(&self) -> VersionError {
        self.source
    }

    /// Consumes this error and returns only the prior committed work.
    #[must_use]
    pub fn into_partial_report(self) -> RuntimeDrainReport {
        *self.partial_report
    }
}

impl fmt::Display for RuntimeDrainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "runtime drain {:?} transaction failed: {}",
            self.lane, self.source
        )
    }
}

impl Error for RuntimeDrainError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

struct DrainOverflow {
    code: RuntimeDrainErrorCode,
    provenance: InputProvenance,
    surface: Option<SurfaceRef>,
    source: VersionError,
}

impl DrainOverflow {
    fn state(provenance: InputProvenance, source: VersionError) -> Self {
        Self {
            code: RuntimeDrainErrorCode::StateVersionOverflow,
            provenance,
            surface: None,
            source,
        }
    }

    fn surface(provenance: InputProvenance, surface: SurfaceRef, source: VersionError) -> Self {
        Self {
            code: RuntimeDrainErrorCode::SurfaceInvalidationOverflow,
            provenance,
            surface: Some(surface),
            source,
        }
    }

    fn into_error(self, lane: RuntimeLane, report: RuntimeDrainReport) -> RuntimeDrainError {
        RuntimeDrainError::new(
            self.code,
            lane,
            self.provenance,
            self.surface,
            report,
            self.source,
        )
    }
}

impl Default for Runtime<(), (), ()> {
    fn default() -> Self {
        Self::new((), ())
    }
}
