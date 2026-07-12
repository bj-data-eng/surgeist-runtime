# C04 Reducer, Effects, Provenance, And Versions Plan

Cycle ID: `C04`

Owning repository: `/Users/codex/Development/surgeist-runtime`

Status: `complete`

Cycle base: `935068e5e63903b01059f968c940a5c9112176d7`

Reviewed specification: `plans/specs/runtime-review-findings-resolution.md` at
normalized SHA-256
`b7876239cca9dbc12ccac157a897acb9284e9ee726c22c06e672f05277ee4c40`.

Applicable specification sections: S1 `diagnostic.rs`, `effect.rs`, `input.rs`/
`provenance.rs`, `reducer.rs`, and `task.rs` export rows plus C04's drain error/
report slice from `runtime.rs`; S3/S3A final Runtime redraw paragraphs; S4; S5;
S6 C04 drain error/report contract; S7; S13 state-version and drain-preflight
overflow excluding C05's pending-count composition; S14 C04 coverage.

Reviewed sequence: `plans/sequences/runtime-review-findings-resolution.md` at
normalized SHA-256
`18508f7cb08b4577ffc13fc264948199c652f0aaa12a0b449b80dcd6a6d7a251`, entry
`C04 - Reducer, Effects, Provenance, And Versions`.

Bounded outcome: make reducer commits atomic, apply diagnostics/redraws, forward
all adapter work as typed intents, reject invalid effects, preserve explicit
causality, and make state/surface preflight overflow requeue the exact input.

## Boundary

In scope: correlation/provenance, reducer commits, effect payloads/outcomes/intents,
Runtime reduction/effect transactions and redraw selection, typed drain errors/
partial committed-work reports, checked versions, focused docs/tests, plan status.

Out of scope: queue capacity/error/default/fairness and pending-count report
fields/proof; proxy wake; concrete service/resource/task/Tokio execution; manifest/
snapshot validation; final docs/examples; root/sibling writes; dependency/feature,
script/CI/generator/generated artifacts, and compatibility shims.

Root communication remains on hold. Prepare evidence locally, send no root
message. Each task uses a fresh clean-context worker and task reviewer; canonical
holistic/publication rules remain applicable to the leaf.

## Baseline Evidence

- C03 candidate `935068e5e63903b01059f968c940a5c9112176d7` is published,
  read back, and is the clean local/tracking base.
- The base passes 91 tests, doctests, strict Clippy, format, metadata Rust `1.89`,
  and owned-Rust unsafe/boundary scans.
- Correlation uses an ambiguous zero-capable ID and provenance lacks independent
  explicit current/parent absence and origin-safe surface attachment.
- Reducers receive mutable state and failure can coexist structurally with effects;
  state version advancement is unchecked.
- Runtime reports executed effects without complete applied/forwarded/rejected
  outcomes, abstract intents, registry-validated redraws, or transactional
  changed-state invalidation/overflow requeue.

## Impacts

- API: intentionally breaking provenance, reducer, effect, report, and drain-error
  contracts; C04-owned S1 exports become exact.
- Dependencies/features/artifacts: unchanged; generated artifacts stay root-owned.
- Docs: focused Rustdoc for changed public semantics; C06 owns final examples and
  missing-doc closure.
- MSRV/unsafe: preserve Rust `1.89` and absolute unsafe prohibition.
- Root: hold typed intent/provenance/redraw API evidence locally; no message.

## Tasks

### C04-T01 - Explicit Correlation And Provenance

Files/area: correlation support in `src/ids.rs`, `src/provenance.rs`, direct
construction/accessor updates in `src/input.rs` and `src/task.rs`, C04 reexports
in `src/lib.rs`, and provenance-focused tests.

Intended behavior: implement S7 nonzero `CorrelationId`, explicit independent
`Correlation` fields, complete constructors/set-clear/accessors, generation-
qualified origin data, and origin-specific idempotent/rejected surface attachment
with typed errors. Remove synthetic zero and retained-source concepts.

RED evidence: first add tests for zero rejection/default absence, independent
current/parent set-clear, every origin constructor/source, surface generation,
idempotent attachment, overwrite, already-attached, and unsupported origins.

Acceptance: invalid correlation is unconstructable; no constructor invents causal
data; changing one causal field leaves all others unchanged; every surface error
contains exact origin/existing/attempted values and returns no partial provenance.

Commands: `cargo test --offline --locked -p surgeist-runtime`; `cargo test
--offline --locked -p surgeist-runtime --doc`; `cargo clippy --offline --locked
-p surgeist-runtime --all-targets -- -F unsafe-code -D warnings`; `cargo fmt --check`.

Dependencies: none beyond the reviewed cycle base.

Intended commit: `feat: make input provenance explicit`.

### C04-T02 - Immutable Reducer Commit Model

Files/area: `src/reducer.rs`, checked `StateVersion` in `src/snapshot.rs`, minimal
direct-result plumbing in `src/runtime.rs`, `src/lib.rs` reexports, focused tests.

Intended behavior: define immutable `Reducer`, explicit commit/change, disjoint
failure, complete accessors, and checked `StateVersion`. Adapt Runtime only to
consume the new result; T04 owns atomic overflow/effect/redraw semantics.

RED evidence: first prove failure excludes state/effects, successful commit shapes/
provenance/order, immutable state, checked version overflow, and unchanged runtime
state/effect behavior through the new result; record compile/assertion failure.

Acceptance: mutate-then-fail is unrepresentable; successes carry commits; failure
has only message/provenance; overflow is typed/atomic; direct Runtime plumbing
preserves current behavior and implements no T04 transaction semantics.

