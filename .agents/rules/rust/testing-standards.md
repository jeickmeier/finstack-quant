---
trigger: model_decision
description: Rust testing framework for the rust finstack-quant library.
globs:
---
### Rust Testing Standards (Finstack)

These standards apply to all Rust crates in the Finstack Quant workspace (`finstack-quant/core` through `portfolio`; see the crate map in `ARCHITECTURE.md`) and should be read alongside our code standards. They reinforce our core invariants: determinism, currency‑safety, stable serde schemas, and parity across bindings.

### Guiding principles

- **Deterministic and reproducible**: Tests must not depend on wall‑clock time, locale, or network. Randomness must be seeded. Within each API's declared reproducibility tier ([ADR 0021](../../../docs/decisions/standards/0021-reproducibility-guarantees.md)), serial and parallel execution must agree, and `Decimal`/`Money` results must match exactly.
- **Public API first**: Prioritize coverage on public, user‑facing APIs; internal details are covered as needed to protect invariants.
- **Pragmatic coverage**: Aim for **80%+** line/branch coverage overall. Do not chase 100% if it materially increases complexity or brittles tests.
- **Currency safety and serde stability**: Assert currency‑safe behaviors and strict deserialization with denied unknown fields where applicable.
- **Performance awareness without flakiness**: Keep perf checks in benchmarks; avoid time‑based assertions in tests.
- **Fix the root causes**: Don't relax tolerances, fix the root causes of the tests errors.

### Test types and when to use them

- **Unit tests (`#[cfg(test)]` in modules)**
  - Validate small, isolated behaviors and edge cases.
  - Prefer property‑based tests for numeric invariants where helpful (e.g., monotonicity, conservation).
  - Avoid unnecessary I/O; keep fast.
- **Integration tests (`tests/` per crate)**
  - Treat as example‑equivalent end‑to‑end scenarios that exercise the public surface area.
  - Build objects via public constructors/builders; do not reach into private internals.
  - Where a result carries `ResultsMeta`, assert its stamps: `numeric_mode` (always `"f64"` today), `rounding`, `parallel`, and `fx_policy_applied` where relevant.
- **Golden/parity tests**
  - Validate against market‑standard sources (e.g., Bloomberg, QuantLib, FinancePy) and standards bodies (e.g., ISDA) when legally and practically feasible.
  - Store canonical inputs plus expected outputs (CSV/JSON/Arrow) in the crate's `tests/data/` or `tests/golden/` directory, or in a fixture shared with the Python/WASM suites. Each case records provenance, methodology and version, valuation date, conventions, units, and tolerance rationale ([ADR 0048](../../../docs/decisions/project/0048-reference-provenance-and-version-pinning.md)).
  - Assert values within documented tolerances; Decimal paths should match exactly unless rounding policy dictates otherwise.
- **Doc tests**
  - Include only when they provide a useful, copy‑pasteable working example of a public API and ensure it compiles.
  - Do not use doc tests for simple assertions or internal behaviors—place those in unit/integration tests instead.

### Determinism requirements

- **Fixed seeds** for any randomized logic; prefer explicit seed injection in APIs used in tests.
- **Clock isolation**: Use explicit dates/calendars from `core/dates`; never read system time.
- **Stable ordering**: When comparing collections/frames, enforce stable sort and strict schemas before assertions.
- **Parallel parity**: Where an API promises thread-count independence, compare serial and parallel runs for each promised thread-count class, and check the `ResultsMeta.parallel` stamp where the result carries it. Compare `f64::to_bits()` for bit-identity claims and use justified tolerances otherwise ([ADR 0042](../../../docs/decisions/standards/0042-numerical-accumulation-and-comparisons.md)); single-target tests do not establish cross-target bit identity.

### Coverage policy

