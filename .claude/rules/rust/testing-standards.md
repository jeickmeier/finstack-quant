---
trigger: model_decision
description: Rust testing framework for the rust finstack library.
globs:
---
### Rust Testing Standards (Finstack)

These standards apply to all Rust crates in the Finstack workspace (`finstack/core`, `statements`, `valuations`, `scenarios`, `portfolio`, `io`) and should be read alongside our code standards. They reinforce our core invariants: determinism, currency‑safety, stable serde schemas, and parity across bindings.

### Guiding principles

- **Deterministic and reproducible**: Tests must not depend on wall‑clock time, locale, or network. Randomness must be seeded. Serial and parallel execution must produce identical Decimal results.
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
  - Ensure results capture our policy stamps (numeric mode, parallel flag, rounding context; FX policy where relevant).
- **Golden/parity tests**
  - Validate against market‑standard sources (e.g., Bloomberg, QuantLib, FinancePy) and standards bodies (e.g., ISDA) when legally and practically feasible.
  - Store canonical inputs plus expected outputs (CSV/JSON/Arrow) under `tests/data/` with provenance notes.
  - Assert values within documented tolerances; Decimal paths should match exactly unless rounding policy dictates otherwise.
- **Doc tests**
  - Include only when they provide a useful, copy‑pasteable working example of a public API and ensure it compiles.
  - Do not use doc tests for simple assertions or internal behaviors—place those in unit/integration tests instead.

### Determinism requirements

- **Fixed seeds** for any randomized logic; prefer explicit seed injection in APIs used in tests.
- **Clock isolation**: Use explicit dates/calendars from `core/dates`; never read system time.
- **Stable ordering**: When comparing collections/frames, enforce stable sort and strict schemas before assertions.
- **Parallel parity**: Where parallel features exist, include tests that assert serial ≡ parallel Decimal results and stamp the parallel flag consistently in metadata.

### Coverage policy

- **Goal**: ≥80% per crate; near‑100% on thin public API layers and critical safety code (currency checks, serde, rounding contexts).
- **Exclusions**: Boilerplate error/display impls, defensive branches that cannot be triggered without UB, and platform shims may be reasonably under‑covered.
- **Measurement**: Use project coverage tooling (e.g., `mise run rust-test-cov` / `test-python-cov` / `test-wasm-cov`) and watch trend lines; coverage should not regress meaningfully without justification.

### Assertions and tolerances

- **Decimal mode**: Expect exact equality unless rounding policy explicitly allows tolerance.
- **Float mode/testing against external sources**: Use tight, documented tolerances appropriate for the instrument/domain and data granularity.
- **Policy visibility**: Assert presence and correctness of `RoundingContext`, FX policy stamps, and numeric mode in result envelopes where applicable.

### Test data and fixtures

- Place small fixtures under `tests/data/` within each crate. Prefer CSV/JSON/Arrow for tabular/time‑series data.
- Document fixture provenance, units, currencies, calendars, and rounding assumptions in a short README alongside data when non‑obvious.
- Keep fixtures minimal but representative; avoid embedding large blobs in source.

### Structure, naming, and style

- **Unit tests**: colocate with code under `mod tests { … }`; name tests descriptively (behavior‑oriented, not ticket IDs).
- **Integration tests**: under `crate/tests/`; one file per scenario or cohesive feature area; mirror user workflows.
- **Golden tests**: suffix files or directories with the source (e.g., `*_bloomberg.rs`, `*_quantlib.rs`) and include data provenance notes.
- **Helpers**: Common test helpers live in `tests/common.rs` or a dedicated `tests/common/` module; avoid coupling helpers to private internals.

### Performance and benchmarks

- Keep performance checks in Criterion benches under `benches/`. Benchmarks must not gate CI by default.
- Use realistic but small inputs to keep benches and tests fast; document scaling assumptions.

### CI and local workflow

- Run `mise run all-lint` and `mise run rust-test` before committing. Fix failures rather than adding ignores.
- Tests must be hermetic: no network, no external services, no reliance on machine configuration.
- If enabling new features behind flags (e.g., parallel), ensure CI executes representative test matrices.

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
