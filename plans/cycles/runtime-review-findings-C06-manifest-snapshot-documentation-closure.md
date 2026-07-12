# C06 Manifest, Snapshot, Documentation, And Initiative Closure Plan

Cycle ID: `C06`

Owning repository: `/Users/codex/Development/surgeist-runtime`

Status: `complete`

Cycle base: `e2fb9ea48bd2e47005fa09b519986dca6d233711`

Reviewed specification: `plans/specs/runtime-review-findings-resolution.md` at
normalized SHA-256
`060185a41f9b7f1505bb7db7717a52acbe983c6a1f22f637b528d7e478a1af11`.

Applicable specification sections: S1 `command.rs`/`event.rs`, `descriptor.rs`, and
`snapshot.rs` export rows; S11; S12 public errors/docs/examples and final unsafe/MSRV
verification; S14 remaining and integrated coverage; S15.

Reviewed sequence: `plans/sequences/runtime-review-findings-resolution.md` at
normalized SHA-256
`67393b975028cde627e495468925f050f1e16f7b7441e0a2af2f769429b72610`, entry
`C06 - Manifest, Snapshot, Documentation, And Initiative Closure`.

Bounded outcome: make authored manifests and snapshots cross a validated boundary,
align the leaf to root-authorized Rust `1.97`, complete the public error/docs
contract, and prove the finite initiative checklist against the integrated crate.

## Boundary

In scope: leaf MSRV metadata, validated names/descriptors, manifest
validation/indexes and `App` ownership, manifest-bound snapshot construction and
entries, exact C06-owned reexports, all remaining public error traits/accessors and
non-exhaustive classifications, missing Rustdoc, representative compile-checked
examples, README ownership example, and S14/S15 integrated acceptance evidence.

Out of scope: compatibility shims; new dependencies/features/scripts/generators/CI;
root-owned API artifacts; concrete task execution, codecs, schemas, adapters, or
host wake bridges; root or sibling writes/messages; and redesign of C01-C05 behavior.

Root communication remains on hold. Retain the final candidate SHA, API/dependency
delta, verification evidence, and root adapter/`WakeBridge` obligations locally.
Each task uses a fresh worker who stops after one logical commit and returns exact
RED/GREEN/span evidence without review, later tasks, finalization, landing, or
publication; a fresh task reviewer and the canonical final gates remain required.

## Baseline Evidence

- C01-C05 are published and read back; local `main`, tracking `origin/main`, and
  observed remote `main` equal the clean C06 base.
- `CommandName`, `EventName`, descriptor payload strings, manifest append methods,
  and snapshot text are currently unchecked; `App` stores only `AppDescriptor`.
- `AppSnapshot::new` has no root/declaration binding or value-entry path.
- The C06 base forbids unsafe and declares historical Rust `1.89`, but lacks
  `#![warn(missing_docs)]`, the S12 representative doctests, and standard behavior
  for every named public error.
- During T05, root authority and installed `rustc` advanced to `1.97.0`; the user
  authorized leaf alignment, with no acquisition or root mutation by this cycle.

## Impacts

- API: intentionally breaking constructors and final exact S1 reexports; authored
  `AppManifest` is consumed into immutable `ValidatedAppManifest` owned by `App`.
- Dependencies/features/artifacts: unchanged; generated API artifacts stay root-owned.
- Docs/examples: complete all exported-item Rustdoc, six S12 public doctest contracts,
  and one concise README ownership example.
- MSRV/unsafe: advance `rust-version` to `1.97`, reject post-1.97 contracts, keep
  `#![forbid(unsafe_code)]`, and add the missing-doc lint.
- Root: retain facade/adapter and concrete deferred-wake test obligations locally;
  send no message.

## Tasks

### C06-T01 - Validate Authored Names And Descriptor Payload Types

