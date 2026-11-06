# CALIBRATION-FRAMEWORK-014: Residual acceptance, plan aggregation and report units

- Decision status: **Draft (observed implementation; adoption pending)**.
- Implementation status: **Observed for the stated source scope**; discrepancies and limits are stated below.
- Validation status: **Evidence inspected/located, not executed**. No numerical qualification or external compliance certification is implied.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Observed choice and rationale

Step acceptance uses each observation's unweighted absolute residual, while solver objective and iteration counters retain method-specific meanings. The engine normalizes residuals by step tolerance to compare heterogeneous steps. The report schema also retains raw pooled summary fields, which require careful interpretation.

## Equations and stages

The generic residual summary treats nonfinite values or |r|≥5e5 as penalties. If any valid values exist, `max_residual` and RMSE use **only valid** values; otherwise they use all. The worst-quote selector independently ranks nonfinite values as infinite magnitude and otherwise absolute residual. Empty per-type residual maps fail. Nonempty maps fail if any penalty exists or max absolute residual exceeds tolerance; equality passes. `CalibrationReport::new` accepts caller-supplied success and sets objective to RMSE without independently overriding success.

Global fits overwrite objective with weighted L2, attach actual LM stats and units; bootstrap reports use objective-evaluation count as `iterations`, unlike LM iteration count. Final clamping and validation failures force success false. Warn-mode validation only adds metadata and leaves success/`validation_passed` intact. `quote_rows` prefers saved diagnostics rows, otherwise returns residual-only rows with missing quote value and NaN sensitivity.

For each plan step, require finite positive `success_tolerance`; construct residuals `a_i=|r_i|/tolerance_step`. Plan success is the conjunction of all step success and validation flags. Generic report construction computes ratio max/RMS, then `with_plan_aggregation` stores those in `max_residual_ratio`/`rmse_ratio` and replaces raw max/RMS with max of step maxima and quote-count-weighted RMS across step RMSEs. Objective remains the ratio RMSE and `residual_units` is absolute-residual-over-step-tolerance. Raw step residuals remain in `step_reports`. Traces merge in lexicographic step-ID order, not original plan order, and preserve truncation flags.

## Confirmed discrepancies and limits

The fixed 5e5 penalty threshold is reused on dimensionless ratios. In a permissive plan, raw residual 0.01 with tolerance 1e-8 produces ratio 1e6; if another ratio is 0.1, the report summary excludes 1e6 and reports max ratio 0.1. Plan success remains false from the step reports. This conclusion follows directly from the aggregation equations; no runtime test was executed.

Raw maxima/RMS pooled across quoted volatility, PV per unit notional and upfront-fraction steps are not one economic unit. When a step excludes penalties, pooling its valid-only RMSE using the full residual count also differs from pooling all valid raw observations. NaN-only maps can have finite zero maximum despite failure; fallback quote-row NaNs need host serialization care. Error strings claiming max residual exceeded tolerance can misdescribe a validation/clamping failure.

## Alternatives

Per-unit raw summaries or ratio-only plan summaries would be meaningful alternatives; choosing one is an API policy decision.

## Source and validation evidence

- Implementation: [report.rs](../../../../finstack-quant/calibration/src/report.rs) — penalty classification, max/RMS, quote rows, success and pooled summaries.
- Implementation: [api/engine.rs](../../../../finstack-quant/calibration/src/api/engine.rs) — tolerance ratios, conjunction, trace merging and fit error creation.
- Implementation: [validation/mod.rs](../../../../finstack-quant/calibration/src/validation/mod.rs) — Warn/Error attachment.
- Implementation: [versions.rs](../../../../finstack-quant/calibration/src/versions.rs) — human-readable model labels only.
- Test evidence: [src/report.rs](../../../../finstack-quant/calibration/src/report.rs) — threshold/nonfinite acceptance, RMS and optional-wire regressions; inspected or located, not executed.
- Test evidence: [src/api/engine.rs](../../../../finstack-quant/calibration/src/api/engine.rs) — plan residual aggregation tests located; inspected or located, not executed.
- Test evidence: [tests/calibration/tolerances.rs](../../../../finstack-quant/calibration/tests/calibration/tolerances.rs) — target acceptance tolerance tests located; inspected or located, not executed.
- Reference gap: no method-specific independent source or external benchmark is established by this record; source and regression evidence describe implementation behavior.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
