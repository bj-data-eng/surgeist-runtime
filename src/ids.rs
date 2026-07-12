use std::{error::Error, fmt, num::NonZeroU64};

macro_rules! string_id {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[doc = concat!("Opaque string identifier for [`", stringify!($name), "`].")]
        pub struct $name(String);

        impl $name {
            #[must_use]
            /// Stores caller-provided identifier text without interpretation.
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            #[must_use]
            /// Returns the identifier text.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

macro_rules! numeric_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[doc = concat!("Opaque numeric identifier for [`", stringify!($name), "`].")]
        pub struct $name(u64);

        impl $name {
            #[must_use]
            /// Creates an identifier from its exact numeric representation.
            pub const fn from_u64(value: u64) -> Self {
                Self(value)
            }

            #[must_use]
            /// Returns the exact numeric representation.
            pub const fn as_u64(self) -> u64 {
                self.0
            }
        }
    };
}

string_id!(AppId);
string_id!(RootId);
string_id!(ResourceId);
string_id!(ServiceId);
string_id!(CustomScopeId);
string_id!(ExpressionId);
string_id!(CalcId);
string_id!(ValueExprId);

numeric_id!(WindowId);
numeric_id!(SurfaceId);
numeric_id!(ElementId);
numeric_id!(SurfaceGeneration);
numeric_id!(SurfaceInvalidationGeneration);
numeric_id!(ResourceGeneration);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// A nonzero identifier for correlating related input causality.
///
/// Construct it with [`Self::try_from_u64`]; zero is rejected so a present
/// correlation can never be ambiguous with absence.
pub struct CorrelationId(NonZeroU64);

impl CorrelationId {
    /// Validates and constructs a nonzero correlation identifier.
    ///
    /// Returns [`CorrelationError::Zero`] for `0`.
    pub fn try_from_u64(value: u64) -> Result<Self, CorrelationError> {
        NonZeroU64::new(value)
            .map(Self)
            .ok_or(CorrelationError::Zero)
    }

    #[must_use]
    /// Returns the validated nonzero numeric value.
    pub const fn get(self) -> u64 {
        self.0.get()
    }

    #[cfg(test)]
    pub(crate) fn from_u64(value: u64) -> Self {
        Self::try_from_u64(value).expect("crate-local correlation IDs must be nonzero")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
/// Rejects an invalid correlation identifier.
pub enum CorrelationError {
    /// The supplied numeric value was zero.
    Zero,
}

impl fmt::Display for CorrelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero => formatter.write_str("correlation ID must be nonzero"),
        }
    }
}

impl Error for CorrelationError {}

impl SurfaceGeneration {
    #[must_use]
    /// Returns generation zero for a newly created surface.
    pub const fn initial() -> Self {
        Self(0)
    }
}

impl SurfaceInvalidationGeneration {
    #[must_use]
    /// Returns invalidation generation zero before the surface records a mutation.
    pub const fn initial() -> Self {
        Self(0)
    }
}

impl ResourceGeneration {
    #[must_use]
    /// Returns generation zero for a newly created resource state.
    pub const fn initial() -> Self {
        Self(0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// A nonzero operation identity issued by a [`ResourceState`](crate::ResourceState).
///
/// Resource state owns issuance; callers can inspect this value but cannot construct
/// one, preventing completion and cancellation calls from inventing an operation.
pub struct ResourceOperationId(NonZeroU64);

impl ResourceOperationId {
    pub(crate) const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }

    #[must_use]
    /// Returns the nonzero numeric token issued for this operation.
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
/// Reports that a checked runtime counter cannot advance further.
pub enum VersionError {
    /// Incrementing a generation or operation counter would overflow its representation.
    Overflow,
}

impl fmt::Display for VersionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Overflow => formatter.write_str("version overflow"),
        }
    }
}

impl Error for VersionError {}

pub(crate) trait CheckedNext: Sized {
    fn checked_next(self) -> Result<Self, VersionError>;
}

macro_rules! checked_generation {
    ($name:ident) => {
        impl CheckedNext for $name {
            fn checked_next(self) -> Result<Self, VersionError> {
                self.0
                    .checked_add(1)
                    .map(Self)
                    .ok_or(VersionError::Overflow)
            }
        }
    };
}

checked_generation!(SurfaceGeneration);
checked_generation!(SurfaceInvalidationGeneration);
checked_generation!(ResourceGeneration);

impl Default for AppId {
    fn default() -> Self {
        Self::new("app")
    }
}
