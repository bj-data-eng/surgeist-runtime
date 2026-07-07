#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CommandName(String);

impl CommandName {
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
pub struct AppCommand {
    name: CommandName,
}

impl AppCommand {
    #[must_use]
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: CommandName::new(name),
        }
    }

    #[must_use]
    pub fn name(&self) -> &CommandName {
        &self.name
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandDescriptor {
    name: CommandName,
    payload_type: &'static str,
}

impl CommandDescriptor {
    #[must_use]
    pub fn new(name: impl Into<String>, payload_type: &'static str) -> Self {
        Self {
            name: CommandName::new(name),
            payload_type,
        }
    }

    #[must_use]
    pub fn name(&self) -> &CommandName {
        &self.name
    }

    #[must_use]
    pub fn payload_type(&self) -> &'static str {
        self.payload_type
    }
}
