macro_rules! string_id {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            #[must_use]
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

macro_rules! numeric_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(u64);

        impl $name {
            #[must_use]
            pub const fn from_u64(value: u64) -> Self {
                Self(value)
            }

            #[must_use]
            pub const fn as_u64(self) -> u64 {
                self.0
            }
        }
    };
}

string_id!(AppId);
string_id!(RootId);
string_id!(ResourceId);
string_id!(TaskName);
string_id!(TaskKey);
string_id!(ServiceId);
string_id!(CustomScopeId);
string_id!(ExpressionId);
string_id!(CalcId);
string_id!(ValueExprId);

numeric_id!(SurfaceId);
numeric_id!(TaskId);
numeric_id!(TaskAttemptId);
numeric_id!(CorrelationId);

impl Default for AppId {
    fn default() -> Self {
        Self::new("app")
    }
}
