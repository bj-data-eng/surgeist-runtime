use std::fmt;

/// An invalid runtime name or payload type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NameError {
    field: &'static str,
    value: String,
}

impl NameError {
    fn new(field: &'static str, value: String) -> Self {
        Self { field, value }
    }

    /// Returns the semantic field whose value was rejected.
    #[must_use]
    pub const fn field(&self) -> &'static str {
        self.field
    }

    /// Returns the rejected value exactly as supplied.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
}

impl fmt::Display for NameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid runtime name for {}: {:?}",
            self.field, self.value
        )
    }
}

impl std::error::Error for NameError {}

pub(crate) fn validate_name(value: String, field: &'static str) -> Result<String, NameError> {
    if value.trim().is_empty() || value.bytes().any(|byte| byte.is_ascii_control()) {
        return Err(NameError::new(field, value));
    }

    Ok(value)
}

/// A semantic runtime payload type name.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PayloadTypeName(String);

impl PayloadTypeName {
    /// Creates a payload type name after validating its text.
    pub fn try_new(value: impl Into<String>) -> Result<Self, NameError> {
        Self::try_new_for_field(value, "payload_type")
    }

    pub(crate) fn try_new_for_field(
        value: impl Into<String>,
        field: &'static str,
    ) -> Result<Self, NameError> {
        validate_name(value.into(), field).map(Self)
    }

    /// Returns the payload type name text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A semantic runtime command name.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CommandName(String);

impl CommandName {
    /// Creates a command name after validating its text.
    pub fn try_new(value: impl Into<String>) -> Result<Self, NameError> {
        validate_name(value.into(), "command.name").map(Self)
    }

    /// Returns the command name text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A command emitted by the runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppCommand {
    name: CommandName,
}

impl AppCommand {
    /// Creates a command from a validated name.
    #[must_use]
    pub const fn named(name: CommandName) -> Self {
        Self { name }
    }

    /// Returns the command name.
    #[must_use]
    pub fn name(&self) -> &CommandName {
        &self.name
    }
}

/// Declares a command and its semantic payload type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandDescriptor {
    name: CommandName,
    payload_type: PayloadTypeName,
}

impl CommandDescriptor {
    /// Creates a command descriptor after validating its name and payload type.
    pub fn try_new(
        name: impl Into<String>,
        payload_type: impl Into<String>,
    ) -> Result<Self, NameError> {
        Ok(Self {
            name: CommandName::try_new(name)?,
            payload_type: PayloadTypeName::try_new_for_field(payload_type, "command.payload_type")?,
        })
    }

    /// Returns the command name.
    #[must_use]
    pub fn name(&self) -> &CommandName {
        &self.name
    }

    /// Returns the semantic payload type name.
    #[must_use]
    pub fn payload_type(&self) -> &PayloadTypeName {
        &self.payload_type
    }
}
