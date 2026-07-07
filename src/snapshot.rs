#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StateVersion(u64);

impl StateVersion {
    #[must_use]
    pub const fn initial() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn from_u64(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotBinding {
    id: SnapshotBindingId,
    source_type: SnapshotSourceType,
}

impl SnapshotBinding {
    #[must_use]
    pub const fn new(id: SnapshotBindingId, source_type: SnapshotSourceType) -> Self {
        Self { id, source_type }
    }

    #[must_use]
    pub fn id(&self) -> &SnapshotBindingId {
        &self.id
    }

    #[must_use]
    pub const fn source_type(&self) -> SnapshotSourceType {
        self.source_type
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SnapshotBindingId(String);

impl SnapshotBindingId {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SnapshotSourceType(&'static str);

impl SnapshotSourceType {
    #[must_use]
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppSnapshot {
    version: StateVersion,
    bindings: Vec<SnapshotBinding>,
}

impl AppSnapshot {
    #[must_use]
    pub const fn new(version: StateVersion) -> Self {
        Self {
            version,
            bindings: Vec::new(),
        }
    }

    #[must_use]
    pub const fn version(&self) -> StateVersion {
        self.version
    }

    #[must_use]
    pub fn bindings(&self) -> &[SnapshotBinding] {
        &self.bindings
    }
}
