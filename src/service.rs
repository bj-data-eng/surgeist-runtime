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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MailboxOverflow {
    RejectNewest,
    DropNewest,
    DropOldest,
    CoalesceByKey,
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

    pub fn push(&mut self, message: T) {
        if self.messages.len() < self.policy.capacity() {
            self.messages.push_back(message);
            return;
        }

        self.record_overflow();
        match self.policy.overflow() {
            MailboxOverflow::RejectNewest
            | MailboxOverflow::DropNewest
            | MailboxOverflow::CoalesceByKey => {}
            MailboxOverflow::DropOldest => {
                if self.policy.capacity() > 0 {
                    self.messages.pop_front();
                    self.messages.push_back(message);
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
