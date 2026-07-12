# C03 Surface Lifecycle And Runtime Registry Plan

Cycle ID: `C03`

Owning repository: `/Users/codex/Development/surgeist-runtime`

Status: `draft`

Cycle base: `cca1b9281a883417cbe30a648f80d80ca7da0bf4`

Reviewed specification: `plans/specs/runtime-review-findings-resolution.md` at
normalized SHA-256
`b7876239cca9dbc12ccac157a897acb9284e9ee726c22c06e672f05277ee4c40`.

Applicable specification sections: S2 registry-dependent Runtime validation and
mutation clauses; S3 except its final Runtime redraw-target paragraph assigned to
C04; S3A except its final effect/redraw lookup paragraph assigned to C04; S9
registry observer validation and lifecycle/replacement/removal cleanup clauses;
S13 Runtime-owned surface/invalidation overflow; S14 C03 coverage.

Reviewed sequence: `plans/sequences/runtime-review-findings-resolution.md` at
normalized SHA-256
`f98521823c097e27166e4e72933fe3047201531dce20e148849ed2f9dd45457b`, entry
`C03 - Surface Lifecycle And Runtime Registry`.

Bounded outcome: make Runtime the authoritative generation-qualified surface
registry, complete lifecycle/invalidation/render semantics, stage all registry
mutation atomically, and bind subscriptions to current nonterminal registrations.

## Boundary

In scope: surface lifecycle/render source, Runtime surface registry and dedicated
surface APIs, coordination integration already modeled by C02, C03 public
reexports, focused tests, and this plan's status.

Out of scope: final Runtime redraw-effect target selection and effect rejection;
reducer-driven automatic invalidation; resource/task/service lowering; queues and
wake scheduling; root or sibling writes; dependencies/features; scripts, CI,
generators, generated API artifacts, or compatibility shims.

Root communication is explicitly on hold by user instruction. Prepare candidate
evidence locally but do not send a root handoff message. Each task uses a fresh
clean-context worker and distinct task reviewer; canonical holistic/publication
rules remain applicable to the leaf.

## Baseline Evidence

- C02 candidate `cca1b9281a883417cbe30a648f80d80ca7da0bf4` is published,
  read back, and is the clean local/tracking base.
- The base passes 70 tests, doctests, strict Clippy, format, metadata Rust `1.89`,
  and owned-Rust unsafe/boundary scans.
- `UiSurface` has runtime-owned local values but no fallible lifecycle matrix,
  render frame/ack state, or terminal mutation enforcement.
- `Runtime` uses `add_surface` and a plain map without tombstones, staged updates,
  generation checks, coordination ownership, or dedicated interaction/render APIs.
- C02 coordination already provides exact keys/refcounts/aggregates and the
  private exact-observer cleanup primitive needed here.

## Impacts

- API: intentionally breaking registry, lifecycle, validation, interaction, and
  render contracts; direct mutable surface access remains unavailable.
- Dependencies/features/artifacts: unchanged; root-owned generated artifacts are
  untouched.
- Docs: focused Rustdoc for changed public state/transition semantics; C06 owns
  final examples and missing-doc closure.
- MSRV/unsafe: preserve Rust `1.89`, use no newer API, and retain absolute unsafe
  prohibition.
- Root: prepare new SurfaceRef/registry/render contracts and stale-reference rules
  as evidence only; send no message until authorized.

## Tasks

### C03-T01 - Fallible Surface Lifecycle And Render State

Files/area: `src/surface.rs`, only necessary C03 surface reexports in `src/lib.rs`,
and focused surface tests in `src/tests.rs` or private module tests.

Intended behavior: implement the exact S3 lifecycle matrix, terminal mutation
rules, checked/coalesced invalidation state, render frame/state/ack value model,
local begin/ack eligibility and monotonic consumption, replay semantics, and
failure atomicity. Preserve C01 local validation/mutation contracts.

RED evidence: first add tests for every lifecycle edge/rejection, terminal no-op
rejection, render eligibility, stale/replayed frames, invalidations after frame
begin, newer snapshot retention, consumed/remaining counts, and both overflow
paths; record expected failure.

