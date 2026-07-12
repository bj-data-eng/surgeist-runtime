# C01 Leaf Boundary And Model Foundations Plan

Cycle ID: `C01`

Owning repository: `/Users/codex/Development/surgeist-runtime`

Status: `draft`

Cycle base: `34251095c626923ce7375555b74c67520f83078f`

Reviewed specification: `plans/specs/runtime-review-findings-resolution.md` at
normalized SHA-256
`b7876239cca9dbc12ccac157a897acb9284e9ee726c22c06e672f05277ee4c40`.

Applicable specification sections: S0; S1 dependency exclusions,
bridge/AppHandler removal, AppLoop and test-fixture clauses, and `ids.rs` and
`surface.rs` export rows; S2 except registry-dependent `Runtime` methods; S12
unsafe and MSRV clauses; S13 `UiSurface`-local generation/invalidation overflow;
S14 C01 acceptance coverage.

Reviewed sequence: `plans/sequences/runtime-review-findings-resolution.md` at
normalized SHA-256
`f98521823c097e27166e4e72933fe3047201531dce20e148849ed2f9dd45457b`, entry
`C01 - Leaf Boundary And Model Foundations`.

Bounded outcome: remove concrete retained/window/task integration and production
fixtures; establish runtime-owned local surface values and failure-atomic local
mutation; reduce `AppLoop` to deterministic runtime orchestration; declare Rust
`1.89` and forbid unsafe code.

## Boundary

In scope: leaf manifest metadata/dependencies, `src/` implementation and focused
unit helpers/tests, crate exports, and README ownership/check documentation.

Out of scope: root or sibling writes; root adapters and lowering; generated API
artifacts; `Runtime` surface registry methods; lifecycle/render semantics owned by
C03; reducer/effect/queue/resource/manifest behavior owned by later cycles;
compatibility shims; new features, dependencies, scripts, CI, or generators.

The API may break. Each task uses a fresh clean-context worker and distinct fresh
task reviewer. Canonical implementation, holistic review, landing, publication,
and handoff rules come from `$surgeist-agent` and are not restated here.

## Baseline Evidence

- Leaf `main` and `origin/main` resolve to the cycle base; only this initiative's
  untracked canonical planning packet is present.
- `cargo check --offline --locked -p surgeist-runtime` passes at the cycle base.
- `cargo test --offline --locked -p surgeist-runtime` passes 47 unit tests and 0
  doctests at the cycle base.
- `Cargo.toml` directly depends on `surgeist-retained` and `surgeist-window`;
  source exposes retained/window types, `bridge`, `AppHandler`, and `testing`.
- Root `Cargo.toml` at `a32d078bbc7b841486fcf010a1fef0c8844e5119`
  declares Rust `1.89`. Exact toolchain `1.89` is not installed and must not be
  acquired; S12 requires metadata, configured gates, and source review instead.

## Impacts

- API: intentionally breaking; only the exact C01-owned S1 exports remain or are
  introduced here.
- Dependencies/features: remove retained/window dependencies, add no task or
  other dependency, and add no feature.
- Artifacts: leaf source is authoritative; root-owned generated artifacts are
  unchanged and reported in handoff.
- Docs: update README ownership language and baseline command inventory only.
- MSRV/unsafe: set `rust-version = "1.89"`; add `#![forbid(unsafe_code)]`; reject
  post-1.89 APIs by review; do not acquire a toolchain.
- Root: report removed facade symbols and runtime-owned replacements; do not
  adapt root in this cycle.

## Tasks

### C01-T01 - Runtime-Owned Local Surface Model

Files/area: `src/ids.rs`, `src/surface.rs`, C01-focused portions of
`src/tests.rs`, and `src/lib.rs`; direct compile-caller type substitutions only
in `src/effect.rs`, `src/runtime.rs`, and private `src/testing.rs` helpers.

Intended behavior: implement every S2 runtime-owned identity, geometry,
element/route, error, invalidation, and `UiSurface`-local contract assigned to
C01. Include checked local root replacement/invalidation and exact validation
precedence. Migrate only immediate callers to the new values; do not add effect
disposition behavior, registry-dependent `Runtime` methods, or C03 lifecycle/
render behavior.

RED evidence: first add focused tests for ID/geometry round trips, element phase
registration, one-target route ordering, stale/mismatched local references,
idempotent local field changes, and atomic root-replacement/invalidation overflow;
record the failing compile or assertions before implementation.

Acceptance: the focused tests pass; no retained/window type remains in
`src/ids.rs` or `src/surface.rs`; zero-valued IDs and negative points round-trip;
route and element errors use specified codes/order; every failed local mutation
leaves all observable state unchanged.

