# FACTOR-007: Normal position VaR, ES, marginals, and removal risk

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the stated source scope; limitations and discrepancies below remain open.

**Validation status:** Evidence inspected or located, not executed. No numerical qualification is implied.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Compute normal one-period position risk with covariance Euler contributions and optional exact remove-one-position VaR changes. Confidence defaults to 0.95 and losses are negative.

## Input domain and conventions

Position IDs align one-for-one with finite weights; flat covariance is n×n, finite, symmetric within the covariance tolerance and PSD. Weights may be signed and need not sum to one. Their units determine whether outputs are returns or monetary P&L when multiplied by position-return covariance; no currency or annualization is inserted. Duplicate position IDs are not rejected here.

## Algorithm stages

1. Form q=Σw and v=wᵀq. Reject v < -1e-12, floor smaller negative values to zero, and set sigma=sqrt(v).
2. Report VaR=-z_alpha sigma and ES=-phi(z_alpha)sigma/(1-alpha).
3. If sigma>sqrt(1e-12), set inverse sigma to 1/sigma; otherwise warn and use zero for all component/marginal calculations while retaining totals.
4. For each position use component VaR=-w_i q_i z_alpha/sigma and marginal VaR=-q_i z_alpha/sigma; use the ES multiplier analogously. Relative fields divide by total only when its magnitude exceeds 1e-12.
5. Optional removal risk uses v_without_i=max(v-2w_iq_i+w_i²Σ_ii,0), then VaR_full - VaR_without_i. A single position gets its entire VaR.
6. Report Euler residual as portfolio VaR minus summed component VaR; empty portfolios return zero.

## Selection and controls

`parametric_var_decomposition` accepts an optional confidence and a Boolean incremental switch. The report wrapper selects this path explicitly. No drift, nonlinear revaluation or stressed covariance is generated internally. Accumulation is serial ordinary f64 in input order.

## Limitations, failures, and fallbacks

The small-sigma cutoff deliberately permits nonzero totals with zero components, contrary to an unqualified exact-Euler claim. Unlike the factor-level transform, this path does not reject nonfinite accumulated variance/results after finite input validation. Static IEEE arithmetic with weight 1e200 and covariance [1] overflows the quadratic variance and can produce infinity/NaN components; no runtime reproduction was performed. Removal variance is clamped unconditionally.

## Alternatives and tradeoffs

A consistently scaled zero-risk policy or relative tolerance could preserve scale invariance. Recomputing each reduced portfolio is slower but supplies a useful independent removal-risk oracle. Historical or nonlinear scenario P&L avoids normality assumptions but changes the decomposition convention.

## Implementation and evidence

- [src/risk/position.rs](../../../../finstack-quant/factor/src/risk/position.rs) — inspected implementation.
- [src/risk/mod.rs](../../../../finstack-quant/factor/src/risk/mod.rs) — inspected implementation.
- [src/covariance.rs](../../../../finstack-quant/factor/src/covariance.rs) — inspected implementation.

[src/risk/position.rs](../../../../finstack-quant/factor/src/risk/position.rs) tests include hand-structured covariance portfolios, rank deficiency, signed losses, Euler exhaustion, single/empty cases and removal-risk checks. [Tasche](../../../REFERENCES.md#tasche-2008-capital-allocation) and [Meucci](../../../REFERENCES.md#meucci-risk-and-asset-allocation) are theory anchors. Existing finite-input tests are not overflow evidence.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models). This record does not
authorize a runtime change or certify the observed method.
