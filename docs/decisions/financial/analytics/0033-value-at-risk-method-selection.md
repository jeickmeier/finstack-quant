# ADR 0033: Return-series VaR and expected-shortfall method selection

**Decision status:** Accepted for fixed historical tail-method selection on 2026-11-07. Other inputs, estimators and failure policies retain their owning scopes.

**Implementation status:** Implemented by the `Performance` scalar and fixed-summary dispatch. Gaussian and Cornish–Fisher remain explicit alternatives.

**Validation status:** See the [implementation evidence](README.md#implementation-evidence) for executed checks and their limits.

**Audit provenance:** Original source inspection: 2026-11-05 at `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Decision reconciliation: 2026-11-08.

## Accepted decision: fixed historical tail selection

**Accepted:** 2026-11-07.

The canonical `value_at_risk` and `expected_shortfall` methods use the
accepted historical pair: R-7 interpolated VaR and exact fractional empirical
ES. Standard `summary(...)` reports that same pair in its VaR/ES columns,
using each ticker's active return history and explicit confidence. Historical
estimates remain native-period signed decimal returns, with the owner-defined
nonempty-sample availability rules.

Gaussian `parametric_var` and `cornish_fisher_var` remain separately named,
explicit calculations. Calling either does not change the summary's method.
History length, unavailable estimates and invalid CF curves never select a
substitute estimator. Preserve the requested estimator's undefined or error
outcome. No summary model selector or automatic dispatch is provided.

For example, requesting ten-period Gaussian VaR does not cause a subsequent
summary to report ten-period Gaussian risk: its tail columns remain native-
period historical estimates.

This gives the standard performance report a fixed descriptive interpretation
without selecting Gaussian distribution assumptions or a higher-moment CF
approximation for the caller. It is a product policy. Sample
representativeness and tail resolution limit historical estimates; this
selection does not qualify forecasting accuracy.

The summary's explicit cash/MAR/Omega/confidence inputs, non-tail metrics,
provenance, result serialization and whole-report failure contract follow
[record 01](01-panels-windows-and-wire.md#accepted-decision-panels-windows-wire-and-summary).
A computable historical tail statistic does not imply that CAGR or any other
summary statistic is available.

## Selection and estimator ownership

| Entry point | Selected method and contract |
| --- | --- |
| `value_at_risk(confidence)` | Native-period signed R-7 [historical VaR](07-historical-tail-risk.md). |
| `expected_shortfall(confidence)` | Native-period signed [fractional empirical ES](07-historical-tail-risk.md). |
| `summary(...)` | The same historical VaR/ES pair, with no model override. |
| `parametric_var(confidence, horizon_periods)` | Explicit [Gaussian additive-horizon quantile](08-moments-gaussian-and-cornish-fisher.md). |
| `cornish_fisher_var(confidence, horizon_periods)` | Explicit [classical CF plug-in quantile](08-moments-gaussian-and-cornish-fisher.md), subject to the global analytic ordering gate. |

Every tail request requires finite `0.5 < confidence < 1`. Gaussian/CF
horizons are exact whole observation counts in `1..=2^53-1`; omission means
one. Historical methods have no horizon input. Invalid CF curves produce an
explicit error, with no Gaussian fallback or repaired quantile. The [tail
owner](08-moments-gaussian-and-cornish-fisher.md) defines estimator-specific
availability, model support, moment and numerical-output rules.

These estimators operate on return observations. Instrument repricing,
position revaluation, monetary loss conversion, portfolio/factor-model VaR
and currency conversion have separate owners.

The [scalar and summary dispatch](../../../../finstack-quant/analytics/src/performance/scalar.rs)
and the [tail kernels](../../../../finstack-quant/analytics/src/risk_metrics/tail_risk.rs)
implement this selection. The [analytics inventory](README.md) links the panel,
wealth, numerical and host contracts needed to interpret their results.
