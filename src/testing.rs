use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use super::{
    AppInput, AppProxy, AppProxyError, AppProxyErrorCode, CorrelationId, InputProvenance,
    ProxyInput, QueuePolicy, Reducer, ReducerCommit, ReducerResult, Runtime, RuntimeBudget,
    ServiceId, ServiceInput, ServiceStatus, TaskInput, TaskIntentAttemptId, TaskIntentId, UiInput,
    WakeBridge,
};

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

pub struct HeadlessHarness {
    clock: FakeClock,
}

impl HeadlessHarness {
    #[must_use]
    pub fn counter() -> Self {
        Self {
            clock: FakeClock::default(),
        }
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
    last_drain_inputs: usize,
    wake: FakeWakeBridge,
    proxy: AppProxy<PrototypeInput>,
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
        let report = self
            .runtime
            .drain_once(self.budget)
            .expect("prototype fixtures do not construct overflowing runtime transactions");
        self.last_drain_inputs = report.drained_inputs();
    }

    pub fn drain_all(&mut self) {
        loop {
            self.drain();
            if self.proxy.pending_len() == 0 && self.last_drain_inputs == 0 {
                break;
            }
        }
    }

    #[must_use]
    pub fn search_results(&self) -> &[String] {
        &self.runtime.state().search_results
    }

    #[must_use]
    pub fn log_lines(&self) -> &[String] {
        &self.runtime.state().log_lines
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
            last_drain_inputs: 0,
            wake,
            proxy,
            next_request_id: 1,
        }
    }

    fn flush_proxy(&mut self) {
        for input in self.proxy.drain_pending(usize::MAX) {
            match input {
                ProxyInput::Task(input) => self
                    .runtime
                    .enqueue_task(input)
                    .expect("prototype task input should fit the runtime queue"),
                ProxyInput::Service(input) => self
                    .runtime
                    .enqueue_service(input)
                    .expect("prototype service input should fit the runtime queue"),
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
        self.runtime
            .enqueue_ui(UiInput::new(input, InputProvenance::system()).expect(
                "prototype setup action should be accepted as deterministic UI/system input",
            ))
            .expect("prototype UI input should fit the runtime queue");
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
        state: &PrototypeState,
        input: &AppInput<PrototypeInput>,
    ) -> ReducerResult<PrototypeState> {
        let mut next_state = state.clone();
        if next_state.reducing {
            next_state.reducer_reentry_count += 1;
        }
        next_state.reducing = true;

        let changed = match input.payload() {
            PrototypeInput::SearchStarted { query, attempt } => {
                next_state.active_search_query = Some(query.clone());
                next_state.active_search_attempt = Some(*attempt);
                true
            }
            PrototypeInput::SearchComplete { attempt, results } => {
                if next_state.active_search_attempt == Some(*attempt) {
                    next_state.search_results.clone_from(results);
                    true
                } else {
                    false
                }
            }
            PrototypeInput::LogLine(line) => {
                next_state.log_lines.push(line.clone());
                true
            }
            PrototypeInput::Progress(_index) => {
                next_state.progress_count += 1;
                true
            }
            PrototypeInput::ServiceProgress { request, message } => {
                next_state
                    .service_progress
                    .entry(*request)
                    .or_default()
                    .push(message.clone());
                true
            }
            PrototypeInput::ServiceResponse { request, message } => {
                next_state.responses.insert(*request, message.clone());
                next_state
                    .request_status
                    .insert(*request, ServiceRequestStatus::Completed);
                true
            }
            PrototypeInput::ServiceCancelled { request } => {
                next_state
                    .request_status
                    .insert(*request, ServiceRequestStatus::Cancelled);
                true
            }
            PrototypeInput::ServiceTimedOut { request } => {
                next_state
                    .request_status
                    .insert(*request, ServiceRequestStatus::TimedOutAfterCancel);
                true
            }
            PrototypeInput::ServiceReconnected => {
                next_state.jsonrpc_status = ServiceStatus::Running;
                true
            }
            PrototypeInput::ToolCallStarted { request } => {
                next_state
                    .request_status
                    .insert(*request, ServiceRequestStatus::Pending);
                true
            }
        };

        next_state.reducing = false;
        if changed {
            ReducerResult::changed(next_state, ReducerCommit::new())
        } else {
            ReducerResult::unchanged(ReducerCommit::new())
        }
    }
}

const SEARCH_TASK_ID: TaskIntentId = TaskIntentId::from_u64(1);
const LOG_TASK_ID: TaskIntentId = TaskIntentId::from_u64(2);
const PROGRESS_TASK_ID: TaskIntentId = TaskIntentId::from_u64(3);

fn jsonrpc_service_id() -> ServiceId {
    ServiceId::new("jsonrpc")
}
