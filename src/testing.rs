use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use super::Freshness;
use super::{
    AppEffect, AppInput, AppProxy, AppProxyError, AppProxyErrorCode, CorrelationId, DiagnosticLog,
    InputProvenance, ProxyInput, QueuePolicy, RedrawTarget, Reducer, ReducerResult, ResourceId,
    ResourceState, ResourceStateReadyTransition, ResourceStatus, RootId, Runtime, RuntimeBudget,
    RuntimeDrainReport, RuntimeInputError, ServiceId, ServiceInput, ServiceStatus, SurfaceId,
    TaskInput, TaskIntentAttemptId, TaskIntentHandle, TaskIntentId, UiInput, UiSurface, WakeBridge,
    WindowRoot,
};
use surgeist_window as window;

#[derive(Clone, Debug, Default)]
pub struct FakeWakeBridge {
    state: Arc<Mutex<FakeWakeState>>,
}

#[derive(Clone, Debug, Default)]
struct FakeWakeState {
    closed: bool,
    wakes: usize,
}

impl FakeWakeBridge {
    #[must_use]
    pub fn closed() -> Self {
        let bridge = Self::default();
        bridge.state.lock().expect("fake wake bridge lock").closed = true;
        bridge
    }

    #[must_use]
    pub fn wake_count(&self) -> usize {
        self.state.lock().expect("fake wake bridge lock").wakes
    }
}

