# C02 Resources, Coordination, And Service Mailboxes Plan

Cycle ID: `C02`

Owning repository: `/Users/codex/Development/surgeist-runtime`

Status: `complete`

Cycle base: `72dc89edfc14f21e26a2d8b248f1fa07aff1824c`

Reviewed specification: `plans/specs/runtime-review-findings-resolution.md` at
normalized SHA-256
`b7876239cca9dbc12ccac157a897acb9284e9ee726c22c06e672f05277ee4c40`.

Applicable specification sections: S1 `coord.rs`, `resource.rs`, and `service.rs`
export rows; S8; S9 coordination key, mutation, refcount, aggregate, and query
clauses, excluding registry observer validation and lifecycle cleanup assigned to
C03; S10; S13 resource generation/operation overflow; S14 C02 coverage.

Reviewed sequence: `plans/sequences/runtime-review-findings-resolution.md` at
normalized SHA-256
`f98521823c097e27166e4e72933fe3047201531dce20e148849ed2f9dd45457b`, entry
`C02 - Resources, Coordination, And Service Mailboxes`.

Bounded outcome: close resource operations, establish complete
generation-qualified subscription keys/refcounts/aggregates, and expose only
reject-newest/drop-oldest service mailboxes with typed outcomes.

## Boundary

In scope: `ids.rs` support needed by S8, resource/coordination/service source,
their C02 public reexports, focused unit tests, and this cycle plan's status.

Out of scope: root or sibling writes; Runtime surface registry validation,
subscription lifecycle cleanup, or public subscribe/unsubscribe entry points;
resource effect lowering; reducer, scheduling, manifest, snapshot, and final
documentation work; dependencies, features, scripts, CI, generators, or API
artifacts. Breaking API changes are authorized; compatibility shims are not.

Each task uses a fresh clean-context worker and distinct fresh task reviewer.
Canonical task, holistic, landing, publication, and handoff rules come from
`$surgeist-agent`.

## Baseline Evidence

- C01 candidate `72dc89edfc14f21e26a2d8b248f1fa07aff1824c` is on leaf
  `origin/main`, read back, and is the clean local/tracking base.
- The base passes 50 unit tests, doctests, strict Clippy, format, metadata Rust
  `1.89`, and owned-Rust unsafe/boundary scans.
- `ResourceState` has open mutations, app `StateVersion`, observer count, and
  unsupported `Starting`/`Running`; no operation token/error model exists.
- `CoordinationState` stores target-to-`SurfaceId` sets without full keys,
  refcounts, deterministic aggregates, or typed changes/errors.
- `ServiceMailbox::push` returns no typed outcome and retains unsupported
  `DropNewest`/`CoalesceByKey` policies.

## Impacts

- API: intentionally breaking resource transitions, subscription constructors/
  changes/queries, and mailbox policies/outcomes; C02-owned S1 exports become
  exact.
- Dependencies/features/artifacts: no change; root-owned generated artifacts stay
  untouched.
- Docs: focused rustdoc needed for changed public semantics only; C06 owns final
  examples and missing-doc closure.
- MSRV/unsafe: preserve Rust `1.89`, use no newer APIs, and retain the absolute
  unsafe prohibition.
- Root: hand off opaque resource tokens, full subscription keys, and mailbox
  outcomes; root adaptation remains separate.

## Tasks

### C02-T01 - Resource Operation State Machine

Files/area: `src/resource.rs`, resource-generation/operation support in
`src/ids.rs`, C02 resource reexports in `src/lib.rs`, and resource-focused tests
in `src/tests.rs` or a private module-local test section.

Intended behavior: implement the complete S8 status/operation/error model, exact
transition and observable-field matrices, checked generation/operation issuance,
replay/error precedence, snapshots without observers, and failure atomicity.
Remove old status variants, open mutators, observer storage, app-version coupling,
and `ResourceStateReadyTransition`.

RED evidence: first add focused tests for overlap/mismatch, cancellation replay,
field preservation/clearing, operation tokens, stale idempotence, and both
overflow paths; record expected compile/assertion failures.

Acceptance: every S8 transition and error precedence is observable; successful
non-idempotent transitions advance generation exactly once; issued tokens cannot
be publicly forged; failures and overflow change no observable/internal counter;
snapshot and renderability semantics are exact.

Commands: `cargo test --offline --locked -p surgeist-runtime`; `cargo clippy
--offline --locked -p surgeist-runtime --all-targets -- -F unsafe-code -D
warnings`; `cargo fmt --check`.

