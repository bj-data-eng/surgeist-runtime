use surgeist_retained as retained;
use surgeist_window as window;

use super::{RootId, StateVersion, SurfaceId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowRoot {
    id: RootId,
    element: retained::Element,
}

impl WindowRoot {
    #[must_use]
    pub fn new(id: RootId) -> Self {
        Self::with_element(id, retained::Element::root())
    }

    #[must_use]
    pub const fn with_element(id: RootId, element: retained::Element) -> Self {
        Self { id, element }
    }

    #[must_use]
    pub fn id(&self) -> &RootId {
        &self.id
    }

    #[must_use]
    pub fn element(&self) -> &retained::Element {
        &self.element
    }

    fn retained_model(&self) -> retained::Model {
        retained::Model::new(self.element.clone()).expect("window root retained element is valid")
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
pub enum SurfaceInvalidation {
    RootReplaced,
    SnapshotChanged(StateVersion),
    ViewportChanged,
    RetainedChanged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SurfaceRetainedRoot {
    surface_id: SurfaceId,
    generation: u64,
    retained_id: retained::Id,
}

impl SurfaceRetainedRoot {
    #[must_use]
    pub const fn surface_id(self) -> SurfaceId {
        self.surface_id
    }

    #[must_use]
    pub const fn generation(self) -> u64 {
        self.generation
    }

    #[must_use]
    pub const fn retained_id(self) -> retained::Id {
        self.retained_id
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SurfaceRetained<'a> {
    model: &'a retained::Model,
    surface_id: SurfaceId,
    generation: u64,
}

impl SurfaceRetained<'_> {
    #[must_use]
    pub fn model(&self) -> &retained::Model {
        self.model
    }

    #[must_use]
    pub fn root(&self) -> SurfaceRetainedRoot {
        SurfaceRetainedRoot {
            surface_id: self.surface_id,
            generation: self.generation,
            retained_id: self.model.root(),
        }
    }
}

#[derive(Debug)]
pub struct UiSurface {
    id: SurfaceId,
    window_id: window::Id,
    root: WindowRoot,
    retained: retained::Model,
    retained_generation: u64,
    lifecycle: SurfaceLifecycle,
    viewport: window::Size,
    invalidations: Vec<SurfaceInvalidation>,
    last_rendered_state_version: Option<StateVersion>,
    hovered: Option<retained::Id>,
    focused: Option<retained::Id>,
    native_focused: bool,
    scroll_offset: window::Point,
}

impl UiSurface {
    #[must_use]
    pub fn new(id: SurfaceId, window_id: window::Id, root: WindowRoot) -> Self {
        let retained = root.retained_model();

        Self {
            id,
            window_id,
            root,
            retained,
            retained_generation: 0,
            lifecycle: SurfaceLifecycle::Created,
            viewport: window::Size::default(),
            invalidations: Vec::new(),
            last_rendered_state_version: None,
            hovered: None,
            focused: None,
            native_focused: false,
            scroll_offset: window::Point::default(),
        }
    }

    #[must_use]
    pub const fn id(&self) -> SurfaceId {
        self.id
    }

    #[must_use]
    pub const fn window_id(&self) -> window::Id {
        self.window_id
    }

    #[must_use]
    pub const fn root(&self) -> &WindowRoot {
        &self.root
    }

    #[must_use]
    pub const fn retained(&self) -> SurfaceRetained<'_> {
        SurfaceRetained {
            model: &self.retained,
            surface_id: self.id,
            generation: self.retained_generation,
        }
    }

    #[must_use]
    pub const fn lifecycle(&self) -> SurfaceLifecycle {
        self.lifecycle
    }

    #[must_use]
    pub const fn viewport(&self) -> window::Size {
        self.viewport
    }

    #[must_use]
    pub fn invalidations(&self) -> &[SurfaceInvalidation] {
        &self.invalidations
    }

    #[must_use]
    pub const fn last_rendered_state_version(&self) -> Option<StateVersion> {
        self.last_rendered_state_version
    }

    #[must_use]
    pub const fn hovered(&self) -> Option<retained::Id> {
        self.hovered
    }

    #[must_use]
    pub const fn focused(&self) -> Option<retained::Id> {
        self.focused
    }

    #[must_use]
    pub const fn native_focused(&self) -> bool {
        self.native_focused
    }

    #[must_use]
    pub const fn scroll_offset(&self) -> window::Point {
        self.scroll_offset
    }

    pub fn ready(&mut self) {
        self.transition(SurfaceLifecycle::Ready);
    }

    pub fn resized(&mut self, viewport: window::Size) {
        if self.lifecycle.is_terminal() {
            return;
        }

        self.viewport = viewport;
        self.transition(SurfaceLifecycle::Resized);
        self.invalidate(SurfaceInvalidation::ViewportChanged);
    }

    pub fn hidden(&mut self) {
        self.transition(SurfaceLifecycle::Hidden);
    }

    pub fn occluded(&mut self) {
        self.transition(SurfaceLifecycle::Occluded);
    }

    pub fn suspended(&mut self) {
        self.transition(SurfaceLifecycle::Suspended);
    }

    pub fn closing(&mut self) {
        self.transition(SurfaceLifecycle::Closing);
    }

    pub fn closed(&mut self) {
        if self.lifecycle == SurfaceLifecycle::Destroyed {
            return;
        }

        self.lifecycle = SurfaceLifecycle::Closed;
    }

    pub fn destroyed(&mut self) {
        self.lifecycle = SurfaceLifecycle::Destroyed;
    }

    pub fn replace_root(&mut self, root: WindowRoot) {
        self.retained = root.retained_model();
        self.retained_generation += 1;
        self.root = root;
        self.hovered = None;
        self.focused = None;
        self.invalidate(SurfaceInvalidation::RootReplaced);
    }

    pub fn invalidate(&mut self, invalidation: SurfaceInvalidation) {
        self.invalidations.push(invalidation);
    }

    pub fn mark_rendered(&mut self, version: StateVersion) {
        self.last_rendered_state_version = Some(version);
    }

    pub fn set_hovered(&mut self, hovered: Option<retained::Id>) {
        self.hovered = hovered;
    }

    pub fn set_focused(&mut self, focused: Option<retained::Id>) {
        self.focused = focused;
    }

    pub fn set_native_focused(&mut self, native_focused: bool) {
        self.native_focused = native_focused;
    }

    pub fn set_scroll_offset(&mut self, scroll_offset: window::Point) {
        self.scroll_offset = scroll_offset;
    }

    fn transition(&mut self, lifecycle: SurfaceLifecycle) {
        if !self.lifecycle.is_terminal() {
            self.lifecycle = lifecycle;
        }
    }
}

impl SurfaceLifecycle {
    #[must_use]
    const fn is_terminal(self) -> bool {
        matches!(self, Self::Closed | Self::Destroyed)
    }
}
