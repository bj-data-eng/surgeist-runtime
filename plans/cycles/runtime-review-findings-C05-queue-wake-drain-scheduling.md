# C05 Queue, Wake, And Drain Scheduling Plan

Cycle ID: `C05`

Owning repository: `/Users/codex/Development/surgeist-runtime`

Status: `in_progress`

Cycle base: `3db9f2bd523b0d32f02e90aaf4d5e3161c7d2366`

Reviewed specification: `plans/specs/runtime-review-findings-resolution.md` at
normalized SHA-256
`b7876239cca9dbc12ccac157a897acb9284e9ee726c22c06e672f05277ee4c40`.

Applicable specification sections: S1 `loop_.rs`/`proxy.rs` exports other than
`AppLoop`, remaining `runtime.rs` queue/scheduling exports and report fields; S6
excluding C04's drain-error and effect/outcome contract; S13 requeued-input pending
counts/final composition; S14 C05 coverage.

Reviewed sequence: `plans/sequences/runtime-review-findings-resolution.md` at
normalized SHA-256
`18508f7cb08b4577ffc13fc264948199c652f0aaa12a0b449b80dcd6a6d7a251`, entry
`C05 - Queue, Wake, And Drain Scheduling`.

Bounded outcome: make runtime/proxy admission failure-atomic, rotate bounded lane
drains fairly with complete pending reports, and guarantee deferred proxy wake
coverage for every accepted input and partial-drain continuation.

## Boundary

In scope: runtime queue policy/errors/admission, exact budgets, persistent fair lane
selection, pending report/error composition, proxy policy/errors/wake state,
continuation reports, focused public docs/tests, and plan status.

Out of scope: reducer/effect/resource/service execution redesign, manifest/snapshot
validation, final docs/examples, root wake-adapter implementation/tests, root or
sibling writes/messages, dependencies/features, scripts, CI, generators, generated
API artifacts, compatibility shims, or C06 work.

Root communication remains on hold. Retain the candidate SHA, API delta, and root
`WakeBridge` obligations locally. Each task uses a fresh clean-context worker and
task reviewer; canonical holistic/publication rules remain applicable.

## Baseline Evidence

- C04 candidate `3db9f2bd523b0d32f02e90aaf4d5e3161c7d2366` is published,
  read back, and is the clean local/tracking base.
- The base passes metadata Rust `1.89`, check, 108 tests, doctests, strict Clippy,
  format, unsafe/boundary scans, and a clean holistic review.
- Runtime policy omits the UI lane, mutates via a builder, and queue overflow drops
  input behind nongeneric errors; lane-priority draining can starve later lanes.
- Runtime budgets/report fields are incomplete, including pending counts after a
  C04 overflow requeue.
- Proxy uses a boolean wake flag, returns no exact rejected input, leaves failed-wake
  delivery ambiguous, and has no successor wake/error report after partial drain.

## Impacts

- API: intentionally breaking runtime/proxy policy, error, budget, drain, and wake
  contracts; C05-owned S1 exports become exact.
- Dependencies/features/artifacts: unchanged; generated artifacts stay root-owned.
- Docs: focused Rustdoc for deferred/non-reentrant wake and changed public values;
  C06 owns final examples and missing-doc closure.
- MSRV/unsafe: preserve Rust `1.89` and the absolute unsafe prohibition.
- Root: retain concrete deferred-wake and continuation-failure obligations locally;
  send no message.

## Tasks

### C05-T01 - Lossless Typed Runtime Queue Admission

Files/area: runtime queue policy/error/admission in `src/runtime.rs`, exact reexports
in `src/lib.rs`, direct test-fixture construction, focused tests, and Rustdoc.

Intended behavior: implement exact three-lane `RuntimeQueuePolicy`, immutable
construction/defaults, `Runtime::new_with_queue_policy`, and generic
`RuntimeQueueError<T>` returning the exact rejected lane wrapper. Capacity zero
rejects every enqueue; each rejection records one diagnostic and changes no queue.

RED evidence: first add tests for exact defaults/builders/accessors, custom and zero
capacities on all lanes, exact code/lane/capacity/rejected values, retry identity,
one diagnostic, and unchanged FIFO contents; record expected failure.

Acceptance: no queue failure consumes input; all lane capacities are observable and
immutable after construction; `Runtime::new` delegates to the exact default; no
implicit unbounded or lossy admission remains.