Dependencies: none beyond the reviewed cycle base.

Intended commit: `feat: add resource operation state machine`.

### C02-T02 - Coordination-Owned Subscription Identity

Files/area: `src/coord.rs`, C02 coordination reexports in `src/lib.rs`, and
subscription-focused portions of `src/tests.rs`.

Intended behavior: implement S9 full keys, exact constructors/accessors,
checked refcounts, typed changes/errors, crate-private mutation, deterministic
aggregates, and unique resource observer counts. Define the registry-facing error
variants and private exact-observer cleanup primitive without implementing C03
Runtime validation or lifecycle calls.

RED evidence: first add focused tests for replay/decrement/not-found outcomes,
scope/observer/priority identity, deterministic deduplication/ordering, resource
observer counts, exact cleanup, and refcount overflow atomicity; record failure.

Acceptance: one complete key owns one checked refcount; mutation never aliases a
partial key; aggregates count distinct keys and deduplicate observers/scopes;
missing unsubscribe is idempotent; cleanup affects only the exact `SurfaceRef`;
all failures preserve coordination state.

Commands: `cargo test --offline --locked -p surgeist-runtime`; `cargo clippy
--offline --locked -p surgeist-runtime --all-targets -- -F unsafe-code -D
warnings`; `cargo fmt --check`.

Dependencies: `TASK_CLEAN` for C02-T01 because both tasks update the shared public
front door and test module.

Intended commit: `feat: own runtime subscriptions`.

### C02-T03 - Typed Service Mailbox Outcomes

Files/area: `src/service.rs`, C02 service reexports in `src/lib.rs`, and
mailbox-focused portions of `src/tests.rs`.

Intended behavior: reduce overflow policy to `RejectNewest` and `DropOldest`;
make every push return `Accepted`, exact rejected newest input, or exact dropped
oldest input; implement zero-capacity and overflow-count behavior from S10 without
changing unrelated service lifecycle semantics.

RED evidence: first add tests covering both policies below/at/over capacity,
both zero-capacity cases, exact returned values, queue contents/order, and overflow
count enabled/disabled; record failure.

Acceptance: unsupported variants and behavior are absent; accepted messages keep
FIFO order; every rejected/dropped value is recoverable unchanged; zero capacity
stores nothing; overflow observation increments exactly once only when enabled.

Commands: `cargo test --offline --locked -p surgeist-runtime`; `cargo clippy
--offline --locked -p surgeist-runtime --all-targets -- -F unsafe-code -D
warnings`; `cargo fmt --check`.

Dependencies: `TASK_CLEAN` for C02-T02 because both tasks update the shared public
front door and test module.

Intended commit: `feat: define service mailbox outcomes`.

## Completion

After all tasks are `TASK_CLEAN`, make the status-only `complete` commit and run:
`cargo metadata --offline --locked --no-deps --format-version 1`;
`cargo check --offline --locked -p surgeist-runtime`;
`cargo test --offline --locked -p surgeist-runtime`;
`cargo test --offline --locked -p surgeist-runtime --doc`;
`cargo clippy --offline --locked -p surgeist-runtime --all-targets -- -F
unsafe-code -D warnings`;
`cargo fmt --check`;
`git ls-files -co --exclude-standard -- '*.rs'`;
`! rg -n --pcre2 '#\s*\[\s*(?:unsafe\s*\(|no_mangle\b|export_name\b)|
\bunsafe\s*(?:\{|fn\b|trait\b|impl\b|extern\b)|\bstatic\s+mut\b|
\bextern\s*(?:"[^"]*")?\s*\{' $(git ls-files -co --exclude-standard -- '*.rs')`;
`! rg -n 'surgeist_(retained|window|task)|surgeist-(retained|window|task)'
Cargo.toml src`;
`! rg -n '\b(Starting|Running)\b|ResourceStateReadyTransition'
src/resource.rs src/lib.rs`;
`! rg -n 'DropNewest|CoalesceByKey' src/service.rs src/lib.rs`.

Metadata must retain Rust `1.89`, no dependencies, and default-only features.
Run the complete final set before holistic review, after CLEAN review at the exact
head, and after landing on local `main`. Publish/read back the immutable C02 SHA,
then hand root the API delta and make it C03's base. Failures and blockers follow
the canonical correction, landing, and blocker contracts.
