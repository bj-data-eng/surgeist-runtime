VERDICT: NOT CLEAN

SCOPE: Repository-wide completeness, correctness, and quality review of the `surgeist-runtime` leaf at exact HEAD `0bd492f1e17d1332d6ce67a77f10ef75d7599c7b`. Product code remained read-only. This document is review evidence, not an active specification, implementation sequence, or cycle plan.

EVIDENCE CHECKED: `AGENTS.md`, `Cargo.toml`, `README.md`, `.gitignore`, `LICENSE`, all tracked `src/*.rs`, all 47 unit tests, public-surface and call-site scans, recent relevant Git history, and the directly called retained-model validation boundary. Local `main`, `origin/main`, and HEAD all resolved to the reviewed SHA, with a clean worktree before the report was written. `CARGO_NET_OFFLINE=true cargo check --offline -p surgeist-runtime`, `CARGO_NET_OFFLINE=true cargo test --offline -p surgeist-runtime`, `CARGO_NET_OFFLINE=true cargo clippy --offline -p surgeist-runtime --all-targets -- -F unsafe-code -D warnings`, `cargo fmt --check`, offline metadata resolution, and offline package listing passed. Tests reported 47 passed unit tests and zero doctests. The owned-Rust unsafe scan found no executable unsafe construct, and scans found no broad lint suppression or unresolved source placeholder.

FINDINGS:

[Important] Root-owned cross-crate adapters are exposed from the leaf
Location: `AGENTS.md:50-56`; `README.md:10-13`; `Cargo.toml:14-16`; `src/bridge.rs:3-12`; `src/loop_.rs:21-24`; `src/surface.rs:1-4,107-121`; `src/lib.rs:31,56,79-80`
Evidence: Committed policy assigns Surgeist-to-Surgeist integration and adapters to root and excludes retained-tree and host implementation details from this crate. The manifest nevertheless depends directly on `surgeist-retained` and `surgeist-window`, while the public surface exposes retained commands/models/IDs and a concrete `surgeist_window::Loop`.
Impact: The leaf contract crosses its declared ownership boundary, couples callers to sibling models, and forces root either to accept leaf-owned integration or duplicate it.
Required remediation: Use a cross-repository design cycle to move retained-command decoding and concrete window/retained composition to root-owned adapters, leaving runtime-owned orchestration contracts and opaque semantic identities in this leaf.

[Important] Advertised effects have no successful runtime or adapter path
Location: `src/effect.rs:43-57,74-226`; `src/runtime.rs:352-370,374-467,513-567`; `src/tests.rs:802-918`; `src/testing.rs:54-97`
Evidence: `SCHEDULE_TIMER` and `WINDOW_COMMAND` are public effect kinds with no payload variants or constructors. Persist, load/invalidate-resource, and start/stop/call-service effects have public constructors but Runtime always converts them to `EFFECT_FAILED`; only task intents and redraw IDs escape in `RuntimeDrainReport`. Failed effects still increment `executed_effects`, and their diagnostics always use system provenance rather than the triggering input. The timer test drives `FakeClock` directly instead of Runtime.
Impact: Callers can emit apparently supported operations that are guaranteed to fail, hosts cannot lower them, reports misstate failure as execution, and causal provenance is lost.
Required remediation: Remove unsupported public kinds or expose typed forwarded/applied/failed outcomes for every supported effect, carrying effective input provenance and a clear adapter handoff; test every effect through Runtime and define the public enum's evolution policy.

[Important] Recoverable reducer failure can commit unversioned partial state
Location: `src/reducer.rs:3-38,41-77`; `src/runtime.rs:352-367`; `src/tests.rs:386-410,659-684`
Evidence: A reducer receives `&mut State` before choosing its `ReducerResult`. It can mutate state and then return `recoverable_failure`; Runtime records a diagnostic and returns without rolling back or advancing `StateVersion`. The result builder also permits effects on a failure result, which Runtime silently skips. The only failure test deliberately leaves state untouched.
Impact: Observable state can change while snapshots, invalidation, and version-based consumers retain the old revision, so recovery is not failure-atomic.
Required remediation: Stage reducer changes and commit them only with a successful typed outcome, or otherwise guarantee rollback and version coherence. Represent semantic failure as a disjoint typed result and add a mutate-then-fail regression.

