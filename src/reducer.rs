use super::{AppEffect, AppInput, EffectBatch, InputProvenance};

/// Reduces an immutable borrowed state and input into an explicit commit result.
///
/// Reducers may mutate their own implementation state, but receive neither the
/// application state nor input by mutable reference.
pub trait Reducer<State, Input> {
    /// Produces an unchanged or changed commit, or a recoverable failure.
    ///
    /// `state` and `input` are borrowed immutably; a changed result owns the
    /// replacement state.
    fn reduce(&mut self, state: &State, input: &AppInput<Input>) -> ReducerResult<State>;
}

/// The effects and optional provenance override for a successful reducer result.
///
/// Effects retain their batch order. A commit with no provenance leaves
/// provenance selection to its consumer.
#[derive(Clone, Debug)]
pub struct ReducerCommit {
    effects: EffectBatch,
    provenance: Option<InputProvenance>,
}

impl ReducerCommit {
    /// Creates an empty commit with no effects or provenance override.
    ///
    /// This is also the value returned by [`Default::default`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            effects: EffectBatch::new(),
            provenance: None,
        }
    }

    /// Appends an effect after every effect already in this commit.
    #[must_use]
    pub fn with_effect(mut self, effect: AppEffect) -> Self {
        self.effects = self.effects.push(effect);
        self
    }

    /// Replaces this commit's ordered effect batch.
    #[must_use]
    pub fn with_effects(mut self, effects: EffectBatch) -> Self {
        self.effects = effects;
        self
    }

    /// Sets the explicit provenance override for this commit.
    #[must_use]
    pub fn with_provenance(mut self, provenance: InputProvenance) -> Self {
        self.provenance = Some(provenance);
        self
    }

    /// Returns the commit's effects in their declared order.
    #[must_use]
    pub const fn effects(&self) -> &EffectBatch {
        &self.effects
    }

    /// Returns the explicit provenance override, when one was set.
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

/// A successful reducer result that owns a replacement application state.
#[derive(Clone, Debug)]
pub struct ReducerChange<State> {
    state: State,
    commit: ReducerCommit,
}

impl<State> ReducerChange<State> {
    /// Combines an owned replacement state with its successful commit.
    #[must_use]
    pub fn new(state: State, commit: ReducerCommit) -> Self {
        Self { state, commit }
    }

    /// Returns the owned replacement state by shared reference.
    #[must_use]
    pub const fn state(&self) -> &State {
        &self.state
    }

    /// Returns the commit accompanying this replacement state.
    #[must_use]
    pub const fn commit(&self) -> &ReducerCommit {
        &self.commit
    }

    pub(crate) fn into_parts(self) -> (State, ReducerCommit) {
        (self.state, self.commit)
    }
}

/// A recoverable reducer failure with no state replacement or effect batch.
///
/// Its structure keeps failure disjoint from successful commits.
#[derive(Clone, Debug)]
pub struct ReducerFailure {
    message: String,
    provenance: Option<InputProvenance>,
}

impl ReducerFailure {
    /// Creates a recoverable failure with no provenance override.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            provenance: None,
        }
    }

    /// Sets the explicit provenance override for this failure.
    #[must_use]
    pub fn with_provenance(mut self, provenance: InputProvenance) -> Self {
        self.provenance = Some(provenance);
        self
    }

    /// Returns the failure message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the explicit provenance override, when one was set.
    #[must_use]
    pub const fn provenance(&self) -> Option<&InputProvenance> {
        self.provenance.as_ref()
    }
}

/// The explicit outcome of reducing one input.
///
/// Successful variants carry an explicit commit. The recoverable-failure variant
/// is structurally disjoint from both replacement state and effects.
#[derive(Clone, Debug)]
pub enum ReducerResult<State> {
    /// A successful commit that retains the current state.
    Unchanged(ReducerCommit),
    /// A successful commit that installs an owned replacement state.
    Changed(ReducerChange<State>),
    /// A recoverable failure that carries neither state nor effects.
    RecoverableFailure(ReducerFailure),
}

impl<State> ReducerResult<State> {
    /// Creates a successful commit that leaves the state unchanged.
    #[must_use]
    pub fn unchanged(commit: ReducerCommit) -> Self {
        Self::Unchanged(commit)
    }

    /// Creates a successful commit with an owned replacement state.
    #[must_use]
    pub fn changed(state: State, commit: ReducerCommit) -> Self {
        Self::Changed(ReducerChange::new(state, commit))
    }

    /// Creates a recoverable failure that cannot carry state or effects.
    #[must_use]
    pub fn recoverable_failure(failure: ReducerFailure) -> Self {
        Self::RecoverableFailure(failure)
    }
}