- **Goal**: ≥80% per crate; near‑100% on thin public API layers and critical safety code (currency checks, serde, rounding contexts).
- **Exclusions**: Boilerplate error/display impls, defensive branches that cannot be triggered without UB, and platform shims may be reasonably under‑covered.
- **Measurement**: Use project coverage tooling (`mise run rust-test --coverage` / `mise run python-test --all --coverage`) and watch trend lines; coverage should not regress meaningfully without justification.

### Assertions and tolerances

- **`Decimal`/`Money` paths**: Expect exact equality unless a rounding policy explicitly allows tolerance. Use exact `f64` equality only for intended IEEE semantics such as exact-zero sentinels.
- **`f64` results and external sources**: Use tight, documented tolerances appropriate for the instrument/domain and data granularity.
- **Policy visibility**: Assert presence and correctness of the `ResultsMeta` fields (`rounding`, `fx_policy_applied`, `numeric_mode`) in result envelopes where applicable.

### Test data and fixtures

- Place small fixtures under the crate's `tests/data/` or `tests/golden/` directory, or share one fixture between host suites (location does not establish independence of the expected values). Prefer CSV/JSON/Arrow for tabular/time‑series data.
- Document fixture provenance, units, currencies, calendars, and rounding assumptions in a short README alongside data when non‑obvious.
- Keep fixtures minimal but representative; avoid embedding large blobs in source.

### Structure, naming, and style

- **Unit tests**: a `#[cfg(test)] mod tests`, inline or as the sibling file `tests.rs`; no other sibling name (enforced by `scripts/check_lint_suppressions.py`). Name tests descriptively (behavior‑oriented, not ticket IDs). Integration tests live under the crate's `tests/`.
- **Integration tests**: under `crate/tests/`; one file per scenario or cohesive feature area; mirror user workflows.
- **Golden tests**: suffix files or directories with the source (e.g., `*_bloomberg.rs`, `*_quantlib.rs`) and include data provenance notes.
- **Helpers**: Common test helpers live in `tests/common.rs` or a dedicated `tests/common/` module; avoid coupling helpers to private internals.

### Performance and benchmarks

- Keep performance checks in Criterion benches under `benches/`. Benchmarks must not gate CI by default.
- Use realistic but small inputs to keep benches and tests fast; document scaling assumptions.

### CI and local workflow

- **While iterating:** lint and test the affected crate only:
  - `mise run rust-lint-crate -- <package>`
  - `mise run rust-test-crate -- <package>` / `rust-test-integration` / `rust-test-filter`
  - `mise run rust-bench-crate -- <package> <bench>` for performance work
  - See `.agents/rules/selective-test-running.mdc` for the path → package table.
- **Before commit or at plan end:** `mise run rust-lint` and `mise run rust-test` (full workspace).
- Tests must be hermetic: no network, no external services, no reliance on machine configuration.
- This workspace has no `parallel` feature; vary thread counts and compilation targets (native vs wasm32) in the relevant tests rather than adding feature gates, and keep CI covering each promised configuration.
- Do not run `cargo test` directly; use the `mise run rust-test-*` tasks (nextest, no doc tests).

### Integration test guidance (example‑equivalent rule)

- Think of each integration test as an example script a user could read and replicate to understand the feature set.
- Prefer end‑to‑end construction → evaluation → assertion flow, using only public APIs and standard configuration points.
- Where outputs are large (DataFrames), validate schemas, metadata, ordering, and key aggregates rather than every cell unless golden parity is the goal.

### Golden/parity test guidance

- Prefer file‑backed expectations over ad‑hoc textual snapshots; keep expectations in stable, machine‑readable formats.
- Record rounding, calendars, day‑count conventions, currencies, and pricing conventions with the data.
- When external sources change (e.g., vendor updates), update fixtures and document the change rationale in the test comments/README.

### Doctest guidance

- Keep examples minimal, idiomatic, and build‑only when heavy computations are involved.
- Use doctests to showcase public API ergonomics and typical usage patterns; link to deeper integration tests when relevant.

### Exceptions

- Deviations from these standards require a brief justification in the PR description and, where applicable, a TODO to bring tests in line later.
