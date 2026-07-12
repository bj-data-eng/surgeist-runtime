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

## Baseline Checks

Run these before handing off crate-local runtime work:

```sh
cargo check -p surgeist-runtime
cargo test -p surgeist-runtime
cargo clippy -p surgeist-runtime --all-targets -- -F unsafe-code -D warnings
cargo fmt --check
```
