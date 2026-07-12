# Runtime Review Findings Resolution Specification

Status: draft

Owning repository: `/Users/codex/Development/surgeist-runtime`

Review evidence: `plans/2026-07-11-repository-review.md` at
`34251095c626923ce7375555b74c67520f83078f`

## S0. Coordinator Envelope

Resolve every actionable finding in the repository review for this leaf crate.
Allowed mutations are crate-local manifest/dependency metadata, runtime source,
focused tests, README/check documentation, and canonical planning artifacts.
Logical cycle commits, publication to this repository's `origin/main`, remote
readback, and candidate handoffs are workflow outputs owned by `$surgeist-agent`.
Root `surgeist` and sibling crate writes remain out of scope; root adaptation is
returned as handoff after each published leaf candidate that changes facade
integration.

The crate is in progress and the user has authorized breaking runtime API
changes. Compatibility shims are not required when they would preserve a finding.

## S1. Public Dependency Boundary

`Cargo.toml` has no dependencies on `surgeist-retained`, `surgeist-window`, or
`surgeist-task`. Runtime source may mention sibling crate names only in comments
or docs that describe root-owned integration.

The public front door exports exactly these runtime-owned type families. Helper
types, traits, and structs not named here are private or crate-private even when
shown in later pseudo-code.

| Module | Public exports |
| --- | --- |
| `ids.rs` | `AppId`, `WindowId`, `SurfaceId`, `RootId`, `ElementId`, `SurfaceGeneration`, `SurfaceInvalidationGeneration`, `ResourceGeneration`, `ResourceOperationId`, `ResourceId`, `ServiceId`, `CustomScopeId`, `ExpressionId`, `CalcId`, `ValueExprId`, `TaskIntentId`, `TaskIntentAttemptId`, `CorrelationId`, `VersionError` |
| `surface.rs` | `UiSurface`, `SurfaceRef`, `SurfaceRoot`, `SurfaceElements`, `ElementRegistration`, `ElementPhase`, `SurfaceElementRef`, `SurfaceRoute`, `SurfaceRouteStep`, `SurfaceSize`, `SurfacePoint`, `SurfaceMutation`, `SurfaceLifecycle`, `SurfaceInvalidation`, `SurfaceInvalidationKind`, `SurfaceRenderState`, `SurfaceRenderFrame`, `SurfaceRenderAck`, `SurfaceError`, `SurfaceErrorCode` |
| `coord.rs` | `AppScope`, `ScopePathSegment`, `CoordinationState`, `Subscription`, `SubscriptionAggregate`, `SubscriptionKey`, `SubscriptionChange`, `SubscriptionError`, `SubscriptionErrorCode`, `SubscriptionPriority`, `SubscriptionTarget`, `SubscriptionTargetKindId` |
| `command.rs` / `event.rs` | `AppCommand`, `CommandDescriptor`, `CommandName`, `AppEvent`, `EventDescriptor`, `EventName`, `PayloadTypeName`, `NameError` |
| `descriptor.rs` | `App`, `AppDescriptor`, `AppManifest`, `ValidatedAppManifest`, `ManifestValidationError`, `ManifestValidationErrorCode`, `ManifestValidationIssue`, `ResourceDescriptor`, `RootDescriptor`, `StartupWindow`, `TaskDescriptor`, `WindowDescriptor`, `WindowDescriptorId` |
| `diagnostic.rs` | `Diagnostic`, `DiagnosticCode`, `DiagnosticLog`, `DiagnosticSeverity`, `QueueDiagnostic` |
| `effect.rs` | `AppEffect`, `AppEffectPayload`, `EffectBatch`, `EffectKindId`, `EffectDisposition`, `EffectOutcome`, `RuntimeIntent`, `RedrawTarget`, `RequestRedrawEffect`, `PersistEffect`, `DiagnosticEffect`, `LoadResourceEffect`, `InvalidateResourceEffect`, `StartTaskEffect`, `CancelTaskEffect`, `ReprioritizeTaskEffect`, `StartServiceEffect`, `StopServiceEffect`, `CallServiceEffect`, `ServiceDiagnosticEffect` |
| `input.rs` / `provenance.rs` | `AppInput`, `Correlation`, `CorrelationError`, `InputOrigin`, `InputProvenance`, `InputSourceId`, `ProvenanceError`, `ProvenanceErrorCode`, `ServiceProvenance`, `SurfaceProvenance`, `TaskProvenance` |
| `loop_.rs` / `proxy.rs` | `AppLoop`, `AppProxy`, `AppProxyError`, `AppProxyErrorCode`, `ProxyDrainReport`, `ProxyInput`, `QueuePolicy`, `WakeBridge`, `WakeError` |
| `reducer.rs` | `Reducer`, `ReducerCommit`, `ReducerChange`, `ReducerFailure`, `ReducerResult` |
| `resource.rs` | `FailureVisibility`, `Freshness`, `ResourceOperation`, `ResourceSnapshot`, `ResourceState`, `ResourceStateError`, `ResourceStateErrorCode`, `ResourceStatus` |
| `runtime.rs` | `Runtime`, `RuntimeBudget`, `RuntimeDrainError`, `RuntimeDrainErrorCode`, `RuntimeDrainReport`, `RuntimeInputError`, `RuntimeLane`, `RuntimeQueueError`, `RuntimeQueueErrorCode`, `RuntimeQueuePolicy`, `ServiceInput`, `TaskInput`, `UiInput` |
| `service.rs` / `snapshot.rs` | `MailboxOverflow`, `MailboxPolicy`, `MailboxPushOutcome`, `ServiceCommandName`, `ServiceCommandPayload`, `ServiceMailbox`, `ServiceRegistration`, `ServiceRestart`, `ServiceShutdown`, `ServiceStartup`, `ServiceStatus`, `AppSnapshot`, `SnapshotBinding`, `SnapshotBindingId`, `SnapshotEntry`, `SnapshotError`, `SnapshotErrorCode`, `SnapshotSourceType`, `SnapshotValue`, `StateVersion` |
| `task.rs` | `TaskIntentAttemptId`, `TaskIntentHandle`, `TaskIntentId`, `TaskIntentKey`, `TaskIntentName`, `TaskPriorityHint` |

`AppScope::window` accepts `WindowId`; `AppScope::window_id` returns
`Option<WindowId>`. `Diagnostic::with_window` and `Diagnostic::window_id` use
`WindowId`. `RedrawTarget::Window` carries `WindowId`, while
`RedrawTarget::Surface` carries a generation-qualified `SurfaceRef`.

`AppHandler` is removed. Host callbacks and native event-loop traits are root-owned
adapter surface. Runtime's app loop is only a deterministic orchestration owner:

```rust
pub struct AppLoop<State, R, Input> { runtime: Runtime<State, R, Input> }
AppLoop::new(runtime) -> Self
AppLoop::runtime(&self) -> &Runtime<State, R, Input>
AppLoop::runtime_mut(&mut self) -> &mut Runtime<State, R, Input>
AppLoop::step(&mut self, budget: RuntimeBudget) -> Result<RuntimeDrainReport, RuntimeDrainError>
AppLoop::into_runtime(self) -> Runtime<State, R, Input>
```

`step` delegates exactly once to `Runtime::drain_once`. `AppLoop` has no native
loop generic, callback trait, concrete host storage, wake bridge, or proxy-drain
lowering. Root drains `AppProxy`, handles its continuation report, and enqueues
the resulting typed task/service inputs through the public runtime queue methods.

The `bridge` module is removed from the public runtime API. Retained command
decoding, retained route traversal, retained model access, and native window
loop composition are root-owned adapter work.

`testing.rs` is not an unconditional public module and no public support feature
is created for this initiative. App fixtures such as `HeadlessApp`,
`HeadlessHarness`, fake native windows, fake clocks, fake wake bridges, and
fixture reducers are moved under `#[cfg(test)]` in `src/tests.rs` or made private
test helpers. Production exports remove `pub mod testing` and all `pub use
testing::*` reexports.

## S2. Runtime-Owned Surface Model

`WindowId`, `SurfaceId`, and `ElementId` are distinct private-field `u64`
newtypes. Every `u64`, including zero, is a valid opaque identity; zero has no
sentinel meaning. Each derives `Clone`, `Copy`, `Debug`, `Eq`, `Hash`, `Ord`, and
the corresponding partial traits and exposes only:

```rust
WindowId::from_u64(value) -> Self
WindowId::as_u64(self) -> u64
SurfaceId::from_u64(value) -> Self
SurfaceId::as_u64(self) -> u64
ElementId::from_u64(value) -> Self
ElementId::as_u64(self) -> u64
```

These conversions are total identity mappings for root adapters. The newtypes do
not implement cross-ID conversions, so window, surface, and element values cannot
be mixed accidentally.

`SurfaceSize` stores unsigned logical viewport dimensions; zero width or height is
valid. `SurfacePoint` stores signed logical coordinates; every `i32` pair is valid
so adapters can preserve negative scroll/overscroll positions without clipping.

`surface.rs` owns these public types:

```rust
pub struct SurfaceRoot { id: RootId, elements: SurfaceElements }
pub struct SurfaceElements { registrations: BTreeMap<ElementId, ElementRegistration> }
pub struct ElementRegistration { id: ElementId, phases: BTreeSet<ElementPhase> }
pub struct SurfaceRef { surface_id: SurfaceId, generation: SurfaceGeneration }
pub struct SurfaceElementRef { surface: SurfaceRef, element_id: ElementId }
pub struct SurfaceRoute {
    surface: SurfaceRef,
    steps: Vec<SurfaceRouteStep>,
}
pub struct SurfaceRouteStep { element_id: ElementId, phase: ElementPhase }
pub struct SurfaceSize { width: u32, height: u32 }
pub struct SurfacePoint { x: i32, y: i32 }
pub struct SurfaceMutation {
    changed: bool,
    invalidation_generation: Option<SurfaceInvalidationGeneration>,
    redraw_required: bool,
}
pub enum ElementPhase { Capture, Target, Bubble }
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
pub struct SurfaceError {
    code: SurfaceErrorCode,
    message: String,
    source: Option<VersionError>,
}
```