Commands: `cargo test --offline --locked -p surgeist-runtime`; `cargo test
--offline --locked -p surgeist-runtime --doc`; `cargo clippy --offline --locked
-p surgeist-runtime --all-targets -- -F unsafe-code -D warnings`; `cargo fmt --check`.

Dependencies: none beyond the reviewed C05 base.

Intended commit: `feat: make runtime queue admission lossless`.

### C05-T02 - Fair Budgeted Runtime Drain Reports

Files/area: scheduling/budget/report composition in `src/runtime.rs`, direct
`src/testing.rs` drain helpers, focused tests, and changed public Rustdoc.

Intended behavior: implement exact four-limit `RuntimeBudget`, persistent cyclic
`next_drain_lane`, per-call lane counters, and complete remaining/has-pending report
fields. Stop when no lane is budget-eligible. On C04 overflow, restore the lane
by pushing the exact input to that lane's front, setting `next_drain_lane` to the
failing lane, and including the requeued input in every pending count.

RED evidence: first test exact defaults/builders/zero limits, rotation, starvation,
lane skipping, and pending fields. Then commit other-lane work before overflowing a
target-lane input with a later same-lane peer already queued; assert complete error
counts, `next_drain_lane` set to that target, and the next drain retries the failed
input before its peer; record failure.

Acceptance: repeated one-input drains rotate among eligible lanes; no lane exceeds
its local/global budget; every normal/error report reflects all queues after final
disposition; helpers terminate only when `has_pending_inputs` is false.

Commands: `cargo test --offline --locked -p surgeist-runtime`; `cargo test
--offline --locked -p surgeist-runtime --doc`; `cargo clippy --offline --locked
-p surgeist-runtime --all-targets -- -F unsafe-code -D warnings`; `cargo fmt --check`.

Dependencies: `TASK_CLEAN` for T01 because policy/admission fixes the queue model.

Intended commit: `feat: drain runtime queues fairly`.

### C05-T03 - Deferred Reliable Proxy Wake Delivery

Files/area: `src/proxy.rs`, proxy reexports in `src/lib.rs`, direct fake/fixture
updates in `src/testing.rs`, focused sequential/concurrent tests, and Rustdoc.

Intended behavior: implement exact `QueuePolicy`, `WakeError`, generic
`AppProxyError<Input>`, `ProxyInput`, and `ProxyDrainReport`; use one mutex/condition-
variable `Idle`/`Waking`/`Signaled`/`NeedsWake` state with private entry tokens.
`drain_pending` accepts `NonZeroUsize`; every direct fixture/test caller constructs
a nonzero limit.
Wake runs outside the lock and must only signal a future host turn. Failed owners
roll back exactly their input; waiters re-signal backlog. Partial drains issue a
successor wake and expose continuation failure without pretending it is signaled.
`drain_pending` waits on the condition variable while `Waking`, then consumes only
after resolution to `Signaled` or `NeedsWake`.

RED evidence: first add deterministic tests for defaults/zero capacity, exact
rejected inputs/error sources, deferred non-reentrancy, failed owner rollback,
concurrent accepted-send drainability, one wake per covered backlog, partial-drain
`NonZeroUsize` report behavior/successor wake, continuation failure, and later
`NeedsWake` recovery. A controllable bridge must prove a racing drain neither
consumes nor returns before blocked wake success/failure resolves; no sleeps.

Acceptance: every `Ok` send is covered by a successful pending wake; failure never
strands another accepted input; no drain consumes during `Waking`; no user wake code
runs under the mutex; empty drain returns `Idle`; partial reports are complete and
root-actionable.

Commands: `cargo test --offline --locked -p surgeist-runtime`; `cargo test
--offline --locked -p surgeist-runtime --doc`; `cargo clippy --offline --locked
-p surgeist-runtime --all-targets -- -F unsafe-code -D warnings`; `cargo fmt --check`.

Dependencies: `TASK_CLEAN` for T02 so shared fixtures/tests consume final reports.

Intended commit: `feat: make proxy wake delivery reliable`.

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
`! rg -n 'max_task_events|max_service_events|wake_pending' src`.

Metadata must retain Rust `1.89`, no dependencies, and default-only features.
Run the complete final set before holistic review, after CLEAN review at the exact
head, and after landing on local `main`. Publish/read back the immutable C05 SHA,
retain all handoff evidence locally, send no root message, and use it as C06's base.
