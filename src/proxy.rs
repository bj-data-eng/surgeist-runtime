use std::{
    collections::VecDeque,
    fmt,
    sync::{Arc, Mutex},
};

use super::{ServiceInput, TaskInput};

pub trait WakeBridge: Send + Sync + 'static {
    fn wake(&self) -> Result<(), AppProxyError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProxyInput<Input> {
    Task(TaskInput<Input>),
    Service(ServiceInput<Input>),
}

#[derive(Clone)]
pub struct AppProxy<Input> {
    state: Arc<Mutex<ProxyState<Input>>>,
    policy: QueuePolicy,
    wake: Arc<dyn WakeBridge>,
}

impl<Input> AppProxy<Input> {
    #[must_use]
    pub fn new(wake: impl WakeBridge, policy: QueuePolicy) -> Self {
        Self {
            state: Arc::new(Mutex::new(ProxyState::default())),
            policy,
            wake: Arc::new(wake),
        }
    }

    pub fn send_task(&self, input: TaskInput<Input>) -> Result<(), AppProxyError> {
        self.enqueue(ProxyInput::Task(input))
    }

    pub fn send_service(&self, input: ServiceInput<Input>) -> Result<(), AppProxyError> {
        self.enqueue(ProxyInput::Service(input))
    }

    #[must_use]
    pub fn drain_pending(&self, limit: usize) -> Vec<ProxyInput<Input>> {
        let mut state = self.state.lock().expect("app proxy lock");
        let drain_len = limit.min(state.queue.len());
        let drained = state.queue.drain(..drain_len).collect();
        if state.queue.is_empty() {
            state.wake_pending = false;
        }
        drained
    }

    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.state.lock().expect("app proxy lock").queue.len()
    }

    fn enqueue(&self, input: ProxyInput<Input>) -> Result<(), AppProxyError> {
        let needs_wake = {
            let mut state = self.state.lock().expect("app proxy lock");
            self.policy.check_capacity(state.queue.len())?;
            state.queue.push_back(input);
            if state.wake_pending {
                false
            } else {
                state.wake_pending = true;
                true
            }
        };

        if needs_wake {
            self.wake.wake().inspect_err(|_error| {
                self.state.lock().expect("app proxy lock").wake_pending = false;
            })?;
        }

        Ok(())
    }
}

#[derive(Clone, Debug)]
struct ProxyState<Input> {
    queue: VecDeque<ProxyInput<Input>>,
    wake_pending: bool,
}

impl<Input> Default for ProxyState<Input> {
    fn default() -> Self {
        Self {
            queue: VecDeque::new(),
            wake_pending: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueuePolicy {
    capacity: usize,
}

impl QueuePolicy {
    #[must_use]
    pub const fn bounded(capacity: usize) -> Self {
        Self { capacity }
    }

    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    fn check_capacity(&self, current_len: usize) -> Result<(), AppProxyError> {
        if current_len >= self.capacity {
            return Err(AppProxyError::queue_overflow(self.capacity));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppProxyError {
    code: AppProxyErrorCode,
    message: String,
}

impl AppProxyError {
    #[must_use]
    pub fn new(code: AppProxyErrorCode) -> Self {
        let message = match code {
            AppProxyErrorCode::WakeFailed => "native wake bridge failed".to_owned(),
            AppProxyErrorCode::QueueOverflow => "app proxy queue overflow".to_owned(),
        };
        Self { code, message }
    }

    #[must_use]
    pub fn queue_overflow(capacity: usize) -> Self {
        Self {
            code: AppProxyErrorCode::QueueOverflow,
            message: format!("app proxy queue overflow at capacity {capacity}"),
        }
    }

    #[must_use]
    pub const fn code(&self) -> AppProxyErrorCode {
        self.code
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for AppProxyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for AppProxyError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppProxyErrorCode {
    WakeFailed,
    QueueOverflow,
}
