use std::{
    collections::VecDeque,
    error::Error,
    fmt,
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex, MutexGuard},
};

use super::{ServiceInput, TaskInput};

/// A bridge failure reported while asking the host for a future turn.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WakeError {
    message: String,
}

impl WakeError {
    #[must_use]
    /// Creates a wake failure from a host-provided diagnostic message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    #[must_use]
    /// Returns the host-provided diagnostic message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for WakeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for WakeError {}

/// Arranges a future host turn for draining an [`AppProxy`].
///
/// Implementations must return without synchronously calling `send_task`,
/// `send_service`, or `drain_pending` on the same shared proxy, directly or
/// indirectly, and must not wait for a drain to occur. The runtime never calls
/// this bridge while holding the proxy mutex.
pub trait WakeBridge: Send + Sync + 'static {
    /// Requests one future host turn without synchronously draining the proxy.
    fn wake(&self) -> Result<(), WakeError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// An input awaiting transfer from a host-facing proxy into runtime.
pub enum ProxyInput<Input> {
    /// A validated input from a task attempt.
    Task(TaskInput<Input>),
    /// A validated input from a service.
    Service(ServiceInput<Input>),
}

#[derive(Clone)]
/// Thread-safe staging queue that asks a host for future drain turns.
pub struct AppProxy<Input> {
    shared: Arc<Shared<Input>>,
    policy: QueuePolicy,
    wake: Arc<dyn WakeBridge>,
}

impl<Input> AppProxy<Input> {
    #[must_use]
    /// Creates an empty proxy with immutable queue policy and wake bridge.
    pub fn new(wake: impl WakeBridge, policy: QueuePolicy) -> Self {
        Self {
            shared: Arc::new(Shared::default()),
            policy,
            wake: Arc::new(wake),
        }
    }

    /// Enqueues a task input and requests a future host turn when needed.
    pub fn send_task(&self, input: TaskInput<Input>) -> Result<(), AppProxyError<Input>> {
        self.enqueue(ProxyInput::Task(input))
    }

    /// Enqueues a service input and requests a future host turn when needed.
    pub fn send_service(&self, input: ServiceInput<Input>) -> Result<(), AppProxyError<Input>> {
        self.enqueue(ProxyInput::Service(input))
    }

    /// Removes up to `limit` inputs in FIFO order and requests a continuation wake if needed.
    pub fn drain_pending(&self, limit: NonZeroUsize) -> ProxyDrainReport<Input> {
        let mut state = self.lock_state();
        while matches!(&state.phase, ProxyPhase::Waking(_))
            || (matches!(&state.phase, ProxyPhase::NeedsWake) && state.waiting_senders != 0)
        {
            #[cfg(test)]
            {
                state.waiting_drains += 1;
            }
            state = self.wait_for_change(state);
            #[cfg(test)]
            {
                state.waiting_drains -= 1;
            }
        }

        if state.queue.is_empty() {
            state.phase = ProxyPhase::Idle;
            return ProxyDrainReport::new(Vec::new(), 0, None);
        }

        let drain_len = limit.get().min(state.queue.len());
        let drained = state
            .queue
            .drain(..drain_len)
            .map(|entry| entry.input)
            .collect();
        if state.queue.is_empty() {
            state.phase = ProxyPhase::Idle;
            return ProxyDrainReport::new(drained, 0, None);
        }

        state.phase = ProxyPhase::Waking(WakeOwner::Drain);
        drop(state);

        let continuation_wake_error = self.wake.wake().err();
        let mut state = self.lock_state();
        debug_assert!(state.is_drain_waking());
        state.phase = if continuation_wake_error.is_some() {
            ProxyPhase::NeedsWake
        } else {
            ProxyPhase::Signaled
        };
        let remaining_len = state.queue.len();
        self.shared.changed.notify_all();
        ProxyDrainReport::new(drained, remaining_len, continuation_wake_error)
    }

    #[must_use]
    /// Returns the number of task or service inputs still awaiting a host drain.
    pub fn pending_len(&self) -> usize {
        self.lock_state().queue.len()
    }

    #[cfg(test)]
    pub(crate) fn waiting_sender_count(&self) -> usize {
        self.lock_state().waiting_senders
    }

    #[cfg(test)]
    pub(crate) fn waiting_drain_count(&self) -> usize {
        self.lock_state().waiting_drains
    }

