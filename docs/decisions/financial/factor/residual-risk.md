# FACTOR-006: Residual variance overlay and contribution rescaling

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the stated source scope; limitations and discrepancies below remain open.

**Validation status:** Evidence inspected or located, not executed. No numerical qualification is implied.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Apply all residual variance allocations once to a factor-only decomposition, treating their aggregate as additive to systematic variance. The caller owns derivation of residual allocations and their issuer/position dependence assumptions.

## Input domain and conventions

Individual residual variance allocations must be finite and may be negative; only their aggregate must be nonnegative. Signed allocations can express hedging within a shared residual source. Their units are variance, even when total risk is volatility/VaR/ES. `source` identifies provenance but is not used to infer covariance.

## Algorithm stages

1. Reject a decomposition already carrying residual risk or residual rows. Validate its risk measure and finite total.
2. Sum the supplied residual variances; reject a nonfinite or negative aggregate. If zero, append rows and return.
3. Recover systematic variance from the existing measure: total for variance, total² for volatility, or squared total divided by the normal tail multiplier for VaR/ES; reject a wrong total sign on this nonzero-overlay path.
4. Add aggregate residual variance and obtain the new measure scale from the shared factor-risk transform.
5. Rescale existing factor absolute/marginal and position-factor contributions by new_scale/old_scale (zero if old scale is zero). Recompute relative factor shares, replace total risk, set residual risk to residual_variance times new_scale, and retain supplied per-position variance rows.

## Selection and controls

The overlay is an explicit API call; no residual risk is synthesized by `decompose_factors`. It assumes a factor-only input whose total corresponds to the selected risk measure and whose contribution fields are internally consistent. The aggregate is not a direct sum of volatilities or VaR values.

## Limitations, failures, and fallbacks

The zero-aggregate early return skips total-sign recovery; it can retain cancelling nonzero position allocations. The helper does not verify position IDs, duplicate rows, contribution conservation, or finiteness of existing contribution fields. It does not compute cross-covariance between systematic and residual returns. The retained per-position residual fields remain variances, not already-scaled contributions.

## Alternatives and tradeoffs

A joint augmented covariance matrix would model residual correlations explicitly and make all marginals endogenous, at a larger state cost. Rejecting negative individual allocations would be simpler but would exclude signed Euler allocations from shared issuer risk.

## Implementation and evidence

- [src/risk/residual.rs](../../../../finstack-quant/factor/src/risk/residual.rs) — inspected implementation.
- [src/risk/types.rs](../../../../finstack-quant/factor/src/risk/types.rs) — inspected implementation.

[src/risk/residual.rs](../../../../finstack-quant/factor/src/risk/residual.rs) contains overlay/rescaling and repeated-application checks. [Euler allocation reference](../../../REFERENCES.md#tasche-2008-capital-allocation) supports the interpretation; derivation of the actual residual allocations belongs to the portfolio/model caller and must be evidenced separately.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models). This record does not
authorize a runtime change or certify the observed method.