impl WakeBridge for FakeWakeBridge {
    fn wake(&self) -> Result<(), AppProxyError> {
        let mut state = self.state.lock().expect("fake wake bridge lock");
        if state.closed {
            return Err(AppProxyError::new(AppProxyErrorCode::WakeFailed));
        }
        state.wakes += 1;
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FakeClock {
    now: Duration,
    next_sequence: u64,
    timers: Vec<ScheduledTimer>,
}

impl FakeClock {
    #[must_use]
    pub const fn now(&self) -> Duration {
        self.now
    }

    pub fn advance(&mut self, duration: Duration) {
        self.now += duration;
    }

    pub fn schedule_timer(&mut self, id: impl Into<String>, delay: Duration) {
        self.timers.push(ScheduledTimer {
            id: id.into(),
            due_at: self.now + delay,
            sequence: self.next_sequence,
        });
        self.next_sequence += 1;
    }

    #[must_use]
    pub fn drain_due_timers(&mut self) -> Vec<String> {
        let mut due = Vec::new();
        let mut pending = Vec::new();

        for timer in self.timers.drain(..) {
            if timer.due_at <= self.now {
                due.push(timer);
            } else {
                pending.push(timer);
            }
        }

        due.sort_by_key(|timer| (timer.due_at, timer.sequence));
        self.timers = pending;
        due.into_iter().map(|timer| timer.id).collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScheduledTimer {
    id: String,
    due_at: Duration,
    sequence: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FakeWindowBridge {
    redraws: Vec<SurfaceId>,
    commands: Vec<FakeWindowCommand>,
}

impl FakeWindowBridge {
    pub fn request_redraw(&mut self, surface_id: SurfaceId) {
        self.redraws.push(surface_id);
    }

    pub fn record_native_command(&mut self, window_id: window::Id, command: impl Into<String>) {
        self.commands.push(FakeWindowCommand::Native {
            window_id,
            command: command.into(),
        });
    }

    #[must_use]
    pub fn redraws(&self) -> &[SurfaceId] {
        &self.redraws
    }

    #[must_use]
    pub fn commands(&self) -> &[FakeWindowCommand] {
        &self.commands
    }

    fn record_open_surface(
        &mut self,
        name: impl Into<String>,
        surface_id: SurfaceId,
        window_id: window::Id,
        root_id: RootId,
    ) {
        self.commands.push(FakeWindowCommand::OpenSurface {
            name: name.into(),
            surface_id,
            window_id,
            root_id,
        });
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FakeWindowCommand {
    OpenSurface {
        name: String,
        surface_id: SurfaceId,
        window_id: window::Id,
        root_id: RootId,
    },
    Native {
        window_id: window::Id,
        command: String,
    },
}

pub struct HeadlessHarness<State, R, Input = ()> {
    runtime: Runtime<State, R, Input>,
    fake_window: FakeWindowBridge,
    clock: FakeClock,
    surfaces: BTreeMap<String, SurfaceId>,
    next_surface_id: u64,
    next_window_id: u64,
    last_report: Option<RuntimeDrainReport>,
}

impl<State, R, Input> HeadlessHarness<State, R, Input>
where
    Input: 'static,
{
    #[must_use]
    pub fn new(state: State, reducer: R) -> Self {
        let runtime = Runtime::new(state, reducer);

        Self {
            runtime,
            fake_window: FakeWindowBridge::default(),
            clock: FakeClock::default(),
            surfaces: BTreeMap::new(),
            next_surface_id: 1,
            next_window_id: 1,
            last_report: None,
        }
    }

    #[must_use]
    pub const fn runtime(&self) -> &Runtime<State, R, Input> {
        &self.runtime
    }

    pub fn runtime_mut(&mut self) -> &mut Runtime<State, R, Input> {
        &mut self.runtime
    }

    #[must_use]
    pub const fn state(&self) -> &State {
        self.runtime.state()
    }

    #[must_use]
    pub const fn fake_window(&self) -> &FakeWindowBridge {
        &self.fake_window
    }

    #[must_use]
    pub const fn clock(&self) -> &FakeClock {
        &self.clock
    }

    pub fn clock_mut(&mut self) -> &mut FakeClock {
        &mut self.clock
    }

    pub fn schedule_timer(&mut self, id: impl Into<String>, delay: Duration) {
        self.clock.schedule_timer(id, delay);
    }

    #[must_use]
    pub fn due_timers(&mut self) -> Vec<String> {
        self.clock.drain_due_timers()
    }

    pub fn open_surface(&mut self, name: impl Into<String>) -> SurfaceId {
        let name = name.into();
        if let Some(surface_id) = self.surfaces.get(&name) {
            return *surface_id;
        }

        let surface_id = SurfaceId::from_u64(self.next_surface_id);
        self.next_surface_id += 1;
        let window_id = window::Id::from_u64(self.next_window_id);
        self.next_window_id += 1;
        let root_id = RootId::new(name.clone());

        self.runtime.add_surface(UiSurface::new(
            surface_id,
            window_id,
            WindowRoot::new(root_id.clone()),
        ));
        self.fake_window
            .record_open_surface(name.clone(), surface_id, window_id, root_id);
        self.surfaces.insert(name, surface_id);
        surface_id
    }

    #[must_use]
    pub fn surface_id(&self, name: &str) -> SurfaceId {
        *self
            .surfaces
            .get(name)
            .expect("headless surface should be open")
    }

    pub fn enqueue_ui(
        &mut self,
        input: Input,
        provenance: InputProvenance,
    ) -> Result<(), RuntimeInputError> {
        self.runtime.enqueue_ui(UiInput::new(input, provenance)?);
        Ok(())
    }

    #[must_use]
    pub const fn last_report(&self) -> Option<&RuntimeDrainReport> {
        self.last_report.as_ref()
    }
}

impl<State, R, Input> HeadlessHarness<State, R, Input>
where
    R: Reducer<State, Input>,
    Input: 'static,
{
    pub fn drain(&mut self) -> RuntimeDrainReport {
        let report = self.runtime.drain_once(RuntimeBudget::default());
        for surface_id in report.redraw_requests() {
            self.fake_window.request_redraw(*surface_id);
        }
        self.last_report = Some(report.clone());
        report
    }
}

impl HeadlessHarness<CounterState, CounterReducer, CounterInput> {
    #[must_use]
    pub fn counter() -> Self {
        Self::new(CounterState::default(), CounterReducer)
    }

    pub fn input_increment(&mut self) {
        let surface_id = self.primary_surface_id();
        self.enqueue_ui(CounterInput::Increment, InputProvenance::ui(surface_id))
            .expect("counter input should be valid ui input");
    }

    #[must_use]
    pub fn counter_value(&self) -> u32 {
        self.state().value
    }

    fn primary_surface_id(&self) -> SurfaceId {
        self.surfaces
            .values()
            .next()
            .copied()
            .unwrap_or_else(|| SurfaceId::from_u64(1))
    }
}

pub struct HeadlessApp;

impl HeadlessApp {
    #[must_use]
    pub fn counter() -> CounterApp {
        CounterApp {
            harness: HeadlessHarness::counter(),
        }
    }
}

pub struct CounterApp {
    harness: HeadlessHarness<CounterState, CounterReducer, CounterInput>,
}

impl CounterApp {
    pub fn open_surface(&mut self, name: &str) -> SurfaceId {
        self.harness.open_surface(name)
    }

    pub fn input_increment(&mut self) {
        self.harness.input_increment();
    }

    pub fn drain(&mut self) -> RuntimeDrainReport {
        self.harness.drain()
    }

    #[must_use]
    pub fn counter(&self) -> u32 {
        self.harness.counter_value()
    }

    #[must_use]
    pub fn surface_id(&self, name: &str) -> SurfaceId {
        self.harness.surface_id(name)
    }

    #[must_use]
    pub const fn fake_window(&self) -> &FakeWindowBridge {
        self.harness.fake_window()
    }
}

pub struct ThumbnailImportExample {
    harness: HeadlessHarness<ThumbnailImportState, ThumbnailImportReducer, ThumbnailImportInput>,
    proxy: AppProxy<ThumbnailImportInput>,
    wake: FakeWakeBridge,
    import_handle: TaskIntentHandle,
    gallery_surface: SurfaceId,
    remaining_task_inputs: usize,
}

impl ThumbnailImportExample {
    #[must_use]
    pub fn new() -> Self {
        let gallery_surface = SurfaceId::from_u64(1);
        let wake = FakeWakeBridge::default();
        let proxy = AppProxy::new(wake.clone(), QueuePolicy::bounded(128));
        let import_handle =
            TaskIntentHandle::new(THUMBNAIL_IMPORT_TASK_ID, TaskIntentAttemptId::from_u64(1));
        let mut harness = HeadlessHarness::new(
            ThumbnailImportState::new(gallery_surface),
            ThumbnailImportReducer,
        );
        harness.open_surface("gallery");

        Self {
            harness,
            proxy,
            wake,
            import_handle,
            gallery_surface,
            remaining_task_inputs: 0,
        }
    }

    pub fn choose_folder(&mut self, folder: &str) {
        self.enqueue_ui(ThumbnailImportInput::FolderChosen {
            folder: folder.to_owned(),
        });
    }

    pub fn drain_once(&mut self) -> RuntimeDrainReport {
        self.flush_proxy();
        let report = self.harness.drain();
        self.remaining_task_inputs = report.remaining_task_inputs();
        report
    }

    pub fn drain_all(&mut self) {
        loop {
            self.drain_once();
            if self.proxy.pending_len() == 0 && self.remaining_task_inputs == 0 {
                break;
            }
        }
    }

    #[must_use]
    pub fn initial_tile_count(&self) -> usize {
        self.harness.state().tiles.len()
    }

    #[must_use]
    pub fn thumbnail_status(&self, index: usize) -> ResourceStatus {
        self.harness
            .state()
            .tiles
            .get(index)
            .expect("thumbnail tile should exist")
            .status()
    }

    pub fn finish_thumbnail(&mut self, index: usize) {
        self.proxy
            .send_task(
                TaskInput::new(
                    ThumbnailImportInput::ThumbnailFinished {
                        index,
                        value: format!("thumbnail-{index}"),
                    },
                    InputProvenance::task(self.import_handle.id(), self.import_handle.attempt_id()),
                )
                .expect("thumbnail completion should be a task input"),
            )
            .expect("thumbnail completion should enqueue");
    }

    pub fn refresh_thumbnail(&mut self, index: usize) {
        self.enqueue_ui(ThumbnailImportInput::RefreshThumbnail { index });
    }

    pub fn navigate_away(&mut self) {
        self.enqueue_ui(ThumbnailImportInput::NavigateAway);
    }

    #[must_use]
    pub fn redrawn_surfaces(&self) -> &[SurfaceId] {
        self.harness.fake_window().redraws()
    }

    #[must_use]
    pub const fn gallery_surface(&self) -> SurfaceId {
        self.gallery_surface
    }

    #[must_use]
    pub const fn fake_wake(&self) -> &FakeWakeBridge {
        &self.wake
    }

    fn enqueue_ui(&mut self, input: ThumbnailImportInput) {
        self.harness
            .enqueue_ui(input, InputProvenance::ui(self.gallery_surface))
            .expect("thumbnail example input should be valid ui input");
    }

    fn flush_proxy(&mut self) {
        for input in self.proxy.drain_pending(usize::MAX) {
            match input {
                ProxyInput::Task(input) => self.harness.runtime_mut().enqueue_task(input),
                ProxyInput::Service(input) => self.harness.runtime_mut().enqueue_service(input),
            }
        }
    }
}

impl Default for ThumbnailImportExample {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ThumbnailImportState {
    gallery_surface: SurfaceId,
    folder: Option<String>,
    tiles: Vec<ResourceState<String, String>>,
    observing_gallery: bool,
}

impl ThumbnailImportState {
    fn new(gallery_surface: SurfaceId) -> Self {
        Self {
            gallery_surface,
            folder: None,
            tiles: Vec::new(),
            observing_gallery: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ThumbnailImportInput {
    FolderChosen { folder: String },
    ThumbnailFinished { index: usize, value: String },
    RefreshThumbnail { index: usize },
    NavigateAway,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ThumbnailImportReducer;

impl Reducer<ThumbnailImportState, ThumbnailImportInput> for ThumbnailImportReducer {
    fn reduce(
        &mut self,
        state: &mut ThumbnailImportState,
        input: AppInput<ThumbnailImportInput>,
    ) -> ReducerResult {
        let changed = match input.payload() {
            ThumbnailImportInput::FolderChosen { folder } => {
                state.folder = Some(folder.clone());
                state.observing_gallery = true;
                state.tiles = initial_thumbnail_tiles(folder);
                for tile in &mut state.tiles {
                    tile.starting();
                    tile.add_observer();
                }
                true
            }
            ThumbnailImportInput::ThumbnailFinished { index, value } => {
                if let Some(tile) = state.tiles.get_mut(*index) {
                    tile.ready(value.clone(), Freshness::Fresh);
                    true
                } else {
                    false
                }
            }
            ThumbnailImportInput::RefreshThumbnail { index } => {
                if let Some(tile) = state.tiles.get_mut(*index) {
                    tile.refreshing();
                    true
                } else {
                    false
                }
            }
            ThumbnailImportInput::NavigateAway => {
                if state.observing_gallery {
                    state.observing_gallery = false;
                    for tile in &mut state.tiles {
                        tile.remove_observer();
                    }
                    true
                } else {
                    false
                }
            }
        };

        if changed {
            ReducerResult::changed().with_effect(AppEffect::request_redraw(RedrawTarget::surface(
                state.gallery_surface,
            )))
        } else {
            ReducerResult::unchanged()
        }
    }
}

fn initial_thumbnail_tiles(folder: &str) -> Vec<ResourceState<String, String>> {
    (0..3)
        .map(|index| {
            ResourceState::idle(ResourceId::new(format!("thumbnail:{folder}:photo-{index}")))
        })
        .collect()
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CounterState {
    value: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CounterInput {
    Increment,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CounterReducer;

impl Reducer<CounterState, CounterInput> for CounterReducer {
    fn reduce(
        &mut self,
        state: &mut CounterState,
        input: super::AppInput<CounterInput>,
    ) -> ReducerResult {
        match input.payload() {
            CounterInput::Increment => {
                state.value += 1;
                let surface_id = input
                    .provenance()
                    .surface_id()
                    .unwrap_or_else(|| SurfaceId::from_u64(1));
                ReducerResult::changed()
                    .with_effect(AppEffect::request_redraw(RedrawTarget::surface(surface_id)))
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ServiceRequestId(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceRequestStatus {
    Pending,
    Completed,
    Cancelled,
    TimedOutAfterCancel,
}

pub struct PrototypeApp {
    budget: RuntimeBudget,
    runtime: Runtime<PrototypeState, PrototypeReducer, PrototypeInput>,
    remaining_task_inputs: usize,
    wake: FakeWakeBridge,
    proxy: AppProxy<PrototypeInput>,
    surfaces: BTreeMap<String, SurfaceId>,
    next_surface_id: u64,
    next_window_id: u64,
    next_request_id: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrototypeInput {
    SearchStarted {
        query: String,
        attempt: TaskIntentAttemptId,
    },
    SearchComplete {
        attempt: TaskIntentAttemptId,
        results: Vec<String>,
    },
    LogLine(String),
    Progress(usize),
    ServiceProgress {
        request: ServiceRequestId,
        message: String,
    },
    ServiceResponse {
        request: ServiceRequestId,
        message: String,
    },
    ServiceCancelled {
        request: ServiceRequestId,
    },
    ServiceTimedOut {
        request: ServiceRequestId,
    },
    ServiceReconnected,
    ToolCallStarted {
        request: ServiceRequestId,
    },
}

impl PrototypeApp {
    #[must_use]
    pub fn latest_search() -> Self {
        Self::new(RuntimeBudget::default())
    }

    #[must_use]
    pub fn log_stream(budget: RuntimeBudget) -> Self {
        Self::new(budget)
    }

    #[must_use]
    pub fn progress_counter(budget: RuntimeBudget) -> Self {
        Self::new(budget)
    }

    #[must_use]
    pub fn jsonrpc_service() -> Self {
        let mut app = Self::new(RuntimeBudget::default());
        app.reconnect();
        app.drain_all();
        app
    }

    pub fn start_search(&mut self, query: &str, attempt: TaskIntentAttemptId) {
        self.enqueue_ui(PrototypeInput::SearchStarted {
            query: query.to_owned(),
            attempt,
        });
    }

    pub fn complete_search(&mut self, attempt: TaskIntentAttemptId, results: Vec<&str>) {
        self.complete_search_with_provenance(attempt, attempt, results);
    }

    pub fn complete_search_with_provenance(
        &mut self,
        provenance_attempt: TaskIntentAttemptId,
        payload_attempt: TaskIntentAttemptId,
        results: Vec<&str>,
    ) {
        let results = results
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<String>>();
        self.proxy
            .send_task(
                TaskInput::new(
                    PrototypeInput::SearchComplete {
                        attempt: payload_attempt,
                        results,
                    },
                    InputProvenance::task(SEARCH_TASK_ID, provenance_attempt),
                )
                .expect("prototype search completion should be a task input"),
            )
            .expect("prototype search completion should enqueue");
    }

    pub fn push_log_line(&mut self, line: String) {
        self.proxy
            .send_task(
                TaskInput::new(
                    PrototypeInput::LogLine(line),
                    InputProvenance::task(LOG_TASK_ID, TaskIntentAttemptId::from_u64(1)),
                )
                .expect("prototype log line should be a task input"),
            )
            .expect("prototype log line should enqueue");
    }

    pub fn drain(&mut self) {
        self.flush_proxy();
        let report = self.runtime.drain_once(self.budget);
        self.remaining_task_inputs = report.remaining_task_inputs();
    }

    pub fn drain_all(&mut self) {
        loop {
            self.drain();
            if self.proxy.pending_len() == 0 && self.remaining_task_inputs == 0 {
                break;
            }
        }
    }

    #[must_use]
    pub fn search_results(&self) -> &[String] {
        &self.runtime.state().search_results
    }

    #[must_use]
    pub const fn diagnostics(&self) -> &DiagnosticLog {
        self.runtime.diagnostics()
    }

    #[must_use]
    pub fn log_lines(&self) -> &[String] {
        &self.runtime.state().log_lines
    }

    #[must_use]
    pub const fn remaining_task_inputs(&self) -> usize {
        self.remaining_task_inputs
    }

    #[must_use]
    pub const fn progress_count(&self) -> usize {
        self.runtime.state().progress_count
    }

    #[must_use]
    pub const fn reducer_reentry_count(&self) -> usize {
        self.runtime.state().reducer_reentry_count
    }

    #[must_use]
    pub const fn fake_wake(&self) -> &FakeWakeBridge {
        &self.wake
    }

    #[must_use]
    pub const fn proxy(&self) -> &AppProxy<PrototypeInput> {
        &self.proxy
    }

    #[must_use]
    pub fn progress_event(&self, index: usize) -> TaskInput<PrototypeInput> {
        TaskInput::new(
            PrototypeInput::Progress(index),
            InputProvenance::task(PROGRESS_TASK_ID, TaskIntentAttemptId::from_u64(1)),
        )
        .expect("prototype progress should be a task input")
    }

    pub fn open_surface(&mut self, name: &str) -> SurfaceId {
        if let Some(surface_id) = self.surfaces.get(name) {
            return *surface_id;
        }

        let surface_id = SurfaceId::from_u64(self.next_surface_id);
        self.next_surface_id += 1;
        let window_id = window::Id::from_u64(self.next_window_id);
        self.next_window_id += 1;
        self.runtime.add_surface(UiSurface::new(
            surface_id,
            window_id,
            WindowRoot::new(RootId::new(name)),
        ));
        self.surfaces.insert(name.to_owned(), surface_id);
        surface_id
    }

    pub fn call_tool(&mut self, _name: &str) -> ServiceRequestId {
        let request = ServiceRequestId(self.next_request_id);
        self.next_request_id += 1;
        self.enqueue_ui(PrototypeInput::ToolCallStarted { request });
        request
    }

    pub fn notify_progress(&mut self, request: ServiceRequestId, message: &str) {
        self.enqueue_service(
            PrototypeInput::ServiceProgress {
                request,
                message: message.to_owned(),
            },
            request,
        );
    }

    pub fn respond(&mut self, request: ServiceRequestId, message: &str) {
        self.enqueue_service(
            PrototypeInput::ServiceResponse {
                request,
                message: message.to_owned(),
            },
            request,
        );
    }

    pub fn cancel(&mut self, request: ServiceRequestId) {
        self.enqueue_service(PrototypeInput::ServiceCancelled { request }, request);
    }

    pub fn timeout(&mut self, request: ServiceRequestId) {
        self.enqueue_service(PrototypeInput::ServiceTimedOut { request }, request);
    }

    pub fn reconnect(&mut self) {
        self.proxy
            .send_service(
                ServiceInput::new(
                    PrototypeInput::ServiceReconnected,
                    InputProvenance::service(jsonrpc_service_id()),
                )
                .expect("prototype reconnect should be a service input"),
            )
            .expect("prototype reconnect should enqueue");
    }

    #[must_use]
    pub fn response(&self, request: ServiceRequestId) -> Option<&str> {
        self.runtime
            .state()
            .responses
            .get(&request)
            .map(String::as_str)
    }

    #[must_use]
    pub fn request_status(&self, request: ServiceRequestId) -> ServiceRequestStatus {
        self.runtime
            .state()
            .request_status
            .get(&request)
            .copied()
            .unwrap_or(ServiceRequestStatus::Pending)
    }

    #[must_use]
    pub fn service_status(&self, service: ServiceId) -> ServiceStatus {
        if service == jsonrpc_service_id() {
            self.runtime.state().jsonrpc_status
        } else {
            ServiceStatus::Stopped
        }
    }

    fn new(budget: RuntimeBudget) -> Self {
        let wake = FakeWakeBridge::default();
        let proxy = AppProxy::new(wake.clone(), QueuePolicy::bounded(20_000));

        Self {
            budget,
            runtime: Runtime::new(PrototypeState::default(), PrototypeReducer),
            remaining_task_inputs: 0,
            wake,
            proxy,
            surfaces: BTreeMap::new(),
            next_surface_id: 1,
            next_window_id: 1,
            next_request_id: 1,
        }
    }

    fn flush_proxy(&mut self) {
        for input in self.proxy.drain_pending(usize::MAX) {
            match input {
                ProxyInput::Task(input) => self.runtime.enqueue_task(input),
                ProxyInput::Service(input) => self.runtime.enqueue_service(input),
            }
        }
    }

    fn enqueue_service(&self, input: PrototypeInput, request: ServiceRequestId) {
        self.proxy
            .send_service(
                ServiceInput::new(
                    input,
                    InputProvenance::service(jsonrpc_service_id())
                        .with_correlation(CorrelationId::from_u64(request.0)),
                )
                .expect("prototype service event should be a service input"),
            )
            .expect("prototype service event should enqueue");
    }

    fn enqueue_ui(&mut self, input: PrototypeInput) {
        self.runtime.enqueue_ui(
            UiInput::new(input, InputProvenance::system()).expect(
                "prototype setup action should be accepted as deterministic UI/system input",
            ),
        );
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PrototypeState {
    active_search_query: Option<String>,
    active_search_attempt: Option<TaskIntentAttemptId>,
    search_results: Vec<String>,
    log_lines: Vec<String>,
    progress_count: usize,
    reducer_reentry_count: usize,
    reducing: bool,
    jsonrpc_status: ServiceStatus,
    request_status: BTreeMap<ServiceRequestId, ServiceRequestStatus>,
    responses: BTreeMap<ServiceRequestId, String>,
    service_progress: BTreeMap<ServiceRequestId, Vec<String>>,
}

impl Default for PrototypeState {
    fn default() -> Self {
        Self {
            active_search_query: None,
            active_search_attempt: None,
            search_results: Vec::new(),
            log_lines: Vec::new(),
            progress_count: 0,
            reducer_reentry_count: 0,
            reducing: false,
            jsonrpc_status: ServiceStatus::Stopped,
            request_status: BTreeMap::new(),
            responses: BTreeMap::new(),
            service_progress: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct PrototypeReducer;

impl Reducer<PrototypeState, PrototypeInput> for PrototypeReducer {
    fn reduce(
        &mut self,
        state: &mut PrototypeState,
        input: AppInput<PrototypeInput>,
    ) -> ReducerResult {
        if state.reducing {
            state.reducer_reentry_count += 1;
        }
        state.reducing = true;

        let changed = match input.payload() {
            PrototypeInput::SearchStarted { query, attempt } => {
                state.active_search_query = Some(query.clone());
                state.active_search_attempt = Some(*attempt);
                true
            }
            PrototypeInput::SearchComplete { attempt, results } => {
                if state.active_search_attempt == Some(*attempt) {
                    state.search_results.clone_from(results);
                    true
                } else {
                    false
                }
            }
            PrototypeInput::LogLine(line) => {
                state.log_lines.push(line.clone());
                true
            }
            PrototypeInput::Progress(_index) => {
                state.progress_count += 1;
                true
            }
            PrototypeInput::ServiceProgress { request, message } => {
                state
                    .service_progress
                    .entry(*request)
                    .or_default()
                    .push(message.clone());
                true
            }
            PrototypeInput::ServiceResponse { request, message } => {
                state.responses.insert(*request, message.clone());
                state
                    .request_status
                    .insert(*request, ServiceRequestStatus::Completed);
                true
            }
            PrototypeInput::ServiceCancelled { request } => {
                state
                    .request_status
                    .insert(*request, ServiceRequestStatus::Cancelled);
                true
            }
            PrototypeInput::ServiceTimedOut { request } => {
                state
                    .request_status
                    .insert(*request, ServiceRequestStatus::TimedOutAfterCancel);
                true
            }
            PrototypeInput::ServiceReconnected => {
                state.jsonrpc_status = ServiceStatus::Running;
                true
            }
            PrototypeInput::ToolCallStarted { request } => {
                state
                    .request_status
                    .insert(*request, ServiceRequestStatus::Pending);
                true
            }
        };

        state.reducing = false;
        if changed {
            ReducerResult::changed()
        } else {
            ReducerResult::unchanged()
        }
    }
}

const SEARCH_TASK_ID: TaskIntentId = TaskIntentId::from_u64(1);
const LOG_TASK_ID: TaskIntentId = TaskIntentId::from_u64(2);
const PROGRESS_TASK_ID: TaskIntentId = TaskIntentId::from_u64(3);
const THUMBNAIL_IMPORT_TASK_ID: TaskIntentId = TaskIntentId::from_u64(5);

fn jsonrpc_service_id() -> ServiceId {
    ServiceId::new("jsonrpc")
}
