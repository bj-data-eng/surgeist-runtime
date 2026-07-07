# Runtime Task Intents Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove duplicate task subsystem semantics from `surgeist-runtime` and leave runtime with only abstract task intents plus task-originated queue ingress for root to lower into `surgeist-task`.

**Architecture:** `surgeist-task` owns task execution, lifecycle, cancellation, queues, events, policies, and Tokio integration. `surgeist-runtime` owns app loop orchestration and emits runtime-owned task intents without depending on `surgeist-task`. Root owns `RuntimeTaskAdapter`, converting runtime task intents into `surgeist-task` requests and converting `surgeist-task` events back into runtime task-lane inputs.

**Tech Stack:** Rust 2024, `surgeist-runtime`, existing `surgeist-retained` and `surgeist-window` path dependencies, root-owned adapter outside this crate.

---

## Constraints

- Stay inside `/Users/codex/Development/surgeist-runtime`.
- Do not edit sibling crate repos, including `../surgeist-task`.
- Do not update root `surgeist` submodule pointers.
- Use `apply_patch` for manual edits.
- Use the current `main` branch.
- This crate does not need backwards compatibility.
- Runtime must not depend on `surgeist-task` after this plan completes.
- Runtime must not duplicate task execution, task lifecycle, cancellation truth, retry, queue coalescing, or Tokio executor semantics.
- Root is expected to adapt to runtime API changes and owns the lowering bridge.

## Target Boundary

Runtime keeps:

- task-originated input lane: `TaskInput<Input>`
- task provenance sufficient for root-supplied task identity, diagnostics, and correlation
- abstract task intent effects: start, cancel, and priority hint
- budgeted draining of UI/task/service lanes
- runtime diagnostics for runtime-owned concerns such as queue overflow and reducer errors

Runtime removes:

- direct `surgeist-task` dependency
- runtime-owned task progress coalescing API
- `CoalescingKey`
- `ProgressEvent`
- `RuntimeExecutor`
- `SpawnRequest`
- `ExecutorEvent`
- `ExecutorEventPayload`
- `ExecutorTaskHandle`
- `ExecutorError`
- `FakeExecutor`
- runtime-owned `TaskPolicy`
- runtime-owned `UnobservedPolicy`
- runtime-owned `BlockingPolicy`
- runtime-owned `TaskRecord` lifecycle state machine
- runtime-owned `CancellationToken`
- runtime-owned stale task lifecycle filtering

Root owns:

- mapping runtime task intent names/keys/handles into `surgeist-task`
- choosing concrete task jobs and policies
- mapping `surgeist-task` events into runtime `TaskInput<Input>`
- stale event rejection that requires task lifecycle knowledge

## File Map

- Modify `Cargo.toml`
  - Remove `surgeist-task` dependency in the final cleanup task.
- Modify `src/lib.rs`
  - Export runtime task intent types.
  - Stop exporting removed executor/task subsystem types.
- Modify `src/task.rs`
  - First add runtime task intent types beside the old task subsystem types.
  - Later remove old task subsystem types.
- Modify `src/effect.rs`
  - Make task effects abstract intents for root to lower.
- Modify `src/runtime.rs`
  - Remove executor field, `with_executor`, `register_task_record`, task allocation, spawn/cancel execution, and local task lifecycle checks.
  - Preserve queueing and budgeted draining of `TaskInput<Input>`.
- Modify `src/provenance.rs`, `src/diagnostic.rs`, `src/coord.rs`, `src/descriptor.rs`, `src/testing.rs`, and `src/tests.rs`
  - Update imports and tests to the runtime task intent surface.
- Delete `src/executor.rs`
  - Delete once runtime no longer owns executor behavior.

## Task 1: Add Runtime Task Intent Types Without Changing Behavior

**Files:**
- Modify: `src/task.rs`
- Modify: `src/lib.rs`
- Modify: `src/tests.rs`

- [ ] **Step 1: Write failing tests for runtime task intent identities**

Add to `src/tests.rs`:

```rust
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
```

