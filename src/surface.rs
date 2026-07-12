use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use super::{
    ElementId, RootId, StateVersion, SurfaceGeneration, SurfaceId, SurfaceInvalidationGeneration,
    VersionError, WindowId,
};
use crate::ids::CheckedNext;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurfaceRoot {
    id: RootId,
    elements: SurfaceElements,
}

impl SurfaceRoot {
    #[must_use]
    pub fn new(id: RootId) -> Self {
        Self {
            id,
            elements: SurfaceElements::default(),
        }
    }

    #[must_use]
    pub const fn id(&self) -> &RootId {
        &self.id
    }

    pub fn register_element(
        &mut self,
        registration: ElementRegistration,
    ) -> Result<(), SurfaceError> {
        if self.elements.registrations.contains_key(&registration.id) {
            return Err(SurfaceError::new(
                SurfaceErrorCode::DuplicateElement,
                "surface root already contains this element",
            ));
        }

        self.elements
            .registrations
            .insert(registration.id, registration);
        Ok(())
    }

    #[must_use]
    pub const fn elements(&self) -> &SurfaceElements {
        &self.elements
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SurfaceElements {
    registrations: BTreeMap<ElementId, ElementRegistration>,
}

impl SurfaceElements {
    #[must_use]
    pub fn get(&self, element_id: ElementId) -> Option<&ElementRegistration> {
        self.registrations.get(&element_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &ElementRegistration> {
        self.registrations.values()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ElementRegistration {
    id: ElementId,
    phases: BTreeSet<ElementPhase>,
}

impl ElementRegistration {
    pub fn try_new(
        id: ElementId,
        phases: impl IntoIterator<Item = ElementPhase>,
    ) -> Result<Self, SurfaceError> {
        let phases = phases.into_iter().collect::<BTreeSet<_>>();
        if phases.is_empty() {
            return Err(SurfaceError::new(
                SurfaceErrorCode::MissingElementPhase,
                "element registration requires at least one phase",
            ));
        }

        Ok(Self { id, phases })
    }

    #[must_use]
    pub const fn id(&self) -> ElementId {
        self.id
    }

    pub fn phases(&self) -> impl Iterator<Item = ElementPhase> + '_ {
        self.phases.iter().copied()
    }

    fn supports(&self, phase: ElementPhase) -> bool {
        self.phases.contains(&phase)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ElementPhase {
    Capture,
    Target,
    Bubble,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SurfaceRef {
    surface_id: SurfaceId,
    generation: SurfaceGeneration,
}

impl SurfaceRef {
    #[must_use]
    pub const fn new(surface_id: SurfaceId, generation: SurfaceGeneration) -> Self {
        Self {
            surface_id,
            generation,
        }
    }

    #[must_use]
    pub const fn surface_id(&self) -> SurfaceId {
        self.surface_id
    }

    #[must_use]
    pub const fn generation(&self) -> SurfaceGeneration {
        self.generation
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SurfaceElementRef {
    surface: SurfaceRef,
    element_id: ElementId,
}

impl SurfaceElementRef {
    #[must_use]
    pub const fn new(surface: SurfaceRef, element_id: ElementId) -> Self {
        Self {
            surface,
            element_id,
        }
    }

    #[must_use]
    pub const fn surface(&self) -> SurfaceRef {
        self.surface
    }

    #[must_use]
    pub const fn surface_id(&self) -> SurfaceId {
        self.surface.surface_id()
    }

    #[must_use]
    pub const fn generation(&self) -> SurfaceGeneration {
        self.surface.generation()
    }

    #[must_use]
    pub const fn element_id(&self) -> ElementId {
        self.element_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurfaceRoute {
    surface: SurfaceRef,
    steps: Vec<SurfaceRouteStep>,
}

impl SurfaceRoute {
    pub fn try_new(
        surface: SurfaceRef,
        steps: impl IntoIterator<Item = SurfaceRouteStep>,
    ) -> Result<Self, SurfaceError> {
        let steps = steps.into_iter().collect::<Vec<_>>();
        if steps.is_empty() {
            return Err(SurfaceError::new(
                SurfaceErrorCode::EmptyRoute,
                "surface route requires at least one step",
            ));
        }

        let target_indices = steps
            .iter()
            .enumerate()
            .filter_map(|(index, step)| (step.phase == ElementPhase::Target).then_some(index))
            .collect::<Vec<_>>();
        let Some(target_index) = target_indices.first().copied() else {
            return Err(SurfaceError::new(
                SurfaceErrorCode::MissingRouteTarget,
                "surface route requires one target step",
            ));
        };
        if target_indices.len() > 1 {
            return Err(SurfaceError::new(
                SurfaceErrorCode::MultipleRouteTargets,
                "surface route permits exactly one target step",
            ));
        }
        if steps.iter().enumerate().any(|(index, step)| {
            (step.phase == ElementPhase::Capture && index > target_index)
                || (step.phase == ElementPhase::Bubble && index < target_index)
        }) {
            return Err(SurfaceError::new(
                SurfaceErrorCode::InvalidRoutePhaseOrder,
                "surface route phases must be capture, target, then bubble",
            ));
        }

        Ok(Self { surface, steps })
    }

    #[must_use]
    pub const fn surface(&self) -> SurfaceRef {
        self.surface
    }

    #[must_use]
    pub const fn surface_id(&self) -> SurfaceId {
        self.surface.surface_id()
    }

    #[must_use]
    pub const fn generation(&self) -> SurfaceGeneration {
        self.surface.generation()
    }

    #[must_use]
    pub fn steps(&self) -> &[SurfaceRouteStep] {
        &self.steps
    }

    #[must_use]
    pub fn target(&self) -> SurfaceElementRef {
        let step = self
            .steps
            .iter()
            .find(|step| step.phase == ElementPhase::Target)
            .expect("checked route contains exactly one target");
        SurfaceElementRef::new(self.surface, step.element_id)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SurfaceRouteStep {
    element_id: ElementId,
    phase: ElementPhase,
}

impl SurfaceRouteStep {
    #[must_use]
    pub const fn new(element_id: ElementId, phase: ElementPhase) -> Self {
        Self { element_id, phase }
    }

    #[must_use]
    pub const fn element_id(&self) -> ElementId {
        self.element_id
    }

    #[must_use]
    pub const fn phase(&self) -> ElementPhase {
        self.phase
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SurfaceSize {
    width: u32,
    height: u32,
}

impl SurfaceSize {
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SurfacePoint {
    x: i32,
    y: i32,
}

impl SurfacePoint {
    #[must_use]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    #[must_use]
    pub const fn origin() -> Self {
        Self::new(0, 0)
    }

    #[must_use]
    pub const fn x(&self) -> i32 {
        self.x
    }

    #[must_use]
    pub const fn y(&self) -> i32 {
        self.y
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SurfaceMutation {
    changed: bool,
    invalidation_generation: Option<SurfaceInvalidationGeneration>,
    redraw_required: bool,
}

impl SurfaceMutation {
    #[must_use]
    const fn unchanged() -> Self {
        Self {
            changed: false,
            invalidation_generation: None,
            redraw_required: false,
        }
    }

    #[must_use]
    const fn changed_result(
        invalidation_generation: SurfaceInvalidationGeneration,
        redraw_required: bool,
    ) -> Self {
        Self {
            changed: true,
            invalidation_generation: Some(invalidation_generation),
            redraw_required,
        }
    }

    #[must_use]
    pub const fn changed(&self) -> bool {
        self.changed
    }

    #[must_use]
    pub const fn invalidation_generation(&self) -> Option<SurfaceInvalidationGeneration> {
        self.invalidation_generation
    }

    #[must_use]
    pub const fn redraw_required(&self) -> bool {
        self.redraw_required
    }
}

/// The current runtime lifecycle phase of a surface.
///
/// Transitions are validated by [`UiSurface::transition_to`]. Only `Ready` and
/// `Resized` surfaces may render; `Closing`, `Closed`, and `Destroyed` reject
/// local mutation, targeting, invalidation, and rendering operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceLifecycle {
    Created,
    Ready,
    Resized,
    Hidden,
    Occluded,
    Suspended,
    Closing,
    Closed,
    Destroyed,
}

impl SurfaceLifecycle {
    const fn allows(self, next: Self) -> bool {
        match self {
            Self::Created => matches!(
                next,
                Self::Ready | Self::Closing | Self::Closed | Self::Destroyed
            ),
            Self::Ready => matches!(
                next,
                Self::Resized
                    | Self::Hidden
                    | Self::Occluded
                    | Self::Suspended
                    | Self::Closing
                    | Self::Closed
                    | Self::Destroyed
            ),
            Self::Resized => matches!(
                next,
                Self::Ready
                    | Self::Hidden
                    | Self::Occluded
                    | Self::Suspended
                    | Self::Closing
                    | Self::Closed
                    | Self::Destroyed
            ),
            Self::Hidden => matches!(
                next,
                Self::Ready | Self::Closing | Self::Closed | Self::Destroyed
            ),
            Self::Occluded => matches!(
                next,
                Self::Ready
                    | Self::Hidden
                    | Self::Suspended
                    | Self::Closing
                    | Self::Closed
                    | Self::Destroyed
            ),
            Self::Suspended => matches!(
                next,
                Self::Ready | Self::Hidden | Self::Closing | Self::Closed | Self::Destroyed
            ),
            Self::Closing => matches!(next, Self::Closed | Self::Destroyed),
            Self::Closed => matches!(next, Self::Destroyed),
            Self::Destroyed => false,
        }
    }

    const fn is_terminal(self) -> bool {
        matches!(self, Self::Closing | Self::Closed | Self::Destroyed)
    }
}

/// One generation-qualified reason that a surface needs rendering work.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurfaceInvalidation {
    generation: SurfaceInvalidationGeneration,
    kind: SurfaceInvalidationKind,
}

impl SurfaceInvalidation {
    /// Returns this invalidation's monotonically issued per-surface generation.
    #[must_use]
    pub const fn generation(&self) -> SurfaceInvalidationGeneration {
        self.generation
    }

    /// Returns the change that made rendering work necessary.
    #[must_use]
    pub const fn kind(&self) -> &SurfaceInvalidationKind {
        &self.kind
    }
}

/// The kind of state change represented by a [`SurfaceInvalidation`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SurfaceInvalidationKind {
    /// A root replacement created a new [`SurfaceGeneration`].
    RootReplaced {
        surface_generation: SurfaceGeneration,
    },
    /// A newer application state snapshot is available for rendering.
    SnapshotChanged { version: StateVersion },
    /// The viewport size changed.
    ViewportChanged,
    /// Local surface interaction state changed.
    SurfaceChanged,
}

/// Immutable metadata captured when a render begins for one surface.
///
/// Frames are issued only by crate-private surface operations. Their accessors
/// let a renderer identify the registered surface, state revision, and the
/// highest invalidation generation visible when rendering started.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SurfaceRenderFrame {
    surface: SurfaceRef,
    state_version: StateVersion,
    invalidation_generation: Option<SurfaceInvalidationGeneration>,
}

impl SurfaceRenderFrame {
    /// Returns the generation-qualified surface rendered by this frame.
    #[must_use]
    pub const fn surface(&self) -> SurfaceRef {
        self.surface
    }

    /// Returns the application state version captured for this frame.
    #[must_use]
    pub const fn state_version(&self) -> StateVersion {
        self.state_version
    }

    /// Returns the highest invalidation generation visible when the frame began.
    #[must_use]
    pub const fn invalidation_generation(&self) -> Option<SurfaceInvalidationGeneration> {
        self.invalidation_generation
    }

    #[cfg(test)]
    pub(crate) const fn new_for_test(
        surface: SurfaceRef,
        state_version: StateVersion,
        invalidation_generation: Option<SurfaceInvalidationGeneration>,
    ) -> Self {
        Self {
            surface,
            state_version,
            invalidation_generation,
        }
    }
}

/// An immutable application-state view paired with its render frame metadata.
#[derive(Debug)]
pub struct SurfaceRenderState<'a, State> {
    state: &'a State,
    frame: SurfaceRenderFrame,
}

impl<'a, State> SurfaceRenderState<'a, State> {
    /// Returns the immutable application state protected by this render view.
    #[must_use]
    pub const fn state(&self) -> &'a State {
        self.state
    }

    /// Returns metadata for the render work associated with this state view.
    #[must_use]
    pub const fn frame(&self) -> &SurfaceRenderFrame {
        &self.frame
    }

    /// Consumes the state view and releases its state borrow, returning the frame.
    #[must_use]
    pub fn into_frame(self) -> SurfaceRenderFrame {
        self.frame
    }

    #[cfg(test)]
    pub(crate) const fn new_for_test(state: &'a State, frame: SurfaceRenderFrame) -> Self {
        Self { state, frame }
    }
}

/// The result of accepting a render frame acknowledgement.
///
/// The counts describe the invalidation queue after acknowledging the frame. A
/// redraw is required only when work remains and the surface is renderable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SurfaceRenderAck {
    surface: SurfaceRef,
    state_version: StateVersion,
    acknowledged_frame_generation: Option<SurfaceInvalidationGeneration>,
    consumed_invalidations: usize,
    remaining_invalidations: usize,
    redraw_required: bool,
}

impl SurfaceRenderAck {
    /// Returns the generation-qualified surface that accepted the frame.
    #[must_use]
    pub const fn surface(&self) -> SurfaceRef {
        self.surface
    }

    /// Returns the acknowledged application state version.
    #[must_use]
    pub const fn state_version(&self) -> StateVersion {
        self.state_version
    }

    /// Returns the highest invalidation generation captured by the frame.
    #[must_use]
    pub const fn acknowledged_frame_generation(&self) -> Option<SurfaceInvalidationGeneration> {
        self.acknowledged_frame_generation
    }

    /// Returns the number of invalidations consumed by this acknowledgement.
    #[must_use]
    pub const fn consumed_invalidations(&self) -> usize {
        self.consumed_invalidations
    }

    /// Returns the number of invalidations remaining after acknowledgement.
    #[must_use]
    pub const fn remaining_invalidations(&self) -> usize {
        self.remaining_invalidations
    }

    /// Reports whether remaining work requires another render now.
    #[must_use]
    pub const fn redraw_required(&self) -> bool {
        self.redraw_required
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SurfaceErrorCode {
    DuplicateElement,
    MissingElementPhase,
    DuplicateSurface,
    UnknownSurface,
    InvalidLifecycleTransition,
    TerminalSurface,
    SurfaceMismatch,
    StaleSurfaceGeneration,
    UnknownElement,
    IneligibleElementTarget,
    EmptyRoute,
    MissingRouteTarget,
    MultipleRouteTargets,
    InvalidRoutePhaseOrder,
    StaleRenderAck,
    VersionOverflow,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurfaceError {
    code: SurfaceErrorCode,
    message: String,
    source: Option<VersionError>,
}

impl SurfaceError {
    fn new(code: SurfaceErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            source: None,
        }
    }

    fn version_overflow() -> Self {
        Self {
            code: SurfaceErrorCode::VersionOverflow,
            message: "surface version overflow".to_owned(),
            source: Some(VersionError::Overflow),
        }
    }

    #[must_use]
    pub const fn code(&self) -> SurfaceErrorCode {
        self.code
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for SurfaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for SurfaceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_ref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

#[derive(Debug)]
pub struct UiSurface {
    id: SurfaceId,
    window_id: WindowId,
    root: SurfaceRoot,
    generation: SurfaceGeneration,
    lifecycle: SurfaceLifecycle,
    viewport: SurfaceSize,
    scroll_offset: SurfacePoint,
    focused: Option<SurfaceElementRef>,
    hovered: Option<SurfaceElementRef>,
    invalidations: Vec<SurfaceInvalidation>,
    last_invalidation_generation: Option<SurfaceInvalidationGeneration>,
    last_rendered_state_version: Option<StateVersion>,
    last_rendered_invalidation_generation: Option<SurfaceInvalidationGeneration>,
}

impl UiSurface {
    pub fn try_new(
        id: SurfaceId,
        window_id: WindowId,
        root: SurfaceRoot,
    ) -> Result<Self, SurfaceError> {
        Ok(Self {
            id,
            window_id,
            root,
            generation: SurfaceGeneration::initial(),
            lifecycle: SurfaceLifecycle::Created,
            viewport: SurfaceSize::default(),
            scroll_offset: SurfacePoint::origin(),
            focused: None,
            hovered: None,
            invalidations: Vec::new(),
            last_invalidation_generation: None,
            last_rendered_state_version: None,
            last_rendered_invalidation_generation: None,
        })
    }

    #[must_use]
    pub const fn id(&self) -> SurfaceId {
        self.id
    }

    #[must_use]
    pub const fn window_id(&self) -> WindowId {
        self.window_id
    }

    #[must_use]
    pub const fn generation(&self) -> SurfaceGeneration {
        self.generation
    }

    #[must_use]
    pub const fn surface_ref(&self) -> SurfaceRef {
        SurfaceRef::new(self.id, self.generation)
    }

    #[must_use]
    pub const fn root(&self) -> &SurfaceRoot {
        &self.root
    }

    #[must_use]
    pub const fn lifecycle(&self) -> SurfaceLifecycle {
        self.lifecycle
    }

    #[must_use]
    pub const fn viewport(&self) -> SurfaceSize {
        self.viewport
    }

    #[must_use]
    pub const fn scroll_offset(&self) -> SurfacePoint {
        self.scroll_offset
    }

    #[must_use]
    pub const fn focused_element(&self) -> Option<SurfaceElementRef> {
        self.focused
    }

    #[must_use]
    pub const fn hovered_element(&self) -> Option<SurfaceElementRef> {
        self.hovered
    }

    #[must_use]
    pub fn invalidations(&self) -> &[SurfaceInvalidation] {
        &self.invalidations
    }

    /// Moves the surface to an allowed lifecycle phase.
    ///
    /// The transition matrix is closed: every transition not listed by
    /// [`SurfaceLifecycle`] is rejected with
    /// [`SurfaceErrorCode::InvalidLifecycleTransition`] without changing state.
    pub fn transition_to(
        &mut self,
        next: SurfaceLifecycle,
    ) -> Result<SurfaceLifecycle, SurfaceError> {
        if !self.lifecycle.allows(next) {
            return Err(SurfaceError::new(
                SurfaceErrorCode::InvalidLifecycleTransition,
                "surface lifecycle transition is not allowed",
            ));
        }

        self.lifecycle = next;
        Ok(next)
    }

    /// Transitions the surface to [`SurfaceLifecycle::Ready`].
    pub fn ready(&mut self) -> Result<SurfaceLifecycle, SurfaceError> {
        self.transition_to(SurfaceLifecycle::Ready)
    }

    /// Transitions the surface to [`SurfaceLifecycle::Resized`].
    pub fn resized(&mut self) -> Result<SurfaceLifecycle, SurfaceError> {
        self.transition_to(SurfaceLifecycle::Resized)
    }

    /// Transitions the surface to [`SurfaceLifecycle::Hidden`].
    pub fn hidden(&mut self) -> Result<SurfaceLifecycle, SurfaceError> {
        self.transition_to(SurfaceLifecycle::Hidden)
    }

    /// Transitions the surface to [`SurfaceLifecycle::Occluded`].
    pub fn occluded(&mut self) -> Result<SurfaceLifecycle, SurfaceError> {
        self.transition_to(SurfaceLifecycle::Occluded)
    }

    /// Transitions the surface to [`SurfaceLifecycle::Suspended`].
    pub fn suspended(&mut self) -> Result<SurfaceLifecycle, SurfaceError> {
        self.transition_to(SurfaceLifecycle::Suspended)
    }

    /// Transitions the surface to [`SurfaceLifecycle::Closing`].
    pub fn closing(&mut self) -> Result<SurfaceLifecycle, SurfaceError> {
        self.transition_to(SurfaceLifecycle::Closing)
    }

    /// Transitions the surface to [`SurfaceLifecycle::Closed`].
    pub fn closed(&mut self) -> Result<SurfaceLifecycle, SurfaceError> {
        self.transition_to(SurfaceLifecycle::Closed)
    }

    /// Transitions the surface to [`SurfaceLifecycle::Destroyed`].
    pub fn destroyed(&mut self) -> Result<SurfaceLifecycle, SurfaceError> {
        self.transition_to(SurfaceLifecycle::Destroyed)
    }

    pub fn replace_root(&mut self, root: SurfaceRoot) -> Result<SurfaceGeneration, SurfaceError> {
        self.ensure_not_terminal()?;
        let generation = self
            .generation
            .checked_next()
            .map_err(|_| SurfaceError::version_overflow())?;
        let invalidation_generation = self.next_invalidation_generation()?;

        self.root = root;
        self.generation = generation;
        self.focused = None;
        self.hovered = None;
        self.last_rendered_invalidation_generation = None;
        self.push_invalidation(
            invalidation_generation,
            SurfaceInvalidationKind::RootReplaced {
                surface_generation: generation,
            },
        );
        Ok(generation)
    }

    pub fn element_ref(&self, element_id: ElementId) -> Result<SurfaceElementRef, SurfaceError> {
        self.ensure_not_terminal()?;
        let reference = SurfaceElementRef::new(self.surface_ref(), element_id);
        self.validate_element_ref(reference)?;
        Ok(reference)
    }

    pub(crate) fn validate_element_ref(
        &self,
        reference: SurfaceElementRef,
    ) -> Result<(), SurfaceError> {
        if reference.surface_id() != self.id {
            return Err(SurfaceError::new(
                SurfaceErrorCode::SurfaceMismatch,
                "element reference belongs to another surface",
            ));
        }
        if reference.generation() != self.generation {
            return Err(SurfaceError::new(
                SurfaceErrorCode::StaleSurfaceGeneration,
                "element reference uses a stale surface generation",
            ));
        }
        self.ensure_not_terminal()?;
        if self.root.elements().get(reference.element_id()).is_none() {
            return Err(SurfaceError::new(
                SurfaceErrorCode::UnknownElement,
                "element reference is not registered on this surface",
            ));
        }
        Ok(())
    }

    pub(crate) fn validate_element(
        &self,
        reference: SurfaceElementRef,
        phase: ElementPhase,
    ) -> Result<(), SurfaceError> {
        self.validate_element_ref(reference)?;
        if !self
            .root
            .elements()
            .get(reference.element_id())
            .expect("validated element reference is registered")
            .supports(phase)
        {
            return Err(SurfaceError::new(
                SurfaceErrorCode::IneligibleElementTarget,
                "element does not support the requested route phase",
            ));
        }
        Ok(())
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "C03 Runtime registry validation will delegate routes to this C01 local validator"
        )
    )]
    pub(crate) fn validate_route(
        &self,
        route: &SurfaceRoute,
    ) -> Result<SurfaceElementRef, SurfaceError> {
        if route.surface_id() != self.id {
            return Err(SurfaceError::new(
                SurfaceErrorCode::SurfaceMismatch,
                "route belongs to another surface",
            ));
        }
        if route.generation() != self.generation {
            return Err(SurfaceError::new(
                SurfaceErrorCode::StaleSurfaceGeneration,
                "route uses a stale surface generation",
            ));
        }
        for step in route.steps() {
            self.validate_element(
                SurfaceElementRef::new(route.surface(), step.element_id()),
                step.phase(),
            )?;
        }
        Ok(route.target())
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "C03 Runtime registry mutation will delegate viewport updates to this C01 local primitive"
        )
    )]
    pub(crate) fn set_viewport(
        &mut self,
        viewport: SurfaceSize,
    ) -> Result<SurfaceMutation, SurfaceError> {
        self.ensure_not_terminal()?;
        if !matches!(
            self.lifecycle,
            SurfaceLifecycle::Ready | SurfaceLifecycle::Resized
        ) {
            return Err(SurfaceError::new(
                SurfaceErrorCode::InvalidLifecycleTransition,
                "surface viewport updates require a renderable lifecycle",
            ));
        }
        if self.viewport == viewport {
            return Ok(SurfaceMutation::unchanged());
        }
        self.apply_change(SurfaceInvalidationKind::ViewportChanged, |surface| {
            surface.viewport = viewport;
            surface.lifecycle = SurfaceLifecycle::Resized;
        })
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "C03 Runtime registry mutation will delegate scroll updates to this C01 local primitive"
        )
    )]
    pub(crate) fn set_scroll_offset(
        &mut self,
        scroll_offset: SurfacePoint,
    ) -> Result<SurfaceMutation, SurfaceError> {
        self.ensure_not_terminal()?;
        if self.scroll_offset == scroll_offset {
            return Ok(SurfaceMutation::unchanged());
        }
        self.apply_change(SurfaceInvalidationKind::SurfaceChanged, |surface| {
            surface.scroll_offset = scroll_offset;
        })
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "C03 Runtime registry mutation will delegate focus updates to this C01 local primitive"
        )
    )]
    pub(crate) fn set_focus(
        &mut self,
        focused: Option<SurfaceElementRef>,
    ) -> Result<SurfaceMutation, SurfaceError> {
        self.ensure_not_terminal()?;
        if let Some(reference) = focused {
            self.validate_element_ref(reference)?;
        }
        if self.focused == focused {
            return Ok(SurfaceMutation::unchanged());
        }
        self.apply_change(SurfaceInvalidationKind::SurfaceChanged, |surface| {
            surface.focused = focused;
        })
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "C03 Runtime registry mutation will delegate hover updates to this C01 local primitive"
        )
    )]
    pub(crate) fn set_hover(
        &mut self,
        hovered: Option<SurfaceElementRef>,
    ) -> Result<SurfaceMutation, SurfaceError> {
        self.ensure_not_terminal()?;
        if let Some(reference) = hovered {
            self.validate_element_ref(reference)?;
        }
        if self.hovered == hovered {
            return Ok(SurfaceMutation::unchanged());
        }
        self.apply_change(SurfaceInvalidationKind::SurfaceChanged, |surface| {
            surface.hovered = hovered;
        })
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "C04 reducer commits will record local snapshot invalidations"
        )
    )]
    pub(crate) fn invalidate_snapshot(
        &mut self,
        version: StateVersion,
    ) -> Result<SurfaceMutation, SurfaceError> {
        self.ensure_not_terminal()?;
        self.apply_change(SurfaceInvalidationKind::SnapshotChanged { version }, |_| {})
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "C03 Runtime registry rendering will issue local render frames"
        )
    )]
    pub(crate) fn begin_render(
        &self,
        state_version: StateVersion,
    ) -> Result<SurfaceRenderFrame, SurfaceError> {
        self.ensure_renderable()?;
        Ok(SurfaceRenderFrame {
            surface: self.surface_ref(),
            state_version,
            invalidation_generation: self
                .invalidations
                .last()
                .map(SurfaceInvalidation::generation),
        })
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "C03 Runtime registry rendering will accept local render acknowledgements"
        )
    )]
    pub(crate) fn acknowledge_render(
        &mut self,
        frame: SurfaceRenderFrame,
    ) -> Result<SurfaceRenderAck, SurfaceError> {
        self.validate_frame_surface(frame.surface())?;
        self.ensure_renderable()?;

        if let Some(last_rendered) = self.last_rendered_state_version {
            if frame.state_version() < last_rendered {
                return Err(SurfaceError::new(
                    SurfaceErrorCode::StaleRenderAck,
                    "render acknowledgement has a stale state version",
                ));
            }
            if frame.state_version() == last_rendered
                && frame.invalidation_generation() <= self.last_rendered_invalidation_generation
            {
                return Ok(self.render_ack(frame, 0));
            }
        }

        let captured_generation = frame.invalidation_generation();
        let mut consumed_invalidations = 0;
        self.invalidations.retain(|invalidation| {
            let captured = captured_generation
                .is_some_and(|generation| invalidation.generation() <= generation);
            let represented = !matches!(
                invalidation.kind(),
                SurfaceInvalidationKind::SnapshotChanged { version }
                    if *version > frame.state_version()
            );
            let consumed = captured && represented;
            consumed_invalidations += usize::from(consumed);
            !consumed
        });
        self.last_rendered_state_version = Some(frame.state_version());
        self.last_rendered_invalidation_generation = frame.invalidation_generation();

        Ok(self.render_ack(frame, consumed_invalidations))
    }

