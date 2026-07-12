# Runtime Review Findings Resolution Sequence

Specification: `plans/specs/runtime-review-findings-resolution.md`

Reviewed specification revision (normalized SHA-256):
`b7876239cca9dbc12ccac157a897acb9284e9ee726c22c06e672f05277ee4c40`

Owning design repository: `/Users/codex/Development/surgeist-runtime`

This sequence orders leaf implementation only. Root `surgeist` owns facade
adaptation, cross-crate lowering, generated API artifacts, gitlink promotion,
and root integration evidence.

## C01 - Leaf Boundary And Model Foundations

Owning repository: `/Users/codex/Development/surgeist-runtime`

Bounded outcome: remove concrete retained/window/task integration and public
fixtures; establish runtime-owned identities, geometry, routes, and
`UiSurface`-local state; reduce `AppLoop` to deterministic orchestration; and
declare the leaf unsafe and MSRV policy.

Specification sections: S0; S1 dependency exclusions, bridge/AppHandler removal,
AppLoop and test-fixture clauses, and the `ids.rs` and `surface.rs` export rows;
S2 except registry-dependent `Runtime` methods; S12 unsafe and MSRV clauses; S13
`UiSurface`-local generation/invalidation overflow; S14 C01 acceptance coverage.

Prerequisites: the cited specification revision is clean; leaf `main` is the
reviewed baseline; root MSRV evidence remains Rust `1.89`.

Entry state: sibling crates still supply concrete UI types and adapters, fixtures
are production-visible, and the runtime-owned local surface model is incomplete.

Exit evidence: forbidden sibling dependencies and adapter surfaces are absent;
C01-owned exports are exact; local construction, route/element validation,
mutation, root replacement, invalidation, and overflow are failure-atomic; the
MSRV and unsafe declarations are present; the candidate is remotely readable.

Handoff: return the breaking facade delta and candidate SHA to root; the verified
SHA becomes C02's base without waiting for root adaptation.

## C02 - Resources, Coordination, And Service Mailboxes

Owning repository: `/Users/codex/Development/surgeist-runtime`

Bounded outcome: close resource operations, establish complete subscription
keys/refcounts/aggregates in coordination state, and expose only the two specified
service-mailbox policies with typed outcomes.

Specification sections: S1 `coord.rs`, `resource.rs`, and `service.rs` export
rows; S8; S9 coordination key, mutation, refcount, aggregate, and query clauses,
excluding registry observer validation and lifecycle cleanup assigned to C03;
S10; S13 resource generation/operation overflow; S14 C02 acceptance coverage.

Prerequisites: C01's IDs, `SurfaceRef`, public boundary, and checked-version
primitives are published and remotely verified.

Entry state: resource transitions are open-ended, subscriptions lack the complete
identity/aggregation model, and mailbox overflow is not a closed two-policy API.

Exit evidence: resource transitions are operation- and generation-safe;
coordination preserves exact keys, refcounts, aggregate priority, and idempotent
change outcomes; mailbox pushes expose exact rejection or eviction results; the
candidate is remotely readable.

Handoff: return resource, coordination, and service API deltas plus the candidate
SHA to root; the verified SHA becomes C03's base.

## C03 - Surface Lifecycle And Runtime Registry

Owning repository: `/Users/codex/Development/surgeist-runtime`

Bounded outcome: make `Runtime` own generation-qualified surface registration,
lifecycle, interaction state, invalidation, render frames/acknowledgements,
registry-staged root replacement, tombstones, and subscription cleanup.

Specification sections: S2 registry-dependent `Runtime` validation/mutation
clauses; S3 except its final Runtime redraw-target paragraph assigned to C04; S3A
except its final effect/redraw lookup paragraph assigned to C04; S9 registry
observer validation and lifecycle/replacement/removal cleanup clauses; S13
Runtime-owned surface/invalidation overflow; S14 C03 acceptance coverage.

Prerequisites: C01's local surface model and C02's complete coordination model are
published and remotely verified.

Entry state: local surface values exist, but runtime registry authority,
lifecycle/render transitions, tombstones, and observer cleanup are incomplete.