Construction and inspection are public through checked methods:

```rust
SurfaceGeneration::initial() -> Self
SurfaceGeneration::from_u64(value) -> Self
SurfaceGeneration::as_u64() -> u64
SurfaceInvalidationGeneration::initial() -> Self
SurfaceInvalidationGeneration::from_u64(value) -> Self
SurfaceInvalidationGeneration::as_u64() -> u64
SurfaceSize::new(width, height) -> Self
SurfaceSize::default() == SurfaceSize::new(0, 0)
SurfaceSize::width(&self) -> u32
SurfaceSize::height(&self) -> u32
SurfacePoint::new(x, y) -> Self
SurfacePoint::origin() -> Self
SurfacePoint::default() == SurfacePoint::origin()
SurfacePoint::x(&self) -> i32
SurfacePoint::y(&self) -> i32
SurfaceMutation::changed(&self) -> bool
SurfaceMutation::invalidation_generation(&self) -> Option<SurfaceInvalidationGeneration>
SurfaceMutation::redraw_required(&self) -> bool
ElementRegistration::try_new(id, phases) -> Result<Self, SurfaceError>
ElementRegistration::id(&self) -> ElementId
ElementRegistration::phases(&self) -> impl Iterator<Item = ElementPhase>
SurfaceRoot::new(root_id) -> Self
SurfaceRoot::id(&self) -> &RootId
SurfaceRoot::register_element(registration) -> Result<(), SurfaceError>
SurfaceRoot::elements() -> &SurfaceElements
SurfaceElements::get(element_id) -> Option<&ElementRegistration>
SurfaceElements::iter() -> impl Iterator<Item = &ElementRegistration>
SurfaceRef::new(surface_id, generation) -> Self
SurfaceRef::surface_id(&self) -> SurfaceId
SurfaceRef::generation(&self) -> SurfaceGeneration
SurfaceElementRef::new(surface, element_id) -> Self
SurfaceElementRef::surface(&self) -> SurfaceRef
SurfaceElementRef::surface_id(&self) -> SurfaceId
SurfaceElementRef::generation(&self) -> SurfaceGeneration
SurfaceElementRef::element_id(&self) -> ElementId
SurfaceRouteStep::new(element_id, phase) -> Self
SurfaceRouteStep::element_id(&self) -> ElementId
SurfaceRouteStep::phase(&self) -> ElementPhase
SurfaceRoute::try_new(surface, steps) -> Result<Self, SurfaceError>
SurfaceRoute::surface(&self) -> SurfaceRef
SurfaceRoute::surface_id(&self) -> SurfaceId
SurfaceRoute::generation(&self) -> SurfaceGeneration
SurfaceRoute::steps(&self) -> &[SurfaceRouteStep]
SurfaceRoute::target(&self) -> SurfaceElementRef
```

All private-field types above expose accessors for their stored semantic values.
Both generation `initial()` values are zero. A surface with no prior invalidation
uses `SurfaceInvalidationGeneration::initial()` for its first invalidation and
then checked successors, retaining the last issued counter after acknowledgements
empty the queue.
`ElementRegistration::try_new` rejects an empty phase set with
`MissingElementPhase`. Adding an element rejects duplicate `ElementId` values.
The runtime does not store retained elements and therefore cannot panic on
retained model validation.

`SurfaceError` exposes `code()` and `message()`; `Error::source` returns the
`VersionError` only for `VersionOverflow`.

`SurfaceRoute::try_new` rejects an empty route, requires exactly one `Target`
step, requires every `Capture` step to precede that target, and requires every
`Bubble` step to follow it. It reports `EmptyRoute`, `MissingRouteTarget`,
`MultipleRouteTargets`, or `InvalidRoutePhaseOrder` respectively. Its `target()`
accessor returns the target as a generation-qualified `SurfaceElementRef`.

`UiSurface::try_new(surface_id, window_id, root) -> Result<Self, SurfaceError>`
constructs a surface at generation `SurfaceGeneration::initial()`, lifecycle
`Created`, zero viewport, origin scroll offset, no focus/hover, and no
invalidations.

`UiSurface::replace_root(root) -> Result<SurfaceGeneration, SurfaceError>`
increments generation with the checked generation policy, clears
focus and hover, records a new invalidation with
`SurfaceInvalidationKind::RootReplaced { surface_generation: generation }`, and is
atomic on error. `UiSurface::id()`, `window_id()`, `generation()`, `root()`, and
`lifecycle()` expose current runtime identity and state.

`UiSurface::surface_ref()` returns its current generation-qualified identity.
`Runtime::register_surface` is the authoritative issuer because it may replace the
initial generation with a tombstone successor during re-registration.

Focus, hover, and adapter-originated element input use `SurfaceElementRef`.
`UiSurface::element_ref(element_id)` constructs a reference from its current
`SurfaceRef` only after proving the element exists.
Root adapters may also retain
or reconstruct earlier references with `SurfaceElementRef::new` so runtime can
reject them as stale.
The underlying crate-private
`UiSurface::validate_element_ref(reference) -> Result<(), SurfaceError>` rejects
surface mismatch, stale generation, and unknown element in that order.
Crate-private `UiSurface::validate_element(reference, phase)`
first applies `validate_element_ref`, then rejects a phase not registered for that
element with `IneligibleElementTarget`.

Crate-private `UiSurface::validate_route(route)`
first applies the same surface and generation checks to the route, then validates
every step against the root registration for that element and phase. It returns
the route target only after every step succeeds. A route containing an unknown
element or a step whose phase was not registered returns `UnknownElement` or
`IneligibleElementTarget`; route-shape errors retain the constructor codes above.
Root owns retained-tree traversal, but it must lower the result into this route and
pass Runtime validation before dispatching element input:

```rust
Runtime::validate_element(reference: SurfaceElementRef, phase: ElementPhase) -> Result<(), SurfaceError>
Runtime::validate_route(route: &SurfaceRoute) -> Result<SurfaceElementRef, SurfaceError>
```

Both methods apply exact precedence: absent surface ID is `UnknownSurface`;
generation mismatch is `StaleSurfaceGeneration`; `Created`, `Hidden`, `Occluded`,
or `Suspended` is `InvalidLifecycleTransition`; `Closing`, `Closed`, or
`Destroyed` is `TerminalSurface`; only then are unknown element, phase
eligibility, and route-step checks applied. `Ready` and `Resized` are the only
targetable lifecycles. `Runtime::surface` is a read-only state view and does not
expose the crate-private validators, so root cannot bypass registry or lifecycle
validation.

Runtime owns adapter-originated focus, hover, viewport, and scroll mutation for a
registered surface:

```rust
Runtime::resize(surface: SurfaceRef, viewport: SurfaceSize) -> Result<SurfaceMutation, SurfaceError>
Runtime::set_scroll_offset(surface: SurfaceRef, offset: SurfacePoint) -> Result<SurfaceMutation, SurfaceError>
Runtime::set_focus(surface: SurfaceRef, element: Option<SurfaceElementRef>) -> Result<SurfaceMutation, SurfaceError>
Runtime::set_hover(surface: SurfaceRef, element: Option<SurfaceElementRef>) -> Result<SurfaceMutation, SurfaceError>
UiSurface::viewport(&self) -> SurfaceSize
UiSurface::scroll_offset(&self) -> SurfacePoint
UiSurface::focused_element(&self) -> Option<SurfaceElementRef>
UiSurface::hovered_element(&self) -> Option<SurfaceElementRef>
```

Every Runtime mutation validates the target registration as unknown/stale first.
Focus, hover, and scroll changes are allowed in `Created`, `Ready`, `Resized`,
`Hidden`, `Occluded`, and `Suspended` and return `TerminalSurface` in `Closing`,
`Closed`, or `Destroyed`. A non-`None` focus/hover reference is then checked with
`validate_element_ref`; clearing with `None` needs no element validation. Resize
uses the lifecycle rules in S3. Lifecycle validation precedes duplicate/idempotent
comparison, so a terminal surface never accepts a nominal no-op.

Setting the same focus, hover, scroll offset, or viewport is idempotent and
returns `SurfaceMutation { changed: false, invalidation_generation: None,
redraw_required: false }`. A real change preflights the next invalidation
generation, mutates exactly that field, and records one `SurfaceChanged`
invalidation (`ViewportChanged` for resize). Overflow returns `VersionOverflow`
without mutation. A changed result exposes the new invalidation generation and
sets `redraw_required` exactly when the post-change lifecycle is `Ready` or
`Resized`; inactive surfaces retain the invalidation for
`renderable_invalidated_surfaces`. Focus and hover are independent. Root
replacement clears both and relies on its single `RootReplaced` invalidation
rather than recording extra focus/hover invalidations.

## S3. Surface Lifecycle And Render Acknowledgement

Surface lifecycle transitions are fallible and use this matrix:

| Current | Allowed next |
| --- | --- |
| Created | Ready, Closing, Closed, Destroyed |
| Ready | Resized, Hidden, Occluded, Suspended, Closing, Closed, Destroyed |
| Resized | Ready, Hidden, Occluded, Suspended, Closing, Closed, Destroyed |
| Hidden | Ready, Closing, Closed, Destroyed |
| Occluded | Ready, Hidden, Suspended, Closing, Closed, Destroyed |
| Suspended | Ready, Hidden, Closing, Closed, Destroyed |
| Closing | Closed, Destroyed |
| Closed | Destroyed |
| Destroyed | none |