[Important] Proxy wake bookkeeping permits ambiguous delivery and stranded work
Location: `src/proxy.rs:45-77`; `src/loop_.rs:72-81`; `src/tests.rs:325-373`
Evidence: Enqueue stores the input and sets `wake_pending` before calling `wake`. Wake failure returns `Err` but leaves the input queued; a concurrent second sender can already have returned `Ok` before the first wake fails and clears the flag. A partial `drain_pending(limit)` leaves `wake_pending` true when backlog remains, and neither the proxy nor `AppLoop::drain_proxy` schedules a successor wake. Tests cover only a full drain and the single failure code.
Impact: Retrying a failed send can duplicate an already accepted input, while successful concurrent sends or a budget-limited backlog can remain indefinitely unprocessed.
Required remediation: Make acceptance and wake ownership one race-safe state transition: roll back the exact failed enqueue or retain it with guaranteed retry, and atomically re-arm or resignal whenever a drain leaves work. Add deterministic wake-failure, concurrency, and partial-drain tests.

[Important] Lane scheduling can starve service work and hide pending inputs
Location: `src/runtime.rs:107-151,291-332,471-559`; `src/testing.rs:744-756`
Evidence: Runtime drains UI, then task, then service inputs against one global budget. The default task limit equals the entire input budget, UI has no queue bound, and earlier lanes can consume every turn. `RuntimeDrainReport` exposes only remaining task inputs. With 33 service-only inputs, the default service limit drains 32, but `PrototypeApp::drain_all` observes an empty proxy and zero remaining tasks and exits with one service input still queued.
Impact: `drain_all` does not drain all, hosts lack a reliable continuation signal, sustained UI/task traffic can indefinitely delay service progress/cancellation/shutdown, and UI input can grow without backpressure.
Required remediation: Define starvation-free per-lane scheduling and backpressure, expose pending counts or a continuation signal for every lane, and make drain helpers stop only when every lane is empty. Test over-budget service input and sustained mixed-lane traffic.

[Important] Registered surface lifecycle is neither closed nor runtime-owned
Location: `src/runtime.rs:154-210,374-390`; `src/surface.rs:107-121,209-258,268-287`; `src/tests.rs:183-303`
Evidence: Runtime consumes a surface into a private map but exposes only insertion: no lookup, lifecycle update, duplicate rejection, or removal. Duplicate IDs silently replace state. `UiSurface` accepts every transition from every nonterminal state, including `Closing -> Ready`, while root/focus/render mutations remain possible after destruction. Redraw-all/window enumerates every registered surface regardless of lifecycle, and redraw-by-surface forwards an unknown ID unchanged.
Impact: Registered surfaces cannot coherently follow native lifecycle, closed/destroyed or nonexistent surfaces can receive redraw work, terminal objects remain mutable, and stale registry entries cannot be retired.
Required remediation: Give Runtime typed register/update/remove operations with duplicate and stale-ID handling, enforce an explicit transition table and terminal mutation guards, and validate redraw targets against active surfaces. Add duplicate, closing, destroyed, removed, and unknown-target tests.

[Important] Render acknowledgements regress and never consume invalidation state
Location: `src/surface.rs:107-121,179-186,213-220,251-266`
Evidence: Invalidations can only be appended and inspected; there is no clear, drain, coalescing, or acknowledgement path. `mark_rendered` unconditionally overwrites the version, so acknowledging version 10 and then a delayed version 9 regresses `last_rendered_state_version`. No acknowledgement retires the invalidations covered by a frame.
Impact: Long-lived surfaces never return to a clean state, the invalidation vector grows without bound, and out-of-order frame completion can move scheduling state backward.
Required remediation: Provide an atomic monotonic render acknowledgement that rejects stale completions and consumes or coalesces the invalidations covered by the accepted frame. Test repeated invalidation and out-of-order completion.

