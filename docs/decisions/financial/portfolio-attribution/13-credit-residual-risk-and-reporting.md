# PORTFOLIO-ATTRIBUTION-013: Issuer-netted credit residual risk and hierarchy reporting

- ID: PORTFOLIO-ATTRIBUTION-013
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the stated owning source scope; discrepancies are named below.
- Validation status: Evidence inspected/located, not executed; independent accuracy qualification remains open.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and domain

The portfolio adapter augments factor-decomposed risk with issuer-specific residual spread variance from credit hierarchical matching configuration. `adder_vol_annualized` is squared into residual variance in the factor model's spread units (basis-point shocks); the factor crate converts combined variance to the selected risk measure. This adapter squares annualized residual volatility directly and does not apply a holding-horizon scale. This is a shared issuer shock across positions, not independent position noise.

## Algorithm

1. Gather issuer residual variances from CreditHierarchical matching configurations. The collector traverses Cascade children in order and uses map insertion, but `FactorModel::from_config` first runs factor configuration validation, which rejects duplicate issuer IDs across nested Cascade children. Therefore replacement of a duplicated issuer is not an admissible constructed-model behavior.
2. For each position, read issuer identity from instrument metadata. Missing identity, missing calibrated issuer or nonpositive residual variance contributes no residual row. Sum that position's central spread-quote exposures over declared credit dependencies using [credit exposure construction](12-factor-assignment-and-credit-exposures.md).
3. Aggregate signed net exposure `E_j=sum_p e_p` and gross exposure `G_j=sum_p |e_p|` by issuer in deterministic key order. Total issuer variance is `E_j^2*sigma_j^2`.
4. Allocate issuer variance to its positions as `E_j*e_p*sigma_j^2`. A hedge leg can have negative allocated variance. If `|E_j|<=1e-12*G_j`, every issuer row is zero; this is the explicit near-flat cutoff. Send position residual contributions to the factor crate's `apply_residual_contributions`, which owns total-risk and Euler rescaling semantics.
5. `build_credit_vol_report` groups factor contributions into the generic credit factor or parsed hierarchy level/bucket. Unknown factor IDs or out-of-range levels error. Bucket maps and optional position rows are deterministically sorted.
6. Sum residual variance allocations and convert each to residual risk contribution by `residual_risk/sum_residual_variance` when the denominator is positive, otherwise zero. Add per-position factor contributions to those residual contributions; copy total risk, risk measure and aggregate residual risk from the decomposition. This report consumes a decomposition; it does not independently revalidate all of its reconciliation identities.

## Horizon composition discrepancy

The factor forecast's `factor_model_at(horizon,measure)` scales systematic covariance for the requested horizon but clones matching configuration unchanged. Constructing this portfolio FactorModel from that config still reads annual `adder_vol_annualized^2`. The residual overlay has no horizon input. Consequently a nonannual forecast config can combine horizon-scaled systematic variance with unscaled annual issuer residual variance. This is a source-traced composition discrepancy; numerical impact was not executed. The [factor forecast owner](../../../../finstack-quant/factor/src/credit/forecast.rs) provides the config transformation, while this record owns how its unchanged matching rows are consumed.

## Alternatives and limitations

Summing `e_p^2*sigma_j^2` independently by position overstates a hedged issuer's residual variance. Full issuer-to-issuer residual covariance would differ from the selected uncorrelated residual model. Position contributions can be negative despite a nonnegative total variance, as expected for hedges. Missing issuer calibration is currently omission, not an unmatched-policy error.

The prior configuration validation is essential to the collector's unique-issuer assumption; the collector itself is not a second validator. Aggregation uses ordinary sums in several exposure/report paths; extreme cancellation and overflow require qualification beyond finite per-exposure guards.

## Evidence

[Model source and issuer-netting/flat-book tests](../../../../finstack-quant/portfolio/src/factor_model/model.rs), [report builder and rejection tests](../../../../finstack-quant/portfolio/src/factor_model/credit_vol_report.rs), and the [factor residual-risk methodology](../factor/residual-risk.md) establish ownership and invariant evidence. [Tasche](../../../REFERENCES.md#tasche-2008-capital-allocation) provides Euler allocation context; no independent calibration or regulatory interpretation is claimed.

The display label “credit vol” must retain its explicit `measure` because the same report can carry another factor risk measure.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution). This record does not
authorize a runtime change or certify the observed method.