    fn enqueue(&self, input: ProxyInput<Input>) -> Result<(), AppProxyError<Input>> {
        let token = EntryToken::new();
        let owns_wake = {
            let mut state = self.lock_state();
            if state.queue.len() >= self.policy.capacity {
                return Err(AppProxyError::queue_overflow(self.policy.capacity, input));
            }
            state.queue.push_back(QueuedInput {
                token: token.clone(),
                input,
            });
            match &state.phase {
                ProxyPhase::Signaled => return Ok(()),
                ProxyPhase::Idle | ProxyPhase::NeedsWake => {
                    state.phase = ProxyPhase::Waking(WakeOwner::Sender(token.clone()));
                    true
                }
                ProxyPhase::Waking(_) => {
                    state.waiting_senders += 1;
                    false
                }
            }
        };

        if owns_wake {
            return self.resolve_sender_wake(token);
        }

        let mut state = self.lock_state();
        loop {
            if !state.has_token(&token) {
                state.waiting_senders -= 1;
                return Ok(());
            }

            if matches!(&state.phase, ProxyPhase::Signaled | ProxyPhase::Idle) {
                state.waiting_senders -= 1;
                return Ok(());
            }
            if matches!(&state.phase, ProxyPhase::NeedsWake) {
                state.waiting_senders -= 1;
                state.phase = ProxyPhase::Waking(WakeOwner::Sender(token.clone()));
                break;
            }
            state = self.wait_for_change(state);
        }
        drop(state);
        self.resolve_sender_wake(token)
    }

    fn resolve_sender_wake(&self, token: EntryToken) -> Result<(), AppProxyError<Input>> {
        match self.wake.wake() {
            Ok(()) => {
                let mut state = self.lock_state();
                debug_assert!(state.is_sender_waking(&token));
                state.phase = ProxyPhase::Signaled;
                self.shared.changed.notify_all();
                Ok(())
            }
            Err(wake_error) => {
                let mut state = self.lock_state();
                debug_assert!(state.is_sender_waking(&token));
                let index = state
                    .queue
                    .iter()
                    .position(|entry| entry.token.matches(&token))
                    .expect("waking sender must remain queued until its wake resolves");
                let rejected = state
                    .queue
                    .remove(index)
                    .expect("queued waking sender must have a valid position")
                    .input;
                state.phase = if state.queue.is_empty() {
                    ProxyPhase::Idle
                } else {
                    ProxyPhase::NeedsWake
                };
                self.shared.changed.notify_all();
                Err(AppProxyError::wake_failed(rejected, wake_error))
            }
        }
    }

    fn lock_state(&self) -> MutexGuard<'_, ProxyState<Input>> {
        self.shared
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn wait_for_change<'a>(
        &self,
        state: MutexGuard<'a, ProxyState<Input>>,
    ) -> MutexGuard<'a, ProxyState<Input>> {
        self.shared
            .changed
            .wait(state)
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

struct Shared<Input> {
    state: Mutex<ProxyState<Input>>,
    changed: Condvar,
}

impl<Input> Default for Shared<Input> {
    fn default() -> Self {
        Self {
            state: Mutex::new(ProxyState::default()),
            changed: Condvar::new(),
        }
    }
}

struct ProxyState<Input> {
    queue: VecDeque<QueuedInput<Input>>,
    phase: ProxyPhase,
    waiting_senders: usize,
    #[cfg(test)]
    waiting_drains: usize,
}

impl<Input> ProxyState<Input> {
    fn has_token(&self, token: &EntryToken) -> bool {
        self.queue.iter().any(|entry| entry.token.matches(token))
    }

    fn is_sender_waking(&self, token: &EntryToken) -> bool {
        matches!(
            &self.phase,
            ProxyPhase::Waking(WakeOwner::Sender(owner)) if owner.matches(token)
        )
    }

    fn is_drain_waking(&self) -> bool {
        matches!(&self.phase, ProxyPhase::Waking(WakeOwner::Drain))
    }
}

impl<Input> Default for ProxyState<Input> {
    fn default() -> Self {
        Self {
            queue: VecDeque::new(),
            phase: ProxyPhase::Idle,
            waiting_senders: 0,
            #[cfg(test)]
            waiting_drains: 0,
        }
    }
}

struct QueuedInput<Input> {
    token: EntryToken,
    input: ProxyInput<Input>,
}

#[derive(Clone)]
struct EntryToken(Arc<()>);

impl EntryToken {
    fn new() -> Self {
        Self(Arc::new(()))
    }