[Important] Invalid authored retained roots panic at a public framework boundary
Location: `src/surface.rs:12-35,123-127,251-258`
Evidence: `WindowRoot::with_element` accepts any `surgeist_retained::Element` infallibly. The directly called `retained::Model::new` validation can reject such an element, but both `UiSurface::new` and `replace_root` reach it through `expect`. A retained tree with duplicate sibling keys is constructible before model validation and triggers this path.
Impact: Invalid caller-authored input crashes the process instead of returning a semantic construction or replacement error.
Required remediation: Validate at `WindowRoot` construction or make surface construction/replacement fallible, preserve the retained validation error, and add invalid-root and failure-atomic replacement tests.

[Important] Retained identity validity is discarded at mutation and bridge boundaries
Location: `src/surface.rs:59-103,251-273`; `src/bridge.rs:32-65,74-109,164-176`; `src/diagnostic.rs:22-25`
Evidence: A retained root exposes a surface generation, but focus and hover setters accept only raw retained IDs, so delayed work can restore an ID from a replaced root. `BridgeContext` carries no generation. If a command target/phase is absent from the supplied route, bridge decoding still succeeds and merely omits sequence provenance. The existing stale-element and ineligible-target diagnostic codes are never emitted.
Impact: Stale or mismatched retained work can mutate the current surface and be attributed to an ineligible route instead of being rejected.
Required remediation: Carry generation-qualified retained identities through surface mutation and bridge context, validate current generation/model membership and route eligibility before decoding, and return typed stale/ineligible diagnostics. Add stale-root and route-mismatch tests.

[Important] The resource state machine is neither closed nor generation-safe
Location: `src/resource.rs:3-101,148-188`; `src/tests.rs:920-956`
Evidence: Public statuses `Running`, `Cancelling`, `Cancelled`, and `Stale` are never assigned. Starting, refreshing, ready, stale, and failed transitions are callable from any current state and return no rejected-transition error. Start/refresh creates no operation attempt identity, so completion from operation A can overwrite the state after a newer operation B has started. Tests cover only one linear success path and one failure path.
Impact: Downstream exhaustive matches include unreachable states, contradictory combinations and illegal transitions are constructible, and stale resource work can overwrite newer requested state.
Required remediation: Define and enforce a finite typed transition table, remove unsupported statuses or implement their semantics, and issue an operation generation/attempt that completion and failure must match. Add invalid-transition, cancellation, and overlapping-operation tests.

[Important] Subscription semantics are discarded and observer ownership is duplicated
Location: `src/coord.rs:208-317`; `src/resource.rs:91-100`; `src/tests.rs:986-1000`
Evidence: `Subscription` identity includes target, scope, observer, and priority, but `CoordinationState` stores only a `HashSet<SurfaceId>` per target. Same-target/same-observer subscriptions with different scopes or priorities collapse; unsubscribing either removes the observer entirely, and priority is discarded. `ResourceState` separately maintains a manually mutated observer count.
Impact: Live registrations can be reported unobserved, scheduling cannot honor declared scope/priority, and the two observer representations can disagree.
Required remediation: Assign observation state one owner and store full subscription identity or explicit reference counts with defined priority aggregation. Derive resource observation from that owner and test duplicate, replay, scope, priority, and unsubscribe ordering.

[Important] Service mailbox policies advertise unimplemented delivery guarantees
Location: `src/service.rs:33-84,204-240`; `src/tests.rs:836-870`
Evidence: Public `DropNewest` and `CoalesceByKey` policies cannot be selected through `MailboxPolicy`. `RejectNewest`, `DropNewest`, and `CoalesceByKey` share the same no-op overflow arm, there is no key contract for coalescing, and `push` returns `()` so default rejection is indistinguishable from acceptance. Tests cover only drop-oldest.
Impact: Service messages are silently lost, callers cannot retry, and public policy names promise materially different behavior that does not exist.
Required remediation: Provide selectable policies and a typed push outcome that can return a rejected message; implement a real coalescing-key contract or remove unsupported variants. Test every policy, including zero capacity.

