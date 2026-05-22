# Consistency Review: Rust Crates

## Summary

Found 12 findings (0 blocker, 4 major, 6 minor, 2 nit) across all 11 workspace crates.

The workspace is remarkably consistent in its lint preamble, builder entry-point convention, and `thiserror` usage. The deviations below are mostly incremental drift from growth, not systemic design disagreements.

---

## Findings

### [MAJOR] Error: `#[non_exhaustive]` missing on sub-crate error enums

**Where:**
- `core/src/credit/migration/error.rs` — `MigrationError`
- `core/src/credit/pd/error.rs` — `PdCalibrationError`
- `core/src/credit/scoring/types.rs` — `CreditScoringError`
- `core/src/math/linalg.rs` — `CholeskyError`
- `core/src/math/time_grid.rs` — `TimeGridError` (struct, not enum)
- `valuations/src/instruments/common/models/pde/grid.rs` — `PdeGridError`
- `valuations/src/instruments/common/models/pde/stepper.rs` — `StepperError`
- `valuations/src/instruments/common/models/pde/operator.rs` — `ThomasError`
- `valuations/src/instruments/common/models/pde/solver.rs` — `PdeSolverError`
- `valuations/src/instruments/common/models/pde/solver2d.rs` — `PdeSolver2DError`

**Pattern A (dominant):** All crate-level `Error` enums (`core::Error`, `portfolio::Error`, `scenarios::Error`, `statements::Error`, `valuations::Error`, `correlation::Error`, `PricingError`) use `#[non_exhaustive]`.

**Pattern B (deviation):** Internal/sub-module error enums in core credit, core math, and valuations PDE modules omit `#[non_exhaustive]`.

**Recommendation:** Add `#[non_exhaustive]` to all public error enums. Although these are "internal" errors, they are still `pub` and cross module boundaries. This prevents downstream matches from breaking on new variants.

---

### [MAJOR] Error derives: Missing `serde::Serialize`/`serde::Deserialize` on some public errors

**Where:**
- `core/src/credit/migration/error.rs` — `#[derive(Debug, Clone, PartialEq, Error)]`
- `core/src/credit/pd/error.rs` — `#[derive(Debug, Clone, PartialEq, Error)]`
- `core/src/credit/scoring/types.rs` — `#[derive(Debug, Clone, PartialEq, Error)]`
- `core/src/math/linalg.rs` — `#[derive(Debug, Clone, PartialEq, Error)]`
- `core/src/math/time_grid.rs` — `#[derive(Debug, Error)]` (also missing `Clone`, `PartialEq`)
- `valuations/…/pde/*.rs` — `#[derive(Debug, Clone, thiserror::Error)]` (missing `PartialEq`, `Serialize`, `Deserialize`)

**Pattern A (dominant):** All wire-facing error enums derive `Debug, Clone, PartialEq, thiserror::Error, serde::Serialize, serde::Deserialize` (per documented derive policy).

**Pattern B (deviation):** Sub-module errors omit `Serialize`/`Deserialize` and sometimes `PartialEq`.

**Recommendation:** The derive policy states errors that *may* cross FFI boundaries should have serde. Sub-module errors in `core::credit` flow through the `Core` variant into binding layers. Add the full derive set for consistency, or document the explicit opt-out per the derive policy. PDE errors in valuations don't cross bindings currently (acceptable opt-out) but should still get `PartialEq` for test ergonomics.

---

### [MAJOR] Error handling: `analytics` and `cashflows` use `pub(crate)` or private Result alias, not a public re-export

**Where:**
- `analytics/src/lib.rs:51` — `type Result<T> = finstack_core::Result<T>;` (private, not `pub`)
- `cashflows/src/lib.rs` — no `Result` alias at all; functions return `finstack_core::Result<T>` directly

**Pattern A (dominant):** Crates with their own error enum (`portfolio`, `scenarios`, `statements`, `valuations`) re-export `pub use error::{Error, Result};` from `lib.rs`.

**Pattern B:** Crates that don't define their own error type (`analytics`, `cashflows`, `margin`, `monte_carlo`, `statements-analytics`) use `finstack_core::Result<T>` directly or have a private alias.