Commands: `cargo test --offline --locked -p surgeist-runtime`; `cargo clippy
--offline --locked -p surgeist-runtime --all-targets -- -F unsafe-code -D
warnings`; `cargo fmt --check`.

Dependencies: none beyond the reviewed cycle base.

Intended commit: `feat: add runtime-owned surface foundations`.

### C01-T02 - Concrete Adapter And Fixture Removal

Files/area: `Cargo.toml`, `src/lib.rs`, `src/loop_.rs`, `src/testing.rs`, and
affected C01-focused portions of `src/tests.rs`; delete `src/bridge.rs`; in
`src/coord.rs` and `src/diagnostic.rs`, change only direct window-identity types
required by dependency removal. Touch another `src/` file only when the compiler
proves it is a direct concrete-type caller.

Intended behavior: remove retained/window dependencies and bridge exports; use
runtime `WindowId`, `SurfaceRef`, `SurfaceSize`, and `SurfacePoint` at direct
callers; remove `AppHandler`; make `AppLoop` contain only `Runtime` and delegate
one `step` call to `drain_once`; keep fixture helpers available only under
`#[cfg(test)]` through a private module; add no task dependency or public support
feature.

RED evidence: first add boundary-policy tests for sibling dependencies/exports,
public bridge, private fixtures, and the new `AppLoop` public use, then record
their failures; remove the two manifest dependencies and record the resulting
unresolved-import `cargo check` before implementation.

Acceptance: the package builds without retained/window/task dependencies;
`src/bridge.rs` and all bridge exports are absent; no production source names a
sibling crate; `AppLoop` matches S1 exactly; production metadata/rustdoc exposes
neither fixtures nor `AppHandler`; `src/testing.rs` is compiled only through
private `#[cfg(test)] mod testing`, with no production module or reexport.

Commands: `cargo test --offline --locked -p surgeist-runtime`; `cargo clippy
--offline --locked -p surgeist-runtime --all-targets -- -F unsafe-code -D
warnings`; `cargo fmt --check`; `! rg -n
'surgeist_(retained|window|task)|surgeist-(retained|window|task)' Cargo.toml src`;
`! rg -n 'AppHandler|RetainedBridge|pub mod testing|pub use testing' src`.

Dependencies: `TASK_CLEAN` for the committed `C01-T01` range.

Intended commit: `refactor: remove concrete runtime adapters`.

### C01-T03 - Safety, MSRV, And Boundary Evidence

Files/area: `Cargo.toml`, `src/lib.rs`, `README.md`, and C01 boundary-policy tests
in `src/tests.rs`.

Intended behavior: declare Rust `1.89`, forbid unsafe code at the crate root,
record the complete configured baseline commands in README, and add deterministic
tests/evidence for the C01 S14 boundary names or documented equivalents.

RED evidence: add boundary-policy tests for manifest MSRV and crate unsafe
prohibition before changing those declarations; record their failures.

Acceptance: metadata reports `rust_version: "1.89"`; the crate-level unsafe
prohibition is present; README lists the exact AGENTS command inventory and keeps
root-owned adapter lowering explicit; boundary scans have only the allowed README
ownership references; no language/library API newer than 1.89 is introduced.

Commands: `cargo metadata --offline --locked --no-deps --format-version 1`;
`cargo check --offline --locked -p surgeist-runtime`; `cargo test --offline
--locked -p surgeist-runtime`; `cargo test --offline --locked -p
surgeist-runtime --doc`; `cargo clippy --offline --locked -p surgeist-runtime
--all-targets -- -F unsafe-code -D warnings`; `cargo fmt --check`.

Dependencies: `TASK_CLEAN` for the committed `C01-T02` range.

Intended commit: `chore: enforce runtime crate safety baseline`.

## Completion

Cycle acceptance requires every task's RED/GREEN evidence and exact-range CLEAN
task review, all task commits in order above, no C02 behavior implemented early,
and these exact final commands on the resulting `main`:

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
`! rg -n 'AppHandler|RetainedBridge|pub mod testing|pub use testing' src`.

After task reviews are CLEAN, change this plan to `complete` in a status-only
commit, run the entire final command set, require metadata Rust `1.89` and a clean
worktree, then obtain a fresh CLEAN holistic review of `cycle_base..cycle_head`.
Rerun the entire final set at the exact reviewed head and again after landing on
local `main`; each run must be fresh, clean, and source-preserving. Then publish,
read remote `main` back, prove local/tracking/observed SHAs agree, and hand root
the immutable candidate SHA, API/dependency replacements, MSRV policy, command
evidence, and deferred adapter work. Failures and blockers follow the canonical
correction, landing, and blocker contracts.