Acceptance: every transition returns the specified result; terminal state never
mutates; frame acknowledgement removes only representable captured work, is
monotonic/idempotent exactly where specified, and reports remaining redraw work;
all rejection and overflow paths preserve state.

Commands: `cargo test --offline --locked -p surgeist-runtime`; `cargo test
--offline --locked -p surgeist-runtime --doc`; `cargo clippy --offline --locked
-p surgeist-runtime --all-targets -- -F unsafe-code -D warnings`; `cargo fmt
--check`.

Dependencies: none beyond the reviewed cycle base.

Intended commit: `feat: define surface lifecycle and render state`.

### C03-T02 - Authoritative Runtime Surface Registry

Files/area: registry/coordination portions of `src/runtime.rs`, only necessary
crate-private coordination integration in `src/coord.rs`, C03 runtime reexports
in `src/lib.rs`, and registry-focused portions of `src/tests.rs`.

Intended behavior: implement S3A registry, tombstones, current `SurfaceRef`
issuance, staged update/remove/re-registration, read-only queries, Runtime-owned
CoordinationState, validated subscribe/unsubscribe, and atomic subscription
cleanup on generation change, first terminal transition, and removal. Exclude the
final effect/redraw lookup paragraph.

RED evidence: first add tests for duplicate/unknown/stale operations,
failure-atomic update closures, removal/reregistration tombstones and overflow,
old-reference isolation, exact cleanup on replacement/terminal/removal, and
validated subscription errors; record failure.

Acceptance: no public map/coordination mutation bypass exists; every operation
uses unknown-before-stale precedence; staged failure changes neither registry nor
coordination; ID reuse advances checked generation and cannot revive old
references/subscriptions; cleanup is exact and idempotent.

Commands: `cargo test --offline --locked -p surgeist-runtime`; `cargo test
--offline --locked -p surgeist-runtime --doc`; `cargo clippy --offline --locked
-p surgeist-runtime --all-targets -- -F unsafe-code -D warnings`; `cargo fmt
--check`.

Dependencies: `TASK_CLEAN` for C03-T01 because registry staging consumes its
lifecycle/generation semantics and all tasks share Runtime-facing tests/exports.

Intended commit: `feat: own the runtime surface registry`.

### C03-T03 - Runtime Interaction Validation And Rendering

Files/area: dedicated surface operations in `src/runtime.rs`, local delegates in
`src/surface.rs` only when required, C03 reexports in `src/lib.rs`, and focused
interaction/render tests in `src/tests.rs`.

Intended behavior: expose S2 Runtime resize/scroll/focus/hover and element/route
validation with exact precedence; complete registry-mediated lifecycle mutation,
S3 Runtime begin-render/mark-rendered, render-state borrow, and
renderable-invalidated iteration. Do not implement explicit redraw effects or
reducer-driven invalidation.

RED evidence: first add tests for unknown/stale/lifecycle/element precedence,
surface mismatch, focus/hover set-clear-idempotence, invalidation/redraw outcomes,
resize rules, route phase validation, borrow-bound render state, and render ack
atomicity/coalescing; record failure.

Acceptance: every public operation resolves the registry once, validates in exact
order, and delegates without bypass; inactive/terminal behavior and idempotence
are exact; render state matches Runtime state/version under its borrow; iterator
order and eligibility are deterministic; no effect/reducer path is added.

Commands: `cargo test --offline --locked -p surgeist-runtime`; `cargo test
--offline --locked -p surgeist-runtime --doc`; `cargo clippy --offline --locked
-p surgeist-runtime --all-targets -- -F unsafe-code -D warnings`; `cargo fmt
--check`.

Dependencies: `TASK_CLEAN` for C03-T02 because all operations require its
authoritative registry and subscription cleanup boundary.

Intended commit: `feat: expose runtime surface operations`.

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
`! rg -n 'pub fn add_surface|pub fn surface_mut' src/runtime.rs`.

Metadata must retain Rust `1.89`, no dependencies, and default-only features.
Run the complete final set before holistic review, after CLEAN review at the exact
head, and after landing on local `main`. Publish/read back the immutable C03 SHA,
record candidate evidence locally for the user, send no root message, then use the
verified SHA as C04's base. Failures follow canonical correction/landing rules.
