use super::{AppEffect, AppInput, EffectBatch, InputProvenance};

pub trait Reducer<State, Input> {
    fn reduce(&mut self, state: &mut State, input: AppInput<Input>) -> ReducerResult;
}

#[derive(Clone, Debug, Default)]
pub struct ReducerResult {
    changed: bool,
    effects: EffectBatch,
    recoverable_error: Option<String>,
    provenance: Option<InputProvenance>,
}

impl ReducerResult {
    #[must_use]
    pub fn changed() -> Self {
        Self {
            changed: true,
            effects: EffectBatch::new(),
            recoverable_error: None,
            provenance: None,
        }
    }

    #[must_use]
    pub fn unchanged() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn recoverable_failure(error: impl Into<String>) -> Self {
        Self {
            changed: false,
            effects: EffectBatch::new(),
            recoverable_error: Some(error.into()),
            provenance: None,
        }
    }

    #[must_use]
    pub fn with_effect(mut self, effect: AppEffect) -> Self {
        self.effects = self.effects.push(effect);
        self
    }

    #[must_use]
    pub fn with_effects(mut self, effects: EffectBatch) -> Self {
        self.effects = effects;
        self
    }

    #[must_use]
    pub fn with_provenance(mut self, provenance: InputProvenance) -> Self {
        self.provenance = Some(provenance);
        self
    }

    #[must_use]
    pub const fn is_changed(&self) -> bool {
        self.changed
    }

    #[must_use]
    pub fn effects(&self) -> &[AppEffect] {
        self.effects.effects()
    }

    #[must_use]
    pub fn recoverable_error(&self) -> Option<&str> {
        self.recoverable_error.as_deref()
    }

    #[must_use]
    pub const fn provenance(&self) -> Option<&InputProvenance> {
        self.provenance.as_ref()
    }
}
