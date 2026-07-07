#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EventName(String);

impl EventName {
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
pub struct AppEvent {
    name: EventName,
}

impl AppEvent {
    #[must_use]
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: EventName::new(name),
        }
    }

    #[must_use]
    pub fn name(&self) -> &EventName {
        &self.name
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventDescriptor {
    name: EventName,
    payload_type: &'static str,
}

impl EventDescriptor {
    #[must_use]
    pub fn new(name: impl Into<String>, payload_type: &'static str) -> Self {
        Self {
            name: EventName::new(name),
            payload_type,
        }
    }

    #[must_use]
    pub fn name(&self) -> &EventName {
        &self.name
    }

    #[must_use]
    pub fn payload_type(&self) -> &'static str {
        self.payload_type
    }
}
