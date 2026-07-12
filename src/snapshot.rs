use std::{collections::BTreeMap, error::Error, fmt};

use crate::{
    RootId,
    ids::{CheckedNext, VersionError},
};

/// A monotonically increasing version of application state.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StateVersion(u64);

impl StateVersion {
    /// Returns the initial state version.
    #[must_use]
    pub const fn initial() -> Self {
        Self(0)
    }

    #[must_use]
    #[cfg(test)]
    pub(crate) const fn from_u64(value: u64) -> Self {
        Self(value)
    }

    /// Returns the numeric version value.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl CheckedNext for StateVersion {
    fn checked_next(self) -> Result<Self, VersionError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(VersionError::Overflow)
    }
}

/// The kind of rejected snapshot input or transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SnapshotErrorCode {
    /// A binding identifier is empty or whitespace-only.
    EmptyBindingId,
    /// A binding identifier contains an ASCII control character.
    InvalidBindingId,
    /// A source type is empty or whitespace-only.
    EmptySourceType,
    /// A source type contains an ASCII control character.
    InvalidSourceType,
    /// A serialized value is empty.
    EmptyValue,
    /// A serialized value contains an embedded NUL character.
    InvalidValue,
    /// The requested root is absent from the validated manifest.
    UnknownRoot,
    /// An entry names no binding declared by the snapshot root.
    UndeclaredBinding,
    /// An entry source type differs from the declared binding source type.
    SourceTypeMismatch,
    /// An entry duplicates a binding already present in the snapshot.
    DuplicateBinding,
}

/// A semantic failure while constructing or extending a snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotError {
    code: SnapshotErrorCode,
    field: &'static str,
    root_id: Option<Box<RootId>>,
    binding_id: Option<Box<SnapshotBindingId>>,
    expected_source_type: Option<Box<SnapshotSourceType>>,
    actual_source_type: Option<Box<SnapshotSourceType>>,
    message: String,
}

impl SnapshotError {
    fn new(code: SnapshotErrorCode, field: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            field,
            root_id: None,
            binding_id: None,
            expected_source_type: None,
            actual_source_type: None,
            message: message.into(),
        }
    }

    pub(crate) fn unknown_root(root_id: RootId) -> Self {
        Self::new(
            SnapshotErrorCode::UnknownRoot,
            "snapshot.root_id",
            "snapshot root is not declared by the manifest",
        )
        .with_root_id(root_id)
    }

    fn with_root_id(mut self, root_id: RootId) -> Self {
        self.root_id = Some(Box::new(root_id));
        self
    }

    fn with_binding_id(mut self, binding_id: SnapshotBindingId) -> Self {
        self.binding_id = Some(Box::new(binding_id));
        self
    }

    fn with_source_types(
        mut self,
        expected_source_type: SnapshotSourceType,
        actual_source_type: SnapshotSourceType,
    ) -> Self {
        self.expected_source_type = Some(Box::new(expected_source_type));
        self.actual_source_type = Some(Box::new(actual_source_type));
        self
    }

    /// Returns the rejected snapshot operation kind.
    #[must_use]
    pub const fn code(&self) -> SnapshotErrorCode {
        self.code
    }

    /// Returns the semantic field whose value or transition was rejected.
    #[must_use]
    pub const fn field(&self) -> &'static str {
        self.field
    }

    /// Returns the root involved in this error, when applicable.
    #[must_use]
    pub fn root_id(&self) -> Option<&RootId> {
        self.root_id.as_deref()
    }

    /// Returns the binding involved in this error, when applicable.
    #[must_use]
    pub fn binding_id(&self) -> Option<&SnapshotBindingId> {
        self.binding_id.as_deref()
    }

    /// Returns the source type declared for the binding, when applicable.
    #[must_use]
    pub fn expected_source_type(&self) -> Option<&SnapshotSourceType> {
        self.expected_source_type.as_deref()
    }

    /// Returns the source type supplied by the rejected entry, when applicable.
    #[must_use]
    pub fn actual_source_type(&self) -> Option<&SnapshotSourceType> {
        self.actual_source_type.as_deref()
    }
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} ({})", self.message, self.field)
    }
}

impl Error for SnapshotError {}

/// A validated semantic identifier for a root snapshot binding.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SnapshotBindingId(String);

impl SnapshotBindingId {
    /// Creates a binding identifier after validating its text.
    pub fn try_new(value: impl Into<String>) -> Result<Self, SnapshotError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(SnapshotError::new(
                SnapshotErrorCode::EmptyBindingId,
                "snapshot.binding_id",
                "snapshot binding identifier must not be empty",
            ));
        }
        if value.bytes().any(|byte| byte.is_ascii_control()) {
            return Err(SnapshotError::new(
                SnapshotErrorCode::InvalidBindingId,
                "snapshot.binding_id",
                "snapshot binding identifier contains an ASCII control character",
            ));
        }

        Ok(Self(value))
    }

    /// Returns the preserved binding identifier text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A validated semantic source type declared for a snapshot binding.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SnapshotSourceType(String);

