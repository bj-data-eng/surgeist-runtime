# surgeist-runtime

App runtime orchestration contracts for Surgeist.

This crate owns the bounded runtime plane for Surgeist apps: app lifecycle
coordination, input and effect scheduling contracts, resource lifecycle policy
hooks, invalidation scheduling, animation/frame scheduling, diagnostics, and the
coordination boundary between app events and pipeline reruns.

Root `surgeist` owns integration with concrete Surgeist crates such as template,
CSS, style, retained, text, layout, render, window, and task. Keep parser,
style-resolution, layout-algorithm, text-shaping, rendering-backend, retained
tree, and host implementation details out of this crate.

## Baseline Checks

Run these before handing off crate-local runtime work:

```sh
cargo test -p surgeist-runtime
cargo clippy -p surgeist-runtime --all-targets -- -D warnings
cargo fmt --check
```
