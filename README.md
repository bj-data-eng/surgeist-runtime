# surgeist-runtime

This crate owns app runtime orchestration contracts for Surgeist. It emits
abstract task intents and accepts task-originated app inputs, but concrete task
execution, cancellation, lifecycle, progress coalescing, and Tokio integration
belong to `surgeist-task`. Root `surgeist` owns the adapter that lowers runtime
task intents into task crate requests and maps task events back into runtime
queues.

Root `surgeist` owns integration with concrete Surgeist crates such as template,
CSS, style, retained, text, layout, render, window, and task. Keep parser,
style-resolution, layout-algorithm, text-shaping, rendering-backend, retained
tree, and host implementation details out of this crate.

## Ownership Example

```rust
use surgeist_runtime::{
    AppEffect, AppInput, AppScope, EffectDisposition, InputProvenance, Reducer, ReducerCommit,
    ReducerResult, ResourceId, Runtime, RuntimeBudget, RuntimeIntent, ServiceId, TaskIntentKey,
    TaskIntentName, UiInput,
};

struct IntentReducer;

impl Reducer<(), ()> for IntentReducer {
    fn reduce(&mut self, _: &(), _: &AppInput<()>) -> ReducerResult<()> {
        ReducerResult::unchanged(
            ReducerCommit::new()
                .with_effect(AppEffect::start_task(
                    TaskIntentName::new("thumbnail"),
                    TaskIntentKey::new("photo:42"),
                    AppScope::app(),
                ))
                .with_effect(AppEffect::invalidate_resource(
                    ResourceId::new("photo:42"),
                    "source changed",
                ))
                .with_effect(AppEffect::start_service(ServiceId::new("indexer"))),
        )
    }
}

let mut runtime = Runtime::new((), IntentReducer);
runtime.enqueue_ui(UiInput::new((), InputProvenance::system())?)?;
let outcomes = runtime.drain_once(RuntimeBudget::default())?;

assert_eq!(outcomes.forwarded_effects(), 3);
assert!(outcomes
    .effect_outcomes()
    .iter()
    .all(|outcome| outcome.disposition() == EffectDisposition::Forwarded));
assert!(matches!(outcomes.intents()[0], RuntimeIntent::StartTask(_)));
assert!(matches!(outcomes.intents()[1], RuntimeIntent::InvalidateResource(_)));
assert!(matches!(outcomes.intents()[2], RuntimeIntent::StartService(_)));
# Ok::<(), Box<dyn std::error::Error>>(())
```

Runtime emits these abstract task, resource, and service intents only. Root
`surgeist` lowers them through concrete adapters into sibling crates, then returns
their resulting inputs through the runtime task and service queues.

## Baseline Checks

Run these before handing off crate-local runtime work:

```sh
cargo check -p surgeist-runtime
cargo test -p surgeist-runtime
cargo clippy -p surgeist-runtime --all-targets -- -F unsafe-code -D warnings
cargo fmt --check
```
