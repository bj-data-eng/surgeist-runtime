use super::command::{NameError, PayloadTypeName, validate_name};

/// A semantic runtime event name.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EventName(String);

impl EventName {
    /// Creates an event name after validating its text.
    pub fn try_new(value: impl Into<String>) -> Result<Self, NameError> {
        validate_name(value.into(), "event.name").map(Self)
    }

    /// Returns the event name text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An event accepted by the runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppEvent {
    name: EventName,
}

impl AppEvent {
    /// Creates an event from a validated name.
    #[must_use]
    pub const fn named(name: EventName) -> Self {
        Self { name }
    }

    /// Returns the event name.
    #[must_use]
    pub fn name(&self) -> &EventName {
        &self.name
    }
}

/// Declares an event and its semantic payload type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventDescriptor {
    name: EventName,
    payload_type: PayloadTypeName,
}

impl EventDescriptor {
    /// Creates an event descriptor after validating its name and payload type.
    pub fn try_new(
        name: impl Into<String>,
        payload_type: impl Into<String>,
    ) -> Result<Self, NameError> {
        Ok(Self {
            name: EventName::try_new(name)?,
            payload_type: PayloadTypeName::try_new_for_field(payload_type, "event.payload_type")?,
        })
    }

    /// Returns the event name.
    #[must_use]
    pub fn name(&self) -> &EventName {
        &self.name
    }

    /// Returns the semantic payload type name.
    #[must_use]
    pub fn payload_type(&self) -> &PayloadTypeName {
        &self.payload_type
    }
}