`UiSurface::transition_to(next)` returns `Result<SurfaceLifecycle, SurfaceError>`.
Convenience methods such as `ready`, `hidden`, and `destroyed` delegate to that
method and return the same result. `Closing`, `Closed`, and `Destroyed` are
terminal for runtime mutation and targeting. Terminal states reject later viewport, root,
focus, hover, invalidation, render-begin, and render-ack operations.

`Runtime::resize` is allowed from `Ready` and `Resized`; a changed viewport leaves
the lifecycle at `Resized` and records `SurfaceInvalidationKind::ViewportChanged`
under S2. `Created`, `Hidden`, `Occluded`, and `Suspended` return
`InvalidLifecycleTransition`; terminal states return `TerminalSurface`. An equal
viewport is the S2 idempotent outcome after lifecycle validation.

Invalidations carry a per-surface checked generation so render acknowledgements
can consume exactly the work visible to a frame:

```rust
pub struct SurfaceInvalidation {
    generation: SurfaceInvalidationGeneration,
    kind: SurfaceInvalidationKind,
}
pub enum SurfaceInvalidationKind {
    RootReplaced { surface_generation: SurfaceGeneration }
    SnapshotChanged { version: StateVersion }
    ViewportChanged
    SurfaceChanged
}
pub struct SurfaceRenderFrame {
    surface: SurfaceRef,
    state_version: StateVersion,
    invalidation_generation: Option<SurfaceInvalidationGeneration>,
}
pub struct SurfaceRenderState<'a, State> {
    state: &'a State,
    frame: SurfaceRenderFrame,
}
pub struct SurfaceRenderAck {
    surface: SurfaceRef,
    state_version: StateVersion,
    acknowledged_frame_generation: Option<SurfaceInvalidationGeneration>,
    consumed_invalidations: usize,
    remaining_invalidations: usize,
    redraw_required: bool,
}
```

Render eligibility is lifecycle-specific:

| Lifecycle | `begin_render` / `mark_rendered` |
| --- | --- |
| Ready, Resized | allowed |
| Created, Hidden, Occluded, Suspended | `InvalidLifecycleTransition` |
| Closing, Closed, Destroyed | `TerminalSurface` |

Only Runtime publicly issues and accepts frames:

```rust
Runtime::begin_render(&self, surface: SurfaceRef) -> Result<SurfaceRenderState<'_, State>, SurfaceError>
Runtime::mark_rendered(&mut self, frame: SurfaceRenderFrame) -> Result<SurfaceRenderAck, SurfaceError>
SurfaceRenderState::state(&self) -> &State
SurfaceRenderState::frame(&self) -> &SurfaceRenderFrame
SurfaceRenderState::into_frame(self) -> SurfaceRenderFrame
SurfaceRenderFrame::surface(&self) -> SurfaceRef
SurfaceRenderFrame::state_version(&self) -> StateVersion
SurfaceRenderFrame::invalidation_generation(&self) -> Option<SurfaceInvalidationGeneration>
SurfaceRenderAck::surface(&self) -> SurfaceRef
SurfaceRenderAck::state_version(&self) -> StateVersion
SurfaceRenderAck::acknowledged_frame_generation(&self) -> Option<SurfaceInvalidationGeneration>
SurfaceRenderAck::consumed_invalidations(&self) -> usize
SurfaceRenderAck::remaining_invalidations(&self) -> usize
SurfaceRenderAck::redraw_required(&self) -> bool
```

`begin_render` validates the current registration and lifecycle, then captures
that `SurfaceRef`, Runtime's current `StateVersion`, and the highest invalidation
generation present when the frame begins. `SurfaceRenderState::state()` returns
the same immutable Runtime state protected by that borrow, while `frame()` exposes
its metadata. Runtime cannot drain or otherwise mutably change state until the
root renderer consumes the view with `into_frame()` and releases the borrow. The
frame has read-only accessors and no public constructor. The underlying
`UiSurface` begin/ack operations are
crate-private so a caller cannot supply an arbitrary state version or bypass
registration validation. Rejection does not change render or invalidation state.

`Runtime::mark_rendered` is allowed only in the lifecycle matrix above and is
monotonic. A frame whose surface ID is not registered returns `UnknownSurface`;
a frame whose generation differs from the current registration returns
`StaleSurfaceGeneration`. A frame for a matching registration then applies the
lifecycle table before any mutation; all rejection changes nothing. A frame whose
state version is lower than
`last_rendered_state_version` returns `StaleRenderAck`. A successful ack stores
the frame state version and removes only invalidations with generation less than
or equal to the frame's captured invalidation generation that the frame could
represent. `SnapshotChanged { version }` is represented only when its version is
not greater than `frame.state_version`; a newer snapshot invalidation remains even
if its invalidation generation was already captured. Root, viewport, and generic surface
invalidations at or below the captured invalidation generation are represented
after the registration/generation checks pass. Invalidations added after
`begin_render` always remain queued.

Replaying an already acknowledged frame at the same state version is idempotent:
it consumes zero additional invalidations, never removes work newer than that
frame, and reports the current remaining count/redraw requirement. A lower state
version is never accepted as a replay.

The returned `SurfaceRenderAck` exposes every field shown above.
`consumed_invalidations` is the number removed by this acknowledgement;
`remaining_invalidations` is the post-ack queue length; and `redraw_required` is
true exactly when remaining invalidations exist and the current lifecycle is
`Ready` or `Resized`. Root must schedule another frame when it is true. An ack for
an older rendered state that leaves a newer `SnapshotChanged` invalidation
therefore reports remaining work instead of making the surface appear clean.

Runtime redraw requests to `RedrawTarget::All` select only current `Ready` and
`Resized` registrations. An explicit `Surface(SurfaceRef)` returns
`UnknownSurface` for an absent ID, `StaleSurfaceGeneration` for a replacement,
`InvalidLifecycleTransition` for `Created`/`Hidden`/`Occluded`/`Suspended`, and
`TerminalSurface` for closing/closed/destroyed. A `Window(WindowId)` target applies
to every matching `Ready` or `Resized` registration and succeeds when at least one
exists. With no matching surfaces it returns `UnknownSurface`; with matches but no
eligible surface it returns `InvalidLifecycleTransition` when any match is
non-terminal, otherwise `TerminalSurface`. Each rejection becomes a diagnostic
and rejected effect outcome.

## S3A. Runtime Surface Registry

`Runtime` jointly owns the surface registry, retired-generation tombstones, and
`CoordinationState`. Direct map or coordination mutation is not exposed.
Public methods are:

```rust
Runtime::register_surface(surface: UiSurface) -> Result<SurfaceRef, SurfaceError>
Runtime::surface(id: SurfaceId) -> Option<&UiSurface>
Runtime::surface_ref(id: SurfaceId) -> Option<SurfaceRef>
Runtime::update_surface(surface: SurfaceRef, update: impl FnOnce(&mut UiSurface) -> Result<(), SurfaceError>) -> Result<(), SurfaceError>
Runtime::remove_surface(surface: SurfaceRef) -> Result<UiSurface, SurfaceError>
Runtime::surface_ids() -> impl Iterator<Item = SurfaceId>
Runtime::renderable_invalidated_surfaces() -> impl Iterator<Item = SurfaceRef>
Runtime::coordination() -> &CoordinationState
Runtime::subscribe(subscription: Subscription) -> Result<SubscriptionChange, SubscriptionError>
Runtime::unsubscribe(key: &SubscriptionKey) -> Result<SubscriptionChange, SubscriptionError>
```

`register_surface` rejects an ID that is currently registered and rejects a
surface not in `Created` with `InvalidLifecycleTransition`; the supplied surface
must also carry `SurfaceGeneration::initial()` or it is
`StaleSurfaceGeneration`. First registration preserves that initial generation.
Removal returns the surface, stores its last
generation as a private tombstone, removes every subscription whose observer is
that surface, and unregisters it from future targeting as one atomic operation.
Re-registering a removed ID assigns the checked successor of the tombstone to the
replacement before insertion and returns the resulting `SurfaceRef`; overflow returns
`VersionOverflow` and changes nothing. Thus stale element references and old
subscriptions never become valid merely because an ID is reused.

`update_surface` and `remove_surface` reject an unknown ID with `UnknownSurface`
and a generation different from the current registration with
`StaleSurfaceGeneration` before mutation. There is no public
`surface_mut`; caller-supplied surface mutation passes through `update_surface`,
while render acknowledgement uses the dedicated Runtime method in S3.
The update runs against staged surface state. If the closure fails, neither the
surface nor coordination changes. If it advances the surface generation (for
example through root replacement), runtime removes every subscription for the old
`SurfaceRef` before committing; callers obtain the new identity through
`surface_ref(id)` and resubscribe explicitly. If it succeeds into `Closing`,
`Closed`, or `Destroyed`, runtime removes every subscription observed by the
current registration before committing both results. Cleanup is idempotent for
later terminal transitions and removal.

`Runtime::subscribe` and `unsubscribe` reject an unknown, stale-generation, or
terminal observer before delegating to coordination. Unsubscribe of an absent key
for the current registration remains idempotent under the S9 change semantics.
Coordination's mutation methods are
crate-private so runtime registry validation and terminal cleanup cannot be
bypassed; the public `CoordinationState` surface is read-only query state.

Runtime effect handling and redraw lookup use only this registry. Redraw to
`All` includes only registered `Ready` and `Resized` surfaces. Explicit targets
use the S3 unknown/stale/ineligible/terminal error mapping and are converted to a
rejected `EffectOutcome` diagnostic. `renderable_invalidated_surfaces` yields
current `Ready` and `Resized` registrations with nonempty invalidation queues in
the same deterministic order; root calls it after lifecycle changes so work that
accumulated while hidden, occluded, suspended, or created becomes schedulable on
return to a renderable lifecycle.

## S4. Reducer Commit API

Reducers receive immutable state and must return an explicit result:

