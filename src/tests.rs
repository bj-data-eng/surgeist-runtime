use super::testing::{FakeWakeBridge, HeadlessHarness, PrototypeApp, ServiceRequestStatus};
use super::*;
use std::time::Duration;

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
    expected_runtime.enqueue_ui(input.clone());
    let expected = expected_runtime.drain_once(RuntimeBudget::new());

    let mut app_loop = AppLoop::new(Runtime::new(CounterState::default(), CounterReducer));
    app_loop.runtime_mut().enqueue_ui(input);

    assert_eq!(app_loop.step(RuntimeBudget::new()), Ok(expected));
    assert_eq!(app_loop.into_runtime().state().value, 1);
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
    assert_eq!(CorrelationId::from_u64(11).as_u64(), 11);
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

    assert!(
        surface
            .set_viewport(SurfaceSize::new(640, 480))
            .unwrap()
            .changed()
    );
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

fn test_surface(surface_id: u64, window_id: u64, root_id: &str) -> UiSurface {
    UiSurface::try_new(
        SurfaceId::from_u64(surface_id),
        WindowId::from_u64(window_id),
        SurfaceRoot::new(RootId::new(root_id)),
    )
    .expect("test surface construction should be valid")
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
    let descriptor = TaskDescriptor::new(TaskIntentName::new("search"), "SearchInput");

    assert_eq!(descriptor.name().as_str(), "search");
    assert_eq!(descriptor.input_type(), "SearchInput");
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
    let parent = CorrelationId::from_u64(1);
    let child = InputProvenance::task(TaskIntentId::from_u64(2), TaskIntentAttemptId::from_u64(3))
        .with_surface(SurfaceId::from_u64(4))
        .with_correlation(CorrelationId::from_u64(5))
        .with_parent(parent);

    assert_eq!(child.source(), &InputSourceId::TASK);
    assert!(matches!(child.origin(), InputOrigin::Task(_)));
    assert_eq!(child.task_id(), Some(TaskIntentId::from_u64(2)));
    assert_eq!(
        child.task_attempt_id(),
        Some(TaskIntentAttemptId::from_u64(3))
    );
    assert_eq!(child.surface_id(), Some(SurfaceId::from_u64(4)));
    assert_eq!(child.correlation_id(), CorrelationId::from_u64(5));
    assert_eq!(child.parent_correlation_id(), Some(parent));
}

