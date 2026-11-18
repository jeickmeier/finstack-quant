# FACTOR-005: Covariance factor-risk decomposition

**Decision status:** Accepted 2026-11-17 for covariance-based variance, volatility and signed normal tail-risk allocation.

**Implementation status:** Covariance forms, risk transforms and signed component allocations are implemented. Checked scale-aware arithmetic, reconciliation and revised zero-risk fields remain pending.

**Validation status:** All 12 focused factor-risk Rust tests passed on 2026-11-17 against current behavior. An independent exact-rational case verified signed component conservation. The accepted extreme-scale and zero-risk behavior remains to be qualified.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision

1. Require the admitted factor basis, covariance horizon and reporting currency. Sum the already-weighted position sensitivities once to obtain `b`. Use `v = bᵀΣb` and volatility `sqrt(v)`. Use zero-drift normal VaR and ES on the signed P&L axis, with `-z sqrt(v)` and `-φ(z) sqrt(v)/(1-confidence)`, respectively. Variance is in reporting currency squared; volatility and both tail measures are in reporting currency. [Factor configuration](factor-configuration.md).
2. Set `q = Σb`. Keep signed factor variance allocations `bⱼqⱼ`, position-factor allocations `deltaᵢⱼqⱼ` and the separate variance derivative `2qⱼ`. Apply the measure's common scale to factor and position-factor allocations for volatility, VaR and ES. With `b=[2,-1]` and covariance `[[1,0.8],[0.8,1]]`, allocations `+2.4` and `−0.6` sum to variance `1.8`.
3. Use checked scale-aware arithmetic and retire the fixed absolute variance clamp. Return a computation error for unresolved negative variance or nonfinite results. Verify position-to-factor and factor-to-total conservation. At verified zero risk, allocations are zero, relative shares and transformed marginals are undefined, and the variance derivative remains defined.

## Observed choice and rationale

Allocate portfolio variance or its normal-distribution transforms using factor exposures and a common covariance matrix. Variance components are half-Euler allocations (summing to variance), while reported variance marginals retain the derivative factor of two.

## Input domain and conventions

Sensitivity rows must be finite and exactly align with covariance factor IDs/order. Sum rows to factor exposures b. Covariance has the factor coordinate/horizon units needed to make bᵀΣb a monetary variance. VaR/ES are negative on the P&L axis; confidence is finite in (0.5,1). No expected-return drift or horizon conversion is inserted.

## Algorithm stages

1. Compute b_j = Σ_i delta_ij, q = Σb, c_j = b_j q_j, and v = Σ_j c_j.
2. Reject nonfinite v and v < -1e-12; floor remaining negative roundoff to zero. Let sigma = sqrt(v).
3. Transform using `(total,scale)`: variance `(v,1)`; volatility `(sigma,1/sigma)`; normal VaR `(-z_alpha sigma,-z_alpha/sigma)`; ES `(-phi(z_alpha)sigma/(1-alpha),-phi(z_alpha)/((1-alpha)sigma))`. Zero sigma gives zero scale for non-variance measures.
4. Factor absolute contribution is c_j times scale; relative is absolute/total or zero. Marginal is q_j times scale, doubled only for variance.
5. Position-factor contribution is delta_ij q_j times scale. Initial residual fields are zero/empty. Empty factors return zero risk and empty contributions.

## Selection and controls

`RiskMeasure` is explicit; default is variance. Calculation uses ordinary serial floating-point additions in stored axis/position order. Normal quantiles/density come from core; no historical or Cornish–Fisher distribution is selected here.

## Limitations, failures, and fallbacks

An absolute variance floor of 1e-12 is not currency/scale normalized. Portfolio-level finiteness is checked; the inspected evidence does not qualify factor contributions and cancellation effects across scales. Negative contributions can represent hedges. Zero-factor output does not enumerate zero rows for each position. A valid covariance cannot establish that linear sensitivities capture nonlinear position P&L.

## Alternatives and tradeoffs

Historical/scenario risk can capture supplied nonlinear P&L at greater data/compute cost. A nonlinear factor expansion changes exposure and covariance contracts. Compensated accumulation could improve cancellation behavior; adoption must preserve the intended determinism tier.

## Implementation and evidence

- [src/risk/parametric.rs](../../../../finstack-quant/factor/src/risk/parametric.rs) — inspected implementation.
- [src/risk/mod.rs](../../../../finstack-quant/factor/src/risk/mod.rs) — inspected implementation.
- [src/risk/types.rs](../../../../finstack-quant/factor/src/risk/types.rs) — inspected implementation.

The [focused Rust tests](../../../../finstack-quant/factor/src/risk/parametric.rs) passed for current measures, axes, rank-deficient PSD, nonfinite sensitivities and zero risk. Qualify cancellation, extreme scales, signed hedge contributions and finite-difference marginals under the accepted contract.

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models).
