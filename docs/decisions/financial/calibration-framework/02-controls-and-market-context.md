# CALIBRATION-FRAMEWORK-002: Numerical policy, freshness and initial context

- Decision status: **Draft (observed implementation; adoption pending)**.
- Implementation status: **Observed for the stated source scope**; discrepancies and limits are stated below.
- Validation status: **Evidence inspected/located, not executed**. No numerical qualification or external compliance certification is implied.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Observed choice and rationale

A shared `CalibrationConfig` controls solver accuracy, acceptance, validation and execution. Its role is to reject inconsistent controls before solving and to materialize an explicit market snapshot. This is observed behavior, not evidence that the chosen numerical limits have been approved for every market.

## Input domain, controls and stages

The solver requires finite positive tolerance and at least one iteration; defaults are 1e-12 and 100. Acceptance tolerances must be finite positive and at least the solver tolerance: default 1e-8 for discount, forward, hazard and inflation, 1e-3 for volatility. Discount-control fields default to `scan_grid_points=48`, minimum scan count 20, `scan_grid_step=1e-4`, DF bounds [1e-12,1e6], bootstrap seeding enabled and Jacobian step 1e-6. The [generic fallback scan](10-sequential-bootstrap.md) interprets the count per side and uses the step; the [discount target](../calibration-targets/03-discount-calibration.md) instead uses a maturity-aware custom grid with fixed log offsets, so those fields do not imply an exponential per-side discount scan. Hazard bounds default [0,10]; CPI [1,10000]. Weighting defaults to square-root time, with equal/linear-time/inverse-duration variants implemented by the targets. Counts are checked for allocation overflow. Recursive JSON override merges objects, replaces other values, preserves disabled explain settings, rejects unknown fields, then revalidates.

The default rate-bound policy is AutoCurrency. Currency rate bounds default to EUR/JPY/CHF [-0.05,0.30], USD/CAD/AUD/NZD [-0.02,0.50], GBP [-0.03,0.50], TRY/ARS/BRL/ZAR/MXN [-0.05,2.00], others [-0.02,0.50]. `with_rate_bounds` selects Explicit. These decimal annual-rate search bounds do not automatically change post-fit `ValidationConfig` (see [curve validation](16-curve-validation.md)). Serial execution, `ValidationMode::Error`, `fail_on_bad_fit=true`, disabled diagnostics/explanations, and no hierarchy are defaults.

Initial context insertion proceeds in order: prior curves/surfaces; nonquote market scalars, dividend schedules, fixing and inflation series, FX delta surfaces and volatility cubes; the FX matrix from explicit quotes; credit-index states; optional hierarchy. Credit-index insertion is delayed until dependencies exist. Calibration quotes never become initial curves. Price versus volatility-index prior tags must agree with their actual `PriceCurveKind`. Duplicate prior objects are not separately rejected by envelope uniqueness checks; insertion semantics govern replacement.

Freshness may be absent, producing `unverified` metadata. If enabled, snapshot time, assessment time and positive max age must all be supplied; timestamps parse as RFC3339, future snapshots fail, and age uses whole seconds. No wall-clock lookup occurs. Quote side Mid/Bid/Ask is metadata over a single-side quote schema, not selection from bid/ask pairs. FX uses a quote-only provider and the configured `FxConfig`.

## Limits and alternatives

Freshness `verified` means the supplied timestamps satisfy the supplied policy; it does not authenticate collection time or provider. The `u64` age limit is cast to `i64` for comparison without an explicit upper-bound guard; extremely large values can become negative and reject valid ages.

Per-instrument calibrated limits would offer more precision than one global tolerance; market-regime-specific bounds would replace static currency heuristics. Both require explicit policy and independent stressed-market evidence.

## Source and validation evidence

- Implementation: [config.rs](../../../../finstack-quant/calibration/src/config.rs) — configuration defaults, range validation, overrides, freshness and currency policy.
- Implementation: [solver/config.rs](../../../../finstack-quant/calibration/src/solver/config.rs) — validated solver wire boundary.
- Implementation: [validation/config.rs](../../../../finstack-quant/calibration/src/validation/config.rs) — independent post-fit policy.
- Implementation: [api/context_builder.rs](../../../../finstack-quant/calibration/src/api/context_builder.rs) — ordered snapshot materialization.
- Test evidence: [src/config.rs](../../../../finstack-quant/calibration/src/config.rs) — invalid controls and freshness regressions; inspected or located, not executed.
- Test evidence: [src/solver/config.rs](../../../../finstack-quant/calibration/src/solver/config.rs) — deserialization validation; inspected or located, not executed.
- Test evidence: [src/api/context_builder.rs](../../../../finstack-quant/calibration/src/api/context_builder.rs) — FX/context materialization regressions; inspected or located, not executed.
- Reference gap: no method-specific independent source or external benchmark is established by this record; source and regression evidence describe implementation behavior.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