- Files/area: `src/command.rs`, `src/event.rs`, descriptor payload fields in
`src/descriptor.rs`, exact `src/lib.rs` reexports, direct callers/fixtures, focused
tests, and changed public Rustdoc.
- Intended behavior: add exact `PayloadTypeName` and field-aware `NameError`; make
command/event names and all descriptor payload/input/value type text use the S11
fallible constructors and preserve accepted text exactly. Remove public unchecked
string constructors; retain semantic runtime names rather than Rust reflection.
- RED evidence: first test empty, whitespace-only, and ASCII-control rejection for
each required field; assert exact field reporting, accepted-text preservation, and
successful construction for command, event, task, and resource descriptors; record
the expected failure before implementation.
- Acceptance: invalid descriptor text is unconstructable through public APIs;
`NameError` has private state, stable semantic accessors, `Display`/`Error`, and no
unchecked compatibility path; existing direct callers use validated values.
- Commands: `cargo test --offline --locked -p surgeist-runtime`; `cargo test
--offline --locked -p surgeist-runtime --doc`; `cargo clippy --offline --locked
-p surgeist-runtime --all-targets -- -F unsafe-code -D warnings`; `cargo fmt --check`.
- Dependencies: none beyond the reviewed C06 base.
- Intended commit: `feat: validate runtime descriptor names`.

### C06-T02 - Consume Authored Manifests Into Validated App State

- Files/area: `src/descriptor.rs`, exact `src/lib.rs` reexports, direct callers and
fixtures, focused manifest tests, and changed public Rustdoc.
- Intended behavior: implement consuming `AppManifest::validate`, private immutable
deterministic indexes in `ValidatedAppManifest`, aggregate ordered issues/codes and
semantic accessors, and `App::try_new` owning exactly one validated manifest.
Validation covers every S11 duplicate, root command/event declaration and payload
mismatch, startup reference/allowance/required-root, and root binding case.
- RED evidence: first test successful deterministic lookup/iteration plus every S11
error code, aggregate issue ordering, and populated root/window/name/payload/binding
context; record failures before implementation.
- Acceptance: unchecked builders cannot be mistaken for apps; validation consumes the
builder, reports all issues deterministically without partial success, and exposes
only immutable lookup/iteration; `App::descriptor()` views `manifest().app()`.
- Commands: `cargo test --offline --locked -p surgeist-runtime`; `cargo test
--offline --locked -p surgeist-runtime --doc`; `cargo clippy --offline --locked
-p surgeist-runtime --all-targets -- -F unsafe-code -D warnings`; `cargo fmt --check`.
- Dependencies: `TASK_CLEAN` for T01 so descriptor identities are validated.
- Intended commit: `feat: validate app manifests`.

### C06-T03 - Bind Snapshot Values To Validated Root Declarations

- Files/area: `src/snapshot.rs`, manifest/App snapshot entry points in
`src/descriptor.rs`, exact `src/lib.rs` reexports, direct callers, focused tests,
and changed public Rustdoc.
- Intended behavior: implement validated binding IDs/source types, opaque values,
entries, private copied declarations, and typed snapshot errors. Only `App` or a
validated manifest can create a root-bound snapshot; entry addition follows S11's
undeclared, mismatch, then duplicate precedence and is failure-atomic.
- RED evidence: first test every text rejection/field, unknown root, valid declaration
copy and entry, undeclared binding, expected/actual type mismatch, duplicate entry,
error context, precedence, and unchanged entries after failure; record failures.
- Acceptance: no public raw snapshot/declaration mutator remains; accepted text is
preserved exactly; snapshots expose their root/version/declarations/entries without
schema interpretation; all failures have stable accessors and `Display`/`Error`.
- Commands: `cargo test --offline --locked -p surgeist-runtime`; `cargo test
--offline --locked -p surgeist-runtime --doc`; `cargo clippy --offline --locked
-p surgeist-runtime --all-targets -- -F unsafe-code -D warnings`; `cargo fmt --check`.
- Dependencies: `TASK_CLEAN` for T02 so snapshot creation uses final validated indexes.
- Intended commit: `feat: bind snapshots to app manifests`.

### C06-T04 - Close Public Error And Missing-Documentation Contracts