**Recommendation:** This split is *intentional* (documented in conventions.md) — not all crates need their own error type. However, the `analytics` private alias `type Result<T> = finstack_core::Result<T>;` should be consistent with `cashflows`/`margin`/`monte_carlo` which simply spell out `finstack_core::Result<T>` at call sites. Either make the alias `pub` and re-export it, or remove it and spell it out. A private alias imported as `use crate::Result` complicates grep-based auditing.

---

### [MAJOR] Builder `#[must_use]`: Inconsistent presence on `pub fn builder()` methods

**Where:**
- **Has `#[must_use]`:** `Portfolio::builder`, `ScenarioSpec::builder`, `DieboldLi::builder`, `Waterfall::builder`
- **Missing `#[must_use]`:** `DiscountCurve::builder`, `HazardCurve::builder`, `ForwardCurve::builder`, `VolSurface::builder`, `DividendSchedule::builder`, `CashFlowSchedule::builder`, `McEngine::builder`, `Tranche::builder`, all valuations instrument builders via `#[derive(FinancialBuilder)]`

**Pattern A:** `#[must_use]` on builder entry points that return an intermediate type that is useless until `.build()` is called.

**Pattern B:** Most builder entry points omit `#[must_use]`.

**Recommendation:** Add `#[must_use]` to all `pub fn builder(…)` methods across term structures, instruments, and engines. This is a mechanical, zero-risk change that catches "builder() constructed but never built" bugs at compile time. Priority: at least the `core` term-structure builders (discount, hazard, forward, vol surface, inflation) because they are most heavily used.

---

### [MINOR] Lint preamble: `test-utils` crate is missing the standard lint set

**Where:** `test-utils/src/lib.rs` — only has `#![forbid(unsafe_code)]`

**Pattern A (all other 12 crates):** Full standard preamble:
```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::new_without_default)]
#![warn(clippy::float_cmp)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![cfg_attr(test, allow(…))]
#![doc(test(attr(allow(clippy::expect_used))))]
```

**Pattern B:** `test-utils` only has `#![forbid(unsafe_code)]`.

**Recommendation:** Add the full lint set. Even though this is a test-support crate, it's a library crate compiled normally (not under `#[cfg(test)]`). Its code should meet the same standards.

---

### [MINOR] Builder entry point: `CashFlowSchedule::builder()` takes no ID, unlike curve builders

**Where:** `cashflows/src/builder/schedule.rs:249` — `pub fn builder() -> CashFlowBuilder`

**Pattern A:** All term-structure builders take `id: impl Into<CurveId>` as a required argument: `DiscountCurve::builder("USD-OIS")`, `HazardCurve::builder("ACME-HZD")`, etc.

**Pattern B:** `CashFlowSchedule::builder()` and instrument builders (via `#[derive(FinancialBuilder)]`) take no args because there's no single "key" field or many required fields.

**Recommendation:** Already documented as intentional in `DiscountCurve::builder()` doc comment. No action needed, but recording for completeness.

---

### [MINOR] Prelude usage: Only 3 of 11 crates have a `prelude.rs`

**Where:**
- **Have prelude:** `core`, `statements`, `valuations`
- **Use a `prelude` module (inline in lib.rs):** `monte_carlo` (as a module, not a file)
- **No prelude:** `analytics`, `cashflows`, `margin`, `portfolio`, `scenarios`, `statements-analytics`

**Pattern A:** Large crates with many public types provide a prelude.

**Pattern B:** Smaller crates rely on root-level `pub use` re-exports.

**Recommendation:** Already documented as intentional. `portfolio` is getting large enough that a prelude may be useful but is not a consistency violation.

---

### [MINOR] Builder terminal: `CashFlowBuilder` uses `.build_with_curves(None)` as the common case

**Where:** `cashflows/src/builder/orchestrator.rs:338`

**Pattern A (all other builders):** Terminal method is `.build()` returning `Result<T>`.

**Pattern B:** `CashFlowBuilder` has no `.build()` method — only `.build_with_curves(curves)`. Callers must pass `None` for the common case.

