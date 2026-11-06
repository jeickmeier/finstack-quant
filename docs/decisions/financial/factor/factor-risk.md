# FACTOR-005: Covariance factor-risk decomposition

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the stated source scope; limitations and discrepancies below remain open.

**Validation status:** Evidence inspected or located, not executed. No numerical qualification is implied.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

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

[Meucci](../../../REFERENCES.md#meucci-risk-and-asset-allocation) and [Tasche](../../../REFERENCES.md#tasche-2008-capital-allocation) are conceptual references, not proofs of this implementation. [src/risk/parametric.rs](../../../../finstack-quant/factor/src/risk/parametric.rs) tests locate hand-structured two-factor component calculations, all measures, exact axes, rank-deficient PSD, nonfinite sensitivities and zero risk.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models). This record does not
authorize a runtime change or certify the observed method.
