# FACTOR-016: Credit covariance strategies and correlation repair

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the stated source scope; limitations and discrepancies below remain open.

**Validation status:** Evidence inspected or located, not executed. No numerical qualification is implied.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

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

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models). This record does not
authorize a runtime change or certify the observed method.
