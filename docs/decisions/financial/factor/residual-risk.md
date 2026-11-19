# FACTOR-006: Residual variance overlay and contribution rescaling

**Decision status:** Accepted 2026-11-17 for checked additive residual variance and common risk transformation.

**Implementation status:** Single-overlay variance addition and component rescaling are implemented in part. Direct retention of variance, basis and source reconciliation, and checked zero-overlay admission remain pending.

**Validation status:** Both focused residual Rust tests passed on 2026-11-17 against current behavior. Independent arithmetic verified the combined variance and signed allocation example. The accepted source and zero-overlay checks remain to be qualified.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision

1. Bind systematic variance and residual sources to the same reporting currency and horizon, and add variance under an explicit zero cross-covariance assumption. Positions on one issuer share its residual shock and net before squaring. The [accepted credit residual model](../portfolio-attribution/13-credit-residual-risk-and-reporting.md#accepted-decision-for-checked-issuer-residual-risk-and-hierarchy-reporting) assumes zero covariance between distinct issuer residuals and between residuals and systematic factors.
2. Carry the checked systematic variance and allocations directly from [FACTOR-005](factor-risk.md). Require nonnegative residual source totals and reconcile their signed position allocations. Transform the combined variance once and use a common component scale for the selected measure. With `25 USD²` systematic variance and `+22` and `−11 USD²` residual allocations, the combined variance is `36 USD²` and volatility is `6 USD`.
3. Check the factor-only result, position and source identities, finite values and conservation before committing an overlay. Apply those checks to zero overlays, retain covered zero rows and reject repeated overlays without changing the result. Publish variance allocations and their transformed contributions explicitly.

## Observed choice and rationale

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

[Residual overlay tests](../../../../finstack-quant/factor/src/risk/residual.rs) passed for current successful rescaling, signed allocations, invalid totals and rejected repeated application. Qualify the accepted shared basis, source-level reconciliation, retained variance fields and zero-overlay behavior.

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models).
