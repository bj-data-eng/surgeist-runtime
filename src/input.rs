use super::InputProvenance;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppInput<T> {
    payload: T,
    provenance: InputProvenance,
}

impl<T> AppInput<T> {
    #[must_use]
    pub fn new(payload: T, provenance: InputProvenance) -> Self {
        Self {
            payload,
            provenance,
        }
    }

    #[must_use]
    pub const fn payload(&self) -> &T {
        &self.payload
    }

    #[must_use]
    pub fn into_payload(self) -> T {
        self.payload
    }

    #[must_use]
    pub const fn provenance(&self) -> &InputProvenance {
        &self.provenance
    }
}