- [ ] **Step 2: Run the focused test and verify failure**

Run:

```sh
cargo test -p surgeist-runtime task_intent_identity_types_are_runtime_owned
```

Expected before implementation: compile failure for missing `TaskIntentName`, `TaskIntentKey`, `TaskIntentId`, or `TaskIntentHandle`.

- [ ] **Step 3: Add intent types to `src/task.rs`**

Append these types above the old task subsystem definitions so the crate still compiles:

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskIntentName(String);

impl TaskIntentName {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskIntentKey(String);

impl TaskIntentKey {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskIntentId(u64);

impl TaskIntentId {
    #[must_use]
    pub const fn from_u64(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskIntentAttemptId(u64);

impl TaskIntentAttemptId {
    #[must_use]
    pub const fn from_u64(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskIntentHandle {
    id: TaskIntentId,
    attempt_id: TaskIntentAttemptId,
}

impl TaskIntentHandle {
    #[must_use]
    pub const fn new(id: TaskIntentId, attempt_id: TaskIntentAttemptId) -> Self {
        Self { id, attempt_id }
    }

    #[must_use]
    pub const fn id(self) -> TaskIntentId {
        self.id
    }

    #[must_use]
    pub const fn attempt_id(self) -> TaskIntentAttemptId {
        self.attempt_id
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TaskPriorityHint {
    Low,
    Normal,
    High,
}
```

- [ ] **Step 4: Export the new types**

In `src/lib.rs`, keep existing task exports and add:

```rust
pub use task::{
    TaskIntentAttemptId, TaskIntentHandle, TaskIntentId, TaskIntentKey, TaskIntentName,
    TaskPriorityHint,
};
```

- [ ] **Step 5: Run focused and baseline checks**

Run:

```sh
cargo test -p surgeist-runtime task_intent_identity_types_are_runtime_owned
cargo test -p surgeist-runtime
cargo clippy -p surgeist-runtime --all-targets -- -D warnings
cargo fmt --check
```

Expected: all pass.

- [ ] **Step 6: Commit**

```sh
git add src/task.rs src/lib.rs src/tests.rs
git commit -m "Add runtime task intent identities"
```

## Task 2: Convert Task Effects To Abstract Intents

**Files:**
- Modify: `src/effect.rs`
- Modify: `src/descriptor.rs`
- Modify: `src/tests.rs`
- Modify: `src/testing.rs`

- [ ] **Step 1: Write failing tests for abstract task effects**

Add to `src/tests.rs`:

```rust
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
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```sh
cargo test -p surgeist-runtime task_effects_are_abstract_runtime_intents
cargo test -p surgeist-runtime cancel_task_effect_carries_runtime_task_intent_handle
```

Expected before implementation: type mismatch because `AppEffect::start_task` and `cancel_task` still use old task subsystem types.

- [ ] **Step 3: Update task effect types**

In `src/effect.rs`, import:

```rust
use super::{
    AppScope, CorrelationId, Diagnostic, ResourceId, ServiceCommandName, ServiceCommandPayload,
    ServiceId, SurfaceId, TaskIntentHandle, TaskIntentKey, TaskIntentName, TaskPriorityHint,
};
```

Change constructors:

```rust
pub fn start_task(name: TaskIntentName, key: TaskIntentKey, scope: AppScope) -> Self
pub fn cancel_task(handle: TaskIntentHandle) -> Self
pub fn reprioritize_task(handle: TaskIntentHandle, priority: TaskPriorityHint) -> Self
```

Change structs:

```rust
pub struct StartTaskEffect {
    name: TaskIntentName,
    key: TaskIntentKey,
    scope: AppScope,
}

pub struct CancelTaskEffect {
    handle: TaskIntentHandle,
}

pub struct ReprioritizeTaskEffect {
    handle: TaskIntentHandle,
    priority: TaskPriorityHint,
}
```

- [ ] **Step 4: Update task descriptors**

In `src/descriptor.rs`, use `TaskIntentName` for `TaskDescriptor`:

```rust
use super::TaskIntentName;

pub struct TaskDescriptor {
    name: TaskIntentName,
    input_type: &'static str,
}
```

- [ ] **Step 5: Update tests and examples that construct task effects**

Replace:

```rust
TaskName::new("search")
TaskKey::new("search:rust")
TaskHandle::new(task_id, attempt_id)
TaskPriority::High
```

with:

```rust
TaskIntentName::new("search")
TaskIntentKey::new("search:rust")
TaskIntentHandle::new(
    TaskIntentId::from_u64(task_id.as_u64()),
    TaskIntentAttemptId::from_u64(attempt_id.as_u64()),
)
TaskPriorityHint::High
```

Only update call sites that are about runtime effects/descriptors. Leave old task subsystem tests alone until Task 4.

- [ ] **Step 6: Run checks**

Run:

```sh
cargo test -p surgeist-runtime task_effects_are_abstract_runtime_intents
cargo test -p surgeist-runtime cancel_task_effect_carries_runtime_task_intent_handle
cargo test -p surgeist-runtime
cargo clippy -p surgeist-runtime --all-targets -- -D warnings
cargo fmt --check
```

Expected: all pass.

- [ ] **Step 7: Commit**

```sh
git add src/effect.rs src/descriptor.rs src/tests.rs src/testing.rs
git commit -m "Make task effects abstract runtime intents"
```

## Task 3: Remove Runtime Executor Ownership

**Files:**
- Modify: `src/runtime.rs`
- Modify: `src/testing.rs`
- Modify: `src/tests.rs`
- Modify: `src/lib.rs`
- Delete: `src/executor.rs`

- [ ] **Step 1: Write failing test for emitting task intents without execution**

Add a `CounterInput::StartTask` test fixture variant and this test in `src/tests.rs`:

```rust
#[test]
fn runtime_reports_task_intents_without_executing_them() {
    let mut runtime = Runtime::new(CounterState::default(), CounterReducer);
    runtime
        .enqueue_ui(UiInput::new(
            CounterInput::StartTask,
            InputProvenance::ui(SurfaceId::from_u64(1)),
        )
        .unwrap());

    let report = runtime.drain_once(RuntimeBudget::new());

    assert_eq!(report.executed_effects(), 1);
    assert_eq!(report.task_intents().len(), 1);
    assert_eq!(report.task_intents()[0].kind().as_str(), "runtime.start_task");
    assert_eq!(runtime.diagnostics().len(), 0);
}
```

Update `CounterReducer` for this variant:

```rust
CounterInput::StartTask => ReducerResult::changed().with_effect(AppEffect::start_task(
    TaskIntentName::new("counter"),
    TaskIntentKey::new("counter:increment"),
    AppScope::app(),
)),
```

- [ ] **Step 2: Run focused test and verify failure**

Run:

```sh
cargo test -p surgeist-runtime runtime_reports_task_intents_without_executing_them
```

Expected before implementation: runtime records an executor-missing diagnostic for task effects or the report has no `task_intents()` accessor.

- [ ] **Step 3: Remove executor-owned fields and methods from runtime**

In `src/runtime.rs`, remove:

```rust
executor: Option<Box<dyn RuntimeExecutor<Input>>>,
tasks: BTreeMap<TaskId, TaskRecord>,
next_task_id: u64,
with_executor
register_task_record
spawn_task
cancel_task
allocate_task_id
drop_stale_task_event
drain_task_input
```

Also remove stale lifecycle report fields and accessors:

```rust
dropped_stale_task_events: usize,
pub const fn dropped_stale_task_events(&self) -> usize
```

In `drain_once`, drain task inputs through `drain_input`:

```rust
let input = self
    .task_queue
    .pop_front()
    .expect("queue was checked before pop");
drained_task_events += 1;
self.drain_input(RuntimeLane::Task, input.into_app_input(), &mut report);
```

Add task intent emission to `RuntimeDrainReport`:

```rust
task_intents: Vec<AppEffect>,

#[must_use]
pub fn task_intents(&self) -> &[AppEffect] {
    &self.task_intents
}
```

Make task effects comparable so `RuntimeDrainReport` can continue deriving `Eq` and `PartialEq`:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppEffect {
    payload: AppEffectPayload,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppEffectPayload {
    ...
}
```

In `execute_effect`, count task intents and append them to the report for root to lower:

```rust
AppEffectPayload::StartTask(_)
| AppEffectPayload::CancelTask(_)
| AppEffectPayload::ReprioritizeTask(_) => {
    report.executed_effects += 1;
    report.task_intents.push(effect.clone());
}
```

- [ ] **Step 4: Remove executor module and exports**

Delete `src/executor.rs`.

In `src/lib.rs`, remove:

```rust
mod executor;
pub use executor::{
    BlockingPolicy, ExecutorError, ExecutorEvent, ExecutorEventPayload, ExecutorTaskHandle,
    FakeExecutor, RuntimeExecutor, SpawnRequest,
};
```

- [ ] **Step 5: Update test harnesses**

In `src/testing.rs`, remove fake executor storage and accessors from `HeadlessHarness`. Construct runtime with:

```rust
let runtime = Runtime::new(state, reducer);
```

Remove `SharedFakeExecutor`.

- [ ] **Step 6: Remove executor-only tests**

Remove tests that validate the old runtime-local executor contract:

- `fake_executor_records_spawn_and_cancel_requests`
- `fake_executor_records_typed_request_input`

- [ ] **Step 7: Run checks**

Run:

```sh
cargo test -p surgeist-runtime runtime_reports_task_intents_without_executing_them
cargo test -p surgeist-runtime runtime_drains_ui_before_task_events_and_respects_budget
cargo test -p surgeist-runtime runtime_task_queue_overflow_records_diagnostic_and_drops_newest
cargo test -p surgeist-runtime
cargo clippy -p surgeist-runtime --all-targets -- -D warnings
cargo fmt --check
```

Expected: all pass.

- [ ] **Step 8: Commit**

```sh
git add src/runtime.rs src/testing.rs src/tests.rs src/lib.rs src/executor.rs
git commit -m "Remove runtime task executor ownership"
```

## Task 4: Remove Runtime-Owned Task Lifecycle Model

**Files:**
- Modify: `src/task.rs`
- Modify: `src/ids.rs`
- Modify: `src/provenance.rs`
- Modify: `src/diagnostic.rs`
- Modify: `src/coord.rs`
- Modify: `src/lib.rs`
- Modify: `src/testing.rs`
- Modify: `src/tests.rs`

- [ ] **Step 1: Write failing test for runtime-owned task provenance**

Add:

```rust
#[test]
fn task_input_uses_runtime_intent_provenance() {
    let provenance = InputProvenance::task(
        TaskIntentId::from_u64(9),
        TaskIntentAttemptId::from_u64(4),
    );
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
```

- [ ] **Step 2: Run focused test and verify failure**

Run:

```sh
cargo test -p surgeist-runtime task_input_uses_runtime_intent_provenance
```

Expected before implementation: `InputProvenance::task` still takes old `TaskId`.

- [ ] **Step 3: Remove old task subsystem types**

In `src/task.rs`, remove:

```rust
TaskStatus
UnobservedPolicy
TaskPriority
TaskPolicy
TaskRegistration
CancellationToken
TaskHandle
TaskRecord
```

Keep:

```rust
TaskIntentName
TaskIntentKey
TaskIntentId
TaskIntentAttemptId
TaskIntentHandle
TaskPriorityHint
```

- [ ] **Step 4: Remove old task ids**

In `src/ids.rs`, remove:

```rust
string_id!(TaskName);
string_id!(TaskKey);
numeric_id!(TaskId);
numeric_id!(TaskAttemptId);
```

- [ ] **Step 5: Update provenance, diagnostics, and coordination**

In `src/provenance.rs`, replace old task ids:

```rust
use super::{CorrelationId, ServiceId, SurfaceId, TaskIntentAttemptId, TaskIntentId};

pub struct TaskProvenance {
    task_id: TaskIntentId,
    task_attempt_id: TaskIntentAttemptId,
    surface_id: Option<SurfaceId>,
}

pub fn task(task_id: TaskIntentId, attempt_id: TaskIntentAttemptId) -> Self
pub fn task_id(&self) -> Option<TaskIntentId>
pub fn task_attempt_id(&self) -> Option<TaskIntentAttemptId>
```

In `src/diagnostic.rs`, replace task diagnostic ids with `TaskIntentId`.

In `src/coord.rs`, use:

```rust
use super::{CustomScopeId, ResourceId, ServiceId, SurfaceId, TaskIntentKey};
```

Keep `SubscriptionTarget::task(key: TaskIntentKey)` only as a runtime-level observer target.

Remove the runtime-owned task progress/coalescing API from `src/coord.rs`:

```rust
CoalescingKey
ProgressEvent
ProgressSlot
CoordinationState::record_progress
CoordinationState::drain_progress_budgeted
```

Keep `CoordinationState` only if it remains useful for subscriptions and observer coordination. Do not preserve task-event queue coalescing, task execution progress semantics, or `surgeist-task` event interpretation in runtime.

- [ ] **Step 6: Update exports**

In `src/lib.rs`, export only:

```rust
pub use task::{
    TaskIntentAttemptId, TaskIntentHandle, TaskIntentId, TaskIntentKey, TaskIntentName,
    TaskPriorityHint,
};
```

- [ ] **Step 7: Remove lifecycle-only tests**

Remove tests that validated duplicate runtime task subsystem behavior:

- `task_registry_records_identity_scope_key_and_policy`
- `task_record_rejects_events_from_stale_attempts`
- `cancellation_status_is_honest_until_terminal_event_arrives`
- `runtime_drops_stale_task_events_with_diagnostics`
- `coordination_coalesces_progress_by_key`
- `prototype_blocking_media_import_reports_cancelling_until_non_abortable_work_finishes`

Remove stale lifecycle diagnostic constants and tests if they are no longer used:

```rust
DiagnosticCode::STALE_TASK_EVENT
```

Update remaining tests and `src/testing.rs` examples to use `TaskIntentId`, `TaskIntentKey`, and `TaskIntentHandle`.

- [ ] **Step 8: Run checks**

Run:

```sh
cargo test -p surgeist-runtime task_input_uses_runtime_intent_provenance
cargo test -p surgeist-runtime app_scope_covers_runtime_ownership_kinds
cargo test -p surgeist-runtime subscriptions_attach_and_detach_observers_without_owning_work
cargo test -p surgeist-runtime
cargo clippy -p surgeist-runtime --all-targets -- -D warnings
cargo fmt --check
```

Expected: all pass.

- [ ] **Step 9: Commit**

```sh
git add src/task.rs src/ids.rs src/provenance.rs src/diagnostic.rs src/coord.rs src/lib.rs src/testing.rs src/tests.rs
git commit -m "Remove runtime task lifecycle model"
```

## Task 5: Remove Task Dependency And Update Documentation

**Files:**
- Modify: `Cargo.toml`
- Modify: `README.md`
- Modify: `src/tests.rs`

- [ ] **Step 1: Write boundary verification test**

Add:

```rust
#[test]
fn crate_identity_remains_runtime_after_task_boundary_cleanup() {
    assert_eq!(crate_name(), "surgeist-runtime");
}
```

- [ ] **Step 2: Remove dependency**

In `Cargo.toml`, dependencies must be:

```toml
[dependencies]
surgeist-retained = { path = "../surgeist-retained", version = "=0.1.0" }
surgeist-window = { path = "../surgeist-window", version = "=0.1.0" }
```

- [ ] **Step 3: Update README**

Replace the role paragraph with:

```markdown
This crate owns app runtime orchestration contracts for Surgeist. It emits
abstract task intents and accepts task-originated app inputs, but concrete task
execution, cancellation, lifecycle, progress coalescing, and Tokio integration
belong to `surgeist-task`. Root `surgeist` owns the adapter that lowers runtime
task intents into task crate requests and maps task events back into runtime
queues.
```

- [ ] **Step 4: Run boundary scan**

Run:

```sh
rg -n "\\b(surgeist_task|RuntimeExecutor|SpawnRequest|ExecutorEvent|ExecutorTaskHandle|ExecutorError|FakeExecutor|TaskPolicy|UnobservedPolicy|BlockingPolicy|CancellationToken|TaskRecord|TaskStatus|TaskName|TaskKey|TaskId|TaskAttemptId|CoalescingKey|ProgressEvent|STALE_TASK_EVENT|dropped_stale_task_events)\\b" Cargo.toml src --glob '!target/**'
```

Expected: no matches. This scan intentionally excludes README prose and the literal crate name `surgeist-task`, because the README should name the external owner of concrete task execution while the runtime source and manifest should not depend on it.

- [ ] **Step 5: Run dependency check**

Run:

```sh
cargo tree -p surgeist-runtime -i surgeist-task
```

Expected: Cargo reports that `surgeist-task` does not match any package in the dependency tree for `surgeist-runtime`, or otherwise shows no reverse path from `surgeist-runtime` to `surgeist-task`. If Cargo exits nonzero only because `surgeist-task` is not present in this package graph, treat that as the expected verification result and record the output in the worker report.

- [ ] **Step 6: Run final checks**

Run:

```sh
cargo test -p surgeist-runtime
cargo clippy -p surgeist-runtime --all-targets -- -D warnings
cargo fmt --check
```

Expected: all pass.

- [ ] **Step 7: Commit**

```sh
git add Cargo.toml README.md src/tests.rs
git commit -m "Document root-owned task adapter boundary"
```

## Task 6: Final Holistic Review

**Files:**
- Review all changed files.
- No planned edits unless review finds issues.
- Plan execution starts after commit `7f54fe0` (`Remove runtime-local tokio adapter`). Use that commit as the base for final implementation diff review unless the coordinator supplies a newer starting commit before execution begins.

- [ ] **Step 1: Inspect status and diff**

Run:

```sh
git status --short --branch
git diff --stat 7f54fe0..HEAD
git diff 7f54fe0..HEAD -- Cargo.toml README.md src
```

- [ ] **Step 2: Verify runtime has no task crate dependency**

Run:

```sh
cargo tree -p surgeist-runtime -i surgeist-task
```

Expected: no dependency path from `surgeist-runtime` to `surgeist-task`. If Cargo exits nonzero only because `surgeist-task` is not present in this package graph, treat that as the expected verification result and record the output in the reviewer report.

- [ ] **Step 3: Verify public API intent**

Run:

```sh
rg -n "TaskIntent|TaskInput|InputProvenance::task|AppEffect::start_task|cancel_task|reprioritize_task|task_intents" src README.md --glob '!target/**'
```

Expected:

- task references are limited to abstract runtime intent/effect/provenance surfaces
- no executor, lifecycle, cancellation-token, task policy, or Tokio references remain

- [ ] **Step 4: Run final checks**

Run:

```sh
cargo test -p surgeist-runtime
cargo clippy -p surgeist-runtime --all-targets -- -D warnings
cargo fmt --check
```

Expected: all pass.

- [ ] **Step 5: Final reviewer handoff**

Ask a clean-context reviewer to inspect the complete result against:

- this plan
- `AGENTS.md`
- crate boundary
- tests
- git diff

The reviewer must report `APPROVED` or `CHANGES_REQUESTED`.

## Completion Criteria

- `surgeist-runtime` no longer depends on `surgeist-task`.
- Runtime no longer exports or implements duplicate task subsystem behavior.
- Runtime still exposes abstract task effects for root to lower.
- Runtime still accepts task-originated inputs through a budgeted task lane.
- Root can own `RuntimeTaskAdapter` without runtime knowing `surgeist-task`.
- Baseline checks pass:

```sh
cargo test -p surgeist-runtime
cargo clippy -p surgeist-runtime --all-targets -- -D warnings
cargo fmt --check
```
