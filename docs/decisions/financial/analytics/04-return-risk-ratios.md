# ANALYTICS-004: Sample volatility, downside risk and return-based reward ratios

**Decision status:** Draft — observed implementation; adoption pending for choices not already settled. Existing accepted directions remain reconciliation requirements.

**Implementation status:** Observed in the source paths below, including the stated discrepancies and fallback behavior.

**Validation status:** Source inspected; test evidence inspected or located as identified below, not executed. No financial accuracy certification or runtime change is implied.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope and exact source manifest](README.md).

## Choice, rationale and domain

The scalar ratios use equal observation weights and the panel's declared observation frequency. Sample volatility uses n−1; downside risk uses all N observations in its lower-partial-moment denominator. These are materially different estimators. Returns, thresholds and MAR are per-observation decimal fractions; Sharpe cash is an annual effective decimal rate converted as in [record 02](02-wealth-and-annualization.md).

## Algorithm and selection

`volatility` is sample standard deviation × sqrt(A), with Welford sample moments supplied by core. The core sample variance is zero for fewer than two observations, so scalar volatility returns zero there; nonpositive/nonfinite A returns NaN. `Performance.volatility(false)` omits annualization.

Sharpe requires at least two returns. It computes `(A*mean(r)−A*q)/(sqrt(A)*sample_sd(r))`, rejecting nonfinite annual moments as NaN. Sortino also requires at least two observations and valid A: downside deviation is `sqrt(sum(min(r−MAR,0)^2)/N)`, and Sortino is `(mean(r)−MAR)*sqrt(A)/downside`. MAR is not converted from an annual effective rate. Public downside deviation is annualized; empty downside input returns zero, while nonfinite input or MAR returns NaN.

The common ratio helper first returns NaN for any nonfinite operand; finite nonzero numerator/zero denominator yields signed infinity; finite 0/0 yields zero. This differs from insufficient-sample NaN and from Omega's all-neutral result. Sortino/Sharpe use the helper, preserving finite constant positive/negative excess signs when risk is zero.

Omega at threshold h is `sum(max(r−h,0))/sum(max(h−r,0))`, with empty input NaN, positive gains/no losses infinity, and all outcomes exactly at h equal one. Gain-to-pain is `sum(r)/sum(abs(r) for r<0)`, with empty NaN and the common zero-denominator policy. Their raw sums are not geometric wealth; Omega accumulation is ordinary addition, gain-to-pain uses Kahan sums.

Modified Sharpe uses annual arithmetic excess return divided by the absolute **annual-horizon** Cornish–Fisher lower-tail return quantile. It requires at least two observations and nonzero finite annual volatility; a nonnegative CF quantile returns NaN. It inherits the Gaussian substitution discrepancy described in [record 08](08-moments-gaussian-and-cornish-fisher.md). It is not simply ordinary Sharpe with skew/kurtosis penalties.

## Controls, failures and alternatives

Sharpe/Sortino free functions accept A explicitly; `Performance` obtains A from frequency. Scalar threshold/MAR/cash inputs do not share a single `Result`-based validator: invalid numerical inputs usually propagate NaN, while tail confidence is checked at the public boundary. The summary uses zero MAR and zero Omega threshold. It does not estimate cash from a curve or apply serial-correlation corrections.

Population risk, downside-only denominators, geometric excess-growth numerators, HAC-adjusted risk and time-weighted irregular observations are unimplemented alternatives here. Choice must match use; current tests alone are not adoption.

## Implementation and validation evidence

- [Return-risk implementations](../../../../finstack-quant/analytics/src/risk_metrics/return_based.rs), [ratio helper](../../../../finstack-quant/analytics/src/risk_metrics/mod.rs), [public dispatch](../../../../finstack-quant/analytics/src/performance/scalar.rs), and [sample moments](../../../../finstack-quant/core/src/math/stats.rs).
- Located inline hand calculations cover downside denominators, threshold Omega, gain-to-pain and MAR consistency. Modified-Sharpe formula comparisons reuse production CF/moment kernels. [Audit regressions](../../../../finstack-quant/analytics/tests/audit_regressions.rs) cover singleton/invalid scalar behavior and are boundary/invariant evidence, not distribution calibration.
- Canonical context: [Sharpe](../../../REFERENCES.md#sharpe1966), [Sortino](../../../REFERENCES.md#sortinoVanDerMeer1991), [Omega](../../../REFERENCES.md#keatingShadwick2002), [gain-to-pain](../../../REFERENCES.md#schwager2012), [modified Sharpe](../../../REFERENCES.md#gregoriou2003). Exact source-to-formula reconciliation and tolerances remain open.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[analytics follow-ups](../FOLLOW_UPS.md#analytics).