impl SnapshotSourceType {
    /// Creates a source type after validating its text.
    pub fn try_new(value: impl Into<String>) -> Result<Self, SnapshotError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(SnapshotError::new(
                SnapshotErrorCode::EmptySourceType,
                "snapshot.source_type",
                "snapshot source type must not be empty",
            ));
        }
        if value.bytes().any(|byte| byte.is_ascii_control()) {
            return Err(SnapshotError::new(
                SnapshotErrorCode::InvalidSourceType,
                "snapshot.source_type",
                "snapshot source type contains an ASCII control character",
            ));
        }

        Ok(Self(value))
    }

    /// Returns the preserved source type text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Declares one snapshot binding and its semantic source type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotBinding {
    id: SnapshotBindingId,
    source_type: SnapshotSourceType,
}

impl SnapshotBinding {
    /// Creates a binding from validated identifier and source-type values.
    #[must_use]
    pub const fn new(id: SnapshotBindingId, source_type: SnapshotSourceType) -> Self {
        Self { id, source_type }
    }

    /// Returns the binding identifier.
    #[must_use]
    pub fn id(&self) -> &SnapshotBindingId {
        &self.id
    }

    /// Returns the declared source type.
    #[must_use]
    pub fn source_type(&self) -> &SnapshotSourceType {
        &self.source_type
    }
}

/// An opaque serialized snapshot payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotValue {
    serialized_text: Box<str>,
}

impl SnapshotValue {
    /// Creates an opaque serialized value without interpreting its codec or schema.
    pub fn try_new(serialized_text: impl Into<Box<str>>) -> Result<Self, SnapshotError> {
        let serialized_text = serialized_text.into();
        if serialized_text.is_empty() {
            return Err(SnapshotError::new(
                SnapshotErrorCode::EmptyValue,
                "snapshot.value",
                "snapshot value must not be empty",
            ));
        }
        if serialized_text.contains('\0') {
            return Err(SnapshotError::new(
                SnapshotErrorCode::InvalidValue,
                "snapshot.value",
                "snapshot value contains an embedded NUL character",
            ));
        }

        Ok(Self { serialized_text })
    }

    /// Returns the preserved opaque serialized text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.serialized_text
    }
}

/// Associates one validated binding with one opaque serialized value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotEntry {
    binding: SnapshotBinding,
    value: SnapshotValue,
}

impl SnapshotEntry {
    /// Creates an entry from a validated binding and opaque value.
    #[must_use]
    pub const fn new(binding: SnapshotBinding, value: SnapshotValue) -> Self {
        Self { binding, value }
    }

    /// Returns the entry binding.
    #[must_use]
    pub fn binding(&self) -> &SnapshotBinding {
        &self.binding
    }

    /// Returns the entry's opaque serialized value.
    #[must_use]
    pub fn value(&self) -> &SnapshotValue {
        &self.value
    }
}

/// A root-bound collection of declared snapshot values at one state version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppSnapshot {
    root_id: RootId,
    version: StateVersion,
    declarations: BTreeMap<SnapshotBindingId, SnapshotSourceType>,
    entries: Vec<SnapshotEntry>,
}

impl AppSnapshot {
    pub(crate) fn from_declarations(
        root_id: RootId,
        version: StateVersion,
        bindings: &[SnapshotBinding],
    ) -> Self {
        Self {
            root_id,
            version,
            declarations: bindings
                .iter()
                .map(|binding| (binding.id().clone(), binding.source_type().clone()))
                .collect(),
            entries: Vec::new(),
        }
    }

    /// Returns the validated root that declared this snapshot.
    #[must_use]
    pub fn root_id(&self) -> &RootId {
        &self.root_id
    }

    /// Returns the state version represented by this snapshot.
    #[must_use]
    pub const fn version(&self) -> StateVersion {
        self.version
    }

    /// Returns the source type declared for a binding, when that binding exists.
    #[must_use]
    pub fn declaration(&self, binding_id: &SnapshotBindingId) -> Option<&SnapshotSourceType> {
        self.declarations.get(binding_id)
    }

    /// Returns entries in their accepted insertion order.
    #[must_use]
    pub fn entries(&self) -> &[SnapshotEntry] {
        &self.entries
    }

    /// Adds an entry after verifying it against this snapshot's copied declarations.
    pub fn add_entry(&mut self, entry: SnapshotEntry) -> Result<(), SnapshotError> {
        let binding_id = entry.binding().id();
        let Some(expected_source_type) = self.declaration(binding_id) else {
            return Err(SnapshotError::new(
                SnapshotErrorCode::UndeclaredBinding,
                "snapshot.entries",
                "snapshot entry binding is not declared by the root",
            )
            .with_root_id(self.root_id.clone())
            .with_binding_id(binding_id.clone()));
        };
        if expected_source_type != entry.binding().source_type() {
            return Err(SnapshotError::new(
                SnapshotErrorCode::SourceTypeMismatch,
                "snapshot.entries",
                "snapshot entry source type differs from the root declaration",
            )
            .with_root_id(self.root_id.clone())
            .with_binding_id(binding_id.clone())
            .with_source_types(
                expected_source_type.clone(),
                entry.binding().source_type().clone(),
            ));
        }
        if self
            .entries
            .iter()
            .any(|existing| existing.binding().id() == binding_id)
        {
            return Err(SnapshotError::new(
                SnapshotErrorCode::DuplicateBinding,
                "snapshot.entries",
                "snapshot entry binding is already present",
            )
            .with_root_id(self.root_id.clone())
            .with_binding_id(binding_id.clone()));
        }

        self.entries.push(entry);
        Ok(())
    }
}