    fn apply_change(
        &mut self,
        kind: SurfaceInvalidationKind,
        change: impl FnOnce(&mut Self),
    ) -> Result<SurfaceMutation, SurfaceError> {
        self.ensure_not_terminal()?;
        let invalidation_generation = self.next_invalidation_generation()?;
        change(self);
        self.push_invalidation(invalidation_generation, kind);
        Ok(SurfaceMutation::changed_result(
            invalidation_generation,
            matches!(
                self.lifecycle,
                SurfaceLifecycle::Ready | SurfaceLifecycle::Resized
            ),
        ))
    }

    fn next_invalidation_generation(&self) -> Result<SurfaceInvalidationGeneration, SurfaceError> {
        self.last_invalidation_generation.map_or(
            Ok(SurfaceInvalidationGeneration::initial()),
            |generation| {
                generation
                    .checked_next()
                    .map_err(|_| SurfaceError::version_overflow())
            },
        )
    }

    fn push_invalidation(
        &mut self,
        generation: SurfaceInvalidationGeneration,
        kind: SurfaceInvalidationKind,
    ) {
        self.invalidations
            .push(SurfaceInvalidation { generation, kind });
        self.last_invalidation_generation = Some(generation);
    }

    fn validate_frame_surface(&self, surface: SurfaceRef) -> Result<(), SurfaceError> {
        if surface.surface_id() != self.id {
            return Err(SurfaceError::new(
                SurfaceErrorCode::SurfaceMismatch,
                "render frame belongs to another surface",
            ));
        }
        if surface.generation() != self.generation {
            return Err(SurfaceError::new(
                SurfaceErrorCode::StaleSurfaceGeneration,
                "render frame uses a stale surface generation",
            ));
        }
        Ok(())
    }