**Recommendation:** Already documented in conventions.md under "Terminal Methods". Acceptable since `.build()` would be a trivial delegate. But adding a `.build()` that delegates to `.build_with_curves(None)` would improve discoverability and align with the dominant pattern.

---

### [MINOR] Builder struct naming: `PortfolioBuilder` lives in its own file but is not `Portfolio::builder()` → `PortfolioBuilder`

**Where:** `portfolio/src/builder.rs:66` — `impl PortfolioBuilder { pub fn new(id: …) }`

**Pattern A:** `Type::builder(id)` on the built type delegates to `TypeBuilder::new(id)` which is a separate struct (DiscountCurve, ScenarioSpec, Waterfall, etc.)

**Pattern B:** `Portfolio::builder(id)` delegates to `PortfolioBuilder::new(id)` — consistent.

**Recommendation:** No action. This is consistent with the convention.

---

### [MINOR] `From<Error> for finstack_core::Error` conversion tests: only 3 of 4 crates with own errors have a test

**Where:**
- **Has test:** `portfolio/src/error.rs`, `scenarios/src/error.rs`, `statements/src/error.rs`
- **Missing test at crate-error level:** none (valuations has it too)

**Recommendation:** Consistent. No action.

---

### [NIT] `thiserror::Error` import style: `use thiserror::Error` vs fully-qualified `thiserror::Error` in derive

**Where:**
- `portfolio/src/error.rs` — `use thiserror::Error;` + `#[derive(…, Error, …)]`
- `scenarios/src/error.rs` — `use thiserror::Error;` + `#[derive(…, Error, …)]`
- `statements/src/error.rs` — `use thiserror::Error;` + `#[derive(…, Error, …)]`
- `valuations/src/error.rs` — `#[derive(…, thiserror::Error, …)]` (fully qualified)
- `valuations/src/pricer/errors.rs` — `#[derive(…, thiserror::Error, …)]` (fully qualified)
- `valuations/src/correlation/error.rs` — `#[derive(…, thiserror::Error, …)]` (fully qualified)
- `core/src/error/mod.rs` — `#[derive(…, thiserror::Error, …)]` (fully qualified)

**Pattern A:** Import `use thiserror::Error;` and use bare `Error` in derive (portfolio, scenarios, statements).

**Pattern B:** Fully qualify `thiserror::Error` in the derive attribute (core, valuations).

**Recommendation:** Both are correct. Per conventions.md (serde qualification note), no standardization required. Record as nit.

---

### [NIT] Module doc comment on `json` module in `cashflows/src/lib.rs`

**Where:** `cashflows/src/lib.rs:150` — `pub mod json;` has no `///` doc comment, while all other `pub mod` declarations in the same file have one.

**Recommendation:** Add a one-line doc comment for consistency within the file.

---

## Convention Inventory

| Pattern | Dominant | Deviation |
|---------|----------|-----------|
| Lint preamble (11 crate set) | Full 9-line block | `test-utils` only `#![forbid(unsafe_code)]` |
| Error module structure | Flat `error.rs` | `core/src/error/mod.rs` (justified by size) |
| Error derives (wire-facing) | `Debug, Clone, PartialEq, thiserror::Error, Serialize, Deserialize` | Sub-module errors in core/valuations omit serde |
| `#[non_exhaustive]` on pub error enums | Present | Missing on 10 sub-module error enums |
| `pub type Result<T>` | Present in crates with own Error | `analytics` has private alias; others spell it out |
| Builder entry point | `Type::builder(id)` for ID-keyed types | `Type::builder()` for instruments/aggregators |
| Builder terminal | `.build() -> Result<T>` | `CashFlowBuilder::build_with_curves(curves)` |
| `#[must_use]` on builder entry | 4 instances present | ~20+ missing |
| Prelude module | core, statements, valuations, monte_carlo (inline) | Other crates use root re-exports |
| `From<CrateError> for finstack_core::Error` | Present for portfolio, scenarios, statements, valuations | N/A for crates reusing core error directly |