#[test]
fn diagnostics_keep_recent_entries_and_counters() {
    let mut log = DiagnosticLog::with_capacity(2);
    log.push(Diagnostic::warning(
        DiagnosticCode::UNKNOWN_RETAINED_COMMAND,
        "missing binding",
        InputProvenance::ui(SurfaceId::from_u64(1)),
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

#[derive(Default)]
struct CounterState {
    value: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CounterInput {
    Increment,
    RedrawAll,
    RedrawWindow(WindowId),
    Save,
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

    let drained = proxy.drain_pending(8);
    assert_eq!(drained.len(), 2);
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
    fn reduce(&mut self, state: &mut CounterState, input: AppInput<CounterInput>) -> ReducerResult {
        match input.payload() {
            CounterInput::Increment => {
                state.value += 1;
                ReducerResult::changed().with_effect(AppEffect::request_redraw(
                    RedrawTarget::surface(SurfaceRef::new(
                        SurfaceId::from_u64(1),
                        SurfaceGeneration::initial(),
                    )),
                ))
            }
            CounterInput::RedrawAll => ReducerResult::unchanged()
                .with_effect(AppEffect::request_redraw(RedrawTarget::all())),
            CounterInput::RedrawWindow(window_id) => ReducerResult::unchanged()
                .with_effect(AppEffect::request_redraw(RedrawTarget::Window(*window_id))),
            CounterInput::Save => ReducerResult::unchanged()
                .with_effect(AppEffect::persist("counter", AppScope::app())),
            CounterInput::StartTask => ReducerResult::changed().with_effect(AppEffect::start_task(
                TaskIntentName::new("counter"),
                TaskIntentKey::new("counter:increment"),
                AppScope::app(),
            )),
        }
    }
}

#[test]
fn reducer_returns_effects_without_executing_them() {
    let mut reducer = CounterReducer;
    let mut state = CounterState::default();
    let result = reducer.reduce(
        &mut state,
        AppInput::new(CounterInput::Increment, InputProvenance::system()),
    );

    assert_eq!(state.value, 1);
    assert!(result.is_changed());
    assert_eq!(result.effects().len(), 1);
    assert_eq!(result.effects()[0].kind(), &EffectKindId::REQUEST_REDRAW);

    let result = reducer.reduce(
        &mut state,
        AppInput::new(CounterInput::Save, InputProvenance::system()),
    );

    assert_eq!(state.value, 1);
    assert!(!result.is_changed());
    assert_eq!(result.effects().len(), 1);
    assert_eq!(result.effects()[0].kind(), &EffectKindId::PERSIST);
}

#[test]
fn runtime_commits_state_before_executing_effects() {
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    runtime.add_surface(test_surface(1, 1, "main"));

    runtime.enqueue_ui(UiInput::new(CounterInput::Increment, InputProvenance::system()).unwrap());
    let report = runtime.drain_once(RuntimeBudget::default());

    assert_eq!(runtime.state().value, 1);
    assert_eq!(runtime.state_version(), StateVersion::from_u64(1));
    assert_eq!(report.executed_effects(), 1);
    assert_eq!(report.redraw_requests(), &[SurfaceId::from_u64(1)]);
}

#[test]
fn runtime_reports_task_intents_without_executing_them() {
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    runtime.enqueue_ui(
        UiInput::new(
            CounterInput::StartTask,
            InputProvenance::ui(SurfaceId::from_u64(1)),
        )
        .unwrap(),
    );

    let report = runtime.drain_once(RuntimeBudget::new());

    assert_eq!(report.executed_effects(), 1);
    assert_eq!(report.task_intents().len(), 1);
    assert_eq!(
        report.task_intents()[0].kind().as_str(),
        "runtime.start_task"
    );
    assert_eq!(runtime.diagnostics().entries().len(), 0);
}

#[test]
fn runtime_drains_ui_before_task_events_and_respects_budget() {
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    runtime.enqueue_task(
        TaskInput::new(
            CounterInput::Increment,
            InputProvenance::task(TaskIntentId::from_u64(1), TaskIntentAttemptId::from_u64(1)),
        )
        .unwrap(),
    );
    runtime.enqueue_ui(
        UiInput::new(
            CounterInput::Increment,
            InputProvenance::ui(SurfaceId::from_u64(1)),
        )
        .unwrap(),
    );

    let report = runtime.drain_once(RuntimeBudget::new().max_inputs(1));

    assert_eq!(runtime.state().value, 1);
    assert_eq!(report.drained_inputs(), 1);
    assert_eq!(report.remaining_task_inputs(), 1);
    assert_eq!(report.first_drained_lane(), Some(RuntimeLane::Ui));
}

#[test]
fn runtime_default_budget_caps_drained_inputs() {
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    for index in 0..65 {
        runtime.enqueue_task(
            TaskInput::new(
                CounterInput::Increment,
                InputProvenance::task(
                    TaskIntentId::from_u64(index),
                    TaskIntentAttemptId::from_u64(1),
                ),
            )
            .unwrap(),
        );
    }

    let report = runtime.drain_once(RuntimeBudget::default());

    assert_eq!(runtime.state().value, 64);
    assert_eq!(report.drained_inputs(), 64);
    assert_eq!(report.remaining_task_inputs(), 1);
}

#[test]
fn runtime_task_queue_overflow_records_diagnostic_and_drops_newest() {
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer)
        .with_queue_policy(RuntimeQueuePolicy::new().max_task_inputs(2));
    for index in 0..3 {
        runtime.enqueue_task(
            TaskInput::new(
                CounterInput::Increment,
                InputProvenance::task(
                    TaskIntentId::from_u64(index),
                    TaskIntentAttemptId::from_u64(1),
                ),
            )
            .unwrap(),
        );
    }

    let diagnostic = runtime
        .diagnostics()
        .entries()
        .into_iter()
        .find(|diagnostic| diagnostic.code() == &DiagnosticCode::QUEUE_OVERFLOW)
        .expect("task queue overflow should emit a diagnostic");

    assert_eq!(diagnostic.queue().unwrap().name(), "runtime.task");
    assert_eq!(diagnostic.queue().unwrap().capacity(), 2);
    assert_eq!(diagnostic.queue().unwrap().dropped(), 1);
    assert_eq!(
        runtime.diagnostics().count(&DiagnosticCode::QUEUE_OVERFLOW),
        1
    );

    let report = runtime.drain_once(RuntimeBudget::default());

    assert_eq!(runtime.state().value, 2);
    assert_eq!(report.drained_inputs(), 2);
    assert_eq!(report.remaining_task_inputs(), 0);
}

#[test]
fn runtime_service_queue_overflow_records_diagnostic_and_drops_newest() {
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer)
        .with_queue_policy(RuntimeQueuePolicy::new().max_service_inputs(1));
    for index in 0..2 {
        runtime.enqueue_service(
            ServiceInput::new(
                CounterInput::Increment,
                InputProvenance::service(ServiceId::new(format!("service.{index}"))),
            )
            .unwrap(),
        );
    }

    let diagnostic = runtime
        .diagnostics()
        .entries()
        .into_iter()
        .find(|diagnostic| diagnostic.code() == &DiagnosticCode::QUEUE_OVERFLOW)
        .expect("service queue overflow should emit a diagnostic");

    assert_eq!(diagnostic.queue().unwrap().name(), "runtime.service");
    assert_eq!(diagnostic.queue().unwrap().capacity(), 1);
    assert_eq!(diagnostic.queue().unwrap().dropped(), 1);
    assert_eq!(
        runtime.diagnostics().count(&DiagnosticCode::QUEUE_OVERFLOW),
        1
    );

    let report = runtime.drain_once(RuntimeBudget::default());

    assert_eq!(runtime.state().value, 1);
    assert_eq!(report.drained_inputs(), 1);
}

#[test]
fn runtime_redraw_all_reports_registered_surface_ids() {
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    runtime.add_surface(test_surface(2, 1, "secondary"));
    runtime.add_surface(test_surface(1, 1, "main"));
    runtime.enqueue_ui(UiInput::new(CounterInput::RedrawAll, InputProvenance::system()).unwrap());

    let report = runtime.drain_once(RuntimeBudget::default());

    assert_eq!(
        report.redraw_requests(),
        &[SurfaceId::from_u64(1), SurfaceId::from_u64(2)]
    );
}

#[test]
fn runtime_redraw_window_reports_surfaces_for_that_window() {
    let target_window = WindowId::from_u64(7);
    let other_window = WindowId::from_u64(8);
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    runtime.add_surface(test_surface(1, other_window.as_u64(), "other"));
    runtime.add_surface(test_surface(3, target_window.as_u64(), "right"));
    runtime.add_surface(test_surface(2, target_window.as_u64(), "left"));
    runtime.enqueue_ui(
        UiInput::new(
            CounterInput::RedrawWindow(target_window),
            InputProvenance::system(),
        )
        .unwrap(),
    );

    let report = runtime.drain_once(RuntimeBudget::default());

    assert_eq!(
        report.redraw_requests(),
        &[SurfaceId::from_u64(2), SurfaceId::from_u64(3)]
    );
}

struct FailingReducer;

impl Reducer<CounterState, CounterInput> for FailingReducer {
    fn reduce(
        &mut self,
        _state: &mut CounterState,
        _input: AppInput<CounterInput>,
    ) -> ReducerResult {
        ReducerResult::recoverable_failure("counter reducer rejected input")
    }
}

#[test]
fn runtime_turns_recoverable_reducer_errors_into_diagnostics() {
    let mut runtime = Runtime::new(CounterState::default(), FailingReducer);
    runtime.enqueue_ui(UiInput::new(CounterInput::Increment, InputProvenance::system()).unwrap());

    let report = runtime.drain_once(RuntimeBudget::default());

    assert_eq!(runtime.state().value, 0);
    assert_eq!(report.reducer_errors(), 1);
    assert_eq!(
        runtime.diagnostics().count(&DiagnosticCode::REDUCER_ERROR),
        1
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
fn resource_effects_expose_typed_payloads_and_kinds() {
    let load = AppEffect::load_resource(ResourceId::new("thumb:1"), AppScope::app());
    assert_eq!(load.kind(), &EffectKindId::LOAD_RESOURCE);
    assert!(matches!(
        load.payload(),
        AppEffectPayload::LoadResource(effect)
            if effect.id() == &ResourceId::new("thumb:1") && effect.scope() == &AppScope::app()
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
fn service_mailbox_reports_overflow_and_keeps_capacity() {
    let policy = MailboxPolicy::bounded(2).drop_oldest().observe_overflow();
    let mut mailbox = ServiceMailbox::<u32>::new(ServiceId::new("rpc"), policy);

    mailbox.push(1);
    mailbox.push(2);
    mailbox.push(3);

    assert_eq!(mailbox.len(), 2);
    assert_eq!(mailbox.overflow_count(), 1);
    assert_eq!(mailbox.drain().collect::<Vec<_>>(), vec![2, 3]);
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
        CorrelationId::from_u64(42),
    );
    assert_eq!(call.kind(), &EffectKindId::CALL_SERVICE);
    assert!(matches!(
        call.payload(),
        AppEffectPayload::CallService(effect)
            if effect.id() == &ServiceId::new("jsonrpc")
                && effect.command().as_str() == "textDocument/hover"
                && effect.payload().as_json_text() == r#"{"line":3}"#
                && effect.correlation() == CorrelationId::from_u64(42)
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
fn subscriptions_attach_and_detach_observers_without_owning_work() {
    let mut coord = CoordinationState::default();
    let sub = Subscription::task(TaskIntentKey::new("compile:main"))
        .scope(AppScope::resource(ResourceId::new("project:main")))
        .observer(SurfaceId::from_u64(1));

    coord.subscribe(sub.clone());
    assert_eq!(coord.observer_count(&sub.target()), 1);
    assert!(coord.is_observed(&sub.target()));

    coord.unsubscribe(&sub);
    assert_eq!(coord.observer_count(&sub.target()), 0);
    assert!(!coord.is_observed(&sub.target()));
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
    let mut app = PrototypeApp::log_stream(RuntimeBudget::new().max_task_events(10));

    for index in 0..35 {
        app.push_log_line(format!("line-{index:02}"));
    }

    assert!(app.log_lines().is_empty());
    app.drain();

    assert_eq!(app.log_lines().len(), 10);
    assert_eq!(app.remaining_task_inputs(), 25);

    app.drain_all();
    assert_eq!(app.log_lines().first().unwrap(), "line-00");
    assert_eq!(app.log_lines().last().unwrap(), "line-34");
}

#[test]
fn stress_ten_thousand_task_events_use_coalesced_wakeups_and_budgeted_drains() {
    let mut app = PrototypeApp::progress_counter(RuntimeBudget::new().max_task_events(128));

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