```rust
pub trait Reducer<State, Input> {
    fn reduce(&mut self, state: &State, input: &AppInput<Input>) -> ReducerResult<State>;
}

pub enum ReducerResult<State> {
    Unchanged(ReducerCommit),
    Changed(ReducerChange<State>),
    RecoverableFailure(ReducerFailure),
}

pub struct ReducerCommit { effects: EffectBatch, provenance: Option<InputProvenance> }
pub struct ReducerChange<State> { state: State, commit: ReducerCommit }
pub struct ReducerFailure { message: String, provenance: Option<InputProvenance> }
```

The complete construction path is:

```rust
ReducerCommit::new() -> Self
ReducerCommit::default() == ReducerCommit::new()
ReducerCommit::with_effect(self, effect) -> Self
ReducerCommit::with_effects(self, batch) -> Self
ReducerCommit::with_provenance(self, provenance) -> Self
ReducerCommit::effects(&self) -> &EffectBatch
ReducerCommit::provenance(&self) -> Option<&InputProvenance>
ReducerChange::new(state, commit) -> Self
ReducerChange::state(&self) -> &State
ReducerChange::commit(&self) -> &ReducerCommit
ReducerFailure::new(message) -> Self
ReducerFailure::with_provenance(self, provenance) -> Self
ReducerFailure::message(&self) -> &str
ReducerFailure::provenance(&self) -> Option<&InputProvenance>
ReducerResult::unchanged(commit: ReducerCommit) -> Self
ReducerResult::changed(state, commit: ReducerCommit) -> Self
ReducerResult::recoverable_failure(failure: ReducerFailure) -> Self
```

Thus both successful variants always carry an explicit commit, including an
explicitly empty `ReducerCommit::new`, and changed reducers can attach effects and
provenance without rebuilding private fields. Failure construction remains
disjoint from commits.

Failure has no effect container and no changed state. Runtime retains ownership of
the queued input while the reducer borrows it. Runtime replaces `state` only for
`Changed`, increments the version only after replacement succeeds, and executes
effects only for `Changed` or `Unchanged`. This makes partial mutate-then-fail
commits unrepresentable and lets runtime requeue an input if a checked commit
cannot advance its version.

A successful `Changed` result always creates render work before its explicit
effects are applied. Runtime preflights the next `StateVersion` and one next
`SurfaceInvalidationGeneration` for every registered non-terminal surface. If any
checked value is exhausted, the entire input follows the S13 requeue/error path;
no state or surface changes. On success Runtime atomically installs the new state
and version, records `SnapshotChanged { version: new_state_version }` on every
registered surface in `Created`, `Ready`, `Resized`, `Hidden`, `Occluded`, or
`Suspended`, and adds every `Ready` or `Resized` registration to the drain report's
redraw requests. Closing, closed, and destroyed surfaces receive neither. An
`Unchanged` commit does not create an automatic snapshot invalidation. Explicit
redraw effects are then applied and deduplicated against the automatic requests in
ascending `SurfaceId` then generation order.

Effective provenance is deterministic. For each drained input, runtime records
`trigger_provenance = input.provenance().clone()`. A `ReducerCommit` or
`ReducerFailure` with explicit provenance overrides that trigger provenance;
otherwise the trigger provenance is used. Reducer-failure diagnostics use the
effective failure provenance. Every effect outcome produced from a successful
commit uses the effective commit provenance. Diagnostic effects may carry their
own diagnostic provenance in the diagnostic value, but the corresponding
`EffectOutcome::provenance` remains the effective commit provenance so adapter
handoff causality is stable.

## S5. Effect Outcomes And Adapter Intents

`EffectKindId` includes only kinds backed by a runtime path:

- `runtime.request_redraw`
- `runtime.persist`
- `runtime.emit_diagnostic`
- `runtime.load_resource`
- `runtime.invalidate_resource`
- `runtime.start_task`
- `runtime.cancel_task`
- `runtime.reprioritize_task`
- `runtime.start_service`
- `runtime.stop_service`
- `runtime.call_service`
- `runtime.service_diagnostic`

`runtime.schedule_timer` and `runtime.window_command` are removed until a future
cycle specifies successful runtime or adapter paths and tests them through
`Runtime`.

Runtime records each effect in `RuntimeDrainReport::effect_outcomes()`:

```rust
pub enum EffectDisposition { Applied, Forwarded, Rejected }
pub enum RuntimeIntent {
    Persist(PersistEffect),
    LoadResource(LoadResourceEffect),
    InvalidateResource(InvalidateResourceEffect),
    StartTask(StartTaskEffect),
    CancelTask(CancelTaskEffect),
    ReprioritizeTask(ReprioritizeTaskEffect),
    StartService(StartServiceEffect),
    StopService(StopServiceEffect),
    CallService(CallServiceEffect),
}
pub struct EffectOutcome {
    kind: EffectKindId,
    disposition: EffectDisposition,
    provenance: InputProvenance,
    intent: Option<RuntimeIntent>,
    diagnostic: Option<Diagnostic>,
}
```

Disposition rules:

- diagnostics are applied by appending to `DiagnosticLog`;
- redraw requests are applied by adding eligible current `SurfaceRef` values to
  `RuntimeDrainReport::redraw_requests`;
- task, resource, persistence, and service work is forwarded as
  `RuntimeIntent` for root adapters;
- invalid targets or impossible state produce `Rejected` with a diagnostic that
  preserves the effective commit provenance.

`AppEffect::load_resource(operation, scope)` and `LoadResourceEffect` carry the
complete `ResourceOperation` issued by `ResourceState`; the resource ID is read
from that token. The forwarded `RuntimeIntent::LoadResource` preserves the same
token unchanged so root can return completion or cancellation against the exact
operation. Runtime never manufactures a sibling task request or strips the
operation generation.

The previous `executed_effects` report field and accessor are removed. Callers
use `applied_effects`, `forwarded_effects`, and `rejected_effects`; no aggregate
field is exported.

## S6. Queue, Wake, And Drain Semantics

`RuntimeQueuePolicy` includes capacities for UI, task, and service lanes. Its
construction and defaults are exact:

```rust
RuntimeQueuePolicy::new(ui_capacity, task_capacity, service_capacity) -> Self
RuntimeQueuePolicy::with_ui_capacity(self, capacity) -> Self
RuntimeQueuePolicy::with_task_capacity(self, capacity) -> Self
RuntimeQueuePolicy::with_service_capacity(self, capacity) -> Self
RuntimeQueuePolicy::ui_capacity(&self) -> usize
RuntimeQueuePolicy::task_capacity(&self) -> usize
RuntimeQueuePolicy::service_capacity(&self) -> usize
RuntimeQueuePolicy::default() == RuntimeQueuePolicy::new(65_536, 65_536, 65_536)
QueuePolicy::bounded(capacity) -> Self
QueuePolicy::capacity(&self) -> usize
QueuePolicy::default() == QueuePolicy::bounded(65_536)
Runtime::new(state, reducer) -> Self
Runtime::new_with_queue_policy(state, reducer, policy) -> Self
Runtime::queue_policy(&self) -> RuntimeQueuePolicy
```

`Runtime::new` delegates to `new_with_queue_policy` with
`RuntimeQueuePolicy::default`. Queue policy is immutable after construction so a
caller cannot shrink capacity beneath already queued work. `AppProxy::new`
requires an explicit `QueuePolicy`; callers may pass its default deliberately.
Capacity zero is valid and rejects every enqueue. There is no implicit unbounded
policy.

Queue rejection is generic over the exact lane wrapper supplied by the caller:

```rust
#[non_exhaustive]
pub enum RuntimeQueueErrorCode { Overflow }
pub struct RuntimeQueueError<T> {
    code: RuntimeQueueErrorCode,
    lane: RuntimeLane,
    capacity: usize,
    rejected: T,
}
Runtime::enqueue_ui(input: UiInput<Input>) -> Result<(), RuntimeQueueError<UiInput<Input>>>
Runtime::enqueue_task(input: TaskInput<Input>) -> Result<(), RuntimeQueueError<TaskInput<Input>>>
Runtime::enqueue_service(input: ServiceInput<Input>) -> Result<(), RuntimeQueueError<ServiceInput<Input>>>
```

Overflow records one queue diagnostic but does not enqueue or consume the input.
`RuntimeQueueError` exposes `code()`, `lane()`, `capacity()`, `rejected()`, and
`into_rejected()` so callers can retry the exact value.

`RuntimeBudget` has these fields and defaults:

```rust
RuntimeBudget::new(max_inputs, max_ui_inputs, max_task_inputs, max_service_inputs) -> Self
RuntimeBudget::with_max_inputs(self, value) -> Self
RuntimeBudget::with_max_ui_inputs(self, value) -> Self
RuntimeBudget::with_max_task_inputs(self, value) -> Self
RuntimeBudget::with_max_service_inputs(self, value) -> Self
RuntimeBudget::max_inputs(&self) -> usize
RuntimeBudget::max_ui_inputs(&self) -> usize
RuntimeBudget::max_task_inputs(&self) -> usize
RuntimeBudget::max_service_inputs(&self) -> usize
RuntimeBudget::default() == RuntimeBudget::new(64, 32, 32, 32)
```

Every constructor and builder accepts zero. A zero global budget drains nothing.
A zero per-lane budget skips that lane for the drain call without changing
pending work.

`RuntimeDrainReport` exposes:

```rust
drained_inputs: usize
applied_effects: usize
forwarded_effects: usize
rejected_effects: usize
reducer_errors: usize
remaining_ui_inputs: usize
remaining_task_inputs: usize
remaining_service_inputs: usize
has_pending_inputs: bool
first_drained_lane: Option<RuntimeLane>
redraw_requests: Vec<SurfaceRef>
intents: Vec<RuntimeIntent>
effect_outcomes: Vec<EffectOutcome>
```

`Runtime::drain_once(budget)` returns
`Result<RuntimeDrainReport, RuntimeDrainError>`. `RuntimeDrainError` is:

