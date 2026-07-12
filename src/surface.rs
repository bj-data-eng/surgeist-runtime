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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurfaceInvalidation {
    generation: SurfaceInvalidationGeneration,
    kind: SurfaceInvalidationKind,
}

impl SurfaceInvalidation {
    #[must_use]
    pub const fn generation(&self) -> SurfaceInvalidationGeneration {
        self.generation
    }

    #[must_use]
    pub const fn kind(&self) -> &SurfaceInvalidationKind {
        &self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SurfaceInvalidationKind {
    RootReplaced {
        surface_generation: SurfaceGeneration,
    },
    SnapshotChanged {
        version: StateVersion,
    },
    ViewportChanged,
    SurfaceChanged,
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

    pub fn replace_root(&mut self, root: SurfaceRoot) -> Result<SurfaceGeneration, SurfaceError> {
        let generation = self
            .generation
            .checked_next()
            .map_err(|_| SurfaceError::version_overflow())?;
        let invalidation_generation = self.next_invalidation_generation()?;

        self.root = root;
        self.generation = generation;
        self.focused = None;
        self.hovered = None;
        self.push_invalidation(
            invalidation_generation,
            SurfaceInvalidationKind::RootReplaced {
                surface_generation: generation,
            },
        );
        Ok(generation)
    }

    pub fn element_ref(&self, element_id: ElementId) -> Result<SurfaceElementRef, SurfaceError> {
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
        if self.viewport == viewport {
            return Ok(SurfaceMutation::unchanged());
        }
        self.apply_change(SurfaceInvalidationKind::ViewportChanged, |surface| {
            surface.viewport = viewport;
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

    fn apply_change(
        &mut self,
        kind: SurfaceInvalidationKind,
        change: impl FnOnce(&mut Self),
    ) -> Result<SurfaceMutation, SurfaceError> {
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