- Files/area: all production `src/*.rs`, especially S12 named errors/state machines,
`src/lib.rs`, focused compile-time/runtime tests, and Rustdoc.
- Intended behavior: add `#![warn(missing_docs)]`; document every exported item and
public state transition; make every S12 error implement its required standard traits,
private-state accessors, generic bounds, and non-exhaustive classification without
changing unrelated C01-C05 semantics.
- RED evidence: first add focused generic trait/accessor/code tests and run documentation
with warnings denied to expose missing items; record expected failures before fixes.
- Acceptance: the exact S12 error list satisfies `Display`/`Error`; all error-semantic
enums are non-exhaustive; no broad lint allowance is added; warnings-denied docs and
strict Clippy pass under the Rust `1.97` contract.
- Commands: `cargo test --offline --locked -p surgeist-runtime`; `RUSTDOCFLAGS="-D
warnings" cargo doc --offline --locked -p surgeist-runtime --no-deps`; `cargo clippy
--offline --locked -p surgeist-runtime --all-targets -- -F unsafe-code -D warnings`;
`cargo fmt --check`.
- Dependencies: `TASK_CLEAN` for T03 so documentation covers the final API.
- Intended commit: `docs: complete runtime public contracts`.

### C06-T05 - Compile Public Examples And Prove Initiative Acceptance

- Files/area: `Cargo.toml`, representative Rustdoc in `src/*.rs`, `README.md`, exact
`src/lib.rs`, integrated `src/tests.rs`, and final source/manifest audits.
- Intended behavior: add all six S12 public-only success/error doctest contracts and
the README ownership example, advance leaf MSRV/test evidence to `1.97`, close every
S14 equivalent, and audit each finding against behavior, tests, docs, exports, and
boundaries.
- RED evidence: record the absent six-doctest baseline; first update the focused MSRV
test to `1.97` and prove it fails against the `1.89` manifest before changing metadata.
- Acceptance: all six example categories compile and assert their specified success or
error path; README assigns concrete lowering to root; metadata/compiler report `1.97`;
every S14/S15 item passes; no root/sibling implementation or test helper leaks in.
- Commands: `rustc --version --verbose`; `cargo metadata --offline --locked --no-deps
--format-version 1`; `cargo test --offline --locked -p surgeist-runtime`; `cargo test
--offline --locked -p surgeist-runtime --doc`; `RUSTDOCFLAGS="-D warnings" cargo doc
--offline --locked -p surgeist-runtime --no-deps`; `cargo clippy --offline --locked
-p surgeist-runtime --all-targets -- -F unsafe-code -D warnings`; `cargo fmt --check`.
- Dependencies: `TASK_CLEAN` for T04 so examples target the final documented surface.
- Intended commit: `docs: prove runtime initiative acceptance`.

## Completion

After all tasks are `TASK_CLEAN`, make the status-only `complete` commit and run:
`rustc --version --verbose`;
`cargo metadata --offline --locked --no-deps --format-version 1`;
`cargo check --offline --locked -p surgeist-runtime`;
`cargo test --offline --locked -p surgeist-runtime`;
`cargo test --offline --locked -p surgeist-runtime --doc`;
`RUSTDOCFLAGS="-D warnings" cargo doc --offline --locked -p surgeist-runtime --no-deps`;
`cargo clippy --offline --locked -p surgeist-runtime --all-targets -- -F unsafe-code -D warnings`;
`cargo fmt --check`; `git ls-files -co --exclude-standard -- '*.rs'`;
`! rg -n --pcre2 '#\s*\[\s*(?:unsafe\s*\(|no_mangle\b|export_name\b)|\bunsafe\s*(?:\{|fn\b|trait\b|impl\b|extern\b)|\bstatic\s+mut\b|\bextern\s*(?:"[^"]*")?\s*\{' $(git ls-files -co --exclude-standard -- '*.rs')`;
`! rg -n 'surgeist_(retained|window|task)|surgeist-(retained|window|task)' Cargo.toml src`;
`! rg -n '#!?\[(?:allow|expect)\([^]]*(?:unsafe_code|missing_docs)' src`.

Compiler/metadata must report Rust `1.97`, no dependencies, and default-only features.
Run the final set, obtain a fresh holistic `CLEAN` review of exact
`cycle_base..cycle_head`, rerun the set at that reviewed head and on local `main`, then
publish/read back C06. Retain root evidence locally and send no message. Finally run
the user's additional clean-context C01-C06 initiative acceptance review; it does not
replace C06's canonical holistic gate. Both reviews and matching main refs complete the goal.
