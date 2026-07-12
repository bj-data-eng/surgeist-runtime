use super::InputProvenance;

#[derive(Clone, Debug, Eq, PartialEq)]
/// A public runtime value with a private representation.
pub struct AppInput<T> {
    payload: T,
    provenance: InputProvenance,
}

impl<T> AppInput<T> {
    #[must_use]
    /// Constructs this runtime value.
    pub fn new(payload: T, provenance: InputProvenance) -> Self {
        Self {
            payload,
            provenance,
        }
    }

    #[must_use]
    /// Performs the associated runtime operation.
    pub const fn payload(&self) -> &T {
        &self.payload
    }

    #[must_use]
    /// Performs the associated runtime operation.
    pub fn into_payload(self) -> T {
        self.payload
    }

    #[must_use]
    /// Performs the associated runtime operation.
    pub const fn provenance(&self) -> &InputProvenance {
        &self.provenance
    }
}
