use super::InputProvenance;

#[derive(Clone, Debug, Eq, PartialEq)]
/// One application payload paired with the provenance that explains its origin.
pub struct AppInput<T> {
    payload: T,
    provenance: InputProvenance,
}

impl<T> AppInput<T> {
    #[must_use]
    /// Stores an owned payload and its already-established input provenance.
    pub fn new(payload: T, provenance: InputProvenance) -> Self {
        Self {
            payload,
            provenance,
        }
    }

    #[must_use]
    /// Borrows the payload without separating it from this input's provenance.
    pub const fn payload(&self) -> &T {
        &self.payload
    }

    #[must_use]
    /// Consumes this input and returns the owned payload, discarding its provenance.
    pub fn into_payload(self) -> T {
        self.payload
    }

    #[must_use]
    /// Borrows the provenance attached when this input was created.
    pub const fn provenance(&self) -> &InputProvenance {
        &self.provenance
    }
}
