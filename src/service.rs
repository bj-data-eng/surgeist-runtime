use std::collections::{VecDeque, vec_deque};

use super::{AppScope, ServiceId};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
/// The observed lifecycle state of a service managed by an adapter.
pub enum ServiceStatus {
    /// The service is not running.
    Stopped,
    /// Startup has been requested but has not completed.
    Starting,
    /// The service is available to process mailbox messages.
    Running,
    /// The service remains available but reports degraded operation.
    Degraded,
    /// Shutdown has been requested while the service may still drain work.
    Stopping,
    /// The last startup or operation failed.
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
/// Selects when a registered service should be started.
pub enum ServiceStartup {
    /// Start as part of application startup.
    Eager,
    /// Start only when requested by an adapter.
    Lazy,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
/// Selects how a running service should be stopped.
pub enum ServiceShutdown {
    /// Stop without waiting for queued mailbox work.
    Immediate,
    /// Allow queued mailbox work to drain before stopping.
    DrainThenStop,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
/// Selects whether a failed service should restart.
pub enum ServiceRestart {
    /// Never restart automatically.
    Never,
    /// Restart after a failure.
    OnFailure,
}

/// Specifies how a full service mailbox handles an incoming message.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MailboxOverflow {
    /// Keeps the queued messages and rejects the incoming message.
    RejectNewest,
    /// Removes the oldest queued message and accepts the incoming message.
    DropOldest,
}

/// Reports the exact result of pushing a message into a service mailbox.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MailboxPushOutcome<T> {
    /// The message was appended to the mailbox.
    Accepted,
    /// The mailbox retained its queued messages and rejected this incoming message.
    RejectedNewest(T),
    /// The mailbox evicted its oldest message before accepting the incoming message.
    DroppedOldest {
        /// The message removed from the front of the mailbox.
        dropped: T,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Immutable capacity and overflow behavior for a service mailbox.
pub struct MailboxPolicy {
    capacity: usize,
    overflow: MailboxOverflow,
    observe_overflow: bool,
}

impl MailboxPolicy {
    #[must_use]
    /// Creates a rejecting policy with `capacity` queued message slots.
    pub const fn bounded(capacity: usize) -> Self {
        Self {
            capacity,
            overflow: MailboxOverflow::RejectNewest,
            observe_overflow: false,
        }
    }

    #[must_use]
    /// Changes a policy to evict the oldest message when a nonempty mailbox is full.
    pub const fn drop_oldest(mut self) -> Self {
        self.overflow = MailboxOverflow::DropOldest;
        self
    }

    #[must_use]
    /// Requests overflow counting while retaining the selected overflow behavior.
    pub const fn observe_overflow(mut self) -> Self {
        self.observe_overflow = true;
        self
    }

    #[must_use]
    /// Returns the number of messages this policy may retain.
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    #[must_use]
    /// Returns the full-mailbox behavior.
    pub const fn overflow(&self) -> MailboxOverflow {
        self.overflow
    }

    #[must_use]
    /// Returns whether overflows increment the mailbox counter.
    pub const fn observes_overflow(&self) -> bool {
        self.observe_overflow
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// FIFO mailbox owned by one service registration.
pub struct ServiceRegistration {
    id: ServiceId,
    scope: AppScope,
    mailbox: MailboxPolicy,
    startup: ServiceStartup,
    shutdown: ServiceShutdown,
    restart: ServiceRestart,
}

impl ServiceRegistration {
    #[must_use]
    /// Creates a registration with application scope, a 64-message
    /// [`MailboxOverflow::RejectNewest`] mailbox with overflow observation disabled,
    /// lazy startup, drain-then-stop shutdown, and no automatic restart.
    pub fn new(id: ServiceId) -> Self {
        Self {
            id,
            scope: AppScope::app(),
            mailbox: MailboxPolicy::bounded(64),
            startup: ServiceStartup::Lazy,
            shutdown: ServiceShutdown::DrainThenStop,
            restart: ServiceRestart::Never,
        }
    }

    #[must_use]
    /// Consumes this registration and replaces its application scope.
    pub fn with_scope(mut self, scope: AppScope) -> Self {
        self.scope = scope;
        self
    }

    #[must_use]
    /// Consumes this registration and replaces its immutable mailbox policy.
    pub const fn with_mailbox_policy(mut self, mailbox: MailboxPolicy) -> Self {
        self.mailbox = mailbox;
        self
    }

    #[must_use]
    /// Consumes this registration and replaces its service-startup policy.
    pub const fn with_startup(mut self, startup: ServiceStartup) -> Self {
        self.startup = startup;
        self
    }

    #[must_use]
    /// Consumes this registration and replaces its service-shutdown policy.
    pub const fn with_shutdown(mut self, shutdown: ServiceShutdown) -> Self {
        self.shutdown = shutdown;
        self
    }

    #[must_use]
    /// Consumes this registration and replaces its failed-service restart policy.
    pub const fn with_restart(mut self, restart: ServiceRestart) -> Self {
        self.restart = restart;
        self
    }

    #[must_use]
    /// Borrows the identity of the service this registration describes.
    pub const fn id(&self) -> &ServiceId {
        &self.id
    }

    #[must_use]
    /// Borrows the scope assigned to this service.
    pub const fn scope(&self) -> &AppScope {
        &self.scope
    }

    #[must_use]
    /// Borrows the immutable mailbox policy selected for this service.
    pub const fn mailbox(&self) -> &MailboxPolicy {
        &self.mailbox
    }

    #[must_use]
    /// Returns the selected startup policy by value.
    pub const fn startup(&self) -> ServiceStartup {
        self.startup
    }

    #[must_use]
    /// Returns the selected shutdown policy by value.
    pub const fn shutdown(&self) -> ServiceShutdown {
        self.shutdown
    }

    #[must_use]
    /// Returns the selected restart policy by value.
    pub const fn restart(&self) -> ServiceRestart {
        self.restart
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// An opaque service-command name passed unchanged to the root adapter.
pub struct ServiceCommandName(String);

impl ServiceCommandName {
    #[must_use]
    /// Stores service-command text without parsing or dispatching it.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    /// Borrows the command text for adapter-owned service dispatch.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// An opaque JSON-text payload passed unchanged to a root-owned service adapter.
pub struct ServiceCommandPayload {
    json_text: Box<str>,
}

impl ServiceCommandPayload {
    #[must_use]
    /// Stores owned JSON text without parsing or validating its schema.
    pub fn from_json_text(json_text: impl Into<String>) -> Self {
        Self {
            json_text: json_text.into().into_boxed_str(),
        }
    }

    #[must_use]
    /// Borrows the exact JSON text supplied to the constructor.
    pub fn as_json_text(&self) -> &str {
        &self.json_text
    }
}

#[derive(Clone, Debug)]
/// A bounded FIFO mailbox owned by one service identity.
pub struct ServiceMailbox<T: Send + 'static> {
    id: ServiceId,
    policy: MailboxPolicy,
    messages: VecDeque<T>,
    overflow_count: usize,
}

impl<T: Send + 'static> ServiceMailbox<T> {
    #[must_use]
    /// Creates an empty mailbox with the supplied immutable overflow policy.
    pub fn new(id: ServiceId, policy: MailboxPolicy) -> Self {
        Self {
            id,
            policy,
            messages: VecDeque::new(),
            overflow_count: 0,
        }
    }

    /// Pushes `message` and reports whether it was accepted, rejected, or replaced an older one.
    ///
    /// Accepted messages retain FIFO order. A zero-capacity mailbox always returns
    /// [`MailboxPushOutcome::RejectedNewest`], including with
    /// [`MailboxOverflow::DropOldest`].
    #[must_use]
    pub fn push(&mut self, message: T) -> MailboxPushOutcome<T> {
        if self.messages.len() < self.policy.capacity() {
            self.messages.push_back(message);
            return MailboxPushOutcome::Accepted;
        }

        self.record_overflow();
        match self.policy.overflow() {
            MailboxOverflow::RejectNewest => MailboxPushOutcome::RejectedNewest(message),
            MailboxOverflow::DropOldest => {
                if self.policy.capacity() == 0 {
                    MailboxPushOutcome::RejectedNewest(message)
                } else {
                    let dropped = self
                        .messages
                        .pop_front()
                        .expect("a full mailbox with nonzero capacity contains a message");
                    self.messages.push_back(message);
                    MailboxPushOutcome::DroppedOldest { dropped }
                }
            }
        }
    }

    #[must_use]
    /// Drains all queued messages in FIFO order through a borrowing iterator.
    ///
    /// The mailbox is empty once the returned drain is exhausted or dropped.
    pub fn drain(&mut self) -> vec_deque::Drain<'_, T> {
        self.messages.drain(..)
    }

    #[must_use]
    /// Returns the number of currently queued messages.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    #[must_use]
    /// Returns whether no messages are currently queued.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    #[must_use]
    /// Returns the number of overflows observed when the policy enables counting.
    pub const fn overflow_count(&self) -> usize {
        self.overflow_count
    }

    #[must_use]
    /// Borrows the identity of the service that owns this mailbox.
    pub const fn id(&self) -> &ServiceId {
        &self.id
    }

    #[must_use]
    /// Borrows the immutable policy that governs this mailbox.
    pub const fn policy(&self) -> &MailboxPolicy {
        &self.policy
    }

    fn record_overflow(&mut self) {
        if self.policy.observes_overflow() {
            self.overflow_count += 1;
        }
    }
}
