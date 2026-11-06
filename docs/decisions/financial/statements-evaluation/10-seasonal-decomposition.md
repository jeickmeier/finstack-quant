# STATEMENTS-EVALUATION-010 — Seasonal decomposition and phase-preserving projection

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed for the stated source scope**, with discrepancies identified below. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: [slice audit](README.md).

## Choice and input domain

Seasonal forecasting decomposes a finite supplied history into a centered moving-average trend and additive or multiplicative phase effects. Season length m must be positive and history must include at least two complete seasons. The vector supplies its own phase and trend; unsupported season-start configuration is rejected. There is no automatic detection of frequency, season length or economic calendar.

## Algorithm

1. Estimate interior trend with a centered moving average of length m. For even m, use m+1 observations with half weight on both endpoints divided by m. Extend trend at both edges linearly using the adjacent interior trend slope; near-zero slopes are snapped to zero by the numerical tolerance.
2. Additive mode calculates phase means of `observation-trend`, then centers phase effects so their average is zero. Multiplicative mode requires nonzero finite trend without a sign crossing between observations; calculate phase means of `observation/trend`, then divide phase factors by their mean when its absolute value exceeds the tolerance. A near-zero mean is left unnormalized.
3. Forecast trend from the final trend using `trend[h]=last_trend*(1+growth)^h`, with growth default zero and measured per model step. Select phase `(history_length+forecast_index) mod m`. Add the phase effect in additive mode or multiply by the factor in multiplicative mode. Require finite output at every step.
4. Compare the supplied final historical level to the evaluator base; a difference larger than `1e-6*max(1,abs(levels))` emits an advisory, but does not re-anchor the forecast. Evaluator-level explicit forecast cells do not reset phase, elapsed steps or the underlying generated run.

## Selection and conventions

The caller explicitly selects Seasonal, mode, season length, historical vector and optional growth. Rates are decimal per step; effects have level units in additive mode and relative units in multiplicative mode. The calculation can support signed additive levels. Multiplicative restrictions constrain the estimated trend's sign/zero domain, not a general guarantee that every economically meaningful level is positive. Actual period lengths and holidays are ignored.

## Rationale, limitations and alternatives

Centered trend removal isolates a repeating phase pattern; that is an interpretation of the code, not evidence that these decomposition assumptions fit the user's series. Edge extrapolation, raw phase averages, the normalization threshold and default flat future trend materially affect forecasts. Negative multiplicative trend, a phase mean near zero, and ordinary summation with large offsets/cancellation are limits of the established qualification evidence.

Possible alternatives include seasonal-dummy regression, STL, Holt–Winters, external seasonal indices or forecast models refitted against dates. They are not implicitly available through this method. No predictive accuracy or accounting-standard adoption is established.

## Evidence

[Seasonal implementation](../../../../finstack-quant/statements/src/forecast/timeseries.rs), [forecast integration](../../../../finstack-quant/statements/src/evaluator/forecast_eval.rs). Located inline tests for decomposition, unsupported season start, zero season length, nonfinite history and preserved multiplicative factors; [forecast regressions](../../../../finstack-quant/statements/tests/finance_forecast_regressions.rs) cover between-observation trend crossing and extreme season lengths. Inspected [reporting regression](../../../../finstack-quant/statements/tests/production_reporting_audit.rs) verifies an explicit leading forecast cell leaves later seasonal phase unchanged. These are algorithm/domain characterizations; independent seasonal-model calibration or out-of-sample evidence is absent from this audit. No tests were executed.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-evaluation-forecasts-and-reporting).