    fn ensure_not_terminal(&self) -> Result<(), SurfaceError> {
        if self.lifecycle.is_terminal() {
            return Err(SurfaceError::new(
                SurfaceErrorCode::TerminalSurface,
                "surface is terminal",
            ));
        }
        Ok(())
    }

    fn ensure_renderable(&self) -> Result<(), SurfaceError> {
        if self.lifecycle.is_terminal() {
            return Err(SurfaceError::new(
                SurfaceErrorCode::TerminalSurface,
                "surface is terminal",
            ));
        }
        if !matches!(
            self.lifecycle,
            SurfaceLifecycle::Ready | SurfaceLifecycle::Resized
        ) {
            return Err(SurfaceError::new(
                SurfaceErrorCode::InvalidLifecycleTransition,
                "surface lifecycle is not renderable",
            ));
        }
        Ok(())
    }

    fn render_ack(
        &self,
        frame: SurfaceRenderFrame,
        consumed_invalidations: usize,
    ) -> SurfaceRenderAck {
        SurfaceRenderAck {
            surface: self.surface_ref(),
            state_version: frame.state_version(),
            acknowledged_frame_generation: frame.invalidation_generation(),
            consumed_invalidations,
            remaining_invalidations: self.invalidations.len(),
            redraw_required: !self.invalidations.is_empty()
                && matches!(
                    self.lifecycle,
                    SurfaceLifecycle::Ready | SurfaceLifecycle::Resized
                ),
        }
    }

    #[cfg(test)]
    pub(crate) fn set_generations_for_test(
        &mut self,
        surface_generation: u64,
        invalidation_generation: Option<u64>,
    ) {
        self.generation = SurfaceGeneration::from_u64(surface_generation);
        self.last_invalidation_generation =
            invalidation_generation.map(SurfaceInvalidationGeneration::from_u64);
    }
}