    fn matches(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

enum ProxyPhase {
    Idle,
    Waking(WakeOwner),
    Signaled,
    NeedsWake,
}

enum WakeOwner {
    Sender(EntryToken),
    Drain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Immutable capacity policy for an [`AppProxy`] staging queue.
///
/// [`Default::default`] retains at most 65,536 inputs.
pub struct QueuePolicy {
    capacity: usize,
}

impl QueuePolicy {
    #[must_use]
    /// Limits a proxy to `capacity` queued inputs; zero rejects every send.
    pub const fn bounded(capacity: usize) -> Self {
        Self { capacity }
    }

    #[must_use]
    /// Returns the maximum number of inputs the proxy may retain at once.
    pub const fn capacity(&self) -> usize {
        self.capacity
    }
}

impl Default for QueuePolicy {
    fn default() -> Self {
        Self::bounded(65_536)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
/// Classifies a proxy rejection while preserving the rejected input.
pub enum AppProxyErrorCode {
    /// The immutable queue capacity was already reached.
    QueueOverflow,
    /// The host wake bridge rejected scheduling a future drain turn.
    WakeFailed,
}

#[derive(Clone, Debug)]
/// A lossless proxy rejection with private diagnostic state.
pub struct AppProxyError<Input> {
    code: AppProxyErrorCode,
    capacity: Option<usize>,
    rejected: ProxyInput<Input>,
    wake_error: Option<WakeError>,
}

impl<Input> AppProxyError<Input> {
    fn queue_overflow(capacity: usize, rejected: ProxyInput<Input>) -> Self {
        Self {
            code: AppProxyErrorCode::QueueOverflow,
            capacity: Some(capacity),
            rejected,
            wake_error: None,
        }
    }

    fn wake_failed(rejected: ProxyInput<Input>, wake_error: WakeError) -> Self {
        Self {
            code: AppProxyErrorCode::WakeFailed,
            capacity: None,
            rejected,
            wake_error: Some(wake_error),
        }
    }

    #[must_use]
    /// Returns why the proxy rejected the input.
    pub const fn code(&self) -> AppProxyErrorCode {
        self.code
    }

    #[must_use]
    /// Returns the capacity for an overflow rejection.
    pub const fn capacity(&self) -> Option<usize> {
        self.capacity
    }

    #[must_use]
    /// Borrows the exact task or service input that was not queued.
    pub fn rejected(&self) -> &ProxyInput<Input> {
        &self.rejected
    }

    #[must_use]
    /// Consumes this error and recovers the exact task or service input without a `Debug` bound.
    pub fn into_rejected(self) -> ProxyInput<Input> {
        self.rejected
    }

    #[must_use]
    /// Returns the host wake failure when wake scheduling caused the rejection.
    pub fn wake_error(&self) -> Option<&WakeError> {
        self.wake_error.as_ref()
    }
}

impl<Input> fmt::Display for AppProxyError<Input> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.code {
            AppProxyErrorCode::QueueOverflow => {
                write!(
                    formatter,
                    "app proxy queue overflow at capacity {}",
                    self.capacity.unwrap_or(0)
                )
            }
            AppProxyErrorCode::WakeFailed => {
                formatter.write_str("native wake bridge failed")?;
                if let Some(wake_error) = &self.wake_error {
                    write!(formatter, ": {wake_error}")?;
                }
                Ok(())
            }
        }
    }
}

impl<Input: fmt::Debug> Error for AppProxyError<Input> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.wake_error.as_ref().map(|error| error as &dyn Error)
    }
}

#[must_use]
/// The result of one host-facing proxy drain.
///
/// It owns the FIFO inputs transferred out of the proxy and records whether the
/// root adapter must arrange another turn after a continuation-wake failure.
pub struct ProxyDrainReport<Input> {
    drained: Vec<ProxyInput<Input>>,
    remaining_len: usize,
    continuation_wake_error: Option<WakeError>,
}

impl<Input> ProxyDrainReport<Input> {
    fn new(
        drained: Vec<ProxyInput<Input>>,
        remaining_len: usize,
        continuation_wake_error: Option<WakeError>,
    ) -> Self {
        Self {
            drained,
            remaining_len,
            continuation_wake_error,
        }
    }

    #[must_use]
    /// Borrows the inputs removed in FIFO order during this drain.
    pub fn drained(&self) -> &[ProxyInput<Input>] {
        &self.drained
    }

    #[must_use]
    /// Consumes the report and returns the drained inputs in FIFO order.
    pub fn into_drained(self) -> Vec<ProxyInput<Input>> {
        self.drained
    }

    #[must_use]
    /// Returns how many inputs remained queued after this drain.
    pub const fn remaining_len(&self) -> usize {
        self.remaining_len
    }

    #[must_use]
    /// Returns whether the proxy retained inputs for a later host turn.
    pub const fn has_remaining(&self) -> bool {
        self.remaining_len != 0
    }

    #[must_use]
    /// Borrows the failed continuation wake, when queued inputs remain unsignaled.
    pub fn continuation_wake_error(&self) -> Option<&WakeError> {
        self.continuation_wake_error.as_ref()
    }
}
