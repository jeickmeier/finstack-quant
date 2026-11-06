# ANALYTICS-007: Historical return VaR, exact-tail expected shortfall and tail ratio

**Decision status:** Draft — observed implementation; adoption pending for choices not already settled. Existing accepted directions remain reconciliation requirements.

**Implementation status:** Observed in the source paths below, including the stated discrepancies and fallback behavior.

**Validation status:** Source inspected; test evidence inspected or located as identified below, not executed. No financial accuracy certification or runtime change is implied.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope and exact source manifest](README.md).

## Choice, rationale and domain

Historical tail metrics are equal-weight empirical estimators on each ticker's finite active observations, assumed to describe the requested tail. Public confidence c must be finite and strictly inside (0,1); invalid confidence produces `Error::Validation` before private kernels. Empty samples return NaN. The returned VaR/ES are signed decimal returns: −0.05 denotes a 5% loss, not a positive monetary loss. All-positive samples can produce positive outputs. There is no money conversion, zero clipping, time decay or volatility rescaling; historical tails stay in the observations' native period.

## Algorithm and selection

VaR copies the observations and calls the core R-7 quantile at p=1−c. For sorted x, h=(N−1)p, j=floor(h), it linearly interpolates x[j] and x[j+1]. Partial selection avoids fully sorting. A private combined VaR/ES route shares the scratch copy for summary but preserves the two distinct estimators.

Expected shortfall instead integrates exact empirical lower-tail mass m=(1−c)N. Let k=floor(m). After selecting the boundary order statistic, sum the k lowest observations with weight 1/m each and add the next observation with weight (m−k)/m. If m<1, return the single worst observation. Ties do not expand the chosen probability mass: the estimator does not average every return below an interpolated VaR threshold. This difference matters in small and tied samples.

Tail ratio is `abs(Q(c))/abs(Q(1−c))`, with positive upper-tail magnitude/zero lower-tail magnitude giving infinity; both zero gives NaN. The use of absolute values means the numerator is not necessarily a gain quantile. Confidence below .5 is allowed even though the labels then swap their intuitive upper/lower roles.

`Performance.value_at_risk`, `expected_shortfall` and `tail_ratio` choose these kernels directly; summary's `value_at_risk` and `expected_shortfall` always select the historical route. `parametric_var` and `cornish_fisher_var` are separate explicit methods, not fallbacks when historical data is short. CDaR calls this ES kernel on negative drawdown observations rather than returns.

## Limitations and alternatives

Resolution is limited by sample size; no minimum effective tail count is enforced. Interpolation affects VaR while ES integrates a step empirical distribution; this deliberate mixed convention needs adoption as a pair. Finite panel validation is relied upon by the ES kernel, which does not perform its own full public input validation. The related [selection ADR 0033](0033-value-at-risk-method-selection.md) owns the cross-estimator selection rationale and prior CF-error requirement; this record owns the historical algorithms.

The public VaR documentation promises non-positive outputs, but the kernel returns the signed quantile without a zero floor. The wording discrepancy does not permit changing the estimator or discarding positive observations under [ADR 0007](../conventions/0007-units-and-sign-conventions.md).

Other empirical quantiles, exceedance-only ES, weighted historical sampling, block bootstrap, filtered historical simulation and full portfolio revaluation are not selected by this API. Naming these alternatives does not approve replacement of the observed estimator.

## Implementation and validation evidence

- [Tail kernels](../../../../finstack-quant/analytics/src/risk_metrics/tail_risk.rs), [public/summary dispatch](../../../../finstack-quant/analytics/src/performance/scalar.rs), and [R-7 implementation](../../../../finstack-quant/core/src/math/stats.rs).
- Inspected [audit regression](../../../../finstack-quant/analytics/tests/audit_regressions.rs) `tail_mass_is_exact_with_ties_and_fractional_observations` specifies independent arithmetic expectations, including −.25/1.5 and worst-return tails smaller than one observation. Inline combined-versus-standalone tests are same-kernel consistency evidence. Confidence rejection tests are located in [correctness regressions](../../../../finstack-quant/analytics/tests/correctness_regressions.rs).
- [Coherent-risk reference](../../../REFERENCES.md#artzner1999CoherentRisk) is contextual. Full estimator/provenance references for the Rockafellar–Uryasev convention named in source and independent implementation comparisons remain unestablished; no external source was newly authenticated.

No historical observations were fetched and no risk estimates were executed.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[analytics follow-ups](../FOLLOW_UPS.md#analytics).
