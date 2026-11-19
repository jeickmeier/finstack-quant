# STATEMENTS-EVALUATION-010 — Seasonal decomposition and phase-preserving projection

Decision status: **Accepted for the full seasonal decomposition and projection contract (2026-11-15)**. Implementation status: **Reconciled (2026-11-17)**. The mode and season length are required, and the history is at least two complete seasons of finite observations, defined as the consecutive slots ending at the forecast opening; all are checked when the model is built. The trend is a centered moving average with `m` equal weights for odd `m` and `m + 1` observations with half-weight endpoints divided by `m` for even `m`, accumulated on scaled values with compensated sums; each edge is extended with the slope between its two nearest centered points, used as computed. Additive effects are phase means of observation minus trend, centered to average zero. Multiplicative mode requires a non-zero trend of one sign and observations of that sign or exactly zero, takes phase means of the non-negative ratios and normalizes them to average one; a set of factors that cannot be normalized is an error. The final trend grows by `(1 + g)^h` with `g` finite and above −1 in multiplicative mode, the phase continues from the history across every forecast slot, and a non-finite output is an error before bounds. The advisory comparing the history's last level with the node's last actual is removed. Validation status: **Qualified** by two exact cycles of a linear trend plus a zero-sum pattern for even and odd `m` (trend, effects and forecast recovered exactly), an unsnapped edge slope of 1e-12, a zero observation and a negative-trend series in multiplicative mode, rejected mixed-sign observations and trends, and the static requirements. The sections after the accepted decision describe the implementation audited before reconciliation. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Accepted decision for seasonal decomposition and projection

1. Select the additive or multiplicative mode and an exact positive integer season length `m`. Use at least two complete cycles of finite, same-unit, eligible observations from consecutive canonical reporting slots. The last historical slot ends at the forecast opening. The supplied history determines trend and phase. [Forecast anchoring and inputs](05-forecast-segments-and-bounds.md#accepted-decision-for-forecast-segments-anchoring-and-output-bounds).
2. Estimate the trend with a centered moving average. For odd `m`, give the `m` observations equal weight. For even `m`, use `m + 1` observations with half weight at each endpoint and divide the weighted sum by `m`. Extend each edge linearly using the slope between its two nearest centered trend points. Preserve the computed slope without snapping it to zero.
3. In additive mode, take each phase's mean observation-minus-trend residual and center the resulting phase effects to an equal-phase average of zero. In multiplicative mode, require a nonzero trend with a fixed sign. Each observation must share that sign or equal exactly zero. Use nonnegative observation-to-trend ratios to calculate the phase factors, then normalize their equal-phase average to one.
4. Grow the final trend by `(1 + g)^h` for forecast step `h`, where `g` is a finite decimal growth rate per model slot and defaults to zero. Multiplicative mode requires `g > −1`. Add the phase effect in additive mode or multiply by the phase factor in multiplicative mode. Continue phase across every canonical forecast slot, including masks and explicit published values. Use checked arithmetic and validate each raw output before applying output bounds. [Output bounds and phase preservation](05-forecast-segments-and-bounds.md#accepted-decision-for-forecast-segments-anchoring-and-output-bounds).

## Audited input domain

Seasonal forecasting decomposes a finite supplied history into a centered moving-average trend and additive or multiplicative phase effects. Season length m must be positive and history must include at least two complete seasons. The vector supplies its own phase and trend; unsupported season-start configuration is rejected. There is no automatic detection of frequency, season length or economic calendar.

## Audited algorithm

1. Estimate interior trend with a centered moving average of length m. For even m, use m+1 observations with half weight on both endpoints divided by m. Extend trend at both edges linearly using the adjacent interior trend slope; near-zero slopes are snapped to zero by the numerical tolerance.
2. Additive mode calculates phase means of `observation-trend`, then centers phase effects so their average is zero. Multiplicative mode requires nonzero finite trend without a sign crossing between observations; calculate phase means of `observation/trend`, then divide phase factors by their mean when its absolute value exceeds the tolerance. A near-zero mean is left unnormalized.
3. Forecast trend from the final trend using `trend[h]=last_trend*(1+growth)^h`, with growth default zero and measured per model step. Select phase `(history_length+forecast_index) mod m`. Add the phase effect in additive mode or multiply by the factor in multiplicative mode. Require finite output at every step.
4. Compare the supplied final historical level to the evaluator base; a difference larger than `1e-6*max(1,abs(levels))` emits an advisory, but does not re-anchor the forecast. Evaluator-level explicit forecast cells do not reset phase, elapsed steps or the underlying generated run.

## Selection and conventions

The caller explicitly selects Seasonal, mode, season length, historical vector and optional growth. Rates are decimal per step; effects have level units in additive mode and relative units in multiplicative mode. The calculation can support signed additive levels. Multiplicative restrictions constrain the estimated trend's sign/zero domain, not a general guarantee that every economically meaningful level is positive. Actual period lengths and holidays are ignored.

## Rationale, limitations and alternatives

Centered trend removal isolates a repeating phase pattern; that is an interpretation of the code, not evidence that these decomposition assumptions fit the user's series. Edge extrapolation, raw phase averages, the normalization threshold and default flat future trend materially affect forecasts. Negative multiplicative trend, a phase mean near zero, and ordinary summation with large offsets/cancellation are limits of the established qualification evidence.

The accepted contract requires aligned history, checked edge extrapolation and explicit multiplicative sign and normalization rules. The audited behavior requires reconciliation.

## Evidence

[Seasonal implementation](../../../../finstack-quant/statements/src/forecast/timeseries.rs), [forecast integration](../../../../finstack-quant/statements/src/evaluator/forecast_eval.rs). Located inline tests for decomposition, unsupported season start, zero season length, nonfinite history and preserved multiplicative factors; [forecast regressions](../../../../finstack-quant/statements/tests/finance_forecast_regressions.rs) cover between-observation trend crossing and extreme season lengths. Inspected [reporting regression](../../../../finstack-quant/statements/tests/production_reporting_audit.rs) verifies an explicit leading forecast cell leaves later seasonal phase unchanged. These are algorithm/domain characterizations; independent seasonal-model calibration or out-of-sample evidence is absent from this audit. No tests were executed.

## Follow-up tracking

This scope has no open follow-ups; the [follow-up register](../FOLLOW_UPS.md) tracks the other scopes.