```rust
#[non_exhaustive]
pub enum RuntimeDrainErrorCode { StateVersionOverflow, SurfaceInvalidationOverflow }
pub struct RuntimeDrainError {
    code: RuntimeDrainErrorCode,
    lane: RuntimeLane,
    provenance: InputProvenance,
    surface: Option<SurfaceRef>,
    partial_report: RuntimeDrainReport,
    source: VersionError,
}
```

It exposes accessors for every field plus `into_partial_report()` and implements
`Error` with the `VersionError` source. `surface` is populated only for
`SurfaceInvalidationOverflow`. The precise overflow disposition is defined in
S13.

`Runtime` stores persistent `next_drain_lane: RuntimeLane`. `drain_once` begins
at that lane, scans UI/task/service in cyclic order for the next lane with both
pending input and remaining per-lane budget, drains one input, then advances
`next_drain_lane` to the following lane. If no eligible lane remains, the drain
call stops even if pending inputs remain in budget-exhausted lanes. This
persistent rotation applies across calls, so repeated `max_inputs: 1` drains
cycle through eligible lanes instead of always starting at UI. Helpers such as
test harness `drain_all` stop only when `has_pending_inputs == false`.

Proxy wake failures use a separate, generic-free bridge error:

```rust
pub struct WakeError { message: String }
pub trait WakeBridge: Send + Sync + 'static {
    fn wake(&self) -> Result<(), WakeError>;
}
#[non_exhaustive]
pub enum AppProxyErrorCode { QueueOverflow, WakeFailed }
pub struct AppProxyError<Input> {
    code: AppProxyErrorCode,
    capacity: Option<usize>,
    rejected: ProxyInput<Input>,
    wake_error: Option<WakeError>,
}
AppProxy::send_task(input) -> Result<(), AppProxyError<Input>>
AppProxy::send_service(input) -> Result<(), AppProxyError<Input>>
```

`WakeBridge` is a deferred-signal capability, not a callback. Its public rustdoc
requires `wake` to arrange a future host turn and return without synchronously
calling `send_task`, `send_service`, or `drain_pending` on the same shared proxy
state, directly or indirectly, and without waiting for a drain to occur. Root-owned wake adapters
must prove this contract in their integration tests. The leaf fake bridge proves
the expected ordering by recording a signal during `wake` and invoking drain only
after the sending call has returned. Synchronous/reentrant drain implementations
are unsupported because they violate the capability precondition; runtime never
invokes user code while holding the proxy mutex.

`WakeError::new(message)` preserves the bridge's diagnostic message and implements
`Display` and `Error`. `AppProxyError` exposes `code()`, `capacity()`, `rejected()`,
`into_rejected()`, and `wake_error()`; `Error::source` returns the wake error when
present. Queue overflow returns the exact input without inserting it. Wake failure
removes and returns only the exact input whose send call owned that failed wake.

The proxy owns one mutex/condition-variable state machine with private states
`Idle`, `Waking`, `Signaled`, and `NeedsWake`. Each enqueued value carries an
unexported identity token used only for exact rollback. The linearization rules
are:

1. A sender inserts only after the capacity check. From `Idle` or `NeedsWake`, it
   becomes the one `Waking` owner, releases the lock, and calls `wake`.
2. Senders arriving during `Waking` enqueue, then wait on the condition variable;
   they do not report acceptance before that wake resolves.
3. Successful wake transitions to `Signaled`, wakes all waiting senders, and lets
   every queued sender return `Ok`. A failed wake removes only its owner's token,
   transitions to `NeedsWake` when other inputs remain or `Idle` when empty, and
   wakes all waiters. One waiter then owns the next wake attempt. Thus every
   `Ok` send observed a successful pending wake, and one failed send cannot strand
   a concurrently accepted input.
4. `Signaled` senders enqueue and return `Ok` without another wake because the
   already successful signal covers the queue.

`AppProxy::drain_pending(limit: NonZeroUsize)` returns a `#[must_use]`
`ProxyDrainReport<Input>` with drained inputs, remaining length,
`has_remaining`, and `continuation_wake_error: Option<WakeError>`. Emptying the
queue transitions to `Idle`. A drain waits while another thread is `Waking`, then
may consume from either `Signaled` or `NeedsWake`; `Idle` is necessarily empty.
A partial drain atomically transitions to `Waking`
and issues a successor wake before returning. Success returns to `Signaled` with
no continuation error. Failure transitions to `NeedsWake` and records the error
in the report; it is never represented as an ordinary signaled continuation.
The root host adapter must inspect that report and immediately continue draining
or surface the wake failure instead of waiting for another external event. A
later send from `NeedsWake` re-signals the entire existing backlog before it can
return `Ok`.

## S7. Provenance And Correlation

`CorrelationId` is a non-zero semantic ID. It has no public unchecked
constructor:

```rust
pub struct CorrelationId(NonZeroU64);
impl CorrelationId {
    pub fn try_from_u64(value: u64) -> Result<Self, CorrelationError>;
    pub const fn get(self) -> u64;
}
#[non_exhaustive]
pub enum CorrelationError { Zero }
pub enum Correlation { Absent, Present(CorrelationId) }
```

`CorrelationId::try_from_u64(0)` returns `CorrelationError`; therefore
`Correlation::Present` cannot carry zero. `Correlation::default()` is `Absent`;
`Correlation::is_absent()` and `id() -> Option<CorrelationId>` expose the state.

`InputProvenance` stores absence explicitly for both current and parent
correlation:

```rust
pub struct InputProvenance {
    origin: InputOrigin,
    correlation: Correlation,
    parent_correlation: Correlation,
    sequence: Option<u64>,
}
pub enum InputOrigin {
    System,
    Ui(SurfaceProvenance),
    Adapter(SurfaceProvenance),
    Task(TaskProvenance),
    Service(ServiceProvenance),
    Window(SurfaceProvenance),
}
pub struct SurfaceProvenance { surface: SurfaceRef }
pub struct TaskProvenance {
    task_id: TaskIntentId,
    task_attempt_id: TaskIntentAttemptId,
    surface: Option<SurfaceRef>,
}
pub struct ServiceProvenance { service_id: ServiceId }
```

Construction and mutation are complete:

```rust
InputProvenance::system() -> Self
InputProvenance::ui(surface: SurfaceRef) -> Self
InputProvenance::adapter(surface: SurfaceRef) -> Self
InputProvenance::task(task_id, attempt_id) -> Self
InputProvenance::service(service_id) -> Self
InputProvenance::window(surface: SurfaceRef) -> Self
InputProvenance::with_correlation(self, id: CorrelationId) -> Self
InputProvenance::without_correlation(self) -> Self
InputProvenance::with_parent_correlation(self, id: CorrelationId) -> Self
InputProvenance::without_parent_correlation(self) -> Self
InputProvenance::with_sequence(self, sequence: u64) -> Self
InputProvenance::without_sequence(self) -> Self
InputProvenance::correlation(&self) -> Correlation
InputProvenance::parent_correlation(&self) -> Correlation
InputProvenance::correlation_id(&self) -> Option<CorrelationId>
InputProvenance::parent_correlation_id(&self) -> Option<CorrelationId>
InputProvenance::sequence(&self) -> Option<u64>
InputProvenance::surface(&self) -> Option<SurfaceRef>
```

Every origin constructor initializes current and parent correlation to `Absent`
and sequence to `None`; there is no synthetic zero ID. Current and parent
correlation are independent causal fields: setting or clearing either leaves the
other unchanged, repeated sets are idempotent, and the nonzero `CorrelationId`
boundary prevents ambiguous values. Sequence setting/clearing is likewise
independent. Origin, source, task, attempt, service, surface, and sequence
accessors expose the corresponding private data. `InputSourceId` provides
`SYSTEM`, `UI`, `ADAPTER`, `TASK`, `SERVICE`, and `WINDOW` constants; there is no
retained-crate source type.

`InputProvenance::with_surface` becomes origin-specific:

```rust
#[non_exhaustive]
pub enum ProvenanceErrorCode {
    SurfaceAlreadyAttached,
    SurfaceOverwriteUnsupported,
    SurfaceUnsupportedOrigin,
}
```

- task provenance may attach one generation-qualified surface with
  `try_with_surface(surface: SurfaceRef) -> Result<Self, ProvenanceError>`;
- attaching the same surface to task provenance is idempotent;
- attaching a different surface to task provenance that already has a surface
  returns `ProvenanceErrorCode::SurfaceAlreadyAttached`;
- UI, window, and adapter provenance accept the same surface idempotently and
  reject a different surface with
  `ProvenanceErrorCode::SurfaceOverwriteUnsupported`;
- system and service provenance reject surface attachment with
  `ProvenanceErrorCode::SurfaceUnsupportedOrigin`.

`ProvenanceError` exposes `code()`, `origin()`, `existing_surface()`, and
`attempted_surface()` accessors. It implements `Display` and
`std::error::Error`.

Root-retained input re-entry is modeled as runtime-owned adapter provenance, not
a retained crate type. The source remains semantic, for example
`InputSourceId::ADAPTER`, with the surface and element reference carried by
runtime types.

## S8. Resource State Machine

`ResourceStatus` is:

```rust
Idle, Loading, Ready, Refreshing, Failed, Cancelling, Cancelled, Stale
```

`Starting` and `Running` are removed. They are not aliases and have no retained
public constructors.

`ResourceState` is the sole issuer of resource operations. Callers and root
adapters may carry a token but cannot construct one unchecked:

```rust
pub struct ResourceOperation {
    resource_id: ResourceId,
    id: ResourceOperationId,
    generation: ResourceGeneration,
}
#[non_exhaustive]
pub enum ResourceStateErrorCode {
    InvalidTransition,
    OperationMismatch,
    OperationOverlap,
    CancellationAlreadyRequested,
    AlreadyCancelled,
    VersionOverflow,
}
pub struct ResourceStateError {
    code: ResourceStateErrorCode,
    resource_id: ResourceId,
    expected_operation: Option<ResourceOperation>,
    actual_operation: Option<ResourceOperation>,
    source: Option<VersionError>,
}
```

`ResourceOperationId` wraps `NonZeroU64`, exposes only `get()`, and has no public
constructor. `ResourceGeneration` is a separate checked `u64` revision and is not
interchangeable with app `StateVersion`. `ResourceOperation` exposes
`resource_id()`, `id()`, and `generation()`. `ResourceState::new(id)` starts at `Idle`, resource generation zero,
next operation ID one, no active operation, and no last-cancelled operation.
`ResourceGeneration::{initial, from_u64, as_u64}` and
`ResourceState::generation()` provide read/test construction without exposing a
state mutation path.
`begin_load()` and `begin_refresh()` issue the next operation token and return it;
the token generation is the successfully advanced resource generation. The
token is carried unchanged by the load effect and all completion/cancellation
calls.

Allowed transitions:

| Current | Operation | Next |
| --- | --- | --- |
| Idle, Failed, Cancelled | begin_load() -> op | Loading |
| Ready, Stale | begin_refresh() -> op | Refreshing |
| Loading, Refreshing | ready(op, value) | Ready |
| Loading, Refreshing | failed(op, error, visibility) | Failed |
| Loading, Refreshing | cancel(op) | Cancelling |
| Cancelling | cancelled(op) | Cancelled |
| Ready | mark_stale(reason) | Stale |
| Stale | mark_stale(reason) | Stale |

`Freshness` is observable only for a retained value; a state with no value always
reports `Freshness::Fresh`. `stale_reason` records explicit invalidation context,
not a copy of a load error. The complete successful result-state matrix is:

| Operation | Status | Value | Error | Freshness | Stale reason | Active operation | Generation |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `new` | Idle | none | none | Fresh | none | none | initial |
| `begin_load` | Loading | preserve | clear | Stale iff a value exists, otherwise Fresh | preserve iff a value exists | issued token | +1 checked |
| `begin_refresh` | Refreshing | preserve | clear | preserve | preserve | issued token | +1 checked |
| `ready` | Ready | replace with new value | clear | Fresh | clear | clear | +1 checked |
| `failed(ClearValue)` | Failed | clear | replace with new error | Fresh | clear | clear | +1 checked |
| `failed(KeepStaleValue)` with a value | Failed | preserve | replace with new error | Stale | preserve | clear | +1 checked |
| `failed(KeepStaleValue)` without a value | Failed | none | replace with new error | Fresh | clear | clear | +1 checked |
| `cancel` | Cancelling | preserve | clear | preserve | preserve | preserve matching token | +1 checked |
| `cancelled` | Cancelled | preserve | clear | Stale iff a value exists, otherwise Fresh | preserve iff a value exists | clear; remember token as last-cancelled | +1 checked |
| `mark_stale(new reason)` | Stale | preserve | clear | Stale | replace with new reason | none | +1 checked |
| `mark_stale(same reason)` | unchanged | unchanged | unchanged | unchanged | unchanged | unchanged | unchanged |

`ResourceState` accessors and `ResourceSnapshot` expose `id`, `status`, `value`,
`error`, `freshness`, `stale_reason`, `generation`, and `active_operation`.
`is_renderable` is exactly `value().is_some()`. A snapshot clones those observable
fields at one generation and never contains observer count or the internal
last-cancelled replay marker.

All transition methods return `Result` and are failure-atomic. Error precedence
and replay behavior are exact:

- either begin method while an operation is `Loading`, `Refreshing`, or
  `Cancelling` returns `OperationOverlap`; a begin method used from any other
  non-allowed state returns `InvalidTransition`;
- a token with a different resource ID, operation ID, or generation from the
  active operation returns `OperationMismatch` before state-specific handling;
- `cancel` repeated with the matching token while `Cancelling` returns
  `CancellationAlreadyRequested`;
- after `cancelled(op)` succeeds, repeating `cancel(op)` or `cancelled(op)` with
  that last-cancelled token returns `AlreadyCancelled`;
- `ready` or `failed` with a matching token while `Cancelling`, and `cancelled`
  with a matching token before `Cancelling`, return `InvalidTransition`;
- any completion or cancellation carrying an older non-active token returns
  `OperationMismatch`;
- an exact repeated `mark_stale` reason while already `Stale` is idempotent and
  does not advance the generation; changing the stale reason is a state transition;
- exhaustion of either the next operation ID or checked resource generation returns
  `VersionOverflow` with `VersionError` as its source and changes no status,
  value, error, token counter, active operation, or version.

Every successful non-idempotent transition advances the resource generation.
Successful `begin_load` and `begin_refresh` clear the prior last-cancelled replay
marker before storing the issued active operation.
Successful `ready` and `failed` clear the active token. Successful `cancelled`
clears it and records it as the last-cancelled token solely for duplicate
cancellation classification. `ResourceStateError` exposes every field shown
above. `ResourceState` no longer stores observer count; observation is derived
from `CoordinationState`.

## S9. Subscription Ownership

`CoordinationState` owns subscriptions as full identities:

```rust
pub struct SubscriptionKey {
    target: SubscriptionTarget,
    scope: AppScope,
    observer: SurfaceRef,
    priority: SubscriptionPriority,
}
pub struct Subscription { key: SubscriptionKey }
pub enum SubscriptionChange {
    Added { key: SubscriptionKey, ref_count: usize },
    Replayed { key: SubscriptionKey, ref_count: usize },
    Decremented { key: SubscriptionKey, ref_count: usize },
    Removed { key: SubscriptionKey },
    NotFound { key: SubscriptionKey },
}
#[non_exhaustive]
pub enum SubscriptionErrorCode {
    UnknownObserver,
    StaleObserver,
    TerminalObserver,
    RefCountOverflow,
}
pub struct SubscriptionError { code: SubscriptionErrorCode, key: SubscriptionKey }
pub struct SubscriptionAggregate {
    target: SubscriptionTarget,
    active_keys: usize,
    observers: Vec<SurfaceRef>,
    scopes: Vec<AppScope>,
    highest_priority: SubscriptionPriority,
}
```

`SubscriptionKey::new(target, scope, observer: SurfaceRef, priority)` and
`Subscription::new(key)` are the only full constructors; convenience constructors
for task, resource, and service targets still require the scope, observer, and
priority instead of inventing a zero observer. All key fields have accessors.

`Runtime::subscribe(subscription)` returns
`Result<SubscriptionChange, SubscriptionError>` and owns one reference on
success after registry validation. An unregistered observer returns
`UnknownObserver`; a registered ID at a different generation returns
`StaleObserver`; a closing, closed, or destroyed matching registration returns
`TerminalObserver`; all leave coordination unchanged. A valid new key returns
`Added { ref_count: 1 }`. Replaying the same
full key increments its checked `usize` reference count and returns `Replayed`
with the new count; it does not create another aggregation identity. Refcount
overflow returns `SubscriptionErrorCode::RefCountOverflow` with the exact key and
leaves state unchanged. `SubscriptionError` exposes `code()` and `key()` and
implements `Display` and `Error`.

`Runtime::unsubscribe(&SubscriptionKey)` applies the same unknown, stale, and
terminal observer validation before touching coordination. For a current
registration it returns `Decremented` while the count remains nonzero, `Removed`
when the final reference is removed, and `NotFound` for an absent exact key.
`NotFound` is an idempotent replay outcome and
does not mutate any other key sharing the target, observer, scope, or priority.
Every `SubscriptionChange` exposes `key()` and the resulting `ref_count()` (`0`
for `Removed` and `NotFound`).

Queries are:

```rust
CoordinationState::ref_count(key: &SubscriptionKey) -> usize
CoordinationState::aggregate(target: &SubscriptionTarget) -> Option<SubscriptionAggregate>
CoordinationState::resource_observer_count(id: &ResourceId) -> usize
```

Coordination mutation primitives are crate-private. Its private
`remove_observer(surface_ref)` removes every full key for that exact registration and all of
their replay refcounts in one infallible operation. Runtime invokes it when a
registered surface changes generation, first becomes terminal, or is removed.
Aggregates and resource observer counts reflect the cleanup immediately.
Re-registering the same `SurfaceId` starts with no subscriptions; old keys are not
restored, and attempts to subscribe or unsubscribe with the prior `SurfaceRef`
return `StaleObserver` without touching replacement keys.

`aggregate` reports the number of distinct active keys, deduplicated observer registrations
and scopes in deterministic order, and the
highest active priority under `Low < Normal < High`. It returns `None` when no key
for the target remains. `resource_observer_count` is the aggregate's unique
observer-registration count for `SubscriptionTarget::resource(id)`, so replayed
references and multiple scopes from one registration do not inflate resource
observation.
`SubscriptionAggregate` exposes `target()`, `active_keys()`, `observers()`,
`scopes()`, and `highest_priority()` views.

## S10. Service Mailbox

Unsupported mailbox policies are removed. `MailboxOverflow` is:

```rust
RejectNewest,
DropOldest,
```

`ServiceMailbox::push(message) -> MailboxPushOutcome<T>`:

```rust
pub enum MailboxPushOutcome<T> {
    Accepted,
    RejectedNewest(T),
    DroppedOldest { dropped: T },
}
```

Zero capacity with `RejectNewest` always returns `RejectedNewest(message)`.
Zero capacity with `DropOldest` also rejects the newest because there is no
storage slot. Overflow observation increments `overflow_count` for both
rejection and drop outcomes when enabled.

## S11. Manifest And Snapshot Validation

Command, event, and payload type names are fallible:

```rust
CommandName::try_new(value) -> Result<CommandName, NameError>
EventName::try_new(value) -> Result<EventName, NameError>
PayloadTypeName::try_new(value) -> Result<PayloadTypeName, NameError>
```

Names reject empty or whitespace-only values and values containing ASCII control
characters. Payload type names use the same validation rule and are semantic
runtime identifiers, not Rust type reflection. Descriptor constructors use
validated names:

```rust
pub struct PayloadTypeName(String);
CommandDescriptor::try_new(name, payload_type) -> Result<CommandDescriptor, NameError>
EventDescriptor::try_new(name, payload_type) -> Result<EventDescriptor, NameError>
TaskDescriptor::try_new(name, input_type) -> Result<TaskDescriptor, NameError>
ResourceDescriptor::try_new(id, value_type) -> Result<ResourceDescriptor, NameError>
```

Existing infallible constructors for unchecked string payload descriptors are
removed or made private to tests. `NameError` reports which field failed:
`command.name`, `command.payload_type`, `event.name`, `event.payload_type`,
`task.input_type`, or `resource.value_type`.

`AppManifest` is the authored builder. Validation consumes it so no caller can
mistake the unchecked builder for an app:

```rust
AppManifest::validate(self) -> Result<ValidatedAppManifest, ManifestValidationError>
App::try_new(manifest: AppManifest) -> Result<Self, ManifestValidationError>
App::manifest(&self) -> &ValidatedAppManifest
ValidatedAppManifest::app(&self) -> &AppDescriptor
ValidatedAppManifest::root(&self, id: &RootId) -> Option<&RootDescriptor>
```

`ValidatedAppManifest` has no public unchecked constructor. It owns the validated
descriptors in deterministic ID/name indexes. `App` owns exactly one
`ValidatedAppManifest`; `App::try_new` delegates to `validate` and stores the
success. `App::descriptor()` is a convenience view of `manifest().app()`.
The validated manifest exposes immutable lookup and deterministic iteration for
commands, events, tasks, resources, windows, roots, and startup windows; it has no
builder or mutation methods.

`ManifestValidationError` contains one or more `ManifestValidationIssue` values:

```rust
#[non_exhaustive]
pub enum ManifestValidationErrorCode {
    DuplicateCommand,
    DuplicateEvent,
    DuplicateTask,
    DuplicateResource,
    DuplicateWindow,
    DuplicateRoot,
    DuplicateRootSnapshotBinding,
    MissingCommand,
    MissingEvent,
    CommandPayloadTypeMismatch,
    EventPayloadTypeMismatch,
    UnknownStartupWindow,
    UnknownStartupRoot,
    DisallowedStartupRoot,
    MissingStartupRoot,
}
pub struct ManifestValidationIssue {
    code: ManifestValidationErrorCode,
    root_id: Option<RootId>,
    window_id: Option<WindowDescriptorId>,
    command_name: Option<CommandName>,
    event_name: Option<EventName>,
    snapshot_binding_id: Option<SnapshotBindingId>,
    expected_payload_type: Option<PayloadTypeName>,
    actual_payload_type: Option<PayloadTypeName>,
}
```

`ManifestValidationError::issues()` returns all issues in deterministic order by
descriptor kind and ID/name. `ManifestValidationIssue` exposes accessors for each
field in the pseudo-code above.

Manifest validation rejects:

- duplicate command, event, task, resource, window, and root IDs;
- a root-required command whose `CommandName` is not declared in
  `AppManifest::commands`;
- a root-emitted event whose `EventName` is not declared in
  `AppManifest::events`;
- a root-required command descriptor whose payload type conflicts with the
  manifest command descriptor of the same name;
- a root-emitted event descriptor whose payload type conflicts with the manifest
  event descriptor of the same name;
- startup windows that reference unknown windows or roots;
- startup root/window pairs not allowed by `WindowDescriptor::allowed_roots`
  when the allowed list is non-empty;
- manifests with windows but no startup root;
- root snapshot bindings that refer to duplicate binding IDs inside a root.

Every rejection maps to the matching `ManifestValidationErrorCode`. Missing and
mismatched root command/event issues report the root ID and descriptor name;
payload mismatches also report both expected and actual payload type names.
Startup issues report the startup window/root pair involved. Duplicate binding
issues report the root ID when the duplicate is scoped to a root.

Snapshots expose validated bindings and opaque serialized values:

```rust
pub struct SnapshotBindingId(String)
pub struct SnapshotSourceType(String)
pub struct SnapshotBinding { id: SnapshotBindingId, source_type: SnapshotSourceType }
pub struct SnapshotValue { serialized_text: Box<str> }
pub struct SnapshotEntry { binding: SnapshotBinding, value: SnapshotValue }
pub struct AppSnapshot {
    root_id: RootId,
    version: StateVersion,
    declarations: BTreeMap<SnapshotBindingId, SnapshotSourceType>,
    entries: Vec<SnapshotEntry>,
}
#[non_exhaustive]
pub enum SnapshotErrorCode {
    EmptyBindingId,
    InvalidBindingId,
    EmptySourceType,
    InvalidSourceType,
    EmptyValue,
    InvalidValue,
    UnknownRoot,
    UndeclaredBinding,
    SourceTypeMismatch,
    DuplicateBinding,
}
pub struct SnapshotError {
    code: SnapshotErrorCode,
    field: &'static str,
    root_id: Option<RootId>,
    binding_id: Option<SnapshotBindingId>,
    expected_source_type: Option<SnapshotSourceType>,
    actual_source_type: Option<SnapshotSourceType>,
    message: String,
}
```

Construction is explicit:

```rust
SnapshotBindingId::try_new(value) -> Result<Self, SnapshotError>
SnapshotSourceType::try_new(value) -> Result<Self, SnapshotError>
SnapshotBinding::new(id, source_type) -> Self
SnapshotValue::try_new(serialized_text) -> Result<Self, SnapshotError>
SnapshotEntry::new(binding, value) -> Self
ValidatedAppManifest::new_snapshot(root_id, version) -> Result<AppSnapshot, SnapshotError>
App::new_snapshot(root_id, version) -> Result<AppSnapshot, SnapshotError>
AppSnapshot::add_entry(entry) -> Result<(), SnapshotError>
```

Binding IDs and source types reject empty or whitespace-only values with
`EmptyBindingId` or `EmptySourceType`, and reject ASCII control characters with
`InvalidBindingId` or `InvalidSourceType`. They preserve accepted text exactly.
Snapshot values are opaque UTF-8 serialized payloads: runtime does not parse a
codec or validate a source schema. `SnapshotValue::try_new` rejects an empty value
with `EmptyValue` and an embedded NUL with `InvalidValue`, then preserves accepted
text exactly. The root adapter or payload producer owns codec and schema
validation before constructing the runtime value.

Snapshot construction is bound to the validated manifest. `new_snapshot` rejects
an unknown root with `UnknownRoot`; otherwise it copies that root's validated
binding ID/source-type declarations into private snapshot state. There is no
public raw `AppSnapshot::new` and no public declaration mutator. `AppSnapshot`
exposes `root_id()`, `version()`, `entries()`, and `declaration(binding_id)` views.

`AppSnapshot::add_entry` first rejects a binding ID absent from the copied root
declarations with `UndeclaredBinding`, then rejects a source type different from
the declaration with `SourceTypeMismatch`, then rejects an already-present ID with
`DuplicateBinding`. Every failure leaves entries unchanged. A type mismatch
reports both expected and actual source types. `SnapshotError` exposes `code()`,
`field()`, `root_id()`, `binding_id()`, `expected_source_type()`, and
`actual_source_type()`; every constructor identifies its rejected field as
`snapshot.root_id`, `snapshot.binding_id`, `snapshot.source_type`,
`snapshot.value`, or `snapshot.entries`. The error implements `Display` and
`std::error::Error`.

## S12. Public Errors, Docs, And Unsafe Gate

The crate root has:

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]
```

By the final cycle, every public item exported by `src/lib.rs` has rustdoc or is
made private. Public state machines document their transition semantics.

Representative public rustdoc examples are compile-checked contracts, not prose
sketches:

- `Runtime`/surface docs show runtime-owned ID and geometry construction,
  registration returning `SurfaceRef`, transition to `Ready`, a borrow-bound
  render-state read, and acknowledgement;
- reducer docs show `ReducerCommit::new` with an effect and provenance attached to
  both `ReducerResult::unchanged` and `changed`;
- manifest/snapshot docs show a validated app constructing a root-bound snapshot
  and adding a declared value;
- queue error docs show zero-capacity rejection and recovery of the exact input
  with `into_rejected`;
- surface error docs show a stale pre-re-registration `SurfaceRef` rejected from
  update/targeting without changing the replacement;
- snapshot error docs show undeclared and source-type-mismatched entries matched
  through non-exhaustive error codes.

Examples use only public front doors and no production-visible test helper.
`cargo test --offline --locked -p surgeist-runtime --doc` must compile and run all
of them; the ordinary package test gate remains required as well. README includes
one concise end-to-end ownership example that emits abstract task/resource/service
intents and states that root performs concrete adapter lowering.

These public errors implement `Display` and `std::error::Error`:

- `RuntimeInputError`
- `RuntimeQueueError`
- `RuntimeDrainError`
- `AppProxyError`
- `WakeError`
- `SurfaceError`
- `ReducerFailure`
- `ResourceStateError`
- `SubscriptionError`
- `ManifestValidationError`
- `SnapshotError`
- `NameError`
- `CorrelationError`
- `ProvenanceError`
- `VersionError`

All public error structs keep fields private and expose stable semantic accessors.
Every public error-code enum and every public enum whose variants are error
semantics is annotated `#[non_exhaustive]`; this includes
`AppProxyErrorCode`, `RuntimeQueueErrorCode`, `RuntimeDrainErrorCode`,
`SurfaceErrorCode`, `ResourceStateErrorCode`, `SubscriptionErrorCode`,
`ManifestValidationErrorCode`, `SnapshotErrorCode`, `ProvenanceErrorCode`, and
the `CorrelationError` and `VersionError` enums. Downstream callers must include a
wildcard arm, allowing the in-progress crate to add rejection cases without
forcing exhaustive-match breakage. Generic `RuntimeQueueError<T>` and
`AppProxyError<Input>` implement
`Display` without inspecting the rejected value and implement `Error` whenever
the rejected payload satisfies `Debug`; `into_rejected` remains available without
that bound.