Commands: `cargo test --offline --locked -p surgeist-runtime`; `cargo test
--offline --locked -p surgeist-runtime --doc`; `cargo clippy --offline --locked
-p surgeist-runtime --all-targets -- -F unsafe-code -D warnings`; `cargo fmt --check`.

Dependencies: `TASK_CLEAN` for T01 because reducer provenance uses its final API
and both tasks update public exports/tests.

Intended commit: `feat: define atomic reducer commits`.

### C04-T03 - Typed Effect Outcomes And Adapter Intents

Files/area: `src/effect.rs`, only necessary diagnostic/task value updates in
`src/diagnostic.rs` and `src/task.rs`, C04 effect reexports in `src/lib.rs`, and
effect-model tests. Do not edit Runtime.

Intended behavior: implement backed kinds, exact payload constructors,
`EffectDisposition`, `RuntimeIntent`, `EffectOutcome`, and resource-operation token
preservation. Remove unsupported kind IDs; T04 replaces Runtime's legacy aggregate.

RED evidence: first add tests for backed/absent kinds, every applied/forwarded/
rejected value shape, intent payload identity, resource operation preservation,
and private invariant enforcement; record failure.

Acceptance: every effect kind has a path; intents preserve payloads; outcome fields
cannot contradict disposition; unsupported kind IDs are absent; no new aggregate
vocabulary is added, while T04 removes the existing Runtime report aggregate.

Commands: `cargo test --offline --locked -p surgeist-runtime`; `cargo test
--offline --locked -p surgeist-runtime --doc`; `cargo clippy --offline --locked
-p surgeist-runtime --all-targets -- -F unsafe-code -D warnings`; `cargo fmt --check`.

Dependencies: `TASK_CLEAN` for T02 because commits own effect batches and shared
exports/tests must remain linear.

Intended commit: `feat: define runtime effect outcomes`.

### C04-T04 - Atomic Runtime Commit And Effect Processing

Files/area: `src/runtime.rs` reduction/drain/effects/redraw; transactional support
in `src/surface.rs`; final outcome-constructor use in `src/effect.rs`; fallible
propagation in `src/loop_.rs`/`src/testing.rs`/direct tests; exact error/report
reexports in `src/lib.rs`.

Intended behavior: integrate S4/S5/S13 and final S3/S3A paragraphs: atomically
preflight/install changed state/version and nonterminal invalidations; deduplicate
redraws; call all three outcome constructors and remove only their transitional
lint expectations; apply diagnostics, forward intents, reject invalid targets; on
overflow restore exact input/lane/start state and prior-work report; propagate the
fallible drain unchanged and leave C05 queue/report composition untouched.

RED evidence: first add tests for AppLoop result/error delegation, failure isolation/
provenance, all dispositions, automatic/explicit redraw validation/deduplication,
state/surface overflow exact requeue/partial prior-work report, and direct caller
propagation; record failure.

Acceptance: no failed/overflowing input commits partial state/surface/effects;
AppLoop/helpers propagate fallibility; all outcome constructors are used and only
their expectations removed; causality/order/redraws/intents are exact; no effect-
model redesign or C05 queue/proxy behavior is implemented.

Commands: `cargo test --offline --locked -p surgeist-runtime`; `cargo test
--offline --locked -p surgeist-runtime --doc`; `cargo clippy --offline --locked
-p surgeist-runtime --all-targets -- -F unsafe-code -D warnings`; `cargo fmt --check`.

Dependencies: `TASK_CLEAN` for T03 and published C03 registry semantics.

Intended commit: `feat: process runtime commits and effects atomically`.

## Completion

After all tasks are `TASK_CLEAN`, make the status-only `complete` commit and run:
`cargo metadata --offline --locked --no-deps --format-version 1`;
`cargo check --offline --locked -p surgeist-runtime`;
`cargo test --offline --locked -p surgeist-runtime`;
`cargo test --offline --locked -p surgeist-runtime --doc`;
`cargo clippy --offline --locked -p surgeist-runtime --all-targets -- -F unsafe-code -D warnings`;
`cargo fmt --check`; `git ls-files -co --exclude-standard -- '*.rs'`;
`! rg -n --pcre2 '#\s*\[\s*(?:unsafe\s*\(|no_mangle\b|export_name\b)|\bunsafe\s*(?:\{|fn\b|trait\b|impl\b|extern\b)|\bstatic\s+mut\b|\bextern\s*(?:"[^"]*")?\s*\{' $(git ls-files -co --exclude-standard -- '*.rs')`;
`! rg -n 'surgeist_(retained|window|task)|surgeist-(retained|window|task)' Cargo.toml src`;
`! rg -n 'schedule_timer|window_command' src/effect.rs src/lib.rs src/runtime.rs`; `! rg -n 'executed_effects' src`; `! rg -n -U --pcre2 'fn\s+reduce\s*\(\s*&\s*(?:\x27[A-Za-z_][A-Za-z0-9_]*\s+)?mut\s+self\s*,\s*[^:,]+:\s*&\s*(?:\x27[A-Za-z_][A-Za-z0-9_]*\s+)?mut\b' src`.

Metadata must retain Rust `1.89`, no dependencies, and default-only features.
Run the complete final set before holistic review, after CLEAN review at the exact
head, and after landing on local `main`. Publish/read back the immutable C04 SHA,
record evidence locally, send no root message, and use it as C05's base. Failures
follow canonical correction/landing rules.
