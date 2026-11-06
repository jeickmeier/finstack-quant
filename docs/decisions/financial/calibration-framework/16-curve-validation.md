# CALIBRATION-FRAMEWORK-016: Sampled curve shape, plausibility and bounds

- Decision status: **Draft (observed implementation; adoption pending)**.
- Implementation status: **Observed for the stated source scope**; discrepancies and limits are stated below.
- Validation status: **Evidence inspected/located, not executed**. No numerical qualification or external compliance certification is implied.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Observed choice and rationale

`CurveValidator` applies financial shape and plausibility checks after construction. The method name `validate_no_arbitrage` includes heuristic economic limits, not a complete mathematical arbitrage certificate. Full validation first validates config, then no-arbitrage, monotonicity and bounds, stopping at the first error.

## Implemented checks

- Discount: when arbitrage checking is enabled, combine zero, all actual knots and standard sample tenors below the last knot, sort/deduplicate, and check segment continuous forward `f=(ln D1−ln D2)/(t2−t1)` against min/max rates. Only finite positive DF pairs enter this check. Monotonicity uses standard times 0 to 50 years only within last knot+0.01; DF rises are allowed only when `allow_negative_rates` and the implied segment rate is below negative tolerance. Bounds at a separate fixed grid require positive DF, DF≤1 (or≤1.5 with negative-rate opt-in), and continuous zero rate within `[5×min_forward_rate,max_forward_rate]`.
- Forward: a fixed grid 0.25–10y checks lower bound when either arbitrage or forward-positivity is enabled. Adjacent grid jumps above 0.10 only log warnings; this monotonicity method never rejects. Bounds on another 0.25–10y grid enforce `[5×min,max]` even when arbitrage checks are disabled.
- Hazard: fixed 0.5–10y grid checks nonnegative hazard and configured maximum; survival on 0–10y must not increase beyond tolerance and bounds lie in [0,1]. Recovery lies in [0,1].
- Inflation: positive CPI on 0–20y grid; annualized growth `(CPI(t)/CPI(0))^(1/t)−1` on 1–10y grid; one-year forward ratio `CPI(t+1)/CPI(t)−1` on 1–30y grid. These are growth/plausibility checks, not literal CPI monotonicity.
- Base correlation: enforce matching vector lengths, nondecreasing correlation across stored detachments within tolerance when arbitrage enabled, and values in [0,1]. The separate monotonicity method is a no-op. Nondecreasing base correlation alone does not prove nonnegative tranche losses or spreads.

Defaults: forward range [-0.01,0.50], tolerance1e-10, max hazard0.50, annual CPI growth[-0.10,0.50], forward inflation[-0.20,0.50], max vol5, negative rates disabled. Recovery consistency tolerance1e-12 and minimum LGD for hazard guess1e-6 are separate controls. See `points.rs` for exact grids.

## Selection, limitations and alternatives

Discount/forward/hazard/inflation/base-correlation and XCCY discount outputs use the runtime validator; target-local checks may add constraints. Parametric outputs are not passed through this generic trait by runtime. Warn adds one warning without changing validation status; Error marks failure for the engine gate. Direct trait submethods do not themselves all validate config.

Most grids omit arbitrary interior extrema, and forward/hazard/inflation probes can extrapolate beyond supplied support. Several scalar comparisons do not explicitly reject NaN, relying on the curve builder/evaluator invariant; discount no-arbitrage skips invalid DF pairs. Search hazard cap10 versus validator cap0.5 and currency-specific rate bounds versus default negative-rate prohibition are intentionally separate implemented policies, not one consistent permitted domain.

Analytic segment checks or adaptive dense extrema searches would provide stronger coverage but require interpolation-specific contracts.

## Source and validation evidence

- Implementation: [validation/curves.rs](../../../../finstack-quant/calibration/src/validation/curves.rs) — five curve implementations and check ordering.
- Implementation: [validation/points.rs](../../../../finstack-quant/calibration/src/validation/points.rs) — exact fixed probe grids.
- Implementation: [validation/config.rs](../../../../finstack-quant/calibration/src/validation/config.rs) — economic thresholds and flag validation.
- Implementation: [validation/mod.rs](../../../../finstack-quant/calibration/src/validation/mod.rs) — Warn/Error result handling.
- Implementation: [step_runtime.rs](../../../../finstack-quant/calibration/src/step_runtime.rs) — which targets receive generic validation.
- Test evidence: [tests/calibration/validation.rs](../../../../finstack-quant/calibration/tests/calibration/validation.rs) — negative-rate opt-in, hidden actual-knot forward floor, CPI/hazard/forward/base-correlation bounds; inspected or located, not executed.
- Reference gap: no method-specific independent source or external benchmark is established by this record; source and regression evidence describe implementation behavior.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