MSRV follows root authority. Root `Cargo.toml` at
`a32d078bbc7b841486fcf010a1fef0c8844e5119` declares Rust `1.89`; this leaf adds
`rust-version = "1.89"` beside edition 2024 and does not raise it. Removing sibling
dependencies adds no MSRV exposure. Implementation and review reject standard
library or language APIs stabilized after 1.89.

The configured repositories provide no dedicated exact-MSRV command. Leaf
verification therefore requires
`cargo metadata --offline --locked --no-deps --format-version 1` to report
`rust_version: "1.89"`, the complete configured
Cargo gate below, and task/holistic source review against the 1.89 API contract.
An exact 1.89 compiler check is run only when that toolchain is already present in
the execution environment; it is not acquired under this initiative. The crate
candidate handoff records this policy so root integration can apply its own
configured workspace environment without changing the leaf MSRV.

`README.md` baseline checks include:

```sh
cargo check -p surgeist-runtime
cargo test -p surgeist-runtime
cargo clippy -p surgeist-runtime --all-targets -- -F unsafe-code -D warnings
cargo fmt --check
```

## S13. State Version And Generation Overflow

`StateVersion`, `SurfaceGeneration`, `SurfaceInvalidationGeneration`, and
`ResourceGeneration` use one checked increment helper:

```rust
#[non_exhaustive]
pub enum VersionError { Overflow }
pub(crate) trait CheckedNext { fn checked_next(self) -> Result<Self, VersionError> where Self: Sized; }
```

Runtime state changes, surface root replacement, surface invalidation recording,
and resource state transitions use `checked_next`. At `u64::MAX`, the operation
returns `VersionError::Overflow` through the owning typed error and is
failure-atomic:

- surface generation or invalidation exhaustion returns
  `SurfaceErrorCode::VersionOverflow`, exposes `VersionError` as the error source,
  and leaves root, generation, lifecycle, focus/hover, viewport, render state, and
  invalidations unchanged;
- resource operation-ID or version exhaustion returns
  `ResourceStateErrorCode::VersionOverflow` under the S8 rules;
- runtime keeps ownership of the popped input while the reducer borrows it. Before
  a `Changed` result commits, runtime preflights the next `StateVersion` and every
  automatic surface invalidation increment required by S4. State exhaustion uses
  `StateVersionOverflow` with no surface; surface exhaustion uses
  `SurfaceInvalidationOverflow` with the first affected `SurfaceRef` in
  deterministic registry order;
- either runtime overflow discards the proposed state and its entire
  `ReducerCommit`, pushes the original input back to the front of the same lane,
  restores `next_drain_lane` to that lane, and stops the drain. No surface is
  invalidated, the input is not counted as drained, no effect outcome or
  diagnostic is produced for it, and all remaining counts include it;
- the error carries the failing lane, trigger provenance, `VersionError` source,
  and a partial report containing only inputs that committed before the failure.
  Those earlier commits remain valid; the failing input itself has no partial
  result.

No code uses unchecked `value + 1` for these versioned identities or operation
counters.

## S14. Required Test Outline

Each implementation cycle adds or updates focused tests with these names or
equivalent names documented in the cycle evidence:

- `runtime_has_no_sibling_dependencies_or_exports`
- `testing_fixtures_are_not_unconditional_public_api`
- `app_loop_has_no_host_handler_or_native_loop`
- `crate_forbids_unsafe_code`
- `manifest_declares_root_msrv_1_89`
- `correlation_zero_is_unconstructable_and_absent_is_explicit`
- `provenance_constructors_default_current_and_parent_correlation_absent`
- `provenance_current_and_parent_correlations_set_and_clear_independently`
- `provenance_surface_attachment_is_origin_specific`
- `provenance_surface_attachment_preserves_surface_generation`
- `effect_outcomes_use_commit_override_or_input_provenance`
- `surface_uses_runtime_window_identity_and_geometry`
- `runtime_owned_surface_primitives_round_trip_for_root_adapters`
- `surface_point_drives_checked_scroll_offset_mutation`
- `retained_bridge_is_not_runtime_public_api`
- `reducer_failure_cannot_commit_state_or_effects`
- `reducer_success_commits_construct_effects_and_provenance_explicitly`
- `runtime_reports_applied_forwarded_and_rejected_effects`
- `unsupported_effect_kinds_are_absent`
- `proxy_wake_failure_rolls_back_delivery`
- `wake_bridge_signal_is_deferred_and_non_reentrant`
- `proxy_concurrent_accepted_sends_remain_drainable`
- `proxy_partial_drain_resignals_remaining_work`
- `proxy_partial_drain_reports_continuation_wake_failure`
- `runtime_queue_overflow_returns_exact_rejected_input`
- `runtime_and_proxy_queue_policy_defaults_and_zero_capacity_are_exact`
- `runtime_custom_queue_policy_and_budget_builders_are_observable`
- `runtime_drain_reports_all_pending_lanes`
- `runtime_scheduler_does_not_starve_service_lane`
- `runtime_scheduler_rotates_starting_lane_across_small_budget_drains`
- `runtime_surface_registry_rejects_duplicate_unknown_removed_and_stale_ids`
- `runtime_surface_updates_are_failure_atomic`
- `stale_surface_refs_cannot_update_or_remove_replacements`
- `terminal_and_removed_surfaces_drop_all_observer_subscriptions`
- `root_replacement_drops_old_generation_subscriptions`
- `reregistered_surface_ids_do_not_restore_old_subscriptions`
- `stale_surface_refs_cannot_mutate_replacement_subscriptions`
- `surface_registration_rejects_duplicate_and_unknown_ids`
- `surface_lifecycle_rejects_invalid_terminal_mutations`
- `render_ack_rejects_stale_versions_and_consumes_invalidations`
- `render_ack_leaves_invalidations_added_after_frame_begin`
- `render_ack_retains_captured_snapshot_newer_than_frame_state`
- `render_state_view_matches_frame_version_and_runtime_state`
- `changed_state_invalidates_nonterminal_and_redraws_renderable_surfaces`
- `superseded_frame_ack_reports_remaining_redraw_work`
- `render_ack_reports_consumed_and_remaining_invalidation_counts`
- `surface_render_begin_and_ack_reject_ineligible_lifecycles_atomically`
- `surface_element_validation_rejects_stale_unknown_and_ineligible_targets`
- `surface_element_validation_rejects_surface_mismatch`
- `runtime_element_and_route_validation_reject_inactive_and_terminal_surfaces`
- `runtime_element_and_route_validation_precedence_is_unknown_stale_lifecycle_element`
- `focus_and_hover_set_clear_and_duplicate_are_deterministic`
- `focus_and_hover_reject_stale_unknown_and_terminal_references_atomically`
- `focus_hover_and_scroll_changes_record_surface_invalidation_and_redraw_outcome`
- `surface_route_requires_one_ordered_target`
- `surface_route_validation_checks_every_registered_phase`
- `resource_state_rejects_invalid_overlap_and_stale_operations`
- `resource_cancel_rejects_mismatched_and_already_cancelled_operations`
- `resource_state_issues_generation_qualified_operation_tokens`
- `resource_transition_matrix_preserves_and_clears_observable_fields`
- `resource_operation_and_version_overflow_are_atomic`
- `subscriptions_preserve_scope_observer_priority_and_refcounts`
- `subscription_replay_and_missing_unsubscribe_report_exact_changes`
- `subscription_aggregate_deduplicates_observers_and_orders_scopes`
- `subscription_refcount_overflow_is_atomic`
- `service_mailbox_reports_reject_and_drop_outcomes`
- `manifest_validation_rejects_duplicates_and_dangling_startup`
- `manifest_validation_rejects_missing_root_commands_and_events`
- `manifest_validation_rejects_root_payload_type_mismatches`
- `validated_app_constructs_a_root_bound_snapshot`
- `descriptor_payload_type_names_are_validated`
- `snapshot_binding_and_value_constructors_reject_invalid_text`
- `snapshot_accepts_a_valid_binding_and_value`
- `snapshot_rejects_undeclared_and_mismatched_root_bindings_atomically`
- `snapshot_entries_reject_duplicate_bindings_atomically`
- `public_errors_implement_display_and_error`
- `public_rustdoc_success_and_error_examples_compile`
- `state_version_overflow_is_checked_and_atomic`
- `runtime_state_version_overflow_requeues_input_and_returns_partial_report`
- `changed_state_invalidation_overflow_requeues_without_partial_surface_updates`

## S15. Initiative Acceptance

The desired state is accepted when every finding in
`plans/2026-07-11-repository-review.md` maps to implemented crate behavior,
focused tests, and public documentation where applicable:

- no runtime manifest or public source path exposes concrete sibling crate types;
- reducer, surface, resource, service, subscription, manifest, snapshot,
  provenance, queue, proxy, effect, version, and error semantics match sections
  S1 through S13;
- all required tests in S14 or their documented equivalents pass;
- leaf manifest metadata reports `rust-version = "1.89"` and changed source uses
  no post-1.89 language or standard-library contract;
- README and rustdocs contain the S12 representative success/error examples, and
  the rustdoc examples pass the dedicated doctest command;
- root-owned facade and adapter updates are reported as candidate handoff rather
  than implemented in this leaf, including a required root integration test that
  each concrete `WakeBridge` schedules a future turn without synchronous proxy
  drain re-entry.
