use super::{AppEffect, AppInput, EffectBatch, InputProvenance};

pub trait Reducer<State, Input> {
    fn reduce(&mut self, state: &State, input: &AppInput<Input>) -> ReducerResult<State>;
}

#[derive(Clone, Debug)]
pub struct ReducerCommit {
    effects: EffectBatch,
    provenance: Option<InputProvenance>,
}

impl ReducerCommit {
    #[must_use]
    pub fn new() -> Self {
        Self {
            effects: EffectBatch::new(),
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
    pub const fn effects(&self) -> &EffectBatch {
        &self.effects
    }

    #[must_use]
    pub const fn provenance(&self) -> Option<&InputProvenance> {
        self.provenance.as_ref()
    }
}

impl Default for ReducerCommit {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct ReducerChange<State> {
    state: State,
    commit: ReducerCommit,
}

impl<State> ReducerChange<State> {
    #[must_use]
    pub fn new(state: State, commit: ReducerCommit) -> Self {
        Self { state, commit }
    }

    #[must_use]
    pub const fn state(&self) -> &State {
        &self.state
    }

    #[must_use]
    pub const fn commit(&self) -> &ReducerCommit {
        &self.commit
    }

    pub(crate) fn into_parts(self) -> (State, ReducerCommit) {
        (self.state, self.commit)
    }
}

#[derive(Clone, Debug)]
pub struct ReducerFailure {
    message: String,
    provenance: Option<InputProvenance>,
}

impl ReducerFailure {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            provenance: None,
        }
    }

    #[must_use]
    pub fn with_provenance(mut self, provenance: InputProvenance) -> Self {
        self.provenance = Some(provenance);
        self
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub const fn provenance(&self) -> Option<&InputProvenance> {
        self.provenance.as_ref()
    }
}

#[derive(Clone, Debug)]
pub enum ReducerResult<State> {
    Unchanged(ReducerCommit),
    Changed(ReducerChange<State>),
    RecoverableFailure(ReducerFailure),
}

impl<State> ReducerResult<State> {
    #[must_use]
    pub fn unchanged(commit: ReducerCommit) -> Self {
        Self::Unchanged(commit)
    }

    #[must_use]
    pub fn changed(state: State, commit: ReducerCommit) -> Self {
        Self::Changed(ReducerChange::new(state, commit))
    }

    #[must_use]
    pub fn recoverable_failure(failure: ReducerFailure) -> Self {
        Self::RecoverableFailure(failure)
    }
}
