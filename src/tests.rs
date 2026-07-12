use super::testing::{FakeWakeBridge, HeadlessHarness, PrototypeApp, ServiceRequestStatus};
use super::*;
use crate::ids::CheckedNext;
use std::{
    error::Error,
    num::NonZeroUsize,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

fn correlation(value: u64) -> CorrelationId {
    CorrelationId::try_from_u64(value).expect("test correlation must be nonzero")
}

fn surface_ref(surface_id: u64, generation: u64) -> SurfaceRef {
    SurfaceRef::new(
        SurfaceId::from_u64(surface_id),
        SurfaceGeneration::from_u64(generation),
    )
}

fn assert_empty_causality(provenance: &InputProvenance) {
    assert_eq!(provenance.correlation(), Correlation::Absent);
    assert_eq!(provenance.parent_correlation(), Correlation::Absent);
    assert_eq!(provenance.sequence(), None);
}

fn assert_surface_error(
    error: ProvenanceError,
    code: ProvenanceErrorCode,
    origin: &InputOrigin,
    existing_surface: Option<SurfaceRef>,
    attempted_surface: SurfaceRef,
) {
    assert_eq!(error.code(), code);
    assert_eq!(error.origin(), origin);
    assert_eq!(error.existing_surface(), existing_surface);
    assert_eq!(error.attempted_surface(), attempted_surface);
}

#[test]
fn runtime_has_no_sibling_dependencies_or_exports() {
    let manifest = include_str!("../Cargo.toml");
    let crate_root = include_str!("lib.rs");

    for (prefix, suffix) in [
        ("surgeist", "-retained"),
        ("surgeist", "-window"),
        ("surgeist", "-task"),
    ] {
        let sibling = format!("{prefix}{suffix}");
        assert!(
            !manifest.contains(&sibling),
            "runtime manifest must not depend on {sibling}"
        );
    }

    for (prefix, suffix) in [
        ("surgeist", "_retained"),
        ("surgeist", "_window"),
        ("surgeist", "_task"),
    ] {
        let sibling = format!("{prefix}{suffix}");
        assert!(
            !crate_root.contains(&sibling),
            "runtime crate root must not expose {sibling}"
        );
    }
}

#[test]
fn retained_bridge_is_not_runtime_public_api() {
    let crate_root = include_str!("lib.rs");

    assert!(!crate_root.contains("mod bridge;"));
    assert!(!crate_root.contains("pub use bridge"));
    assert!(
        !std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/bridge.rs")
            .exists()
    );
}

#[test]
fn testing_fixtures_are_not_unconditional_public_api() {
    let crate_root = include_str!("lib.rs");

    assert!(crate_root.contains("#[cfg(test)]\nmod testing;"));
    assert!(!crate_root.contains(&["pub ", "mod testing;"].concat()));
    assert!(!crate_root.contains(&["pub ", "use testing"].concat()));
}

#[test]
fn app_loop_has_no_host_handler_or_native_loop() {
    let input = UiInput::new(CounterInput::Increment, InputProvenance::system()).unwrap();
    let mut expected_runtime = Runtime::new(CounterState::default(), CounterReducer);
    expected_runtime.enqueue_ui(input.clone()).unwrap();
    let expected = expected_runtime.drain_once(RuntimeBudget::default());

    let mut app_loop = AppLoop::new(Runtime::new(CounterState::default(), CounterReducer));
    app_loop.runtime_mut().enqueue_ui(input).unwrap();

    assert_eq!(app_loop.step(RuntimeBudget::default()), expected);
    assert_eq!(app_loop.into_runtime().state().value, 1);
}

#[test]
fn app_loop_delegates_runtime_drain_errors_without_wrapping() {
    let trigger = InputProvenance::system().with_sequence(101);
    let mut app_loop = AppLoop::new(Runtime::new(CounterState::default(), CounterReducer));
    app_loop
        .runtime_mut()
        .set_state_version_for_test(StateVersion::from_u64(u64::MAX));
    app_loop
        .runtime_mut()
        .enqueue_ui(UiInput::new(CounterInput::Increment, trigger.clone()).unwrap())
        .unwrap();

    let error = app_loop.step(RuntimeBudget::default()).unwrap_err();

    assert_eq!(error.code(), RuntimeDrainErrorCode::StateVersionOverflow);
    assert_eq!(error.lane(), RuntimeLane::Ui);
    assert_eq!(error.provenance(), &trigger);
    assert!(std::error::Error::source(&error).is_some());
}

#[test]
fn manifest_declares_root_msrv_1_89() {
    let manifest = include_str!("../Cargo.toml");

    assert!(
        manifest
            .lines()
            .any(|line| line == "rust-version = \"1.89\""),
        "runtime manifest must declare the root Rust 1.89 MSRV"
    );
}

#[test]
fn crate_forbids_unsafe_code() {
    let crate_root = include_str!("lib.rs");

    assert!(
        crate_root
            .lines()
            .any(|line| line == "#![forbid(unsafe_code)]"),
        "runtime crate root must forbid unsafe code"
    );
}

#[test]
fn typed_ids_are_stable_and_debuggable() {
    assert_eq!(AppId::new("photo.lab").as_str(), "photo.lab");
    assert_eq!(SurfaceId::from_u64(7).as_u64(), 7);
    assert_eq!(TaskIntentAttemptId::from_u64(3).as_u64(), 3);
    assert_eq!(CorrelationId::try_from_u64(11).unwrap().get(), 11);
    assert_eq!(
        format!("{:?}", ResourceId::new("thumbs:42")),
        "ResourceId(\"thumbs:42\")"
    );
}

#[test]
fn runtime_owned_surface_primitives_round_trip_for_root_adapters() {
    let window = WindowId::from_u64(0);
    let surface = SurfaceId::from_u64(0);
    let element = ElementId::from_u64(0);
    let generation = SurfaceGeneration::from_u64(0);
    let invalidation = SurfaceInvalidationGeneration::from_u64(0);
    let size = SurfaceSize::new(0, 480);
    let point = SurfacePoint::new(-12, i32::MAX);

    assert_eq!(window.as_u64(), 0);
    assert_eq!(surface.as_u64(), 0);
    assert_eq!(element.as_u64(), 0);
    assert_eq!(generation, SurfaceGeneration::initial());
    assert_eq!(invalidation, SurfaceInvalidationGeneration::initial());
    assert_eq!(size.width(), 0);
    assert_eq!(size.height(), 480);
    assert_eq!(point.x(), -12);
    assert_eq!(point.y(), i32::MAX);
    assert_eq!(SurfacePoint::origin(), SurfacePoint::default());
}

#[test]
fn surface_root_registration_requires_phases_and_rejects_duplicates() {
    let element = ElementId::from_u64(7);
    let empty = ElementRegistration::try_new(element, [] as [ElementPhase; 0]).unwrap_err();
    assert_eq!(empty.code(), SurfaceErrorCode::MissingElementPhase);

    let registration = ElementRegistration::try_new(
        element,
        [
            ElementPhase::Capture,
            ElementPhase::Target,
            ElementPhase::Bubble,
        ],
    )
    .unwrap();
    let mut root = SurfaceRoot::new(RootId::new("main"));
    root.register_element(registration.clone()).unwrap();

    let duplicate = root.register_element(registration).unwrap_err();
    assert_eq!(duplicate.code(), SurfaceErrorCode::DuplicateElement);
    assert_eq!(root.elements().get(element).unwrap().phases().count(), 3);
}

#[test]
fn surface_route_requires_one_ordered_target() {
    let reference = SurfaceRef::new(SurfaceId::from_u64(3), SurfaceGeneration::initial());
    let element = ElementId::from_u64(4);

    let empty = SurfaceRoute::try_new(reference, []).unwrap_err();
    assert_eq!(empty.code(), SurfaceErrorCode::EmptyRoute);

    let missing = SurfaceRoute::try_new(
        reference,
        [SurfaceRouteStep::new(element, ElementPhase::Capture)],
    )
    .unwrap_err();
    assert_eq!(missing.code(), SurfaceErrorCode::MissingRouteTarget);

    let multiple = SurfaceRoute::try_new(
        reference,
        [
            SurfaceRouteStep::new(element, ElementPhase::Target),
            SurfaceRouteStep::new(element, ElementPhase::Target),
        ],
    )
    .unwrap_err();
    assert_eq!(multiple.code(), SurfaceErrorCode::MultipleRouteTargets);

    let out_of_order = SurfaceRoute::try_new(
        reference,
        [
            SurfaceRouteStep::new(element, ElementPhase::Bubble),
            SurfaceRouteStep::new(element, ElementPhase::Target),
        ],
    )
    .unwrap_err();
    assert_eq!(
        out_of_order.code(),
        SurfaceErrorCode::InvalidRoutePhaseOrder
    );

    let route = SurfaceRoute::try_new(
        reference,
        [
            SurfaceRouteStep::new(element, ElementPhase::Capture),
            SurfaceRouteStep::new(element, ElementPhase::Target),
            SurfaceRouteStep::new(element, ElementPhase::Bubble),
        ],
    )
    .unwrap();
    assert_eq!(route.target(), SurfaceElementRef::new(reference, element));
}

#[test]
fn ui_surface_rejects_mismatched_stale_and_unknown_local_references() {
    let element = ElementId::from_u64(2);
    let mut root = SurfaceRoot::new(RootId::new("main"));
    root.register_element(ElementRegistration::try_new(element, [ElementPhase::Target]).unwrap())
        .unwrap();
    let surface = UiSurface::try_new(SurfaceId::from_u64(1), WindowId::from_u64(9), root).unwrap();

    let mismatch = SurfaceElementRef::new(
        SurfaceRef::new(SurfaceId::from_u64(8), SurfaceGeneration::initial()),
        element,
    );
    assert_eq!(
        surface.validate_element_ref(mismatch).unwrap_err().code(),
        SurfaceErrorCode::SurfaceMismatch
    );

    let stale = SurfaceElementRef::new(
        SurfaceRef::new(SurfaceId::from_u64(1), SurfaceGeneration::from_u64(1)),
        element,
    );
    assert_eq!(
        surface.validate_element_ref(stale).unwrap_err().code(),
        SurfaceErrorCode::StaleSurfaceGeneration
    );

    let unknown = SurfaceElementRef::new(surface.surface_ref(), ElementId::from_u64(3));
    assert_eq!(
        surface.validate_element_ref(unknown).unwrap_err().code(),
        SurfaceErrorCode::UnknownElement
    );

    let route = SurfaceRoute::try_new(
        surface.surface_ref(),
        [SurfaceRouteStep::new(element, ElementPhase::Target)],
    )
    .unwrap();
    assert_eq!(surface.validate_route(&route).unwrap(), route.target());
    assert_eq!(
        surface
            .validate_element(surface.element_ref(element).unwrap(), ElementPhase::Capture)
            .unwrap_err()
            .code(),
        SurfaceErrorCode::IneligibleElementTarget
    );
}

#[test]
fn ui_surface_local_mutations_are_idempotent_and_invalidate_changes() {
    let element = ElementId::from_u64(3);
    let mut root = SurfaceRoot::new(RootId::new("main"));
    root.register_element(ElementRegistration::try_new(element, [ElementPhase::Target]).unwrap())
        .unwrap();
    let mut surface =
        UiSurface::try_new(SurfaceId::from_u64(1), WindowId::from_u64(9), root).unwrap();

    let unchanged = surface.set_scroll_offset(SurfacePoint::origin()).unwrap();
    assert!(!unchanged.changed());
    assert_eq!(unchanged.invalidation_generation(), None);
    assert!(!unchanged.redraw_required());

    let changed = surface.set_scroll_offset(SurfacePoint::new(-1, 2)).unwrap();
    assert!(changed.changed());
    assert_eq!(
        changed.invalidation_generation(),
        Some(SurfaceInvalidationGeneration::initial())
    );
    assert_eq!(surface.scroll_offset(), SurfacePoint::new(-1, 2));
    assert_eq!(surface.invalidations().len(), 1);

    surface.ready().unwrap();
    assert!(
        surface
            .set_viewport(SurfaceSize::new(640, 480))
            .unwrap()
            .changed()
    );
    assert_eq!(surface.lifecycle(), SurfaceLifecycle::Resized);
    let reference = surface.element_ref(element).unwrap();
    assert!(surface.set_focus(Some(reference)).unwrap().changed());
    assert!(surface.set_hover(Some(reference)).unwrap().changed());
    assert!(!surface.set_focus(Some(reference)).unwrap().changed());
    assert_eq!(surface.focused_element(), Some(reference));
    assert_eq!(surface.hovered_element(), Some(reference));

    let generation = surface
        .replace_root(SurfaceRoot::new(RootId::new("replacement")))
        .unwrap();
    assert_eq!(generation, SurfaceGeneration::from_u64(1));
    assert_eq!(surface.focused_element(), None);
    assert_eq!(surface.hovered_element(), None);
    assert!(matches!(
        surface.invalidations().last().map(SurfaceInvalidation::kind),
        Some(SurfaceInvalidationKind::RootReplaced { surface_generation })
            if *surface_generation == generation
    ));
}

#[test]
fn ui_surface_root_replacement_and_invalidation_overflow_are_atomic() {
    let mut surface = UiSurface::try_new(
        SurfaceId::from_u64(1),
        WindowId::from_u64(9),
        SurfaceRoot::new(RootId::new("before")),
    )
    .unwrap();

    surface.set_scroll_offset(SurfacePoint::new(-4, 8)).unwrap();
    let invalidation_count = surface.invalidations().len();
    surface.set_generations_for_test(u64::MAX, None);
    let root_error = surface
        .replace_root(SurfaceRoot::new(RootId::new("after")))
        .unwrap_err();
    assert_eq!(root_error.code(), SurfaceErrorCode::VersionOverflow);
    assert_eq!(surface.root().id(), &RootId::new("before"));
    assert_eq!(surface.generation(), SurfaceGeneration::from_u64(u64::MAX));
    assert_eq!(surface.scroll_offset(), SurfacePoint::new(-4, 8));
    assert_eq!(surface.invalidations().len(), invalidation_count);
    assert!(std::error::Error::source(&root_error).is_some());

    surface.set_generations_for_test(0, Some(u64::MAX));
    let invalidation_error = surface
        .replace_root(SurfaceRoot::new(RootId::new("after")))
        .unwrap_err();
    assert_eq!(invalidation_error.code(), SurfaceErrorCode::VersionOverflow);
    assert_eq!(surface.root().id(), &RootId::new("before"));
    assert_eq!(surface.generation(), SurfaceGeneration::initial());
    assert_eq!(surface.scroll_offset(), SurfacePoint::new(-4, 8));
    assert_eq!(surface.invalidations().len(), invalidation_count);
}

fn surface_for_lifecycle_tests() -> UiSurface {
    UiSurface::try_new(
        SurfaceId::from_u64(31),
        WindowId::from_u64(41),
        SurfaceRoot::new(RootId::new("surface")),
    )
    .unwrap()
}

fn surface_in_lifecycle(lifecycle: SurfaceLifecycle) -> UiSurface {
    let mut surface = surface_for_lifecycle_tests();
    match lifecycle {
        SurfaceLifecycle::Created => {}
        SurfaceLifecycle::Ready => {
            surface.ready().unwrap();
        }
        SurfaceLifecycle::Resized => {
            surface.ready().unwrap();
            surface.resized().unwrap();
        }
        SurfaceLifecycle::Hidden => {
            surface.ready().unwrap();
            surface.hidden().unwrap();
        }
        SurfaceLifecycle::Occluded => {
            surface.ready().unwrap();
            surface.occluded().unwrap();
        }
        SurfaceLifecycle::Suspended => {
            surface.ready().unwrap();
            surface.suspended().unwrap();
        }
        SurfaceLifecycle::Closing => {
            surface.closing().unwrap();
        }
        SurfaceLifecycle::Closed => {
            surface.closed().unwrap();
        }
        SurfaceLifecycle::Destroyed => {
            surface.destroyed().unwrap();
        }
    }
    surface
}

#[test]
fn surface_lifecycle_accepts_exact_transition_matrix_and_convenience_methods() {
    let cases = [
        (
            SurfaceLifecycle::Created,
            &[
                SurfaceLifecycle::Ready,
                SurfaceLifecycle::Closing,
                SurfaceLifecycle::Closed,
                SurfaceLifecycle::Destroyed,
            ][..],
        ),
        (
            SurfaceLifecycle::Ready,
            &[
                SurfaceLifecycle::Resized,
                SurfaceLifecycle::Hidden,
                SurfaceLifecycle::Occluded,
                SurfaceLifecycle::Suspended,
                SurfaceLifecycle::Closing,
                SurfaceLifecycle::Closed,
                SurfaceLifecycle::Destroyed,
            ][..],
        ),
        (
            SurfaceLifecycle::Resized,
            &[
                SurfaceLifecycle::Ready,
                SurfaceLifecycle::Hidden,
                SurfaceLifecycle::Occluded,
                SurfaceLifecycle::Suspended,
                SurfaceLifecycle::Closing,
                SurfaceLifecycle::Closed,
                SurfaceLifecycle::Destroyed,
            ][..],
        ),
        (
            SurfaceLifecycle::Hidden,
            &[
                SurfaceLifecycle::Ready,
                SurfaceLifecycle::Closing,
                SurfaceLifecycle::Closed,
                SurfaceLifecycle::Destroyed,
            ][..],
        ),
        (
            SurfaceLifecycle::Occluded,
            &[
                SurfaceLifecycle::Ready,
                SurfaceLifecycle::Hidden,
                SurfaceLifecycle::Suspended,
                SurfaceLifecycle::Closing,
                SurfaceLifecycle::Closed,
                SurfaceLifecycle::Destroyed,
            ][..],
        ),
        (
            SurfaceLifecycle::Suspended,
            &[
                SurfaceLifecycle::Ready,
                SurfaceLifecycle::Hidden,
                SurfaceLifecycle::Closing,
                SurfaceLifecycle::Closed,
                SurfaceLifecycle::Destroyed,
            ][..],
        ),
        (
            SurfaceLifecycle::Closing,
            &[SurfaceLifecycle::Closed, SurfaceLifecycle::Destroyed][..],
        ),
        (SurfaceLifecycle::Closed, &[SurfaceLifecycle::Destroyed][..]),
        (SurfaceLifecycle::Destroyed, &[][..]),
    ];

    for (current, allowed) in cases {
        for next in [
            SurfaceLifecycle::Created,
            SurfaceLifecycle::Ready,
            SurfaceLifecycle::Resized,
            SurfaceLifecycle::Hidden,
            SurfaceLifecycle::Occluded,
            SurfaceLifecycle::Suspended,
            SurfaceLifecycle::Closing,
            SurfaceLifecycle::Closed,
            SurfaceLifecycle::Destroyed,
        ] {
            let mut surface = surface_in_lifecycle(current);

            if allowed.contains(&next) {
                assert_eq!(surface.transition_to(next), Ok(next));
                assert_eq!(surface.lifecycle(), next);
            } else {
                assert_eq!(
                    surface.transition_to(next).unwrap_err().code(),
                    SurfaceErrorCode::InvalidLifecycleTransition
                );
                assert_eq!(surface.lifecycle(), current);
            }
        }
    }

    let mut surface = surface_for_lifecycle_tests();
    assert_eq!(surface.ready(), Ok(SurfaceLifecycle::Ready));
    assert_eq!(surface.hidden(), Ok(SurfaceLifecycle::Hidden));
    assert_eq!(surface.ready(), Ok(SurfaceLifecycle::Ready));
    assert_eq!(surface.occluded(), Ok(SurfaceLifecycle::Occluded));
    assert_eq!(surface.suspended(), Ok(SurfaceLifecycle::Suspended));
    assert_eq!(surface.closing(), Ok(SurfaceLifecycle::Closing));
    assert_eq!(surface.closed(), Ok(SurfaceLifecycle::Closed));
    assert_eq!(surface.destroyed(), Ok(SurfaceLifecycle::Destroyed));
}

#[test]
fn terminal_surface_rejects_local_mutations_targeting_and_invalidation_without_changes() {
    for lifecycle in [
        SurfaceLifecycle::Closing,
        SurfaceLifecycle::Closed,
        SurfaceLifecycle::Destroyed,
    ] {
        let mut surface = surface_in_lifecycle(lifecycle);
        let before = (
            surface.generation(),
            surface.viewport(),
            surface.scroll_offset(),
            surface.focused_element(),
            surface.hovered_element(),
            surface.invalidations().to_vec(),
        );

        for error in [
            surface
                .replace_root(SurfaceRoot::new(RootId::new("replacement")))
                .unwrap_err(),
            surface.set_viewport(SurfaceSize::default()).unwrap_err(),
            surface
                .set_scroll_offset(SurfacePoint::origin())
                .unwrap_err(),
            surface.set_focus(None).unwrap_err(),
            surface.set_hover(None).unwrap_err(),
            surface
                .invalidate_snapshot(StateVersion::initial())
                .unwrap_err(),
            surface.element_ref(ElementId::from_u64(99)).unwrap_err(),
        ] {
            assert_eq!(error.code(), SurfaceErrorCode::TerminalSurface);
        }

        assert_eq!(
            (
                surface.generation(),
                surface.viewport(),
                surface.scroll_offset(),
                surface.focused_element(),
                surface.hovered_element(),
                surface.invalidations().to_vec(),
            ),
            before
        );
    }
}

#[test]
fn surface_render_begin_and_ack_enforce_lifecycle_eligibility_without_mutation() {
    for lifecycle in [
        SurfaceLifecycle::Created,
        SurfaceLifecycle::Hidden,
        SurfaceLifecycle::Occluded,
        SurfaceLifecycle::Suspended,
    ] {
        let mut surface = surface_in_lifecycle(lifecycle);
        let before = surface.invalidations().to_vec();

        assert_eq!(
            surface
                .begin_render(StateVersion::initial())
                .unwrap_err()
                .code(),
            SurfaceErrorCode::InvalidLifecycleTransition
        );
        let frame =
            SurfaceRenderFrame::new_for_test(surface.surface_ref(), StateVersion::initial(), None);
        assert_eq!(
            surface.acknowledge_render(frame).unwrap_err().code(),
            SurfaceErrorCode::InvalidLifecycleTransition
        );
        assert_eq!(surface.invalidations(), before);
    }

    for lifecycle in [
        SurfaceLifecycle::Closing,
        SurfaceLifecycle::Closed,
        SurfaceLifecycle::Destroyed,
    ] {
        let mut surface = surface_in_lifecycle(lifecycle);
        let before = surface.invalidations().to_vec();

        assert_eq!(
            surface
                .begin_render(StateVersion::initial())
                .unwrap_err()
                .code(),
            SurfaceErrorCode::TerminalSurface
        );
        let frame =
            SurfaceRenderFrame::new_for_test(surface.surface_ref(), StateVersion::initial(), None);
        assert_eq!(
            surface.acknowledge_render(frame).unwrap_err().code(),
            SurfaceErrorCode::TerminalSurface
        );
        assert_eq!(surface.invalidations(), before);
    }
}

#[test]
fn render_ack_rejects_stale_versions_and_replays_without_consuming_new_work() {
    let mut surface = surface_for_lifecycle_tests();
    surface.ready().unwrap();
    surface
        .invalidate_snapshot(StateVersion::from_u64(2))
        .unwrap();
    let frame = surface.begin_render(StateVersion::from_u64(2)).unwrap();
    let first_ack = surface.acknowledge_render(frame).unwrap();
    assert_eq!(first_ack.consumed_invalidations(), 1);
    assert_eq!(first_ack.remaining_invalidations(), 0);

    surface
        .replace_root(SurfaceRoot::new(RootId::new("replacement")))
        .unwrap();
    let replacement_frame = surface.begin_render(StateVersion::from_u64(2)).unwrap();
    assert_eq!(
        surface
            .acknowledge_render(replacement_frame)
            .unwrap()
            .consumed_invalidations(),
        1
    );

    surface.set_scroll_offset(SurfacePoint::new(3, 4)).unwrap();
    let replay = surface.acknowledge_render(replacement_frame).unwrap();
    assert_eq!(replay.consumed_invalidations(), 0);
    assert_eq!(replay.remaining_invalidations(), 1);
    assert!(replay.redraw_required());

    let fresh_frame = surface.begin_render(StateVersion::from_u64(2)).unwrap();
    assert_eq!(
        surface
            .acknowledge_render(fresh_frame)
            .unwrap()
            .consumed_invalidations(),
        1
    );

    let stale =
        SurfaceRenderFrame::new_for_test(surface.surface_ref(), StateVersion::from_u64(1), None);
    assert_eq!(
        surface.acknowledge_render(stale).unwrap_err().code(),
        SurfaceErrorCode::StaleRenderAck
    );
    assert_eq!(surface.invalidations().len(), 0);
}

#[test]
fn root_replacement_preserves_render_version_and_rejects_lower_ack_atomically() {
    let mut surface = surface_for_lifecycle_tests();
    surface.ready().unwrap();
    surface
        .invalidate_snapshot(StateVersion::from_u64(2))
        .unwrap();
    let initial_frame = surface.begin_render(StateVersion::from_u64(2)).unwrap();
    surface.acknowledge_render(initial_frame).unwrap();

    surface
        .replace_root(SurfaceRoot::new(RootId::new("replacement")))
        .unwrap();
    let before_stale_ack = surface.invalidations().to_vec();
    let stale_frame = surface.begin_render(StateVersion::from_u64(1)).unwrap();
    assert_eq!(
        surface.acknowledge_render(stale_frame).unwrap_err().code(),
        SurfaceErrorCode::StaleRenderAck
    );
    assert_eq!(surface.invalidations(), before_stale_ack);

    let current_frame = surface.begin_render(StateVersion::from_u64(2)).unwrap();
    let current_ack = surface.acknowledge_render(current_frame).unwrap();
    assert_eq!(current_ack.consumed_invalidations(), 1);
    assert_eq!(current_ack.remaining_invalidations(), 0);

    surface.set_scroll_offset(SurfacePoint::new(3, 4)).unwrap();
    let newer_frame = surface.begin_render(StateVersion::from_u64(3)).unwrap();
    let newer_ack = surface.acknowledge_render(newer_frame).unwrap();
    assert_eq!(newer_ack.consumed_invalidations(), 1);
    assert_eq!(newer_ack.remaining_invalidations(), 0);
}

#[test]
fn render_ack_retains_post_begin_and_newer_snapshot_invalidations_with_exact_counts() {
    let mut surface = surface_for_lifecycle_tests();
    surface.ready().unwrap();
    surface.set_scroll_offset(SurfacePoint::new(1, 2)).unwrap();
    surface
        .invalidate_snapshot(StateVersion::from_u64(5))
        .unwrap();
    let frame = surface.begin_render(StateVersion::from_u64(4)).unwrap();

    surface
        .invalidate_snapshot(StateVersion::from_u64(6))
        .unwrap();
    surface.set_scroll_offset(SurfacePoint::new(3, 4)).unwrap();
    let ack = surface.acknowledge_render(frame).unwrap();

    assert_eq!(
        ack.acknowledged_frame_generation(),
        frame.invalidation_generation()
    );
    assert_eq!(ack.consumed_invalidations(), 1);
    assert_eq!(ack.remaining_invalidations(), 3);
    assert!(ack.redraw_required());
    assert_eq!(
        surface
            .invalidations()
            .iter()
            .map(SurfaceInvalidation::kind)
            .collect::<Vec<_>>(),
        vec![
            &SurfaceInvalidationKind::SnapshotChanged {
                version: StateVersion::from_u64(5)
            },
            &SurfaceInvalidationKind::SnapshotChanged {
                version: StateVersion::from_u64(6)
            },
            &SurfaceInvalidationKind::SurfaceChanged,
        ]
    );

    let newer_snapshot_frame = SurfaceRenderFrame::new_for_test(
        surface.surface_ref(),
        StateVersion::from_u64(5),
        surface
            .invalidations()
            .last()
            .map(SurfaceInvalidation::generation),
    );
    let newer_snapshot_ack = surface.acknowledge_render(newer_snapshot_frame).unwrap();
    assert_eq!(newer_snapshot_ack.consumed_invalidations(), 2);
    assert_eq!(newer_snapshot_ack.remaining_invalidations(), 1);
    assert!(newer_snapshot_ack.redraw_required());
}

#[test]
fn surface_render_values_expose_captured_metadata_and_state_view() {
    let mut surface = surface_for_lifecycle_tests();
    surface.ready().unwrap();
    surface.set_scroll_offset(SurfacePoint::new(3, 4)).unwrap();
    let frame = surface.begin_render(StateVersion::from_u64(7)).unwrap();
    let state = "runtime state";
    let render_state = SurfaceRenderState::new_for_test(&state, frame);

    assert_eq!(render_state.state(), &state);
    assert_eq!(render_state.frame(), &frame);
    assert_eq!(render_state.into_frame(), frame);
    assert_eq!(frame.surface(), surface.surface_ref());
    assert_eq!(frame.state_version(), StateVersion::from_u64(7));
    assert_eq!(
        frame.invalidation_generation(),
        Some(SurfaceInvalidationGeneration::initial())
    );
}

#[test]
fn terminal_invalidation_overflow_and_render_ack_are_failure_atomic() {
    let mut surface = surface_for_lifecycle_tests();
    surface.ready().unwrap();
    surface.set_scroll_offset(SurfacePoint::new(3, 4)).unwrap();
    surface.set_generations_for_test(0, Some(u64::MAX));
    let before = surface.invalidations().to_vec();

    let overflow = surface
        .invalidate_snapshot(StateVersion::from_u64(9))
        .unwrap_err();
    assert_eq!(overflow.code(), SurfaceErrorCode::VersionOverflow);
    assert!(std::error::Error::source(&overflow).is_some());
    assert_eq!(surface.invalidations(), before);

    let wrong_surface = SurfaceRenderFrame::new_for_test(
        SurfaceRef::new(SurfaceId::from_u64(32), SurfaceGeneration::initial()),
        StateVersion::from_u64(9),
        None,
    );
    assert_eq!(
        surface
            .acknowledge_render(wrong_surface)
            .unwrap_err()
            .code(),
        SurfaceErrorCode::SurfaceMismatch
    );
    assert_eq!(surface.invalidations(), before);
}

fn test_surface(surface_id: u64, window_id: u64, root_id: &str) -> UiSurface {
    UiSurface::try_new(
        SurfaceId::from_u64(surface_id),
        WindowId::from_u64(window_id),
        SurfaceRoot::new(RootId::new(root_id)),
    )
    .expect("test surface construction should be valid")
}

fn test_surface_with_elements(
    surface_id: u64,
    window_id: u64,
    root_id: &str,
    elements: impl IntoIterator<Item = (u64, Vec<ElementPhase>)>,
) -> UiSurface {
    let mut root = SurfaceRoot::new(RootId::new(root_id));
    for (element_id, phases) in elements {
        root.register_element(
            ElementRegistration::try_new(ElementId::from_u64(element_id), phases)
                .expect("test element registration should be valid"),
        )
        .expect("test root should accept unique elements");
    }

    UiSurface::try_new(
        SurfaceId::from_u64(surface_id),
        WindowId::from_u64(window_id),
        root,
    )
    .expect("test surface construction should be valid")
}

fn ready_surface(
    runtime: &mut Runtime<CounterState, CounterReducer, CounterInput>,
    surface: SurfaceRef,
) {
    runtime
        .update_surface(surface, |surface| surface.ready().map(|_| ()))
        .expect("registered created surface should become ready");
}

#[test]
fn runtime_validation_applies_registry_lifecycle_and_element_precedence() {
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    let created = runtime
        .register_surface(test_surface_with_elements(
            1,
            1,
            "created",
            [(1, vec![ElementPhase::Target])],
        ))
        .unwrap();
    let unknown = SurfaceElementRef::new(
        SurfaceRef::new(SurfaceId::from_u64(9), SurfaceGeneration::initial()),
        ElementId::from_u64(99),
    );
    assert_eq!(
        runtime
            .validate_element(unknown, ElementPhase::Bubble)
            .unwrap_err()
            .code(),
        SurfaceErrorCode::UnknownSurface
    );

    let stale = created;
    runtime
        .update_surface(created, |surface| {
            surface.replace_root(SurfaceRoot::new(RootId::new("replacement")))?;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        runtime
            .validate_element(
                SurfaceElementRef::new(stale, ElementId::from_u64(99)),
                ElementPhase::Bubble,
            )
            .unwrap_err()
            .code(),
        SurfaceErrorCode::StaleSurfaceGeneration
    );

    let inactive = runtime
        .register_surface(test_surface_with_elements(
            2,
            1,
            "inactive",
            [(1, vec![ElementPhase::Target])],
        ))
        .unwrap();
    assert_eq!(
        runtime
            .validate_element(
                SurfaceElementRef::new(inactive, ElementId::from_u64(99)),
                ElementPhase::Bubble,
            )
            .unwrap_err()
            .code(),
        SurfaceErrorCode::InvalidLifecycleTransition
    );

    ready_surface(&mut runtime, inactive);
    assert_eq!(
        runtime
            .validate_element(
                SurfaceElementRef::new(inactive, ElementId::from_u64(99)),
                ElementPhase::Bubble,
            )
            .unwrap_err()
            .code(),
        SurfaceErrorCode::UnknownElement
    );
    assert_eq!(
        runtime
            .validate_element(
                SurfaceElementRef::new(inactive, ElementId::from_u64(1)),
                ElementPhase::Bubble,
            )
            .unwrap_err()
            .code(),
        SurfaceErrorCode::IneligibleElementTarget
    );
}

#[test]
fn runtime_route_validation_checks_every_step_and_target_surface_identity() {
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    let surface = runtime
        .register_surface(test_surface_with_elements(
            1,
            1,
            "main",
            [
                (1, vec![ElementPhase::Capture]),
                (2, vec![ElementPhase::Target]),
                (3, vec![ElementPhase::Bubble]),
            ],
        ))
        .unwrap();
    ready_surface(&mut runtime, surface);

    let route = SurfaceRoute::try_new(
        surface,
        [
            SurfaceRouteStep::new(ElementId::from_u64(1), ElementPhase::Capture),
            SurfaceRouteStep::new(ElementId::from_u64(2), ElementPhase::Target),
            SurfaceRouteStep::new(ElementId::from_u64(3), ElementPhase::Bubble),
        ],
    )
    .unwrap();
    assert_eq!(
        runtime.validate_route(&route),
        Ok(SurfaceElementRef::new(surface, ElementId::from_u64(2)))
    );

    let invalid_step = SurfaceRoute::try_new(
        surface,
        [
            SurfaceRouteStep::new(ElementId::from_u64(2), ElementPhase::Capture),
            SurfaceRouteStep::new(ElementId::from_u64(2), ElementPhase::Target),
        ],
    )
    .unwrap();
    assert_eq!(
        runtime.validate_route(&invalid_step).unwrap_err().code(),
        SurfaceErrorCode::IneligibleElementTarget
    );

    let other = runtime
        .register_surface(test_surface_with_elements(
            2,
            1,
            "other",
            [(2, vec![ElementPhase::Target])],
        ))
        .unwrap();
    ready_surface(&mut runtime, other);
    assert_eq!(
        runtime
            .set_focus(
                surface,
                Some(SurfaceElementRef::new(other, ElementId::from_u64(2)))
            )
            .unwrap_err()
            .code(),
        SurfaceErrorCode::SurfaceMismatch
    );
}

#[test]
fn runtime_focus_and_hover_set_clear_and_duplicate_are_deterministic() {
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    let surface = runtime
        .register_surface(test_surface_with_elements(
            1,
            1,
            "main",
            [(7, vec![ElementPhase::Target])],
        ))
        .unwrap();
    ready_surface(&mut runtime, surface);
    let element = SurfaceElementRef::new(surface, ElementId::from_u64(7));

    let focus = runtime.set_focus(surface, Some(element)).unwrap();
    assert!(focus.changed());
    assert!(focus.redraw_required());
    assert_eq!(
        runtime
            .surface(surface.surface_id())
            .unwrap()
            .focused_element(),
        Some(element)
    );
    assert!(!runtime.set_focus(surface, Some(element)).unwrap().changed());

    let hover = runtime.set_hover(surface, Some(element)).unwrap();
    assert!(hover.changed());
    assert_eq!(
        runtime
            .surface(surface.surface_id())
            .unwrap()
            .hovered_element(),
        Some(element)
    );
    assert!(!runtime.set_hover(surface, Some(element)).unwrap().changed());

    assert!(runtime.set_focus(surface, None).unwrap().changed());
    assert!(runtime.set_hover(surface, None).unwrap().changed());
    assert_eq!(
        runtime
            .surface(surface.surface_id())
            .unwrap()
            .focused_element(),
        None
    );
    assert_eq!(
        runtime
            .surface(surface.surface_id())
            .unwrap()
            .hovered_element(),
        None
    );
}

#[test]
fn runtime_surface_mutations_reject_stale_and_terminal_targets_atomically() {
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    let surface = runtime
        .register_surface(test_surface_with_elements(
            1,
            1,
            "main",
            [(7, vec![ElementPhase::Target])],
        ))
        .unwrap();
    ready_surface(&mut runtime, surface);
    let stale = surface;
    runtime
        .update_surface(surface, |surface| {
            surface.replace_root(SurfaceRoot::new(RootId::new("replacement")))?;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        runtime
            .set_scroll_offset(stale, SurfacePoint::new(3, 4))
            .unwrap_err()
            .code(),
        SurfaceErrorCode::StaleSurfaceGeneration
    );

    let current = runtime.surface_ref(surface.surface_id()).unwrap();
    runtime
        .update_surface(current, |surface| surface.closing().map(|_| ()))
        .unwrap();
    let before = runtime
        .surface(current.surface_id())
        .unwrap()
        .invalidations()
        .to_vec();
    assert_eq!(
        runtime
            .set_scroll_offset(current, SurfacePoint::new(3, 4))
            .unwrap_err()
            .code(),
        SurfaceErrorCode::TerminalSurface
    );
    assert_eq!(
        runtime
            .surface(current.surface_id())
            .unwrap()
            .invalidations(),
        before
    );
}

#[test]
fn runtime_scroll_and_resize_record_invalidation_and_precise_redraw_outcomes() {
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    let created = runtime
        .register_surface(test_surface(1, 1, "created"))
        .unwrap();
    let scroll = runtime
        .set_scroll_offset(created, SurfacePoint::new(3, 4))
        .unwrap();
    assert!(scroll.changed());
    assert_eq!(
        scroll.invalidation_generation(),
        Some(SurfaceInvalidationGeneration::initial())
    );
    assert!(!scroll.redraw_required());
    assert_eq!(
        runtime
            .resize(created, SurfaceSize::new(800, 600))
            .unwrap_err()
            .code(),
        SurfaceErrorCode::InvalidLifecycleTransition
    );

    ready_surface(&mut runtime, created);
    let resize = runtime.resize(created, SurfaceSize::new(800, 600)).unwrap();
    assert!(resize.changed());
    assert!(resize.redraw_required());
    assert_eq!(
        runtime.surface(created.surface_id()).unwrap().lifecycle(),
        SurfaceLifecycle::Resized
    );
    assert!(matches!(
        runtime
            .surface(created.surface_id())
            .unwrap()
            .invalidations()
            .last()
            .map(SurfaceInvalidation::kind),
        Some(SurfaceInvalidationKind::ViewportChanged)
    ));
    assert!(
        !runtime
            .resize(created, SurfaceSize::new(800, 600))
            .unwrap()
            .changed()
    );
}

#[test]
fn runtime_render_state_borrows_runtime_state_and_acknowledges_captured_work() {
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    let surface = runtime
        .register_surface(test_surface(1, 1, "main"))
        .unwrap();
    ready_surface(&mut runtime, surface);
    runtime
        .set_scroll_offset(surface, SurfacePoint::new(3, 4))
        .unwrap();
    runtime
        .enqueue_ui(UiInput::new(CounterInput::Increment, InputProvenance::system()).unwrap())
        .unwrap();
    runtime.drain_once(RuntimeBudget::default()).unwrap();

    let render_state = runtime.begin_render(surface).unwrap();
    assert_eq!(render_state.state().value, 1);
    assert_eq!(render_state.frame().surface(), surface);
    assert_eq!(
        render_state.frame().state_version(),
        runtime.state_version()
    );
    let frame = render_state.into_frame();
    let ack = runtime.mark_rendered(frame).unwrap();
    assert_eq!(ack.consumed_invalidations(), 2);
    assert_eq!(ack.remaining_invalidations(), 0);
    assert!(!ack.redraw_required());
}

#[test]
fn runtime_render_ack_is_lifecycle_atomic_and_coalesces_replays() {
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    let surface = runtime
        .register_surface(test_surface(1, 1, "main"))
        .unwrap();
    ready_surface(&mut runtime, surface);
    runtime
        .set_scroll_offset(surface, SurfacePoint::new(1, 2))
        .unwrap();
    let frame = runtime.begin_render(surface).unwrap().into_frame();
    runtime
        .set_scroll_offset(surface, SurfacePoint::new(3, 4))
        .unwrap();

    let first = runtime.mark_rendered(frame).unwrap();
    assert_eq!(first.consumed_invalidations(), 1);
    assert_eq!(first.remaining_invalidations(), 1);
    assert!(first.redraw_required());
    let replay = runtime.mark_rendered(frame).unwrap();
    assert_eq!(replay.consumed_invalidations(), 0);
    assert_eq!(replay.remaining_invalidations(), 1);

    runtime
        .update_surface(surface, |surface| surface.hidden().map(|_| ()))
        .unwrap();
    let before = runtime
        .surface(surface.surface_id())
        .unwrap()
        .invalidations()
        .to_vec();
    assert_eq!(
        runtime.mark_rendered(frame).unwrap_err().code(),
        SurfaceErrorCode::InvalidLifecycleTransition
    );
    assert_eq!(
        runtime
            .surface(surface.surface_id())
            .unwrap()
            .invalidations(),
        before
    );
}

#[test]
fn runtime_renderable_invalidated_surfaces_are_ordered_and_lifecycle_filtered() {
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    let third = runtime
        .register_surface(test_surface(3, 1, "third"))
        .unwrap();
    let first = runtime
        .register_surface(test_surface(1, 1, "first"))
        .unwrap();
    let second = runtime
        .register_surface(test_surface(2, 1, "second"))
        .unwrap();
    ready_surface(&mut runtime, first);
    ready_surface(&mut runtime, second);
    runtime
        .update_surface(third, |surface| {
            surface.ready()?;
            surface.hidden()?;
            Ok(())
        })
        .unwrap();

    runtime
        .set_scroll_offset(first, SurfacePoint::new(1, 1))
        .unwrap();
    runtime
        .set_scroll_offset(third, SurfacePoint::new(3, 3))
        .unwrap();
    assert_eq!(
        runtime
            .renderable_invalidated_surfaces()
            .collect::<Vec<_>>(),
        vec![first]
    );

    runtime
        .update_surface(third, |surface| surface.ready().map(|_| ()))
        .unwrap();
    assert_eq!(
        runtime
            .renderable_invalidated_surfaces()
            .collect::<Vec<_>>(),
        vec![first, third]
    );
}

fn registry_subscription(observer: SurfaceRef) -> Subscription {
    Subscription::resource(
        ResourceId::new("registry"),
        AppScope::app(),
        observer,
        SubscriptionPriority::Normal,
    )
}

#[test]
fn runtime_surface_registry_rejects_duplicate_unknown_removed_and_stale_ids() {
    let mut runtime = Runtime::<_, _, CounterInput>::new(CounterState::default(), CounterReducer);
    let first = runtime
        .register_surface(test_surface(1, 1, "first"))
        .unwrap();
    assert_eq!(first.generation(), SurfaceGeneration::initial());

    assert_eq!(
        runtime
            .register_surface(test_surface(1, 1, "duplicate"))
            .unwrap_err()
            .code(),
        SurfaceErrorCode::DuplicateSurface
    );
    let mut ready = test_surface(2, 1, "ready");
    ready.ready().unwrap();
    assert_eq!(
        runtime.register_surface(ready).unwrap_err().code(),
        SurfaceErrorCode::InvalidLifecycleTransition
    );
    let mut stale_generation = test_surface(3, 1, "stale");
    stale_generation.set_generations_for_test(1, None);
    assert_eq!(
        runtime
            .register_surface(stale_generation)
            .unwrap_err()
            .code(),
        SurfaceErrorCode::StaleSurfaceGeneration
    );
    assert_eq!(
        runtime
            .remove_surface(SurfaceRef::new(
                SurfaceId::from_u64(2),
                SurfaceGeneration::initial(),
            ))
            .unwrap_err()
            .code(),
        SurfaceErrorCode::UnknownSurface
    );

    runtime.remove_surface(first).unwrap();
    let replacement = runtime
        .register_surface(test_surface(1, 1, "replacement"))
        .unwrap();
    assert_eq!(replacement.generation(), SurfaceGeneration::from_u64(1));
    assert_eq!(
        runtime.surface_ids().collect::<Vec<_>>(),
        vec![SurfaceId::from_u64(1)]
    );
    assert_eq!(
        runtime.remove_surface(first).unwrap_err().code(),
        SurfaceErrorCode::StaleSurfaceGeneration
    );
    assert_eq!(
        runtime
            .update_surface(first, |_| Ok(()))
            .unwrap_err()
            .code(),
        SurfaceErrorCode::StaleSurfaceGeneration
    );
}

#[test]
fn runtime_surface_updates_are_failure_atomic() {
    let mut runtime = Runtime::<_, _, CounterInput>::new(CounterState::default(), CounterReducer);
    let surface = runtime
        .register_surface(test_surface(3, 1, "before"))
        .unwrap();
    let subscription = registry_subscription(surface);
    runtime.subscribe(subscription.clone()).unwrap();

    let error = runtime
        .update_surface(surface, |staged| {
            staged.replace_root(SurfaceRoot::new(RootId::new("after")))?;
            staged.transition_to(SurfaceLifecycle::Created).map(|_| ())
        })
        .unwrap_err();

    assert_eq!(error.code(), SurfaceErrorCode::InvalidLifecycleTransition);
    assert_eq!(runtime.surface_ref(surface.surface_id()), Some(surface));
    assert_eq!(
        runtime.surface(surface.surface_id()).unwrap().root().id(),
        &RootId::new("before")
    );
    assert_eq!(runtime.coordination().ref_count(subscription.key()), 1);
}

#[test]
fn reregistered_surface_ids_do_not_restore_old_subscriptions_or_overflow_tombstones() {
    let mut runtime = Runtime::<_, _, CounterInput>::new(CounterState::default(), CounterReducer);
    let first = runtime
        .register_surface(test_surface(4, 1, "first"))
        .unwrap();
    let old_subscription = registry_subscription(first);
    runtime.subscribe(old_subscription.clone()).unwrap();
    runtime.remove_surface(first).unwrap();

    let replacement = runtime
        .register_surface(test_surface(4, 1, "replacement"))
        .unwrap();
    let replacement_subscription = registry_subscription(replacement);
    runtime.subscribe(replacement_subscription.clone()).unwrap();

    assert_eq!(runtime.coordination().ref_count(old_subscription.key()), 0);
    assert_eq!(
        runtime
            .unsubscribe(old_subscription.key())
            .unwrap_err()
            .code(),
        SubscriptionErrorCode::StaleObserver
    );
    assert_eq!(
        runtime
            .coordination()
            .ref_count(replacement_subscription.key()),
        1
    );

    runtime.set_retired_generation_for_test(
        SurfaceId::from_u64(5),
        SurfaceGeneration::from_u64(u64::MAX),
    );
    assert_eq!(
        runtime
            .register_surface(test_surface(5, 1, "overflow"))
            .unwrap_err()
            .code(),
        SurfaceErrorCode::VersionOverflow
    );
    assert!(runtime.surface(SurfaceId::from_u64(5)).is_none());
}

#[test]
fn terminal_and_removed_surfaces_drop_all_observer_subscriptions() {
    let mut runtime = Runtime::<_, _, CounterInput>::new(CounterState::default(), CounterReducer);
    let first = runtime
        .register_surface(test_surface(6, 1, "first"))
        .unwrap();
    let first_subscription = registry_subscription(first);
    runtime.subscribe(first_subscription.clone()).unwrap();

    runtime
        .update_surface(first, |surface| {
            surface.replace_root(SurfaceRoot::new(RootId::new("replacement")))?;
            Ok(())
        })
        .unwrap();
    let replacement = runtime.surface_ref(first.surface_id()).unwrap();
    assert_eq!(
        runtime.coordination().ref_count(first_subscription.key()),
        0
    );

    let replacement_subscription = registry_subscription(replacement);
    runtime.subscribe(replacement_subscription.clone()).unwrap();
    runtime
        .update_surface(replacement, |surface| surface.closing().map(|_| ()))
        .unwrap();
    assert_eq!(
        runtime
            .coordination()
            .ref_count(replacement_subscription.key()),
        0
    );
    runtime
        .update_surface(replacement, |surface| surface.closed().map(|_| ()))
        .unwrap();
    assert_eq!(
        runtime
            .coordination()
            .ref_count(replacement_subscription.key()),
        0
    );

    let removable = runtime
        .register_surface(test_surface(7, 1, "removable"))
        .unwrap();
    let removable_subscription = registry_subscription(removable);
    runtime.subscribe(removable_subscription.clone()).unwrap();
    runtime.remove_surface(removable).unwrap();
    assert_eq!(
        runtime
            .coordination()
            .ref_count(removable_subscription.key()),
        0
    );
}

#[test]
fn runtime_subscription_validation_preserves_current_coordination_state() {
    let mut runtime = Runtime::<_, _, CounterInput>::new(CounterState::default(), CounterReducer);
    let current = runtime
        .register_surface(test_surface(8, 1, "current"))
        .unwrap();
    let current_subscription = registry_subscription(current);
    runtime.subscribe(current_subscription.clone()).unwrap();
    let absent = Subscription::resource(
        ResourceId::new("absent"),
        AppScope::app(),
        current,
        SubscriptionPriority::Normal,
    );
    assert_eq!(
        runtime.unsubscribe(absent.key()).unwrap(),
        SubscriptionChange::NotFound {
            key: absent.key().clone()
        }
    );
    assert_eq!(
        runtime.coordination().ref_count(current_subscription.key()),
        1
    );

    let unknown = registry_subscription(SurfaceRef::new(
        SurfaceId::from_u64(9),
        SurfaceGeneration::initial(),
    ));
    assert_eq!(
        runtime.subscribe(unknown).unwrap_err().code(),
        SubscriptionErrorCode::UnknownObserver
    );
    let unknown_key = registry_subscription(SurfaceRef::new(
        SurfaceId::from_u64(9),
        SurfaceGeneration::initial(),
    ));
    assert_eq!(
        runtime.unsubscribe(unknown_key.key()).unwrap_err().code(),
        SubscriptionErrorCode::UnknownObserver
    );

    let stale = registry_subscription(SurfaceRef::new(
        current.surface_id(),
        SurfaceGeneration::from_u64(1),
    ));
    assert_eq!(
        runtime.subscribe(stale.clone()).unwrap_err().code(),
        SubscriptionErrorCode::StaleObserver
    );
    assert_eq!(
        runtime.coordination().ref_count(current_subscription.key()),
        1
    );

    runtime
        .update_surface(current, |surface| surface.closing().map(|_| ()))
        .unwrap();
    let terminal = registry_subscription(current);
    assert_eq!(
        runtime.subscribe(terminal.clone()).unwrap_err().code(),
        SubscriptionErrorCode::TerminalObserver
    );
    assert_eq!(
        runtime.unsubscribe(terminal.key()).unwrap_err().code(),
        SubscriptionErrorCode::TerminalObserver
    );
    assert_eq!(
        runtime.coordination().ref_count(current_subscription.key()),
        0
    );
}

#[test]
fn task_intent_identity_types_are_runtime_owned() {
    let name = TaskIntentName::new("search");
    let key = TaskIntentKey::new("search:rust");
    let id = TaskIntentId::from_u64(7);
    let attempt = TaskIntentAttemptId::from_u64(2);
    let handle = TaskIntentHandle::new(id, attempt);

    assert_eq!(name.as_str(), "search");
    assert_eq!(key.as_str(), "search:rust");
    assert_eq!(id.as_u64(), 7);
    assert_eq!(handle.id(), id);
    assert_eq!(handle.attempt_id(), attempt);
}

#[test]
fn task_effects_are_abstract_runtime_intents() {
    let effect = AppEffect::start_task(
        TaskIntentName::new("search"),
        TaskIntentKey::new("search:rust"),
        AppScope::app(),
    );

    let AppEffectPayload::StartTask(intent) = effect.payload() else {
        panic!("expected start task intent");
    };

    assert_eq!(intent.name().as_str(), "search");
    assert_eq!(intent.key().as_str(), "search:rust");
    assert!(intent.scope().is_app());
}

#[test]
fn cancel_task_effect_carries_runtime_task_intent_handle() {
    let handle = TaskIntentHandle::new(TaskIntentId::from_u64(7), TaskIntentAttemptId::from_u64(2));
    let effect = AppEffect::cancel_task(handle);

    let AppEffectPayload::CancelTask(intent) = effect.payload() else {
        panic!("expected cancel task intent");
    };

    assert_eq!(intent.handle(), handle);
}

#[test]
fn reprioritize_task_effect_carries_runtime_task_intent_handle_and_priority_hint() {
    let handle = TaskIntentHandle::new(TaskIntentId::from_u64(7), TaskIntentAttemptId::from_u64(2));
    let effect = AppEffect::reprioritize_task(handle, TaskPriorityHint::High);

    let AppEffectPayload::ReprioritizeTask(intent) = effect.payload() else {
        panic!("expected reprioritize task intent");
    };

    assert_eq!(intent.handle(), handle);
    assert_eq!(intent.priority(), TaskPriorityHint::High);
}

#[test]
fn task_descriptor_names_abstract_runtime_intents() {
    let descriptor = TaskDescriptor::try_new(TaskIntentName::new("search"), "SearchInput").unwrap();

    assert_eq!(descriptor.name().as_str(), "search");
    assert_eq!(descriptor.input_type().as_str(), "SearchInput");
}

#[test]
fn descriptor_payload_type_names_are_validated() {
    for value in ["", "   ", "invalid\u{001f}name"] {
        assert_name_error(
            CommandName::try_new(value).unwrap_err(),
            "command.name",
            value,
        );
        assert_name_error(EventName::try_new(value).unwrap_err(), "event.name", value);
        assert_name_error(
            PayloadTypeName::try_new(value).unwrap_err(),
            "payload_type",
            value,
        );

        assert_name_error(
            CommandDescriptor::try_new(value, "Payload").unwrap_err(),
            "command.name",
            value,
        );
        assert_name_error(
            CommandDescriptor::try_new("command", value).unwrap_err(),
            "command.payload_type",
            value,
        );
        assert_name_error(
            EventDescriptor::try_new(value, "Payload").unwrap_err(),
            "event.name",
            value,
        );
        assert_name_error(
            EventDescriptor::try_new("event", value).unwrap_err(),
            "event.payload_type",
            value,
        );
        assert_name_error(
            TaskDescriptor::try_new(TaskIntentName::new("task"), value).unwrap_err(),
            "task.input_type",
            value,
        );
        assert_name_error(
            ResourceDescriptor::try_new(ResourceId::new("resource"), value).unwrap_err(),
            "resource.value_type",
            value,
        );
    }

    let command_name = CommandName::try_new(" command ").unwrap();
    let event_name = EventName::try_new(" event ").unwrap();
    let payload_type = PayloadTypeName::try_new(" Payload ").unwrap();
    assert_eq!(command_name.as_str(), " command ");
    assert_eq!(event_name.as_str(), " event ");
    assert_eq!(payload_type.as_str(), " Payload ");

    let command = CommandDescriptor::try_new("command", "CommandPayload").unwrap();
    let event = EventDescriptor::try_new("event", "EventPayload").unwrap();
    let task = TaskDescriptor::try_new(TaskIntentName::new("task"), "TaskInput").unwrap();
    let resource =
        ResourceDescriptor::try_new(ResourceId::new("resource"), "ResourceValue").unwrap();
    assert_eq!(command.name().as_str(), "command");
    assert_eq!(command.payload_type().as_str(), "CommandPayload");
    assert_eq!(event.name().as_str(), "event");
    assert_eq!(event.payload_type().as_str(), "EventPayload");
    assert_eq!(task.name().as_str(), "task");
    assert_eq!(task.input_type().as_str(), "TaskInput");
    assert_eq!(resource.id().as_str(), "resource");
    assert_eq!(resource.value_type().as_str(), "ResourceValue");

    let error = CommandDescriptor::try_new("command", "\u{001f}").unwrap_err();
    let _: &dyn Error = &error;
    assert!(format!("{error}").contains("command.payload_type"));
}

fn assert_name_error(error: NameError, field: &str, value: &str) {
    assert_eq!(error.field(), field);
    assert_eq!(error.value(), value);
}

#[test]
fn descriptor_names_have_no_unchecked_public_constructors() {
    for source in [include_str!("command.rs"), include_str!("event.rs")] {
        assert!(!source.contains("pub fn new(value: impl Into<String>) -> Self"));
        assert!(!source.contains("pub fn named(name: impl Into<String>) -> Self"));
    }
    assert!(!include_str!("command.rs").contains("pub fn new(name: impl Into<String>"));
    assert!(!include_str!("event.rs").contains("pub fn new(name: impl Into<String>"));
    assert!(!include_str!("descriptor.rs").contains("pub const fn new(name: TaskIntentName"));
    assert!(!include_str!("descriptor.rs").contains("pub const fn new(id: ResourceId"));
}

#[test]
fn manifest_validation_rejects_duplicates_and_dangling_startup() {
    let duplicate_binding = SnapshotBinding::new(
        SnapshotBindingId::new("state"),
        SnapshotSourceType::new("CounterState"),
    );
    let duplicate_manifest = AppManifest::new(AppDescriptor::new(AppId::new("photo.lab"), "1.0"))
        .command(CommandDescriptor::try_new("zeta", "ZetaCommand").unwrap())
        .command(CommandDescriptor::try_new("alpha", "AlphaCommand").unwrap())
        .command(CommandDescriptor::try_new("zeta", "ZetaCommand").unwrap())
        .command(CommandDescriptor::try_new("alpha", "AlphaCommand").unwrap())
        .event(EventDescriptor::try_new("zeta", "ZetaEvent").unwrap())
        .event(EventDescriptor::try_new("alpha", "AlphaEvent").unwrap())
        .event(EventDescriptor::try_new("zeta", "ZetaEvent").unwrap())
        .event(EventDescriptor::try_new("alpha", "AlphaEvent").unwrap())
        .task(TaskDescriptor::try_new(TaskIntentName::new("zeta"), "ZetaInput").unwrap())
        .task(TaskDescriptor::try_new(TaskIntentName::new("alpha"), "AlphaInput").unwrap())
        .task(TaskDescriptor::try_new(TaskIntentName::new("zeta"), "ZetaInput").unwrap())
        .task(TaskDescriptor::try_new(TaskIntentName::new("alpha"), "AlphaInput").unwrap())
        .resource(ResourceDescriptor::try_new(ResourceId::new("zeta"), "ZetaValue").unwrap())
        .resource(ResourceDescriptor::try_new(ResourceId::new("alpha"), "AlphaValue").unwrap())
        .resource(ResourceDescriptor::try_new(ResourceId::new("zeta"), "ZetaValue").unwrap())
        .resource(ResourceDescriptor::try_new(ResourceId::new("alpha"), "AlphaValue").unwrap())
        .window(WindowDescriptor::new(
            WindowDescriptorId::new("zeta"),
            "Zeta",
        ))
        .window(WindowDescriptor::new(
            WindowDescriptorId::new("alpha"),
            "Alpha",
        ))
        .window(WindowDescriptor::new(
            WindowDescriptorId::new("zeta"),
            "Zeta",
        ))
        .window(WindowDescriptor::new(
            WindowDescriptorId::new("alpha"),
            "Alpha",
        ))
        .root(
            RootDescriptor::new(RootId::new("zeta"))
                .binds_snapshot(duplicate_binding.clone())
                .binds_snapshot(duplicate_binding),
        )
        .root(RootDescriptor::new(RootId::new("alpha")))
        .root(RootDescriptor::new(RootId::new("zeta")))
        .root(RootDescriptor::new(RootId::new("alpha")));

    let error = duplicate_manifest.validate().unwrap_err();
    let _: &dyn Error = &error;
    assert!(format!("{error}").contains("manifest validation"));
    assert_eq!(
        error
            .issues()
            .iter()
            .map(ManifestValidationIssue::code)
            .collect::<Vec<_>>(),
        vec![
            ManifestValidationErrorCode::DuplicateCommand,
            ManifestValidationErrorCode::DuplicateCommand,
            ManifestValidationErrorCode::DuplicateEvent,
            ManifestValidationErrorCode::DuplicateEvent,
            ManifestValidationErrorCode::DuplicateTask,
            ManifestValidationErrorCode::DuplicateTask,
            ManifestValidationErrorCode::DuplicateResource,
            ManifestValidationErrorCode::DuplicateResource,
            ManifestValidationErrorCode::DuplicateWindow,
            ManifestValidationErrorCode::DuplicateWindow,
            ManifestValidationErrorCode::DuplicateRoot,
            ManifestValidationErrorCode::DuplicateRoot,
            ManifestValidationErrorCode::DuplicateRootSnapshotBinding,
            ManifestValidationErrorCode::MissingStartupRoot,
        ]
    );
    assert_eq!(error.issues()[0].command_name().unwrap().as_str(), "alpha");
    assert_eq!(error.issues()[1].command_name().unwrap().as_str(), "zeta");
    assert_eq!(error.issues()[2].event_name().unwrap().as_str(), "alpha");
    assert_eq!(error.issues()[3].event_name().unwrap().as_str(), "zeta");
    assert_eq!(error.issues()[8].window_id().unwrap().as_str(), "alpha");
    assert_eq!(error.issues()[9].window_id().unwrap().as_str(), "zeta");
    assert_eq!(error.issues()[10].root_id().unwrap().as_str(), "alpha");
    assert_eq!(error.issues()[11].root_id().unwrap().as_str(), "zeta");
    assert_eq!(error.issues()[12].root_id().unwrap().as_str(), "zeta");
    assert_eq!(
        error.issues()[12].snapshot_binding_id().unwrap().as_str(),
        "state"
    );

    let missing_startup = AppManifest::new(AppDescriptor::new(AppId::new("photo.lab"), "1.0"))
        .window(WindowDescriptor::new(
            WindowDescriptorId::new("main"),
            "Main",
        ))
        .validate()
        .unwrap_err();
    assert_eq!(
        missing_startup.issues()[0].code(),
        ManifestValidationErrorCode::MissingStartupRoot
    );

    let dangling_startup = AppManifest::new(AppDescriptor::new(AppId::new("photo.lab"), "1.0"))
        .window(
            WindowDescriptor::new(WindowDescriptorId::new("main"), "Main")
                .allows_root(RootId::new("allowed")),
        )
        .window(WindowDescriptor::new(
            WindowDescriptorId::new("unrestricted"),
            "Unrestricted",
        ))
        .root(RootDescriptor::new(RootId::new("allowed")))
        .root(RootDescriptor::new(RootId::new("other-root")))
        .startup_window(StartupWindow::new(
            WindowDescriptorId::new("unknown-window"),
            RootId::new("unknown-root"),
            AppScope::app(),
        ))
        .startup_window(StartupWindow::new(
            WindowDescriptorId::new("unrestricted"),
            RootId::new("unknown-root"),
            AppScope::app(),
        ))
        .startup_window(StartupWindow::new(
            WindowDescriptorId::new("main"),
            RootId::new("other-root"),
            AppScope::app(),
        ))
        .validate()
        .unwrap_err();
    assert_eq!(
        dangling_startup
            .issues()
            .iter()
            .map(ManifestValidationIssue::code)
            .collect::<Vec<_>>(),
        vec![
            ManifestValidationErrorCode::DisallowedStartupRoot,
            ManifestValidationErrorCode::UnknownStartupWindow,
            ManifestValidationErrorCode::UnknownStartupRoot,
            ManifestValidationErrorCode::UnknownStartupRoot,
        ]
    );
    assert_eq!(
        dangling_startup.issues()[0].window_id().unwrap().as_str(),
        "main"
    );
    assert_eq!(
        dangling_startup.issues()[0].root_id().unwrap().as_str(),
        "other-root"
    );
    assert_eq!(
        dangling_startup.issues()[1].window_id().unwrap().as_str(),
        "unknown-window"
    );
    assert_eq!(
        dangling_startup.issues()[1].root_id().unwrap().as_str(),
        "unknown-root"
    );
    assert_eq!(
        dangling_startup.issues()[2].window_id().unwrap().as_str(),
        "unknown-window"
    );
    assert_eq!(
        dangling_startup.issues()[2].root_id().unwrap().as_str(),
        "unknown-root"
    );
    assert_eq!(
        dangling_startup.issues()[3].window_id().unwrap().as_str(),
        "unrestricted"
    );
    assert_eq!(
        dangling_startup.issues()[3].root_id().unwrap().as_str(),
        "unknown-root"
    );
}

#[test]
fn manifest_validation_rejects_missing_root_commands_and_events() {
    let error = AppManifest::new(AppDescriptor::new(AppId::new("photo.lab"), "1.0"))
        .root(
            RootDescriptor::new(RootId::new("main"))
                .requires_command(CommandDescriptor::try_new("save", "SaveRequest").unwrap())
                .emits_event(EventDescriptor::try_new("saved", "SaveResult").unwrap()),
        )
        .validate()
        .unwrap_err();

    assert_eq!(
        error
            .issues()
            .iter()
            .map(ManifestValidationIssue::code)
            .collect::<Vec<_>>(),
        vec![
            ManifestValidationErrorCode::MissingCommand,
            ManifestValidationErrorCode::MissingEvent,
        ]
    );
    assert_eq!(error.issues()[0].root_id().unwrap().as_str(), "main");
    assert_eq!(error.issues()[0].command_name().unwrap().as_str(), "save");
    assert_eq!(error.issues()[1].root_id().unwrap().as_str(), "main");
    assert_eq!(error.issues()[1].event_name().unwrap().as_str(), "saved");
}

#[test]
fn manifest_validation_rejects_root_payload_type_mismatches() {
    let error = AppManifest::new(AppDescriptor::new(AppId::new("photo.lab"), "1.0"))
        .command(CommandDescriptor::try_new("save", "SaveRequest").unwrap())
        .event(EventDescriptor::try_new("saved", "SaveResult").unwrap())
        .root(
            RootDescriptor::new(RootId::new("main"))
                .requires_command(CommandDescriptor::try_new("save", "OtherRequest").unwrap())
                .emits_event(EventDescriptor::try_new("saved", "OtherResult").unwrap()),
        )
        .validate()
        .unwrap_err();

    assert_eq!(
        error
            .issues()
            .iter()
            .map(ManifestValidationIssue::code)
            .collect::<Vec<_>>(),
        vec![
            ManifestValidationErrorCode::CommandPayloadTypeMismatch,
            ManifestValidationErrorCode::EventPayloadTypeMismatch,
        ]
    );
    assert_eq!(error.issues()[0].root_id().unwrap().as_str(), "main");
    assert_eq!(error.issues()[0].command_name().unwrap().as_str(), "save");
    assert_eq!(
        error.issues()[0].expected_payload_type().unwrap().as_str(),
        "SaveRequest"
    );
    assert_eq!(
        error.issues()[0].actual_payload_type().unwrap().as_str(),
        "OtherRequest"
    );
    assert_eq!(error.issues()[1].root_id().unwrap().as_str(), "main");
    assert_eq!(error.issues()[1].event_name().unwrap().as_str(), "saved");
    assert_eq!(
        error.issues()[1].expected_payload_type().unwrap().as_str(),
        "SaveResult"
    );
    assert_eq!(
        error.issues()[1].actual_payload_type().unwrap().as_str(),
        "OtherResult"
    );
}

#[test]
fn validated_manifest_has_deterministic_lookup_iteration_and_app_ownership() {
    let manifest = AppManifest::new(AppDescriptor::new(AppId::new("photo.lab"), "1.0"))
        .command(CommandDescriptor::try_new("zeta", "ZetaCommand").unwrap())
        .command(CommandDescriptor::try_new("alpha", "AlphaCommand").unwrap())
        .event(EventDescriptor::try_new("zeta", "ZetaEvent").unwrap())
        .event(EventDescriptor::try_new("alpha", "AlphaEvent").unwrap())
        .task(TaskDescriptor::try_new(TaskIntentName::new("zeta"), "ZetaInput").unwrap())
        .task(TaskDescriptor::try_new(TaskIntentName::new("alpha"), "AlphaInput").unwrap())
        .resource(ResourceDescriptor::try_new(ResourceId::new("zeta"), "ZetaValue").unwrap())
        .resource(ResourceDescriptor::try_new(ResourceId::new("alpha"), "AlphaValue").unwrap())
        .window(WindowDescriptor::new(
            WindowDescriptorId::new("zeta"),
            "Zeta",
        ))
        .window(WindowDescriptor::new(
            WindowDescriptorId::new("alpha"),
            "Alpha",
        ))
        .root(RootDescriptor::new(RootId::new("zeta")))
        .root(RootDescriptor::new(RootId::new("alpha")))
        .startup_window(StartupWindow::new(
            WindowDescriptorId::new("zeta"),
            RootId::new("zeta"),
            AppScope::app(),
        ))
        .startup_window(StartupWindow::new(
            WindowDescriptorId::new("alpha"),
            RootId::new("alpha"),
            AppScope::app(),
        ));

    let validated = manifest.validate().unwrap();
    assert_eq!(validated.app().id().as_str(), "photo.lab");
    assert_eq!(
        validated
            .commands()
            .map(|descriptor| descriptor.name().as_str())
            .collect::<Vec<_>>(),
        ["alpha", "zeta"]
    );
    assert_eq!(
        validated
            .events()
            .map(|descriptor| descriptor.name().as_str())
            .collect::<Vec<_>>(),
        ["alpha", "zeta"]
    );
    assert_eq!(
        validated
            .tasks()
            .map(|descriptor| descriptor.name().as_str())
            .collect::<Vec<_>>(),
        ["alpha", "zeta"]
    );
    assert_eq!(
        validated
            .resources()
            .map(|descriptor| descriptor.id().as_str())
            .collect::<Vec<_>>(),
        ["alpha", "zeta"]
    );
    assert_eq!(
        validated
            .windows()
            .map(|descriptor| descriptor.id().as_str())
            .collect::<Vec<_>>(),
        ["alpha", "zeta"]
    );
    assert_eq!(
        validated
            .roots()
            .map(|descriptor| descriptor.id().as_str())
            .collect::<Vec<_>>(),
        ["alpha", "zeta"]
    );
    assert_eq!(
        validated
            .startup_windows()
            .map(|startup| (startup.window_id().as_str(), startup.root_id().as_str()))
            .collect::<Vec<_>>(),
        [("alpha", "alpha"), ("zeta", "zeta")]
    );
    assert_eq!(
        validated
            .command(&CommandName::try_new("alpha").unwrap())
            .unwrap()
            .payload_type()
            .as_str(),
        "AlphaCommand"
    );
    assert_eq!(
        validated
            .event(&EventName::try_new("alpha").unwrap())
            .unwrap()
            .payload_type()
            .as_str(),
        "AlphaEvent"
    );
    assert_eq!(
        validated
            .task(&TaskIntentName::new("alpha"))
            .unwrap()
            .input_type()
            .as_str(),
        "AlphaInput"
    );
    assert_eq!(
        validated
            .resource(&ResourceId::new("alpha"))
            .unwrap()
            .value_type()
            .as_str(),
        "AlphaValue"
    );
    assert_eq!(
        validated
            .window(&WindowDescriptorId::new("alpha"))
            .unwrap()
            .title(),
        "Alpha"
    );
    assert_eq!(
        validated.root(&RootId::new("alpha")).unwrap().id().as_str(),
        "alpha"
    );

    let app = App::try_new(AppManifest::new(AppDescriptor::new(
        AppId::new("empty.lab"),
        "1.0",
    )))
    .unwrap();
    assert_eq!(app.descriptor(), app.manifest().app());
    assert!(app.manifest().startup_windows().next().is_none());
}

#[test]
fn task_input_uses_runtime_intent_provenance() {
    let provenance =
        InputProvenance::task(TaskIntentId::from_u64(9), TaskIntentAttemptId::from_u64(4));
    let input = TaskInput::new(CounterInput::Increment, provenance.clone()).unwrap();

    assert_eq!(
        input.clone().into_app_input().provenance().task_id(),
        Some(TaskIntentId::from_u64(9))
    );
    assert_eq!(
        input.into_app_input().provenance().task_attempt_id(),
        Some(TaskIntentAttemptId::from_u64(4))
    );
}

#[test]
fn crate_identity_remains_runtime_after_task_boundary_cleanup() {
    assert_eq!(crate_name(), "surgeist-runtime");
}

#[test]
fn provenance_carries_causal_fields() {
    let parent = correlation(1);
    let surface = surface_ref(4, 2);
    let child = InputProvenance::task(TaskIntentId::from_u64(2), TaskIntentAttemptId::from_u64(3))
        .try_with_surface(surface)
        .unwrap()
        .with_correlation(correlation(5))
        .with_parent_correlation(parent);

    assert_eq!(child.source(), &InputSourceId::TASK);
    assert!(matches!(child.origin(), InputOrigin::Task(_)));
    assert_eq!(child.task_id(), Some(TaskIntentId::from_u64(2)));
    assert_eq!(
        child.task_attempt_id(),
        Some(TaskIntentAttemptId::from_u64(3))
    );
    assert_eq!(child.surface(), Some(surface));
    assert_eq!(child.correlation_id(), Some(correlation(5)));
    assert_eq!(child.parent_correlation_id(), Some(parent));
}

#[test]
fn provenance_correlation_rejects_zero_and_defaults_to_absent() {
    assert_eq!(CorrelationId::try_from_u64(0), Err(CorrelationError::Zero));
    assert_eq!(Correlation::default(), Correlation::Absent);
    assert!(Correlation::default().is_absent());
    assert_eq!(Correlation::default().id(), None);
}

#[test]
fn provenance_constructors_preserve_origin_and_start_without_causal_data() {
    let ui_surface = surface_ref(11, 1);
    let adapter_surface = surface_ref(12, 2);
    let window_surface = surface_ref(13, 3);
    let task_id = TaskIntentId::from_u64(21);
    let task_attempt_id = TaskIntentAttemptId::from_u64(22);
    let service_id = ServiceId::new("search");

    let system = InputProvenance::system();
    assert_eq!(system.source(), &InputSourceId::SYSTEM);
    assert!(matches!(system.origin(), InputOrigin::System));
    assert_empty_causality(&system);

    let ui = InputProvenance::ui(ui_surface);
    assert_eq!(ui.source(), &InputSourceId::UI);
    assert!(matches!(ui.origin(), InputOrigin::Ui(_)));
    assert_eq!(ui.surface(), Some(ui_surface));
    assert_empty_causality(&ui);

    let adapter = InputProvenance::adapter(adapter_surface);
    assert_eq!(adapter.source(), &InputSourceId::ADAPTER);
    assert!(matches!(adapter.origin(), InputOrigin::Adapter(_)));
    assert_eq!(adapter.surface(), Some(adapter_surface));
    assert_empty_causality(&adapter);

    let task = InputProvenance::task(task_id, task_attempt_id);
    assert_eq!(task.source(), &InputSourceId::TASK);
    assert!(matches!(task.origin(), InputOrigin::Task(_)));
    assert_eq!(task.task_id(), Some(task_id));
    assert_eq!(task.task_attempt_id(), Some(task_attempt_id));
    assert_eq!(task.surface(), None);
    assert_empty_causality(&task);

    let service = InputProvenance::service(service_id.clone());
    assert_eq!(service.source(), &InputSourceId::SERVICE);
    assert!(matches!(service.origin(), InputOrigin::Service(_)));
    assert_eq!(service.service_id(), Some(service_id));
    assert_empty_causality(&service);

    let window = InputProvenance::window(window_surface);
    assert_eq!(window.source(), &InputSourceId::WINDOW);
    assert!(matches!(window.origin(), InputOrigin::Window(_)));
    assert_eq!(window.surface(), Some(window_surface));
    assert_empty_causality(&window);
}

#[test]
fn provenance_correlation_and_sequence_fields_set_and_clear_independently() {
    let current = correlation(31);
    let parent = correlation(32);
    let provenance =
        InputProvenance::task(TaskIntentId::from_u64(1), TaskIntentAttemptId::from_u64(1))
            .with_correlation(current)
            .with_parent_correlation(parent)
            .with_sequence(7);

    assert_eq!(provenance.correlation(), Correlation::Present(current));
    assert_eq!(
        provenance.parent_correlation(),
        Correlation::Present(parent)
    );
    assert_eq!(provenance.sequence(), Some(7));
    assert_eq!(
        provenance.clone().with_correlation(current),
        provenance.clone(),
        "repeating the current correlation must be idempotent"
    );
    assert_eq!(
        provenance.clone().with_parent_correlation(parent),
        provenance.clone(),
        "repeating the parent correlation must be idempotent"
    );

    let without_current = provenance.clone().without_correlation();
    assert_eq!(without_current.correlation(), Correlation::Absent);
    assert_eq!(
        without_current.parent_correlation(),
        Correlation::Present(parent)
    );
    assert_eq!(without_current.sequence(), Some(7));

    let without_parent = provenance.clone().without_parent_correlation();
    assert_eq!(without_parent.correlation(), Correlation::Present(current));
    assert_eq!(without_parent.parent_correlation(), Correlation::Absent);
    assert_eq!(without_parent.sequence(), Some(7));

    let without_sequence = provenance.without_sequence();
    assert_eq!(
        without_sequence.correlation(),
        Correlation::Present(current)
    );
    assert_eq!(
        without_sequence.parent_correlation(),
        Correlation::Present(parent)
    );
    assert_eq!(without_sequence.sequence(), None);
}

#[test]
fn provenance_surface_attachment_is_generation_qualified_and_origin_safe() {
    let first = surface_ref(41, 1);
    let replacement = surface_ref(41, 2);
    let task = InputProvenance::task(TaskIntentId::from_u64(4), TaskIntentAttemptId::from_u64(5));
    let attached = task.clone().try_with_surface(first).unwrap();
    assert_eq!(attached.surface(), Some(first));
    assert_eq!(
        attached.clone().try_with_surface(first),
        Ok(attached.clone())
    );

    assert_surface_error(
        attached.clone().try_with_surface(replacement).unwrap_err(),
        ProvenanceErrorCode::SurfaceAlreadyAttached,
        attached.origin(),
        Some(first),
        replacement,
    );
    assert_eq!(attached.surface(), Some(first));

    for provenance in [
        InputProvenance::ui(first),
        InputProvenance::adapter(first),
        InputProvenance::window(first),
    ] {
        assert_eq!(
            provenance.clone().try_with_surface(first),
            Ok(provenance.clone())
        );
        assert_surface_error(
            provenance
                .clone()
                .try_with_surface(replacement)
                .unwrap_err(),
            ProvenanceErrorCode::SurfaceOverwriteUnsupported,
            provenance.origin(),
            Some(first),
            replacement,
        );
        assert_eq!(provenance.surface(), Some(first));
    }

    for provenance in [
        InputProvenance::system(),
        InputProvenance::service(ServiceId::new("sync")),
    ] {
        assert_surface_error(
            provenance.clone().try_with_surface(first).unwrap_err(),
            ProvenanceErrorCode::SurfaceUnsupportedOrigin,
            provenance.origin(),
            None,
            first,
        );
        assert_eq!(provenance.surface(), None);
    }
}

#[test]
fn diagnostics_keep_recent_entries_and_counters() {
    let mut log = DiagnosticLog::with_capacity(2);
    log.push(Diagnostic::warning(
        DiagnosticCode::UNKNOWN_RETAINED_COMMAND,
        "missing binding",
        InputProvenance::ui(surface_ref(1, 0)),
    ));
    log.push(
        Diagnostic::error(
            DiagnosticCode::REDUCER_ERROR,
            "reducer rejected input",
            InputProvenance::task(TaskIntentId::from_u64(2), TaskIntentAttemptId::from_u64(1)),
        )
        .with_app(AppId::new("photo.lab"))
        .with_window(WindowId::from_u64(9))
        .with_root(RootId::new("gallery"))
        .with_scope(AppScope::resource(ResourceId::new("thumbs")))
        .with_resource(ResourceId::new("thumbs"))
        .with_queue(QueueDiagnostic::new("task-events", 128).with_age_ms(17))
        .with_effect("request_redraw"),
    );
    log.push(Diagnostic::info(
        DiagnosticCode::QUEUE_COALESCED,
        "progress coalesced",
        InputProvenance::system(),
    ));

    let entries = log.entries();
    assert_eq!(entries.len(), 2);
    assert_eq!(log.dropped_oldest(), 1);
    assert_eq!(log.count(&DiagnosticCode::UNKNOWN_RETAINED_COMMAND), 1);
    assert_eq!(log.count(&DiagnosticCode::QUEUE_COALESCED), 1);
    assert_eq!(entries[0].code(), &DiagnosticCode::REDUCER_ERROR);
    assert_eq!(entries[0].app_id(), Some(&AppId::new("photo.lab")));
    assert_eq!(entries[0].window_id(), Some(WindowId::from_u64(9)));
    assert_eq!(entries[0].root_id(), Some(&RootId::new("gallery")));
    assert_eq!(entries[0].resource_id(), Some(&ResourceId::new("thumbs")));
    assert_eq!(entries[0].emitted_effects(), &["request_redraw"]);
    assert_eq!(entries[0].queue().unwrap().capacity(), 128);
    assert_eq!(entries[0].queue().unwrap().age_ms(), Some(17));
}

#[test]
fn zero_capacity_diagnostic_log_counts_without_retaining_entries() {
    let mut log = DiagnosticLog::with_capacity(0);
    log.push(Diagnostic::warning(
        DiagnosticCode::QUEUE_OVERFLOW,
        "queue disabled",
        InputProvenance::system(),
    ));

    assert!(log.entries().is_empty());
    assert_eq!(log.dropped_oldest(), 1);
    assert_eq!(log.count(&DiagnosticCode::QUEUE_OVERFLOW), 1);
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CounterState {
    value: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CounterInput {
    Increment,
    RedrawAll,
    RedrawWindow(WindowId),
    StartTask,
}

#[test]
fn app_proxy_coalesces_wakeups_while_queue_is_non_empty() {
    let wake = FakeWakeBridge::default();
    let proxy = AppProxy::<CounterInput>::new(wake.clone(), QueuePolicy::bounded(16));

    proxy
        .send_task(
            TaskInput::new(
                CounterInput::Increment,
                InputProvenance::task(TaskIntentId::from_u64(1), TaskIntentAttemptId::from_u64(1)),
            )
            .unwrap(),
        )
        .unwrap();
    proxy
        .send_task(
            TaskInput::new(
                CounterInput::Increment,
                InputProvenance::task(TaskIntentId::from_u64(1), TaskIntentAttemptId::from_u64(1)),
            )
            .unwrap(),
        )
        .unwrap();

    assert_eq!(wake.wake_count(), 1);
    assert_eq!(proxy.pending_len(), 2);

    let drained = proxy.drain_pending(NonZeroUsize::new(8).unwrap());
    assert_eq!(drained.drained().len(), 2);
    assert_eq!(drained.remaining_len(), 0);
    assert!(!drained.has_remaining());
    assert!(drained.continuation_wake_error().is_none());
    assert_eq!(proxy.pending_len(), 0);
}

#[test]
fn app_proxy_reports_closed_native_wake_bridge() {
    let wake = FakeWakeBridge::closed();
    let proxy = AppProxy::<CounterInput>::new(wake, QueuePolicy::bounded(16));

    let error = proxy
        .send_task(
            TaskInput::new(
                CounterInput::Increment,
                InputProvenance::task(TaskIntentId::from_u64(1), TaskIntentAttemptId::from_u64(1)),
            )
            .unwrap(),
        )
        .unwrap_err();

    assert_eq!(error.code(), AppProxyErrorCode::WakeFailed);
    assert_eq!(error.capacity(), None);
    assert!(error.wake_error().is_some());
    assert!(Error::source(&error).is_some());
}

#[test]
fn app_proxy_policy_errors_and_exact_rejected_inputs_are_lossless() {
    assert_eq!(QueuePolicy::default(), QueuePolicy::bounded(65_536));
    assert_eq!(QueuePolicy::bounded(0).capacity(), 0);

    let proxy = AppProxy::<CounterInput>::new(FakeWakeBridge::default(), QueuePolicy::bounded(0));
    let input = counter_task_input(CounterInput::Increment);
    let error = proxy.send_task(input.clone()).unwrap_err();

    assert_eq!(error.code(), AppProxyErrorCode::QueueOverflow);
    assert_eq!(error.capacity(), Some(0));
    assert_eq!(error.rejected(), &ProxyInput::Task(input.clone()));
    assert!(error.wake_error().is_none());
    assert_eq!(error.into_rejected(), ProxyInput::Task(input));
    assert!(
        Error::source(
            &proxy
                .send_task(counter_task_input(CounterInput::Increment))
                .unwrap_err()
        )
        .is_none()
    );
    assert_eq!(proxy.pending_len(), 0);
}

#[test]
fn app_proxy_defers_host_drain_until_send_has_returned() {
    let wake = DeferredWakeBridge::default();
    let proxy = AppProxy::<CounterInput>::new(wake.clone(), QueuePolicy::bounded(4));

    proxy
        .send_task(counter_task_input(CounterInput::Increment))
        .unwrap();

    assert_eq!(wake.wake_count(), 1);
    assert_eq!(proxy.pending_len(), 1);
    let report = proxy.drain_pending(NonZeroUsize::new(1).unwrap());
    assert_eq!(report.into_drained().len(), 1);
}

fn wait_for_proxy_milestone(milestone: &str, expected: usize, mut observed: impl FnMut() -> usize) {
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        let last_observed = observed();
        if last_observed == expected {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {milestone}: expected {expected}, last observed {last_observed}"
        );
        thread::yield_now();
    }
}

#[test]
fn app_proxy_waiter_resignals_after_failed_owner_wake_while_drain_waits() {
    let (wake, releases, started) = BlockingWakeBridge::new();
    let proxy = Arc::new(AppProxy::<CounterInput>::new(wake, QueuePolicy::bounded(4)));
    let owner_input = counter_task_input(CounterInput::Increment);
    let waiter_input = counter_task_input(CounterInput::RedrawAll);
    let owner_proxy = Arc::clone(&proxy);
    let (owner_done_tx, owner_done_rx) = mpsc::channel();
    let owner_input_for_thread = owner_input.clone();
    let owner = thread::spawn(move || {
        owner_done_tx
            .send(owner_proxy.send_task(owner_input_for_thread))
            .unwrap();
    });

    assert_eq!(started.recv_timeout(Duration::from_secs(1)).unwrap(), 1);
    let waiter_proxy = Arc::clone(&proxy);
    let (waiter_done_tx, waiter_done_rx) = mpsc::channel();
    let waiter_input_for_thread = waiter_input.clone();
    let waiter = thread::spawn(move || {
        waiter_done_tx
            .send(waiter_proxy.send_task(waiter_input_for_thread))
            .unwrap();
    });
    wait_for_proxy_milestone("concurrent sender enqueue", 2, || proxy.pending_len());
    wait_for_proxy_milestone("concurrent sender wait registration", 1, || {
        proxy.waiting_sender_count()
    });

    let drain_proxy = Arc::clone(&proxy);
    let (drain_done_tx, drain_done_rx) = mpsc::channel();
    let drainer = thread::spawn(move || {
        drain_done_tx
            .send(drain_proxy.drain_pending(NonZeroUsize::new(4).unwrap()))
            .unwrap();
    });
    wait_for_proxy_milestone("racing drain condition-variable wait", 1, || {
        proxy.waiting_drain_count()
    });

    releases
        .send(Err(WakeError::new("first wake failed")))
        .unwrap();
    let owner_error = owner_done_rx
        .recv_timeout(Duration::from_secs(1))
        .unwrap()
        .unwrap_err();
    assert_eq!(owner_error.code(), AppProxyErrorCode::WakeFailed);
    assert_eq!(owner_error.rejected(), &ProxyInput::Task(owner_input));
    assert_eq!(
        owner_error.wake_error(),
        Some(&WakeError::new("first wake failed"))
    );
    assert_eq!(proxy.pending_len(), 1);

    assert_eq!(started.recv_timeout(Duration::from_secs(1)).unwrap(), 2);
    releases.send(Ok(())).unwrap();
    assert!(
        waiter_done_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .is_ok()
    );
    owner.join().unwrap();
    waiter.join().unwrap();

    assert_eq!(
        drain_done_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .into_drained(),
        vec![ProxyInput::Task(waiter_input)]
    );
    drainer.join().unwrap();
    assert_eq!(proxy.pending_len(), 0);
}

#[test]
fn app_proxy_partial_drain_successor_wake_covers_racing_sender() {
    let (wake, releases, started) = BlockingWakeBridge::new();
    let proxy = Arc::new(AppProxy::<CounterInput>::new(wake, QueuePolicy::bounded(4)));
    let first_input = counter_task_input(CounterInput::Increment);
    let second_input = counter_task_input(CounterInput::RedrawAll);
    let racing_input = counter_task_input(CounterInput::StartTask);
    let first_proxy = Arc::clone(&proxy);
    let (first_done_tx, first_done_rx) = mpsc::channel();
    let first_input_for_thread = first_input.clone();
    let first_sender = thread::spawn(move || {
        first_done_tx
            .send(first_proxy.send_task(first_input_for_thread))
            .unwrap();
    });
    assert_eq!(started.recv_timeout(Duration::from_secs(1)).unwrap(), 1);
    releases.send(Ok(())).unwrap();
    assert!(
        first_done_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .is_ok()
    );
    first_sender.join().unwrap();
    proxy.send_task(second_input.clone()).unwrap();

    let drain_proxy = Arc::clone(&proxy);
    let (drain_done_tx, drain_done_rx) = mpsc::channel();
    let drainer = thread::spawn(move || {
        drain_done_tx
            .send(drain_proxy.drain_pending(NonZeroUsize::new(1).unwrap()))
            .unwrap();
    });
    assert_eq!(started.recv_timeout(Duration::from_secs(1)).unwrap(), 2);

    let racing_proxy = Arc::clone(&proxy);
    let (racing_done_tx, racing_done_rx) = mpsc::channel();
    let racing_input_for_thread = racing_input.clone();
    let racing_sender = thread::spawn(move || {
        racing_done_tx
            .send(racing_proxy.send_task(racing_input_for_thread))
            .unwrap();
    });
    wait_for_proxy_milestone("racing sender enqueue behind drain wake", 2, || {
        proxy.pending_len()
    });
    wait_for_proxy_milestone("racing sender wait registration", 1, || {
        proxy.waiting_sender_count()
    });

    releases.send(Ok(())).unwrap();
    let report = drain_done_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(report.drained(), &[ProxyInput::Task(first_input)]);
    assert_eq!(report.remaining_len(), 2);
    assert!(report.has_remaining());
    assert!(report.continuation_wake_error().is_none());
    assert!(
        racing_done_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .is_ok()
    );
    drainer.join().unwrap();
    racing_sender.join().unwrap();
    assert_eq!(proxy.pending_len(), 2);

    let final_report = proxy.drain_pending(NonZeroUsize::MAX);
    assert_eq!(
        final_report.drained(),
        &[
            ProxyInput::Task(second_input),
            ProxyInput::Task(racing_input)
        ]
    );
    assert_eq!(proxy.pending_len(), 0);
}

#[test]
fn app_proxy_partial_drain_failure_waiter_resignals_without_stranding_inputs() {
    let (wake, releases, started) = BlockingWakeBridge::new();
    let proxy = Arc::new(AppProxy::<CounterInput>::new(wake, QueuePolicy::bounded(4)));
    let first_input = counter_task_input(CounterInput::Increment);
    let second_input = counter_task_input(CounterInput::RedrawAll);
    let racing_input = counter_task_input(CounterInput::StartTask);
    let first_proxy = Arc::clone(&proxy);
    let (first_done_tx, first_done_rx) = mpsc::channel();
    let first_input_for_thread = first_input.clone();
    let first_sender = thread::spawn(move || {
        first_done_tx
            .send(first_proxy.send_task(first_input_for_thread))
            .unwrap();
    });
    assert_eq!(started.recv_timeout(Duration::from_secs(1)).unwrap(), 1);
    releases.send(Ok(())).unwrap();
    assert!(
        first_done_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .is_ok()
    );
    first_sender.join().unwrap();
    proxy.send_task(second_input.clone()).unwrap();

    let drain_proxy = Arc::clone(&proxy);
    let (drain_done_tx, drain_done_rx) = mpsc::channel();
    let drainer = thread::spawn(move || {
        drain_done_tx
            .send(drain_proxy.drain_pending(NonZeroUsize::new(1).unwrap()))
            .unwrap();
    });
    assert_eq!(started.recv_timeout(Duration::from_secs(1)).unwrap(), 2);

    let racing_proxy = Arc::clone(&proxy);
    let (racing_done_tx, racing_done_rx) = mpsc::channel();
    let racing_input_for_thread = racing_input.clone();
    let racing_sender = thread::spawn(move || {
        racing_done_tx
            .send(racing_proxy.send_task(racing_input_for_thread))
            .unwrap();
    });
    wait_for_proxy_milestone("racing sender enqueue behind failed drain wake", 2, || {
        proxy.pending_len()
    });
    wait_for_proxy_milestone("racing sender wait registration", 1, || {
        proxy.waiting_sender_count()
    });

    releases
        .send(Err(WakeError::new("continuation failed")))
        .unwrap();
    let report = drain_done_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(report.drained(), &[ProxyInput::Task(first_input)]);
    assert_eq!(report.remaining_len(), 2);
    assert_eq!(
        report.continuation_wake_error(),
        Some(&WakeError::new("continuation failed"))
    );
    assert_eq!(started.recv_timeout(Duration::from_secs(1)).unwrap(), 3);
    releases.send(Ok(())).unwrap();
    assert!(
        racing_done_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .is_ok()
    );
    drainer.join().unwrap();
    racing_sender.join().unwrap();

    let final_report = proxy.drain_pending(NonZeroUsize::MAX);
    assert_eq!(
        final_report.drained(),
        &[
            ProxyInput::Task(second_input),
            ProxyInput::Task(racing_input)
        ]
    );
    assert_eq!(final_report.remaining_len(), 0);
    assert!(!final_report.has_remaining());
    assert!(final_report.continuation_wake_error().is_none());
    assert_eq!(proxy.pending_len(), 0);
}

fn counter_task_input(payload: CounterInput) -> TaskInput<CounterInput> {
    TaskInput::new(
        payload,
        InputProvenance::task(TaskIntentId::from_u64(1), TaskIntentAttemptId::from_u64(1)),
    )
    .unwrap()
}

#[derive(Clone, Default)]
struct DeferredWakeBridge {
    wakes: Arc<Mutex<usize>>,
}

impl DeferredWakeBridge {
    fn wake_count(&self) -> usize {
        *self.wakes.lock().unwrap()
    }
}

impl WakeBridge for DeferredWakeBridge {
    fn wake(&self) -> Result<(), WakeError> {
        *self.wakes.lock().unwrap() += 1;
        Ok(())
    }
}

struct BlockingWakeBridge {
    wakes: Arc<Mutex<usize>>,
    started: mpsc::Sender<usize>,
    releases: Arc<Mutex<mpsc::Receiver<Result<(), WakeError>>>>,
}

impl BlockingWakeBridge {
    fn new() -> (
        Self,
        mpsc::Sender<Result<(), WakeError>>,
        mpsc::Receiver<usize>,
    ) {
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        (
            Self {
                wakes: Arc::new(Mutex::new(0)),
                started: started_tx,
                releases: Arc::new(Mutex::new(release_rx)),
            },
            release_tx,
            started_rx,
        )
    }
}

impl WakeBridge for BlockingWakeBridge {
    fn wake(&self) -> Result<(), WakeError> {
        let wake = {
            let mut wakes = self.wakes.lock().unwrap();
            *wakes += 1;
            *wakes
        };
        self.started.send(wake).unwrap();
        self.releases.lock().unwrap().recv().unwrap()
    }
}

#[test]
fn fake_clock_advances_scheduled_effects_deterministically() {
    let mut harness = HeadlessHarness::counter();
    harness.schedule_timer("debounce", Duration::from_millis(50));

    assert!(harness.due_timers().is_empty());
    harness.clock_mut().advance(Duration::from_millis(50));

    assert_eq!(harness.due_timers(), vec!["debounce"]);
}

struct CounterReducer;

impl Reducer<CounterState, CounterInput> for CounterReducer {
    fn reduce(
        &mut self,
        state: &CounterState,
        input: &AppInput<CounterInput>,
    ) -> ReducerResult<CounterState> {
        match input.payload() {
            CounterInput::Increment => ReducerResult::changed(
                CounterState {
                    value: state.value + 1,
                },
                ReducerCommit::new().with_effect(AppEffect::request_redraw(RedrawTarget::surface(
                    SurfaceRef::new(SurfaceId::from_u64(1), SurfaceGeneration::initial()),
                ))),
            ),
            CounterInput::RedrawAll => ReducerResult::unchanged(
                ReducerCommit::new().with_effect(AppEffect::request_redraw(RedrawTarget::all())),
            ),
            CounterInput::RedrawWindow(window_id) => ReducerResult::unchanged(
                ReducerCommit::new()
                    .with_effect(AppEffect::request_redraw(RedrawTarget::Window(*window_id))),
            ),
            CounterInput::StartTask => ReducerResult::changed(
                state.clone(),
                ReducerCommit::new().with_effect(AppEffect::start_task(
                    TaskIntentName::new("counter"),
                    TaskIntentKey::new("counter:increment"),
                    AppScope::app(),
                )),
            ),
        }
    }
}

#[test]
fn reducer_borrows_state_and_input_and_returns_a_replacement_state() {
    let mut reducer = CounterReducer;
    let state = CounterState::default();
    let input = AppInput::new(CounterInput::Increment, InputProvenance::system());
    let result = reducer.reduce(&state, &input);

    assert_eq!(state.value, 0);
    assert!(matches!(input.payload(), CounterInput::Increment));
    match result {
        ReducerResult::Changed(change) => {
            assert_eq!(change.state().value, 1);
            assert_eq!(change.commit().effects().effects().len(), 1);
            assert_eq!(
                change.commit().effects().effects()[0].kind(),
                &EffectKindId::REQUEST_REDRAW
            );
        }
        ReducerResult::Unchanged(_) | ReducerResult::RecoverableFailure(_) => {
            panic!("increment must return a changed reducer result")
        }
    }
}

#[test]
fn reducer_success_commits_construct_effects_and_provenance_explicitly() {
    let provenance = InputProvenance::system().with_sequence(17);
    let effects = EffectBatch::new()
        .push(AppEffect::persist("counter", AppScope::app()))
        .push(AppEffect::request_redraw(RedrawTarget::all()));
    let commit = ReducerCommit::default()
        .with_effect(AppEffect::diagnostic(Diagnostic::info(
            DiagnosticCode::QUEUE_COALESCED,
            "counter commit",
            InputProvenance::system(),
        )))
        .with_effects(effects)
        .with_provenance(provenance.clone());

    assert!(ReducerCommit::new().effects().effects().is_empty());
    assert_eq!(ReducerCommit::new().provenance(), None);
    assert_eq!(commit.provenance(), Some(&provenance));
    assert_eq!(commit.effects().effects().len(), 2);
    assert_eq!(commit.effects().effects()[0].kind(), &EffectKindId::PERSIST);
    assert_eq!(
        commit.effects().effects()[1].kind(),
        &EffectKindId::REQUEST_REDRAW
    );

    let changed = ReducerResult::changed(CounterState { value: 2 }, commit);
    match changed {
        ReducerResult::Changed(change) => {
            assert_eq!(change.state(), &CounterState { value: 2 });
            assert_eq!(change.commit().provenance(), Some(&provenance));
        }
        ReducerResult::Unchanged(_) | ReducerResult::RecoverableFailure(_) => {
            panic!("changed constructor must retain state and commit")
        }
    }
}

#[test]
fn reducer_failure_cannot_commit_state_or_effects() {
    let provenance = InputProvenance::system().with_sequence(18);
    let result: ReducerResult<CounterState> = ReducerResult::recoverable_failure(
        ReducerFailure::new("counter reducer rejected input").with_provenance(provenance.clone()),
    );

    match result {
        ReducerResult::RecoverableFailure(failure) => {
            assert_eq!(failure.message(), "counter reducer rejected input");
            assert_eq!(failure.provenance(), Some(&provenance));
        }
        ReducerResult::Unchanged(_) | ReducerResult::Changed(_) => {
            panic!("recoverable failure must remain disjoint from successful commits")
        }
    }
}

#[test]
fn state_version_checked_next_rejects_overflow_without_changing_the_original() {
    let version = StateVersion::from_u64(u64::MAX);

    assert_eq!(version.checked_next(), Err(VersionError::Overflow));
    assert_eq!(version, StateVersion::from_u64(u64::MAX));
}

#[test]
fn runtime_commits_state_before_executing_effects() {
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    runtime
        .register_surface(test_surface(1, 1, "main"))
        .unwrap();
    let surface = runtime.surface_ref(SurfaceId::from_u64(1)).unwrap();
    ready_surface(&mut runtime, surface);

    runtime
        .enqueue_ui(UiInput::new(CounterInput::Increment, InputProvenance::system()).unwrap())
        .unwrap();
    let report = runtime.drain_once(RuntimeBudget::default()).unwrap();

    assert_eq!(runtime.state().value, 1);
    assert_eq!(runtime.state_version(), StateVersion::from_u64(1));
    assert_eq!(report.applied_effects(), 1);
    assert_eq!(report.redraw_requests(), &[surface]);
}

#[test]
fn runtime_forwards_task_work_as_intents_without_executing_it() {
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    runtime
        .enqueue_ui(
            UiInput::new(
                CounterInput::StartTask,
                InputProvenance::ui(surface_ref(1, 0)),
            )
            .unwrap(),
        )
        .unwrap();

    let report = runtime.drain_once(RuntimeBudget::default()).unwrap();

    assert_eq!(report.forwarded_effects(), 1);
    assert_eq!(report.intents().len(), 1);
    assert_eq!(
        report.effect_outcomes()[0].kind().as_str(),
        "runtime.start_task",
    );
    assert_eq!(runtime.diagnostics().entries().len(), 0);
}

#[test]
fn runtime_drains_eligible_lanes_in_cyclic_order_and_respects_budget() {
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    runtime
        .enqueue_task(
            TaskInput::new(
                CounterInput::Increment,
                InputProvenance::task(TaskIntentId::from_u64(1), TaskIntentAttemptId::from_u64(1)),
            )
            .unwrap(),
        )
        .unwrap();
    runtime
        .enqueue_ui(
            UiInput::new(
                CounterInput::Increment,
                InputProvenance::ui(surface_ref(1, 0)),
            )
            .unwrap(),
        )
        .unwrap();

    let report = runtime
        .drain_once(RuntimeBudget::default().with_max_inputs(1))
        .unwrap();

    assert_eq!(runtime.state().value, 1);
    assert_eq!(report.drained_inputs(), 1);
    assert_eq!(report.first_drained_lane(), Some(RuntimeLane::Ui));
    assert_eq!(
        runtime
            .drain_once(RuntimeBudget::default())
            .unwrap()
            .drained_inputs(),
        1
    );
}

#[test]
fn runtime_budget_construction_builders_accessors_and_zero_values_are_exact() {
    assert_eq!(RuntimeBudget::default(), RuntimeBudget::new(64, 32, 32, 32));

    let budget = RuntimeBudget::new(1, 2, 3, 4)
        .with_max_inputs(5)
        .with_max_ui_inputs(6)
        .with_max_task_inputs(7)
        .with_max_service_inputs(8);
    assert_eq!(budget.max_inputs(), 5);
    assert_eq!(budget.max_ui_inputs(), 6);
    assert_eq!(budget.max_task_inputs(), 7);
    assert_eq!(budget.max_service_inputs(), 8);

    let zero = RuntimeBudget::new(0, 0, 0, 0);
    assert_eq!(zero.max_inputs(), 0);
    assert_eq!(zero.max_ui_inputs(), 0);
    assert_eq!(zero.max_task_inputs(), 0);
    assert_eq!(zero.max_service_inputs(), 0);

    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    runtime
        .enqueue_ui(UiInput::new(CounterInput::Increment, InputProvenance::system()).unwrap())
        .unwrap();
    let report = runtime.drain_once(RuntimeBudget::new(0, 1, 1, 1)).unwrap();
    assert_eq!(report.drained_inputs(), 0);
    assert_eq!(report.remaining_ui_inputs(), 1);
    assert!(report.has_pending_inputs());
}

#[test]
fn runtime_reports_all_pending_lanes_and_stops_when_only_exhausted_lanes_remain() {
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    runtime
        .enqueue_ui(UiInput::new(CounterInput::Increment, InputProvenance::system()).unwrap())
        .unwrap();
    runtime
        .enqueue_task(
            TaskInput::new(
                CounterInput::Increment,
                InputProvenance::task(TaskIntentId::from_u64(1), TaskIntentAttemptId::from_u64(1)),
            )
            .unwrap(),
        )
        .unwrap();
    runtime
        .enqueue_service(
            ServiceInput::new(
                CounterInput::Increment,
                InputProvenance::service(ServiceId::new("runtime-test")),
            )
            .unwrap(),
        )
        .unwrap();

    let report = runtime.drain_once(RuntimeBudget::new(3, 1, 0, 1)).unwrap();

    assert_eq!(report.drained_inputs(), 2);
    assert_eq!(report.remaining_ui_inputs(), 0);
    assert_eq!(report.remaining_task_inputs(), 1);
    assert_eq!(report.remaining_service_inputs(), 0);
    assert!(report.has_pending_inputs());

    let report = runtime.drain_once(RuntimeBudget::new(3, 0, 0, 0)).unwrap();
    assert_eq!(report.drained_inputs(), 0);
    assert_eq!(report.remaining_ui_inputs(), 0);
    assert_eq!(report.remaining_task_inputs(), 1);
    assert_eq!(report.remaining_service_inputs(), 0);
    assert!(report.has_pending_inputs());

    let report = runtime.drain_once(RuntimeBudget::new(3, 0, 1, 0)).unwrap();
    assert_eq!(report.drained_inputs(), 1);
    assert_eq!(report.remaining_ui_inputs(), 0);
    assert_eq!(report.remaining_task_inputs(), 0);
    assert_eq!(report.remaining_service_inputs(), 0);
    assert!(!report.has_pending_inputs());
}

#[test]
fn runtime_single_input_drains_rotate_across_mixed_backlogs_without_starving_service() {
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    for index in 0..6 {
        runtime
            .enqueue_ui(UiInput::new(CounterInput::Increment, InputProvenance::system()).unwrap())
            .unwrap();
        runtime
            .enqueue_task(
                TaskInput::new(
                    CounterInput::Increment,
                    InputProvenance::task(
                        TaskIntentId::from_u64(index + 1),
                        TaskIntentAttemptId::from_u64(1),
                    ),
                )
                .unwrap(),
            )
            .unwrap();
    }
    for index in 0..2 {
        runtime
            .enqueue_service(
                ServiceInput::new(
                    CounterInput::Increment,
                    InputProvenance::service(ServiceId::new(format!("service-{index}"))),
                )
                .unwrap(),
            )
            .unwrap();
    }

    let lanes = (0..8)
        .map(|_| {
            runtime
                .drain_once(RuntimeBudget::new(1, 1, 1, 1))
                .unwrap()
                .first_drained_lane()
        })
        .collect::<Vec<_>>();

    assert_eq!(
        lanes,
        vec![
            Some(RuntimeLane::Ui),
            Some(RuntimeLane::Task),
            Some(RuntimeLane::Service),
            Some(RuntimeLane::Ui),
            Some(RuntimeLane::Task),
            Some(RuntimeLane::Service),
            Some(RuntimeLane::Ui),
            Some(RuntimeLane::Task),
        ]
    );
}

#[test]
fn runtime_overflow_requeues_target_lane_before_later_peer_and_reports_complete_pending_work() {
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    runtime.set_state_version_for_test(StateVersion::from_u64(u64::MAX));
    runtime
        .enqueue_ui(UiInput::new(CounterInput::RedrawAll, InputProvenance::system()).unwrap())
        .unwrap();
    let failed = InputProvenance::task(TaskIntentId::from_u64(1), TaskIntentAttemptId::from_u64(1));
    runtime
        .enqueue_task(TaskInput::new(CounterInput::Increment, failed.clone()).unwrap())
        .unwrap();
    runtime
        .enqueue_task(
            TaskInput::new(
                CounterInput::RedrawAll,
                InputProvenance::task(TaskIntentId::from_u64(2), TaskIntentAttemptId::from_u64(1)),
            )
            .unwrap(),
        )
        .unwrap();

    let error = runtime.drain_once(RuntimeBudget::default()).unwrap_err();

    assert_eq!(error.lane(), RuntimeLane::Task);
    assert_eq!(error.provenance(), &failed);
    assert_eq!(error.partial_report().drained_inputs(), 1);
    assert_eq!(
        error.partial_report().first_drained_lane(),
        Some(RuntimeLane::Ui)
    );
    assert_eq!(error.partial_report().remaining_ui_inputs(), 0);
    assert_eq!(error.partial_report().remaining_task_inputs(), 2);
    assert_eq!(error.partial_report().remaining_service_inputs(), 0);
    assert!(error.partial_report().has_pending_inputs());

    runtime.set_state_version_for_test(StateVersion::initial());
    let report = runtime.drain_once(RuntimeBudget::new(1, 1, 1, 1)).unwrap();
    assert_eq!(report.first_drained_lane(), Some(RuntimeLane::Task));
    assert_eq!(runtime.state().value, 1);
    assert_eq!(report.remaining_task_inputs(), 1);
    assert!(report.has_pending_inputs());
}

#[test]
fn runtime_default_budget_caps_drained_inputs() {
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    for index in 0..65 {
        runtime
            .enqueue_task(
                TaskInput::new(
                    CounterInput::Increment,
                    InputProvenance::task(
                        TaskIntentId::from_u64(index),
                        TaskIntentAttemptId::from_u64(1),
                    ),
                )
                .unwrap(),
            )
            .unwrap();
    }

    let report = runtime.drain_once(RuntimeBudget::default()).unwrap();

    assert_eq!(runtime.state().value, 32);
    assert_eq!(report.drained_inputs(), 32);
    assert_eq!(
        runtime
            .drain_once(RuntimeBudget::default())
            .unwrap()
            .drained_inputs(),
        32
    );
    assert_eq!(runtime.state().value, 64);
    assert_eq!(
        runtime
            .drain_once(RuntimeBudget::default())
            .unwrap()
            .drained_inputs(),
        1
    );
}

#[test]
fn runtime_queue_policy_defaults_builders_and_new_construction_are_exact() {
    let default_policy = RuntimeQueuePolicy::default();
    assert_eq!(
        default_policy,
        RuntimeQueuePolicy::new(65_536, 65_536, 65_536)
    );
    assert_eq!(default_policy.ui_capacity(), 65_536);
    assert_eq!(default_policy.task_capacity(), 65_536);
    assert_eq!(default_policy.service_capacity(), 65_536);

    let custom_policy = RuntimeQueuePolicy::new(1, 2, 3)
        .with_ui_capacity(4)
        .with_task_capacity(5)
        .with_service_capacity(6);
    assert_eq!(custom_policy.ui_capacity(), 4);
    assert_eq!(custom_policy.task_capacity(), 5);
    assert_eq!(custom_policy.service_capacity(), 6);

    let default_runtime = Runtime::<CounterState, CounterReducer, CounterInput>::new(
        CounterState::default(),
        CounterReducer,
    );
    let custom_runtime =
        Runtime::<CounterState, CounterReducer, CounterInput>::new_with_queue_policy(
            CounterState::default(),
            CounterReducer,
            custom_policy,
        );
    assert_eq!(default_runtime.queue_policy(), default_policy);
    assert_eq!(custom_runtime.queue_policy(), custom_policy);
}

#[test]
fn zero_capacity_rejects_each_lane_with_exact_input_and_one_diagnostic() {
    let policy = RuntimeQueuePolicy::new(0, 0, 0);
    let mut runtime =
        Runtime::new_with_queue_policy(CounterState::default(), CounterReducer, policy);

    let ui = UiInput::new(CounterInput::Increment, InputProvenance::system()).unwrap();
    let ui_error = runtime.enqueue_ui(ui.clone()).unwrap_err();
    assert_eq!(ui_error.code(), RuntimeQueueErrorCode::Overflow);
    assert_eq!(ui_error.lane(), RuntimeLane::Ui);
    assert_eq!(ui_error.capacity(), 0);
    assert_eq!(ui_error.rejected(), &ui);
    assert_eq!(
        ui_error.to_string(),
        "runtime Ui queue overflow at capacity 0"
    );
    assert!(std::error::Error::source(&ui_error).is_none());
    assert_eq!(ui_error.into_rejected(), ui);

    let task = TaskInput::new(
        CounterInput::Increment,
        InputProvenance::task(TaskIntentId::from_u64(1), TaskIntentAttemptId::from_u64(1)),
    )
    .unwrap();
    let task_error = runtime.enqueue_task(task.clone()).unwrap_err();
    assert_eq!(task_error.code(), RuntimeQueueErrorCode::Overflow);
    assert_eq!(task_error.lane(), RuntimeLane::Task);
    assert_eq!(task_error.capacity(), 0);
    assert_eq!(task_error.rejected(), &task);
    assert_eq!(task_error.into_rejected(), task);

    let service = ServiceInput::new(
        CounterInput::Increment,
        InputProvenance::service(ServiceId::new("counter")),
    )
    .unwrap();
    let service_error = runtime.enqueue_service(service.clone()).unwrap_err();
    assert_eq!(service_error.code(), RuntimeQueueErrorCode::Overflow);
    assert_eq!(service_error.lane(), RuntimeLane::Service);
    assert_eq!(service_error.capacity(), 0);
    assert_eq!(service_error.rejected(), &service);
    assert_eq!(service_error.into_rejected(), service);

    assert_eq!(
        runtime.diagnostics().count(&DiagnosticCode::QUEUE_OVERFLOW),
        3
    );
    let diagnostics = runtime.diagnostics().entries();
    let diagnostic = diagnostics[0].queue().unwrap();
    assert_eq!(diagnostic.name(), "runtime.ui");
    assert_eq!(diagnostic.capacity(), 0);
    assert_eq!(diagnostic.dropped(), 0);
    assert_eq!(
        runtime
            .drain_once(RuntimeBudget::default())
            .unwrap()
            .drained_inputs(),
        0
    );
}

#[test]
fn full_queues_reject_newest_without_reordering_and_allow_exact_retry_after_space() {
    let mut runtime = Runtime::new_with_queue_policy(
        QueueState::default(),
        QueueReducer,
        RuntimeQueuePolicy::new(1, 2, 1),
    );

    runtime
        .enqueue_ui(UiInput::new(QueueInput(10), InputProvenance::system()).unwrap())
        .unwrap();
    let rejected_ui = UiInput::new(QueueInput(11), InputProvenance::system()).unwrap();
    assert_eq!(
        runtime
            .enqueue_ui(rejected_ui.clone())
            .unwrap_err()
            .into_rejected(),
        rejected_ui
    );

    runtime
        .enqueue_task(
            TaskInput::new(
                QueueInput(20),
                InputProvenance::task(TaskIntentId::from_u64(20), TaskIntentAttemptId::from_u64(1)),
            )
            .unwrap(),
        )
        .unwrap();
    runtime
        .enqueue_task(
            TaskInput::new(
                QueueInput(21),
                InputProvenance::task(TaskIntentId::from_u64(21), TaskIntentAttemptId::from_u64(1)),
            )
            .unwrap(),
        )
        .unwrap();
    let rejected_task = TaskInput::new(
        QueueInput(22),
        InputProvenance::task(TaskIntentId::from_u64(22), TaskIntentAttemptId::from_u64(1)),
    )
    .unwrap();
    let task_error = runtime.enqueue_task(rejected_task.clone()).unwrap_err();
    assert_eq!(task_error.code(), RuntimeQueueErrorCode::Overflow);
    assert_eq!(task_error.lane(), RuntimeLane::Task);
    assert_eq!(task_error.capacity(), 2);
    assert_eq!(task_error.rejected(), &rejected_task);
    let rejected_task = task_error.into_rejected();

    runtime
        .enqueue_service(
            ServiceInput::new(
                QueueInput(30),
                InputProvenance::service(ServiceId::new("queue")),
            )
            .unwrap(),
        )
        .unwrap();
    let rejected_service = ServiceInput::new(
        QueueInput(31),
        InputProvenance::service(ServiceId::new("queue")),
    )
    .unwrap();
    assert_eq!(
        runtime
            .enqueue_service(rejected_service.clone())
            .unwrap_err()
            .into_rejected(),
        rejected_service
    );

    assert_eq!(
        runtime.diagnostics().count(&DiagnosticCode::QUEUE_OVERFLOW),
        3
    );
    assert_eq!(
        runtime
            .drain_once(RuntimeBudget::default())
            .unwrap()
            .drained_inputs(),
        4
    );
    assert_eq!(runtime.state().seen, vec![10, 20, 30, 21]);

    runtime.enqueue_task(rejected_task).unwrap();
    assert_eq!(
        runtime
            .drain_once(RuntimeBudget::default())
            .unwrap()
            .drained_inputs(),
        1
    );
    assert_eq!(runtime.state().seen, vec![10, 20, 30, 21, 22]);
}

#[derive(Default)]
struct QueueState {
    seen: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct QueueInput(u8);

struct QueueReducer;

impl Reducer<QueueState, QueueInput> for QueueReducer {
    fn reduce(
        &mut self,
        state: &QueueState,
        input: &AppInput<QueueInput>,
    ) -> ReducerResult<QueueState> {
        let mut next = state.seen.clone();
        next.push(input.payload().0);
        ReducerResult::changed(QueueState { seen: next }, ReducerCommit::new())
    }
}

#[test]
fn runtime_redraw_all_reports_registered_surface_ids() {
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    runtime
        .register_surface(test_surface(2, 1, "secondary"))
        .unwrap();
    runtime
        .register_surface(test_surface(1, 1, "main"))
        .unwrap();
    let first = runtime.surface_ref(SurfaceId::from_u64(1)).unwrap();
    let second = runtime.surface_ref(SurfaceId::from_u64(2)).unwrap();
    ready_surface(&mut runtime, first);
    ready_surface(&mut runtime, second);
    runtime
        .enqueue_ui(UiInput::new(CounterInput::RedrawAll, InputProvenance::system()).unwrap())
        .unwrap();

    let report = runtime.drain_once(RuntimeBudget::default()).unwrap();

    assert_eq!(report.redraw_requests(), &[first, second]);
}

#[test]
fn runtime_redraw_window_reports_surfaces_for_that_window() {
    let target_window = WindowId::from_u64(7);
    let other_window = WindowId::from_u64(8);
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    runtime
        .register_surface(test_surface(1, other_window.as_u64(), "other"))
        .unwrap();
    runtime
        .register_surface(test_surface(3, target_window.as_u64(), "right"))
        .unwrap();
    runtime
        .register_surface(test_surface(2, target_window.as_u64(), "left"))
        .unwrap();
    let left = runtime.surface_ref(SurfaceId::from_u64(2)).unwrap();
    let right = runtime.surface_ref(SurfaceId::from_u64(3)).unwrap();
    ready_surface(&mut runtime, left);
    ready_surface(&mut runtime, right);
    runtime
        .enqueue_ui(
            UiInput::new(
                CounterInput::RedrawWindow(target_window),
                InputProvenance::system(),
            )
            .unwrap(),
        )
        .unwrap();

    let report = runtime.drain_once(RuntimeBudget::default()).unwrap();

    assert_eq!(report.redraw_requests(), &[left, right]);
}

struct FailingReducer;

impl Reducer<CounterState, CounterInput> for FailingReducer {
    fn reduce(
        &mut self,
        _state: &CounterState,
        _input: &AppInput<CounterInput>,
    ) -> ReducerResult<CounterState> {
        ReducerResult::recoverable_failure(ReducerFailure::new("counter reducer rejected input"))
    }
}

#[test]
fn runtime_turns_recoverable_reducer_errors_into_diagnostics() {
    let mut runtime = Runtime::new(CounterState::default(), FailingReducer);
    runtime
        .enqueue_ui(UiInput::new(CounterInput::Increment, InputProvenance::system()).unwrap())
        .unwrap();

    let report = runtime.drain_once(RuntimeBudget::default()).unwrap();

    assert_eq!(runtime.state().value, 0);
    assert_eq!(report.reducer_errors(), 1);
    assert_eq!(
        runtime.diagnostics().count(&DiagnosticCode::REDUCER_ERROR),
        1
    );
}

struct ProvenanceFailingReducer {
    provenance: InputProvenance,
}

impl Reducer<CounterState, CounterInput> for ProvenanceFailingReducer {
    fn reduce(
        &mut self,
        _state: &CounterState,
        _input: &AppInput<CounterInput>,
    ) -> ReducerResult<CounterState> {
        ReducerResult::recoverable_failure(
            ReducerFailure::new("counter reducer rejected input")
                .with_provenance(self.provenance.clone()),
        )
    }
}

#[test]
fn runtime_reducer_failure_isolated_and_uses_effective_provenance() {
    let trigger = InputProvenance::system().with_sequence(3);
    let override_provenance = InputProvenance::system().with_sequence(4);
    let mut runtime = Runtime::new(
        CounterState::default(),
        ProvenanceFailingReducer {
            provenance: override_provenance.clone(),
        },
    );
    runtime
        .enqueue_ui(UiInput::new(CounterInput::Increment, trigger).unwrap())
        .unwrap();

    let report = runtime.drain_once(RuntimeBudget::default()).unwrap();

    assert_eq!(runtime.state(), &CounterState::default());
    assert_eq!(report.drained_inputs(), 1);
    assert_eq!(report.reducer_errors(), 1);
    assert_eq!(report.effect_outcomes(), &[]);
    assert_eq!(
        runtime.diagnostics().entries()[0].provenance(),
        &override_provenance
    );
}

#[test]
fn runtime_changed_commit_invalidates_nonterminal_surfaces_and_redraws_renderable_ones() {
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    let created = runtime
        .register_surface(test_surface(1, 1, "created"))
        .unwrap();
    let ready = runtime
        .register_surface(test_surface(2, 1, "ready"))
        .unwrap();
    let resized = runtime
        .register_surface(test_surface(3, 1, "resized"))
        .unwrap();
    let hidden = runtime
        .register_surface(test_surface(4, 1, "hidden"))
        .unwrap();
    let occluded = runtime
        .register_surface(test_surface(5, 1, "occluded"))
        .unwrap();
    let suspended = runtime
        .register_surface(test_surface(6, 1, "suspended"))
        .unwrap();
    let closing = runtime
        .register_surface(test_surface(7, 1, "closing"))
        .unwrap();
    let closed = runtime
        .register_surface(test_surface(8, 1, "closed"))
        .unwrap();
    let destroyed = runtime
        .register_surface(test_surface(9, 1, "destroyed"))
        .unwrap();
    ready_surface(&mut runtime, ready);
    runtime
        .update_surface(resized, |surface| {
            surface.ready()?;
            surface.resized()?;
            Ok(())
        })
        .unwrap();
    runtime
        .update_surface(hidden, |surface| {
            surface.ready()?;
            surface.hidden()?;
            Ok(())
        })
        .unwrap();
    runtime
        .update_surface(occluded, |surface| {
            surface.ready()?;
            surface.occluded()?;
            Ok(())
        })
        .unwrap();
    runtime
        .update_surface(suspended, |surface| {
            surface.ready()?;
            surface.suspended()?;
            Ok(())
        })
        .unwrap();
    runtime
        .update_surface(closing, |surface| surface.closing().map(|_| ()))
        .unwrap();
    runtime
        .update_surface(closed, |surface| surface.closed().map(|_| ()))
        .unwrap();
    runtime
        .update_surface(destroyed, |surface| surface.destroyed().map(|_| ()))
        .unwrap();
    runtime
        .enqueue_ui(UiInput::new(CounterInput::Increment, InputProvenance::system()).unwrap())
        .unwrap();

    let report = runtime.drain_once(RuntimeBudget::default()).unwrap();

    assert_eq!(report.redraw_requests(), &[ready, resized]);
    for surface in [created, ready, resized, hidden, occluded, suspended] {
        assert!(matches!(
            runtime
                .surface(surface.surface_id())
                .unwrap()
                .invalidations()
                .last()
                .map(SurfaceInvalidation::kind),
            Some(SurfaceInvalidationKind::SnapshotChanged { version })
                if *version == StateVersion::from_u64(1)
        ));
    }
    for surface in [closing, closed, destroyed] {
        assert!(
            runtime
                .surface(surface.surface_id())
                .unwrap()
                .invalidations()
                .is_empty()
        );
    }
}

#[test]
fn runtime_overflow_requeues_the_exact_input_and_returns_prior_work() {
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    let surface = runtime
        .register_surface(test_surface(1, 1, "main"))
        .unwrap();
    runtime
        .update_surface(surface, |surface| {
            surface.ready()?;
            surface.set_generations_for_test(0, Some(u64::MAX));
            Ok(())
        })
        .unwrap();
    runtime
        .enqueue_ui(UiInput::new(CounterInput::RedrawAll, InputProvenance::system()).unwrap())
        .unwrap();
    let trigger = InputProvenance::system().with_sequence(99);
    runtime
        .enqueue_ui(UiInput::new(CounterInput::Increment, trigger.clone()).unwrap())
        .unwrap();

    let error = runtime.drain_once(RuntimeBudget::default()).unwrap_err();

    assert_eq!(
        error.code(),
        RuntimeDrainErrorCode::SurfaceInvalidationOverflow
    );
    assert_eq!(error.lane(), RuntimeLane::Ui);
    assert_eq!(error.provenance(), &trigger);
    assert_eq!(error.surface(), Some(surface));
    assert_eq!(error.source(), VersionError::Overflow);
    assert_eq!(error.partial_report().drained_inputs(), 1);
    assert_eq!(runtime.state(), &CounterState::default());
    runtime
        .update_surface(surface, |surface| {
            surface.set_generations_for_test(0, None);
            Ok(())
        })
        .unwrap();

    let report = runtime.drain_once(RuntimeBudget::default()).unwrap();
    assert_eq!(report.drained_inputs(), 1);
    assert_eq!(runtime.state().value, 1);
}

#[test]
fn runtime_state_version_overflow_requeues_without_counting_the_input() {
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    runtime.set_state_version_for_test(StateVersion::from_u64(u64::MAX));
    runtime
        .enqueue_ui(UiInput::new(CounterInput::RedrawAll, InputProvenance::system()).unwrap())
        .unwrap();
    let trigger = InputProvenance::system().with_sequence(100);
    runtime
        .enqueue_ui(UiInput::new(CounterInput::Increment, trigger.clone()).unwrap())
        .unwrap();

    let error = runtime.drain_once(RuntimeBudget::default()).unwrap_err();

    assert_eq!(error.code(), RuntimeDrainErrorCode::StateVersionOverflow);
    assert_eq!(error.lane(), RuntimeLane::Ui);
    assert_eq!(error.provenance(), &trigger);
    assert_eq!(error.surface(), None);
    assert_eq!(error.partial_report().drained_inputs(), 1);
    assert_eq!(error.partial_report().applied_effects(), 1);
    assert_eq!(
        error.partial_report().first_drained_lane(),
        Some(RuntimeLane::Ui)
    );
    assert_eq!(error.partial_report().remaining_ui_inputs(), 1);
    assert_eq!(error.partial_report().remaining_task_inputs(), 0);
    assert_eq!(error.partial_report().remaining_service_inputs(), 0);
    assert!(error.partial_report().has_pending_inputs());
    assert_eq!(runtime.state(), &CounterState::default());
    runtime.set_state_version_for_test(StateVersion::initial());
    assert_eq!(
        runtime
            .drain_once(RuntimeBudget::default())
            .unwrap()
            .drained_inputs(),
        1
    );
}

#[test]
fn runtime_immediate_overflow_reports_no_first_drained_lane() {
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    runtime.set_state_version_for_test(StateVersion::from_u64(u64::MAX));
    runtime
        .enqueue_ui(UiInput::new(CounterInput::Increment, InputProvenance::system()).unwrap())
        .unwrap();

    let error = runtime.drain_once(RuntimeBudget::default()).unwrap_err();

    assert_eq!(error.partial_report().drained_inputs(), 0);
    assert_eq!(error.partial_report().first_drained_lane(), None);
    assert_eq!(error.partial_report().remaining_ui_inputs(), 1);
    assert!(error.partial_report().has_pending_inputs());
}

struct EffectsReducer {
    commit: ReducerCommit,
}

impl Reducer<CounterState, CounterInput> for EffectsReducer {
    fn reduce(
        &mut self,
        _state: &CounterState,
        _input: &AppInput<CounterInput>,
    ) -> ReducerResult<CounterState> {
        ReducerResult::unchanged(self.commit.clone())
    }
}

#[test]
fn runtime_processes_all_effect_dispositions_and_validates_redraw_targets() {
    let effective_provenance = InputProvenance::system().with_sequence(70);
    let mut resource = ResourceState::<(), ()>::new(ResourceId::new("thumb:1"));
    let operation = resource.begin_load().unwrap();
    let handle = TaskIntentHandle::new(TaskIntentId::from_u64(7), TaskIntentAttemptId::from_u64(2));
    let service = ServiceId::new("jsonrpc");
    let effects = EffectBatch::new()
        .push(AppEffect::diagnostic(Diagnostic::info(
            DiagnosticCode::QUEUE_COALESCED,
            "applied diagnostic",
            InputProvenance::system(),
        )))
        .push(AppEffect::service_diagnostic(
            service.clone(),
            Diagnostic::info(
                DiagnosticCode::QUEUE_COALESCED,
                "applied service diagnostic",
                InputProvenance::system(),
            ),
        ))
        .push(AppEffect::request_redraw(RedrawTarget::all()))
        .push(AppEffect::request_redraw(RedrawTarget::surface(
            surface_ref(1, 0),
        )))
        .push(AppEffect::request_redraw(RedrawTarget::Window(
            WindowId::from_u64(10),
        )))
        .push(AppEffect::request_redraw(RedrawTarget::surface(
            surface_ref(1, 1),
        )))
        .push(AppEffect::request_redraw(RedrawTarget::surface(
            surface_ref(2, 0),
        )))
        .push(AppEffect::request_redraw(RedrawTarget::surface(
            surface_ref(3, 0),
        )))
        .push(AppEffect::request_redraw(RedrawTarget::surface(
            surface_ref(99, 0),
        )))
        .push(AppEffect::request_redraw(RedrawTarget::Window(
            WindowId::from_u64(99),
        )))
        .push(AppEffect::request_redraw(RedrawTarget::Window(
            WindowId::from_u64(11),
        )))
        .push(AppEffect::request_redraw(RedrawTarget::Window(
            WindowId::from_u64(12),
        )))
        .push(AppEffect::persist("session", AppScope::app()))
        .push(AppEffect::load_resource(operation.clone(), AppScope::app()))
        .push(AppEffect::invalidate_resource(
            ResourceId::new("thumb:1"),
            "source changed",
        ))
        .push(AppEffect::start_task(
            TaskIntentName::new("search"),
            TaskIntentKey::new("search:rust"),
            AppScope::app(),
        ))
        .push(AppEffect::cancel_task(handle))
        .push(AppEffect::reprioritize_task(handle, TaskPriorityHint::High))
        .push(AppEffect::start_service(service.clone()))
        .push(AppEffect::stop_service(service.clone()))
        .push(AppEffect::call_service(
            service,
            ServiceCommandName::new("textDocument/hover"),
            ServiceCommandPayload::from_json_text(r#"{"line":3}"#),
            correlation(42),
        ));
    let mut runtime = Runtime::new(
        CounterState::default(),
        EffectsReducer {
            commit: ReducerCommit::new()
                .with_effects(effects)
                .with_provenance(effective_provenance.clone()),
        },
    );
    let ready = runtime
        .register_surface(test_surface(1, 10, "ready"))
        .unwrap();
    let _created = runtime
        .register_surface(test_surface(2, 11, "created"))
        .unwrap();
    let terminal = runtime
        .register_surface(test_surface(3, 12, "terminal"))
        .unwrap();
    runtime
        .update_surface(ready, |surface| surface.ready().map(|_| ()))
        .unwrap();
    runtime
        .update_surface(terminal, |surface| surface.closing().map(|_| ()))
        .unwrap();
    runtime
        .enqueue_ui(UiInput::new(CounterInput::RedrawAll, InputProvenance::system()).unwrap())
        .unwrap();

    let report = runtime.drain_once(RuntimeBudget::default()).unwrap();

    assert_eq!(report.applied_effects(), 5);
    assert_eq!(report.forwarded_effects(), 9);
    assert_eq!(report.rejected_effects(), 7);
    assert_eq!(report.effect_outcomes().len(), 21);
    assert_eq!(report.redraw_requests(), &[ready]);
    assert_eq!(report.intents().len(), 9);
    assert!(matches!(
        &report.intents()[0],
        RuntimeIntent::Persist(effect) if effect.key() == "session"
    ));
    assert!(matches!(
        &report.intents()[1],
        RuntimeIntent::LoadResource(effect) if effect.operation() == &operation
    ));
    assert!(matches!(
        &report.intents()[2],
        RuntimeIntent::InvalidateResource(effect) if effect.reason() == "source changed"
    ));
    assert!(matches!(&report.intents()[3], RuntimeIntent::StartTask(_)));
    assert!(matches!(&report.intents()[4], RuntimeIntent::CancelTask(_)));
    assert!(matches!(
        &report.intents()[5],
        RuntimeIntent::ReprioritizeTask(effect) if effect.priority() == TaskPriorityHint::High
    ));
    assert!(matches!(
        &report.intents()[6],
        RuntimeIntent::StartService(_)
    ));
    assert!(matches!(
        &report.intents()[7],
        RuntimeIntent::StopService(_)
    ));
    assert!(matches!(
        &report.intents()[8],
        RuntimeIntent::CallService(_)
    ));
    assert!(
        report
            .effect_outcomes()
            .iter()
            .all(|outcome| outcome.provenance() == &effective_provenance)
    );
    assert_eq!(
        runtime.diagnostics().count(&DiagnosticCode::EFFECT_FAILED),
        7
    );
}

#[test]
fn runtime_rejects_work_lane_provenance_for_ui_queue() {
    let error = match UiInput::new(
        CounterInput::Increment,
        InputProvenance::task(TaskIntentId::from_u64(1), TaskIntentAttemptId::from_u64(1)),
    ) {
        Ok(_) => panic!("task provenance should not enter the UI queue"),
        Err(error) => error,
    };

    assert_eq!(error.lane(), RuntimeLane::Ui);
}

#[test]
fn effect_batches_preserve_order() {
    let effects = EffectBatch::new()
        .push(AppEffect::diagnostic(Diagnostic::info(
            DiagnosticCode::QUEUE_COALESCED,
            "coalesced",
            InputProvenance::system(),
        )))
        .push(AppEffect::request_redraw(RedrawTarget::all()));

    assert_eq!(effects.effects().len(), 2);
    assert_eq!(effects.effects()[0].kind(), &EffectKindId::EMIT_DIAGNOSTIC);
    assert_eq!(effects.effects()[1].kind(), &EffectKindId::REQUEST_REDRAW);
}

#[test]
fn locally_applied_effect_payloads_preserve_their_values() {
    let redraw = AppEffect::request_redraw(RedrawTarget::surface(surface_ref(3, 2)));
    let persist = AppEffect::persist("session", AppScope::workspace("alpha"));
    let diagnostic = Diagnostic::warning(
        DiagnosticCode::QUEUE_COALESCED,
        "coalesced",
        InputProvenance::system(),
    );
    let diagnostic_effect = AppEffect::diagnostic(diagnostic.clone());

    assert!(matches!(
        redraw.payload(),
        AppEffectPayload::RequestRedraw(effect)
            if effect.target() == &RedrawTarget::surface(surface_ref(3, 2))
    ));
    assert!(matches!(
        persist.payload(),
        AppEffectPayload::Persist(effect)
            if effect.key() == "session" && effect.scope() == &AppScope::workspace("alpha")
    ));
    assert!(matches!(
        diagnostic_effect.payload(),
        AppEffectPayload::Diagnostic(effect) if effect.diagnostic() == &diagnostic
    ));
}

#[test]
fn resource_effects_expose_typed_payloads_and_kinds() {
    let mut resource = ResourceState::<(), ()>::new(ResourceId::new("thumb:1"));
    let operation = resource.begin_load().unwrap();
    let load = AppEffect::load_resource(operation.clone(), AppScope::app());
    assert_eq!(load.kind(), &EffectKindId::LOAD_RESOURCE);
    assert!(matches!(
        load.payload(),
        AppEffectPayload::LoadResource(effect)
            if effect.operation() == &operation
                && effect.operation().id() == operation.id()
                && effect.operation().generation() == operation.generation()
                && effect.id() == &ResourceId::new("thumb:1")
                && effect.scope() == &AppScope::app()
    ));

    let invalidate = AppEffect::invalidate_resource(ResourceId::new("thumb:1"), "source changed");
    assert_eq!(invalidate.kind(), &EffectKindId::INVALIDATE_RESOURCE);
    assert!(matches!(
        invalidate.payload(),
        AppEffectPayload::InvalidateResource(effect)
            if effect.id() == &ResourceId::new("thumb:1") && effect.reason() == "source changed"
    ));
}

#[test]
fn effect_kind_ids_cover_only_backed_runtime_paths() {
    let backed = [
        EffectKindId::REQUEST_REDRAW,
        EffectKindId::PERSIST,
        EffectKindId::EMIT_DIAGNOSTIC,
        EffectKindId::LOAD_RESOURCE,
        EffectKindId::INVALIDATE_RESOURCE,
        EffectKindId::START_TASK,
        EffectKindId::CANCEL_TASK,
        EffectKindId::REPRIORITIZE_TASK,
        EffectKindId::START_SERVICE,
        EffectKindId::STOP_SERVICE,
        EffectKindId::CALL_SERVICE,
        EffectKindId::SERVICE_DIAGNOSTIC,
    ];

    for (kind, expected) in backed.iter().zip([
        "runtime.request_redraw",
        "runtime.persist",
        "runtime.emit_diagnostic",
        "runtime.load_resource",
        "runtime.invalidate_resource",
        "runtime.start_task",
        "runtime.cancel_task",
        "runtime.reprioritize_task",
        "runtime.start_service",
        "runtime.stop_service",
        "runtime.call_service",
        "runtime.service_diagnostic",
    ]) {
        assert_eq!(kind.as_str(), expected);
    }

    let effect_source = include_str!("effect.rs");
    assert!(!effect_source.contains("runtime.schedule_timer"));
    assert!(!effect_source.contains("runtime.window_command"));
}

#[test]
fn effect_outcomes_expose_only_their_matching_optional_values() {
    let provenance = InputProvenance::system().with_correlation(correlation(5));
    let diagnostic = Diagnostic::error(
        DiagnosticCode::EFFECT_FAILED,
        "target is unavailable",
        provenance.clone(),
    );
    let persist = AppEffect::persist("session", AppScope::app());
    let AppEffectPayload::Persist(persist) = persist.payload().clone() else {
        panic!("expected persist payload");
    };

    let applied = EffectOutcome::applied(EffectKindId::REQUEST_REDRAW, provenance.clone());
    assert_eq!(applied.kind(), &EffectKindId::REQUEST_REDRAW);
    assert_eq!(applied.disposition(), EffectDisposition::Applied);
    assert_eq!(applied.provenance(), &provenance);
    assert_eq!(applied.intent(), None);
    assert_eq!(applied.diagnostic(), None);

    let forwarded = EffectOutcome::forwarded(
        EffectKindId::PERSIST,
        provenance.clone(),
        RuntimeIntent::Persist(persist.clone()),
    );
    assert_eq!(forwarded.kind(), &EffectKindId::PERSIST);
    assert_eq!(forwarded.disposition(), EffectDisposition::Forwarded);
    assert_eq!(forwarded.provenance(), &provenance);
    assert_eq!(
        forwarded.intent(),
        Some(&RuntimeIntent::Persist(persist.clone()))
    );
    assert_eq!(forwarded.diagnostic(), None);

    let rejected = EffectOutcome::rejected(
        EffectKindId::REQUEST_REDRAW,
        provenance.clone(),
        diagnostic.clone(),
    );
    assert_eq!(rejected.kind(), &EffectKindId::REQUEST_REDRAW);
    assert_eq!(rejected.disposition(), EffectDisposition::Rejected);
    assert_eq!(rejected.provenance(), &provenance);
    assert_eq!(rejected.intent(), None);
    assert_eq!(rejected.diagnostic(), Some(&diagnostic));
}

#[test]
fn runtime_intents_preserve_each_owned_effect_payload() {
    let scope = AppScope::resource(ResourceId::new("thumb:1"));
    let mut resource = ResourceState::<(), ()>::new(ResourceId::new("thumb:1"));
    let operation = resource.begin_load().unwrap();
    let handle = TaskIntentHandle::new(TaskIntentId::from_u64(7), TaskIntentAttemptId::from_u64(2));
    let service = ServiceId::new("jsonrpc");
    let effects = [
        AppEffect::persist("session", scope.clone()),
        AppEffect::load_resource(operation.clone(), scope.clone()),
        AppEffect::invalidate_resource(ResourceId::new("thumb:1"), "source changed"),
        AppEffect::start_task(
            TaskIntentName::new("search"),
            TaskIntentKey::new("search:rust"),
            scope.clone(),
        ),
        AppEffect::cancel_task(handle),
        AppEffect::reprioritize_task(handle, TaskPriorityHint::High),
        AppEffect::start_service(service.clone()),
        AppEffect::stop_service(service.clone()),
        AppEffect::call_service(
            service,
            ServiceCommandName::new("textDocument/hover"),
            ServiceCommandPayload::from_json_text(r#"{"line":3}"#),
            correlation(42),
        ),
    ];

    let intents = effects
        .each_ref()
        .map(|effect| match effect.payload().clone() {
            AppEffectPayload::Persist(payload) => RuntimeIntent::Persist(payload),
            AppEffectPayload::LoadResource(payload) => RuntimeIntent::LoadResource(payload),
            AppEffectPayload::InvalidateResource(payload) => {
                RuntimeIntent::InvalidateResource(payload)
            }
            AppEffectPayload::StartTask(payload) => RuntimeIntent::StartTask(payload),
            AppEffectPayload::CancelTask(payload) => RuntimeIntent::CancelTask(payload),
            AppEffectPayload::ReprioritizeTask(payload) => RuntimeIntent::ReprioritizeTask(payload),
            AppEffectPayload::StartService(payload) => RuntimeIntent::StartService(payload),
            AppEffectPayload::StopService(payload) => RuntimeIntent::StopService(payload),
            AppEffectPayload::CallService(payload) => RuntimeIntent::CallService(payload),
            AppEffectPayload::RequestRedraw(_)
            | AppEffectPayload::Diagnostic(_)
            | AppEffectPayload::ServiceDiagnostic(_) => panic!("unexpected applied effect payload"),
        });

    assert!(matches!(
        &intents[0],
        RuntimeIntent::Persist(payload) if payload == match effects[0].payload() {
            AppEffectPayload::Persist(payload) => payload,
            _ => unreachable!(),
        }
    ));
    assert!(matches!(
        &intents[1],
        RuntimeIntent::LoadResource(payload)
            if payload.operation() == &operation
                && payload.operation().id() == operation.id()
                && payload.operation().generation() == operation.generation()
                && payload.id() == operation.resource_id()
                && payload == match effects[1].payload() {
                    AppEffectPayload::LoadResource(payload) => payload,
                    _ => unreachable!(),
                }
    ));
    assert!(matches!(
        &intents[2],
        RuntimeIntent::InvalidateResource(payload) if payload == match effects[2].payload() {
            AppEffectPayload::InvalidateResource(payload) => payload,
            _ => unreachable!(),
        }
    ));
    assert!(matches!(
        &intents[3],
        RuntimeIntent::StartTask(payload) if payload == match effects[3].payload() {
            AppEffectPayload::StartTask(payload) => payload,
            _ => unreachable!(),
        }
    ));
    assert!(matches!(
        &intents[4],
        RuntimeIntent::CancelTask(payload) if payload == match effects[4].payload() {
            AppEffectPayload::CancelTask(payload) => payload,
            _ => unreachable!(),
        }
    ));
    assert!(matches!(
        &intents[5],
        RuntimeIntent::ReprioritizeTask(payload) if payload == match effects[5].payload() {
            AppEffectPayload::ReprioritizeTask(payload) => payload,
            _ => unreachable!(),
        }
    ));
    assert!(matches!(
        &intents[6],
        RuntimeIntent::StartService(payload) if payload == match effects[6].payload() {
            AppEffectPayload::StartService(payload) => payload,
            _ => unreachable!(),
        }
    ));
    assert!(matches!(
        &intents[7],
        RuntimeIntent::StopService(payload) if payload == match effects[7].payload() {
            AppEffectPayload::StopService(payload) => payload,
            _ => unreachable!(),
        }
    ));
    assert!(matches!(
        &intents[8],
        RuntimeIntent::CallService(payload) if payload == match effects[8].payload() {
            AppEffectPayload::CallService(payload) => payload,
            _ => unreachable!(),
        }
    ));
}

#[test]
fn service_registration_exposes_mailbox_policy() {
    let registration = ServiceRegistration::new(ServiceId::new("jsonrpc"))
        .with_scope(AppScope::app())
        .with_mailbox_policy(MailboxPolicy::bounded(4).drop_oldest().observe_overflow())
        .with_startup(ServiceStartup::Lazy)
        .with_shutdown(ServiceShutdown::DrainThenStop)
        .with_restart(ServiceRestart::OnFailure);

    assert_eq!(registration.id(), &ServiceId::new("jsonrpc"));
    assert_eq!(registration.scope(), &AppScope::app());
    assert_eq!(registration.mailbox().capacity(), 4);
    assert_eq!(
        registration.mailbox().overflow(),
        MailboxOverflow::DropOldest
    );
    assert!(registration.mailbox().observes_overflow());
    assert_eq!(registration.startup(), ServiceStartup::Lazy);
    assert_eq!(registration.shutdown(), ServiceShutdown::DrainThenStop);
    assert_eq!(registration.restart(), ServiceRestart::OnFailure);
}

#[test]
fn service_mailbox_reject_newest_reports_outcomes_and_preserves_fifo() {
    let policy = MailboxPolicy::bounded(2).observe_overflow();
    let mut mailbox = ServiceMailbox::<u32>::new(ServiceId::new("rpc"), policy);

    assert_eq!(mailbox.push(1), MailboxPushOutcome::Accepted);
    assert_eq!(mailbox.push(2), MailboxPushOutcome::Accepted);
    assert_eq!(mailbox.push(3), MailboxPushOutcome::RejectedNewest(3));

    assert_eq!(mailbox.len(), 2);
    assert_eq!(mailbox.overflow_count(), 1);
    assert_eq!(mailbox.drain().collect::<Vec<_>>(), vec![1, 2]);
}

#[test]
fn service_mailbox_drop_oldest_reports_outcomes_and_preserves_fifo() {
    let policy = MailboxPolicy::bounded(2).drop_oldest().observe_overflow();
    let mut mailbox = ServiceMailbox::<u32>::new(ServiceId::new("rpc"), policy);

    assert_eq!(mailbox.push(1), MailboxPushOutcome::Accepted);
    assert_eq!(mailbox.push(2), MailboxPushOutcome::Accepted);
    assert_eq!(
        mailbox.push(3),
        MailboxPushOutcome::DroppedOldest { dropped: 1 }
    );

    assert_eq!(mailbox.len(), 2);
    assert_eq!(mailbox.overflow_count(), 1);
    assert_eq!(mailbox.drain().collect::<Vec<_>>(), vec![2, 3]);
}

#[test]
fn service_mailbox_zero_capacity_rejects_newest_without_overflow_tracking() {
    let mut mailbox = ServiceMailbox::<u32>::new(ServiceId::new("rpc"), MailboxPolicy::bounded(0));

    assert_eq!(mailbox.push(1), MailboxPushOutcome::RejectedNewest(1));

    assert!(mailbox.is_empty());
    assert_eq!(mailbox.overflow_count(), 0);
}

#[test]
fn service_mailbox_zero_capacity_drop_oldest_rejects_newest_and_tracks_overflow() {
    let policy = MailboxPolicy::bounded(0).drop_oldest().observe_overflow();
    let mut mailbox = ServiceMailbox::<u32>::new(ServiceId::new("rpc"), policy);

    assert_eq!(mailbox.push(1), MailboxPushOutcome::RejectedNewest(1));

    assert!(mailbox.is_empty());
    assert_eq!(mailbox.overflow_count(), 1);
}

#[test]
fn service_effects_expose_typed_payloads_and_kinds() {
    let start = AppEffect::start_service(ServiceId::new("jsonrpc"));
    assert_eq!(start.kind(), &EffectKindId::START_SERVICE);
    assert!(matches!(
        start.payload(),
        AppEffectPayload::StartService(effect) if effect.id() == &ServiceId::new("jsonrpc")
    ));

    let stop = AppEffect::stop_service(ServiceId::new("jsonrpc"));
    assert_eq!(stop.kind(), &EffectKindId::STOP_SERVICE);
    assert!(matches!(
        stop.payload(),
        AppEffectPayload::StopService(effect) if effect.id() == &ServiceId::new("jsonrpc")
    ));

    let call = AppEffect::call_service(
        ServiceId::new("jsonrpc"),
        ServiceCommandName::new("textDocument/hover"),
        ServiceCommandPayload::from_json_text(r#"{"line":3}"#),
        correlation(42),
    );
    assert_eq!(call.kind(), &EffectKindId::CALL_SERVICE);
    assert!(matches!(
        call.payload(),
        AppEffectPayload::CallService(effect)
            if effect.id() == &ServiceId::new("jsonrpc")
                && effect.command().as_str() == "textDocument/hover"
                && effect.payload().as_json_text() == r#"{"line":3}"#
                && effect.correlation() == correlation(42)
    ));

    let diagnostic = Diagnostic::warning(
        DiagnosticCode::SERVICE_MAILBOX_OVERFLOW,
        "service mailbox overflow",
        InputProvenance::system(),
    );
    let service_diagnostic =
        AppEffect::service_diagnostic(ServiceId::new("jsonrpc"), diagnostic.clone());
    assert_eq!(service_diagnostic.kind(), &EffectKindId::SERVICE_DIAGNOSTIC);
    assert!(matches!(
        service_diagnostic.payload(),
        AppEffectPayload::ServiceDiagnostic(effect)
            if effect.id() == &ServiceId::new("jsonrpc")
                && effect.diagnostic() == &diagnostic
    ));
}

#[test]
fn resource_state_tracks_freshness_and_refreshing_independently() {
    let mut resource = ResourceState::<u32, String>::new(ResourceId::new("thumb:1"));

    let load = resource.begin_load().unwrap();
    assert_eq!(resource.status(), ResourceStatus::Loading);
    assert!(!resource.is_renderable());

    resource.ready(&load, 7).unwrap();
    assert_eq!(resource.status(), ResourceStatus::Ready);
    assert_eq!(resource.value(), Some(&7));
    assert!(resource.is_renderable());
    assert_eq!(resource.freshness(), Freshness::Fresh);

    let refresh = resource.begin_refresh().unwrap();
    assert_eq!(resource.status(), ResourceStatus::Refreshing);
    assert_eq!(resource.value(), Some(&7));
    assert!(resource.is_renderable());

    resource.ready(&refresh, 8).unwrap();
    assert_eq!(resource.value(), Some(&8));
}

#[test]
fn resource_failure_preserves_renderable_stale_value() {
    let mut resource = ResourceState::<u32, String>::new(ResourceId::new("query:1"));
    let load = resource.begin_load().unwrap();
    resource.ready(&load, 10).unwrap();

    let refresh = resource.begin_refresh().unwrap();
    resource
        .failed(
            &refresh,
            "timeout".to_string(),
            FailureVisibility::KeepStaleValue,
        )
        .unwrap();

    assert_eq!(resource.status(), ResourceStatus::Failed);
    assert_eq!(resource.value(), Some(&10));
    assert_eq!(resource.error(), Some(&"timeout".to_string()));
    assert!(resource.is_renderable());
    assert_eq!(resource.freshness(), Freshness::Stale);
}

#[test]
fn app_scope_covers_runtime_ownership_kinds() {
    assert!(AppScope::app().is_app());
    assert_eq!(
        AppScope::window(WindowId::from_u64(9)).window_id(),
        Some(WindowId::from_u64(9))
    );
    assert_eq!(
        AppScope::surface(SurfaceId::from_u64(3)).surface_id(),
        Some(SurfaceId::from_u64(3))
    );
    assert_eq!(
        AppScope::resource(ResourceId::new("graph")).resource_id(),
        Some(ResourceId::new("graph"))
    );
    assert_eq!(
        AppScope::custom("workspace:alpha").segments()[0].namespace(),
        "custom"
    );
    assert_eq!(
        AppScope::workspace("alpha")
            .then(ScopePathSegment::new("resource", "graph"))
            .segments()
            .len(),
        2
    );
}

#[test]
fn subscriptions_preserve_scope_observer_priority_and_refcounts() {
    let target = SubscriptionTarget::task(TaskIntentKey::new("compile:main"));
    let observer = SurfaceRef::new(SurfaceId::from_u64(1), SurfaceGeneration::initial());
    let app = Subscription::new(SubscriptionKey::new(
        target.clone(),
        AppScope::app(),
        observer,
        SubscriptionPriority::Normal,
    ));
    let scoped = Subscription::new(SubscriptionKey::new(
        target.clone(),
        AppScope::resource(ResourceId::new("project:main")),
        observer,
        SubscriptionPriority::Normal,
    ));
    let reprioritized = Subscription::new(SubscriptionKey::new(
        target.clone(),
        AppScope::app(),
        observer,
        SubscriptionPriority::High,
    ));
    let next_generation = Subscription::new(SubscriptionKey::new(
        target.clone(),
        AppScope::app(),
        SurfaceRef::new(SurfaceId::from_u64(1), SurfaceGeneration::from_u64(1)),
        SubscriptionPriority::Normal,
    ));
    let mut coordination = CoordinationState::default();

    assert_eq!(app.key().target(), &target);
    assert_eq!(app.key().scope(), &AppScope::app());
    assert_eq!(app.key().observer(), observer);
    assert_eq!(app.key().priority(), SubscriptionPriority::Normal);

    for subscription in [&app, &scoped, &reprioritized, &next_generation] {
        assert!(matches!(
            coordination.subscribe(subscription),
            Ok(SubscriptionChange::Added { .. })
        ));
    }

    assert_eq!(coordination.ref_count(app.key()), 1);
    assert_eq!(coordination.ref_count(scoped.key()), 1);
    assert_eq!(coordination.ref_count(reprioritized.key()), 1);
    assert_eq!(coordination.ref_count(next_generation.key()), 1);
    assert_eq!(coordination.aggregate(&target).unwrap().active_keys(), 4);
}

#[test]
fn subscription_replay_and_missing_unsubscribe_report_exact_changes() {
    let subscription = Subscription::task(
        TaskIntentKey::new("compile:main"),
        AppScope::resource(ResourceId::new("project:main")),
        SurfaceRef::new(SurfaceId::from_u64(1), SurfaceGeneration::initial()),
        SubscriptionPriority::High,
    );
    let key = subscription.key().clone();
    let mut coordination = CoordinationState::default();

    let added = coordination.subscribe(&subscription).unwrap();
    assert_eq!(added.key(), &key);
    assert_eq!(added.ref_count(), 1);
    assert_eq!(
        added,
        SubscriptionChange::Added {
            key: key.clone(),
            ref_count: 1,
        }
    );
    assert_eq!(
        coordination.subscribe(&subscription).unwrap(),
        SubscriptionChange::Replayed {
            key: key.clone(),
            ref_count: 2,
        }
    );
    assert_eq!(
        coordination.unsubscribe(&key),
        SubscriptionChange::Decremented {
            key: key.clone(),
            ref_count: 1,
        }
    );
    assert_eq!(
        coordination.unsubscribe(&key),
        SubscriptionChange::Removed { key: key.clone() }
    );
    assert_eq!(
        coordination.unsubscribe(&key),
        SubscriptionChange::NotFound { key }
    );
}

#[test]
fn subscription_aggregate_deduplicates_observers_and_orders_scopes() {
    let target = SubscriptionTarget::resource(ResourceId::new("graph"));
    let first = SurfaceRef::new(SurfaceId::from_u64(2), SurfaceGeneration::initial());
    let second = SurfaceRef::new(SurfaceId::from_u64(7), SurfaceGeneration::from_u64(1));
    let app = AppScope::app();
    let alpha = AppScope::resource(ResourceId::new("alpha"));
    let beta = AppScope::resource(ResourceId::new("beta"));
    let subscriptions = [
        Subscription::new(SubscriptionKey::new(
            target.clone(),
            beta.clone(),
            second,
            SubscriptionPriority::Low,
        )),
        Subscription::new(SubscriptionKey::new(
            target.clone(),
            alpha.clone(),
            first,
            SubscriptionPriority::High,
        )),
        Subscription::new(SubscriptionKey::new(
            target.clone(),
            app.clone(),
            first,
            SubscriptionPriority::Normal,
        )),
        Subscription::new(SubscriptionKey::new(
            target.clone(),
            alpha.clone(),
            first,
            SubscriptionPriority::High,
        )),
    ];
    let mut coordination = CoordinationState::default();

    for subscription in &subscriptions {
        coordination.subscribe(subscription).unwrap();
    }

    let aggregate = coordination.aggregate(&target).unwrap();
    assert_eq!(aggregate.target(), &target);
    assert_eq!(aggregate.active_keys(), 3);
    assert_eq!(aggregate.observers(), &[first, second]);
    assert_eq!(aggregate.scopes(), &[app, alpha, beta]);
    assert_eq!(aggregate.highest_priority(), SubscriptionPriority::High);
    assert_eq!(
        coordination.resource_observer_count(&ResourceId::new("graph")),
        2
    );
}

#[test]
fn prototype_latest_search_wins_rejects_stale_completion() {
    let mut app = PrototypeApp::latest_search();

    app.start_search("rust", TaskIntentAttemptId::from_u64(1));
    app.start_search("rust async", TaskIntentAttemptId::from_u64(2));
    app.complete_search(TaskIntentAttemptId::from_u64(1), vec!["old"]);
    app.complete_search(TaskIntentAttemptId::from_u64(2), vec!["new"]);

    assert!(app.search_results().is_empty());
    app.drain();

    assert_eq!(app.search_results(), &["new"]);

    app.complete_search_with_provenance(
        TaskIntentAttemptId::from_u64(2),
        TaskIntentAttemptId::from_u64(1),
        vec!["payload-stale"],
    );
    app.drain();

    assert_eq!(app.search_results(), &["new"]);
}

#[test]
fn prototype_log_stream_accumulates_ordered_entries_with_budgeted_draining() {
    let mut app = PrototypeApp::log_stream(RuntimeBudget::default().with_max_task_inputs(10));

    for index in 0..35 {
        app.push_log_line(format!("line-{index:02}"));
    }

    assert!(app.log_lines().is_empty());
    app.drain();

    assert_eq!(app.log_lines().len(), 10);

    app.drain_all();
    assert_eq!(app.log_lines().first().unwrap(), "line-00");
    assert_eq!(app.log_lines().last().unwrap(), "line-34");
}

#[test]
fn stress_ten_thousand_task_events_use_coalesced_wakeups_and_budgeted_drains() {
    let mut app =
        PrototypeApp::progress_counter(RuntimeBudget::default().with_max_task_inputs(128));

    for index in 0..10_000 {
        app.proxy().send_task(app.progress_event(index)).unwrap();
    }

    assert_eq!(app.progress_count(), 0);
    assert!(app.fake_wake().wake_count() < 100);
    app.drain_all();
    assert_eq!(app.progress_count(), 10_000);
    assert_eq!(app.reducer_reentry_count(), 0);
}

#[test]
fn prototype_jsonrpc_service_handles_out_of_order_progress_cancel_timeout_and_reconnect() {
    let mut app = PrototypeApp::jsonrpc_service();

    let first = app.call_tool("compile");
    let second = app.call_tool("docs");
    app.notify_progress(second, "half");
    app.respond(first, "compiled");
    app.cancel(second);
    app.timeout(second);
    app.reconnect();

    assert_eq!(app.response(first), None);
    assert_eq!(app.request_status(second), ServiceRequestStatus::Pending);
    app.drain_all();

    assert_eq!(app.response(first), Some("compiled"));
    assert_eq!(
        app.request_status(second),
        ServiceRequestStatus::TimedOutAfterCancel
    );
    assert_eq!(
        app.service_status(ServiceId::new("jsonrpc")),
        ServiceStatus::Running
    );
}
