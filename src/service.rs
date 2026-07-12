use std::collections::{VecDeque, vec_deque};

use super::{AppScope, ServiceId};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ServiceStatus {
    Stopped,
    Starting,
    Running,
    Degraded,
    Stopping,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ServiceStartup {
    Eager,
    Lazy,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ServiceShutdown {
    Immediate,
    DrainThenStop,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ServiceRestart {
    Never,
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
pub struct MailboxPolicy {
    capacity: usize,
    overflow: MailboxOverflow,
    observe_overflow: bool,
}

impl MailboxPolicy {
    #[must_use]
    pub const fn bounded(capacity: usize) -> Self {
        Self {
            capacity,
            overflow: MailboxOverflow::RejectNewest,
            observe_overflow: false,
        }
    }

    #[must_use]
    pub const fn drop_oldest(mut self) -> Self {
        self.overflow = MailboxOverflow::DropOldest;
        self
    }

    #[must_use]
    pub const fn observe_overflow(mut self) -> Self {
        self.observe_overflow = true;
        self
    }

    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    #[must_use]
    pub const fn overflow(&self) -> MailboxOverflow {
        self.overflow
    }

    #[must_use]
    pub const fn observes_overflow(&self) -> bool {
        self.observe_overflow
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
    pub fn with_scope(mut self, scope: AppScope) -> Self {
        self.scope = scope;
        self
    }

    #[must_use]
    pub const fn with_mailbox_policy(mut self, mailbox: MailboxPolicy) -> Self {
        self.mailbox = mailbox;
        self
    }

    #[must_use]
    pub const fn with_startup(mut self, startup: ServiceStartup) -> Self {
        self.startup = startup;
        self
    }

    #[must_use]
    pub const fn with_shutdown(mut self, shutdown: ServiceShutdown) -> Self {
        self.shutdown = shutdown;
        self
    }

    #[must_use]
    pub const fn with_restart(mut self, restart: ServiceRestart) -> Self {
        self.restart = restart;
        self
    }

    #[must_use]
    pub const fn id(&self) -> &ServiceId {
        &self.id
    }

    #[must_use]
    pub const fn scope(&self) -> &AppScope {
        &self.scope
    }

    #[must_use]
    pub const fn mailbox(&self) -> &MailboxPolicy {
        &self.mailbox
    }

    #[must_use]
    pub const fn startup(&self) -> ServiceStartup {
        self.startup
    }

    #[must_use]
    pub const fn shutdown(&self) -> ServiceShutdown {
        self.shutdown
    }

    #[must_use]
    pub const fn restart(&self) -> ServiceRestart {
        self.restart
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ServiceCommandName(String);

impl ServiceCommandName {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceCommandPayload {
    json_text: Box<str>,
}

impl ServiceCommandPayload {
    #[must_use]
    pub fn from_json_text(json_text: impl Into<String>) -> Self {
        Self {
            json_text: json_text.into().into_boxed_str(),
        }
    }

    #[must_use]
    pub fn as_json_text(&self) -> &str {
        &self.json_text
    }
}

#[derive(Clone, Debug)]
pub struct ServiceMailbox<T: Send + 'static> {
    id: ServiceId,
    policy: MailboxPolicy,
    messages: VecDeque<T>,
    overflow_count: usize,
}

impl<T: Send + 'static> ServiceMailbox<T> {
    #[must_use]
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
    pub fn drain(&mut self) -> vec_deque::Drain<'_, T> {
        self.messages.drain(..)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    #[must_use]
    pub const fn overflow_count(&self) -> usize {
        self.overflow_count
    }

    #[must_use]
    pub const fn id(&self) -> &ServiceId {
        &self.id
    }

    #[must_use]
    pub const fn policy(&self) -> &MailboxPolicy {
        &self.policy
    }

    fn record_overflow(&mut self) {
        if self.policy.observes_overflow() {
            self.overflow_count += 1;
        }
    }
}
