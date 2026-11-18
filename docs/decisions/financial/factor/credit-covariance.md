# FACTOR-016: Credit covariance strategies and correlation repair

**Decision status:** Accepted 2026-11-17.

**Implementation status:** Four strategies and forecasting from stored covariance are implemented. Common-panel admission, explicit repair controls, and published covariance-derived measures require reconciliation.

**Validation status:** Exact arithmetic reproduced fixture shrinkage intensity 17/18 and annualized covariance with diagonal of 30 bp² and off-diagonal of −1 bp². The Rust fixture was inspected. Repair limits and held-out performance remain to be qualified.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision

1. Explicitly elect Diagonal, FullSampleRepaired, Ridge, or LedoitWolf. Use one complete aligned panel of bp changes with at least two observations. Diagonal uses elected Sample or EWMA marginal variances. FullSampleRepaired combines those marginals with sample Pearson correlation. Ridge adds a declared finite nonnegative amount in annualized bp² to each diagonal of the same construction. LedoitWolf supplies the full annualized systematic covariance through the core scaled-identity estimator. Sample or EWMA still governs issuer residual variance. [Variance estimators](credit-variance.md) · [Core shrinkage](../core-numerics/covariance-shrinkage.md).
2. Require defined Pearson correlation between positive-variance factors. If estimated correlation fails PSD admission, FullSampleRepaired and Ridge may use the core nearest-correlation projection with explicit controls. Require a declared maximum repair distance and record the actual distance. Fail if projection exceeds the limit or final covariance fails validation. [Core projection](../core-numerics/nearest-correlation.md) · [Covariance admission](matrix-invariants.md).
3. Use the stored annualized covariance for systematic risk and forecasts. Derive published systematic variances and correlations from it, with correlation undefined on zero-variance axes. If retained, label pre-ridge correlation and standalone Sample or EWMA estimates as diagnostics.

## Observed choice and rationale

Assemble systematic covariance by explicit Diagonal, Ridge, FullSampleRepaired or LedoitWolf strategy. The default is FullSampleRepaired. These are materially different estimators, not interchangeable serialization choices.

## Input domain and conventions

Factor ordering places generic first, then bucket IDs lexically. Factor variances are annualized bp² from Sample/EWMA; factor observations are bp changes (even after a Levels fit). Ridge alpha is additive annualized bp² on each diagonal. Complete covariance must pass the factor matrix PSD validator.

## Algorithm stages

1. Diagonal uses the selected factor variances and identity correlation.
2. FullSampleRepaired estimates pairwise Pearson correlation on overlapping present pairs via OnlineCovariance (finiteness is established at the public calibration entry, not by this helper), with diagonal one. Insufficient/constant overlap yields the core zero-correlation result. Validate correlation; only on failure apply core nearest-correlation with default options. Form Σ=D rho D using selected marginal standard deviations.
3. Ridge uses the same repaired-or-original sample correlation and D rho D, then adds alpha to every covariance diagonal. The stored static correlation remains the pre-ridge correlation.
4. LedoitWolf constructs complete-case observation rows across all factors, requires at least two rows, invokes core identity-target shrinkage, and multiplies the resulting covariance by annualization A. Derive a separate correlation from that covariance, with diagonal one, zero correlation for zero variance and off-diagonal clamp [-1,1].
5. Store covariance after PSD validation. Assemble credit-only factor definitions with empty curve IDs (dependency mapping is supplied by credit matching), require issuer identity in matching, use variance risk, and explicitly choose Warn for unmatched dependencies.

## Selection and controls

LedoitWolf supplies the entire systematic covariance independently of the selected Sample/EWMA variance model. That volatility choice still controls issuer idiosyncratic variance and stored diagnostic factor variance state. Ridge preserves its alpha in the authoritative covariance; recomputing D rho D from diagnostic vol_state would lose it. Forecasting correctly scales the stored covariance.

## Limitations, failures, and fallbacks

Pairwise support exists internally; public calibration rejects missing panels. Correlation repair is conditional and its distance is not exposed in this model diagnostics. Unit diagonal/static-correlation shape validation alone is not PSD proof. Ridge static correlation is not the correlation of the final ridge covariance, by construction. Missing factor variance lookups become zero internally. Warnings and unmatched-factor policy are downstream consumer boundaries.

## Alternatives and tradeoffs

A coherent shrinkage model for both covariance and all variance state, weighted/EWMA correlation, a factor-structured target, or rejection instead of repair would change statistical assumptions. LedoitWolf can improve conditioning but its complete-case requirements and identity target have their own limitations.

## Implementation and evidence

- [src/credit/calibration/statistics.rs](../../../../finstack-quant/factor/src/credit/calibration/statistics.rs) — inspected implementation.
- [src/credit/calibration/assemble.rs](../../../../finstack-quant/factor/src/credit/calibration/assemble.rs) — inspected implementation.
- [src/credit/calibration/config.rs](../../../../finstack-quant/factor/src/credit/calibration/config.rs) — inspected implementation.

[src/credit/calibration/tests.rs](../../../../finstack-quant/factor/src/credit/calibration/tests.rs) contains an inspected hand-derived LedoitWolf adapter example with intensity 17/18, annual covariance [[30,-1],[-1,30]] and correlation -1/30, plus overlap Pearson cases. [tests/credit_calibration.rs](../../../../finstack-quant/factor/tests/credit_calibration.rs) locates Ridge alpha, repair PSD and EWMA/LedoitWolf independence tests. [Ledoit–Wolf](../../../REFERENCES.md#ledoitwolf2004) is the primary catalogued estimator anchor; [core matrix methods](../core-numerics/README.md) own shrinkage/repair kernels.

## Follow-up tracking

Implementation reconciliation and independent qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models).