[Important] Manifest and snapshot APIs cannot construct a validated coherent app
Location: `src/descriptor.rs:23-55,72-167,215-355`; `src/snapshot.rs:26-103`; `src/command.rs:16-59`; `src/event.rs:16-59`
Evidence: `AppManifest` append methods accept duplicate IDs, dangling startup window/root references, disallowed window/root pairs, and root requirements absent from the declared command/event sets; there is no validation or semantic error. `AppSnapshot::new` always creates an empty binding vector and exposes no way to add declared bindings. Commands/events expose names while descriptor payload types are unchecked strings.
Impact: Invalid authored configurations are indistinguishable from valid ones, snapshot declarations cannot be materialized, and consumers must duplicate payload and reference validation.
Required remediation: Introduce a fallible validated manifest boundary with typed errors and referential checks, provide a valid snapshot-binding/value construction path, and keep incomplete authoring types private until connected to that boundary. Add invalid and successful manifest/snapshot tests.

[Important] App-specific test prototypes are unconditional production API
Location: `Cargo.toml:18-19`; `src/lib.rs:26,86`; `src/testing.rs:317-1073`
Evidence: `pub mod testing` is always compiled and exposes the entire 1,073-line module without an opt-in feature, including counter, thumbnail, search, progress, and JSON-RPC application fixtures. Several timer, task, resource, and service demonstrations are implemented inside these prototypes rather than through the corresponding production paths.
Impact: Fixture policy becomes release-facing semver surface, enlarges the default product API, and can be mistaken for evidence that Runtime supplies behavior that the fixtures implement themselves.
Required remediation: Keep only intentional generic harness contracts public, move application fixtures to private tests or examples, and place supported downstream test utilities behind an explicit non-default feature or support crate.

[Minor] Public defaults, errors, and state semantics are effectively undocumented
Location: `src/lib.rs:1-5,31-86`; `src/runtime.rs:86-151,471-559`; all public production modules
Evidence: A static scan counted 549 production `pub` declarations but only 11 rustdoc lines, belonging to crate-level text, `AppHandler`, and `crate_name`. The test run reported zero doctests. Queue capacities, budgets, overflow, lifecycle, failure atomicity, cancellation, stale handling, and public defaults are unspecified; `RuntimeInputError` also implements neither `Display` nor `std::error::Error`.
Impact: Consumers cannot determine valid behavior or error handling from the API, and compatibility-sensitive semantics remain unconstrained.
Required remediation: Document the intended public surface and defaults, add focused usage/error examples, implement standard behavior for public errors, and enable a missing-docs gate for the supported surface.

[Minor] Provenance construction permits silent and ambiguous states
Location: `src/provenance.rs:73-132,186-207`; `src/ids.rs:20-36`
Evidence: `InputProvenance::with_surface` silently does nothing for every origin except task. Every provenance constructor assigns correlation ID zero, while `CorrelationId::from_u64(0)` is also an ordinary explicit identity, so absent and real zero correlation are indistinguishable.
Impact: Callers can believe provenance was enriched when it was not, and causal tracing cannot reliably distinguish missing correlation from an actual identity.
Required remediation: Make surface attachment origin-specific or fallible, and model absent correlation explicitly or adopt a validated/generated nonzero correlation identity with documented zero semantics.

[Minor] State-version overflow has build-profile-dependent behavior
Location: `src/snapshot.rs:10-23`; `src/runtime.rs:365-367`
Evidence: `StateVersion::next` performs unchecked `u64 + 1`, and Runtime duplicates that arithmetic. At `u64::MAX` this panics when overflow checks are enabled and wraps to zero otherwise; no boundary policy or test exists.
Impact: The public revision API can panic or violate monotonicity depending on build profile.
Required remediation: Define one checked, saturating, or explicitly wrapping exhaustion policy, use it consistently, and test the maximum boundary.

[Minor] The unsafe prohibition is not self-enforced by the source or README gate
Location: `src/lib.rs:1`; `README.md:15-23`; `AGENTS.md:64-75`
Evidence: Current owned Rust contains no executable unsafe, but the crate root has no `#![forbid(unsafe_code)]`. README's handoff list omits `cargo check` and runs Clippy without `-F unsafe-code`, while the authoritative repository inventory includes both.
Impact: A contributor following the public handoff instructions can produce incomplete evidence, and a future unsafe addition can pass the README-listed commands despite the absolute repository prohibition.
Required remediation: Add crate-level compiler enforcement where the committed policy permits it, and make README's handoff commands match or explicitly defer to the authoritative inventory.