Exit evidence: registry operations validate generation and lifecycle; stale and
terminal references are rejected; render acknowledgement is monotonic; staged
replacement/removal atomically updates tombstones and subscriptions; inactive
invalidation work becomes renderable on eligible transitions; the candidate is
remotely readable.

Handoff: return surface/registry API deltas and the candidate SHA to root; the
verified SHA becomes C04's base.

## C04 - Reducer, Effects, Provenance, And Versions

Owning repository: `/Users/codex/Development/surgeist-runtime`

Bounded outcome: make reducer commits atomic; apply diagnostics and eligible
redraws inside runtime; forward persistence, resource, task, and service work as
abstract intents; reject invalid effects; preserve explicit origin-specific
provenance/correlation; and complete checked state-version behavior.

Specification sections: S1 `diagnostic.rs`, `effect.rs`, `input.rs`/
`provenance.rs`, `reducer.rs`, and `task.rs` export rows; S3 final Runtime
redraw-target paragraph; S3A final effect/redraw lookup paragraph; S4; S5; S7;
S13 state-version and Runtime drain-preflight overflow; S14 C04 acceptance
coverage.

Prerequisites: C02's resource/service values and C03's final registry/lifecycle
contracts are published and remotely verified.

Entry state: reducer failure can coexist ambiguously with committed data, effects
lack complete dispositions, provenance attachment is ambiguous, and state
advancement is not uniformly checked.

Exit evidence: failed reductions commit nothing; changed commits advance state
and invalidate eligible surfaces atomically; diagnostics/redraws are applied;
adapter work is forwarded unchanged as typed intents; invalid effects are rejected
with effective provenance; overflow restores the triggering input and complete
pending counts; the candidate is remotely readable.

Handoff: return reducer, disposition, intent, provenance, and redraw-validation
deltas plus the candidate SHA to root; the verified SHA becomes C05's base.

## C05 - Queue, Wake, And Drain Scheduling

Owning repository: `/Users/codex/Development/surgeist-runtime`

Bounded outcome: provide failure-atomic typed queues, fair budgeted lane draining,
complete pending reports, and a deferred, non-reentrant proxy wake state machine
that cannot strand accepted work.

Specification sections: S1 `loop_.rs`/`proxy.rs` exports other than C01's
`AppLoop`, and the `runtime.rs` export row; S6; S14 C05 acceptance coverage.

Prerequisites: C04's runtime input, reducer, effect, and intent contracts are
published and remotely verified.

Entry state: queue admission can lose rejected input, lane selection can starve
work, reports omit pending state, and wake failure/partial draining can leave work
without a continuation.

Exit evidence: queue failures return exact input; bounded drains rotate fairly
and report every lane; proxy states preserve empty-to-nonempty signaling and
partial-drain continuation obligations without callback reentrancy; the candidate
is remotely readable.

Handoff: return the queue/proxy delta and candidate SHA to root, explicitly
requiring every concrete root `WakeBridge` test to prove a future host turn,
never synchronous proxy drain, and immediate handling of continuation wake
failure; the verified SHA becomes C06's base.

## C06 - Manifest, Snapshot, Documentation, And Initiative Closure

Owning repository: `/Users/codex/Development/surgeist-runtime`

Bounded outcome: enforce validated descriptors and root-bound snapshots, complete
public errors and compile-checked documentation for the final API, and close every
initiative acceptance condition.

Specification sections: S1 `command.rs`/`event.rs`, `descriptor.rs`, and
`snapshot.rs` export rows; S11; S12 public-error/docs/examples and final
unsafe/MSRV verification; S14 remaining/integrated acceptance coverage; S15.

Prerequisites: C01 through C05 are published and remotely verified, leaving a
stable integrated public contract for validation and documentation.

Entry state: names/manifests and snapshot bindings are not fully validated, and
the integrated public API lacks final error, example, and acceptance closure.

Exit evidence: `App` owns a validated manifest; snapshots reject undeclared,
mismatched, and stale roots with typed errors; public errors/docs/examples satisfy
S12; all S1 export allocations and the finite S15 checklist pass; the final
reviewed candidate is remotely readable.

Handoff: return the immutable final SHA, API/dependency delta, verification
evidence, and root-owned adapter obligations, including tests proving every
concrete `WakeBridge` defers a future host turn, never drains synchronously, and
handles continuation wake failure, for a separate root promotion cycle.
