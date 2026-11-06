# ANALYTICS-008: Fourth moments, Gaussian and Cornish–Fisher horizon quantiles

**Decision status:** Draft — observed implementation; adoption pending for choices not already settled. Existing accepted directions remain reconciliation requirements.

**Implementation status:** Observed in the source paths below, including the stated discrepancies and fallback behavior.

**Validation status:** Source inspected; test evidence inspected or located as identified below, not executed. No financial accuracy certification or runtime change is implied.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope and exact source manifest](README.md).

## Choice and input domain

Explicit Gaussian and Cornish–Fisher methods estimate lower-tail simple-return quantiles from equal-weight observations. Confidence must be finite and strictly inside (0,1); at least two observations and a positive finite optional horizon h are required numerically. Omitted horizon means one observation interval. h is a count of observation periods, not calendar days or a named annualization convention. [Record 01](01-panels-windows-and-wire.md) owns the traced host defaults; [ADR 0033](0033-value-at-risk-method-selection.md) owns cross-estimator selection and its pending rationale.

## Algorithm and controls

`moments4` updates mean and centered second/third/fourth sums online using the stable central-moment recurrence. With N observations, sample variance is M2/(N−1); Fisher adjusted skewness G1 is `N*M3/((N−1)(N−2)*s³)`; adjusted excess kurtosis G2 is `N(N+1)*M4/((N−1)(N−2)(N−3)*s⁴)−3(N−1)²/((N−2)(N−3))`. Skewness returns zero for N<3 or zero variance; excess kurtosis returns zero for N<4 or zero variance. Empty moment input returns all zeros. Those small-sample zeros are computational conventions, not evidence that the underlying distribution is Gaussian.

Gaussian VaR uses sample mean μ, sample standard deviation σ and z=Φ⁻¹(1−c): `μ*h + z*σ*sqrt(h)`. Without h it uses μ+zσ. The normal quantile comes from core. Outputs are signed decimal returns, negative for losses, with no zero floor. No EWMA covariance, Student-t fit or clipping to [−1,∞) is included.

CF scales sample skew S and excess kurtosis K to `S_h=S/sqrt(h)` and `K_h=K/h` under the implementation's i.i.d.-aggregation assumption, then forms:

```text
z_cf = z + (z²−1)S_h/6 + (z³−3z)K_h/24 − (2z³−5z)S_h²/36
Q_cf = μ*h + σ*sqrt(h)*z_cf
```

Before returning it, evaluate the local derivative
`D=1+2z*S_h/6+(3z²−3)K_h/24−(6z²−5)S_h²/36`.
If D<=0, code emits a `tracing::warn!` and returns the **Gaussian** quantile. The public result has no selected-method/fallback flag. This conflicts with the [prior agreed explicit-error direction](0033-value-at-risk-method-selection.md#observed-choice-and-rationale) for invalid CF; Gaussian substitution is not an adoption option. A positive local derivative is not proof of globally valid quantiles; nonfinite moments/derivative can still propagate NaN without this branch. There is no automatic historical substitution.

## Selection, conventions and failure behavior

The methods are explicitly selected on `Performance`; summary remains historical. Bad confidence produces `Error::Validation`; fewer than two observations (including an empty active window) or a non-positive/non-finite horizon returns NaN inside `Ok` results. The invalid-horizon versus unavailable-sample distinction needs agreement under [ADR 0009](../conventions/0009-missing-and-invalid-data.md). The shapes assume equal observation weighting and independent, identically distributed additive horizon aggregation, not compounding of a fitted wealth distribution. `modified_sharpe` requests h=A and inherits the CF fallback. High-order estimates can be unstable in small/extreme samples; no data trimming or parameter regularization is performed.

## Alternatives and evidence

Historical quantiles are already supported separately; skewed parametric fits, rearranged/monotone CF and robust moments are unimplemented alternatives. The prior invalid-CF error contract remains authoritative; the broader CF domain and validity criteria remain unadopted.

- [Owning kernels](../../../../finstack-quant/analytics/src/risk_metrics/tail_risk.rs), [public dispatch](../../../../finstack-quant/analytics/src/performance/scalar.rs), and [modified Sharpe caller](../../../../finstack-quant/analytics/src/risk_metrics/return_based.rs).
- Inspected inline formula/horizon tests reuse core quantiles and production moment functions, so they test wiring/algebra rather than a fully independent oracle. Located `cornish_fisher_var_falls_back_to_parametric_when_non_monotonic` explicitly protects the current conflicting fallback. Small-sample and invalid-horizon tests, including public single-observation sentinels in [correctness regressions](../../../../finstack-quant/analytics/tests/correctness_regressions.rs), were located, not executed.
- Canonical context: [sample moments](../../../REFERENCES.md#joanesGill1998), [Cornish–Fisher](../../../REFERENCES.md#cornishFisher1937), [RiskMetrics](../../../REFERENCES.md#jpmorgan1996RiskMetrics). The named Maillard guard lacks a specific canonical reference entry; RiskMetrics naming does not turn this equal-weight estimator into EWMA. [ADR 0033](0033-value-at-risk-method-selection.md) records the broader selection boundary.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[analytics follow-ups](../FOLLOW_UPS.md#analytics).
