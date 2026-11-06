# FACTOR-014: Sample and normalized EWMA credit variance estimation

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the stated source scope; limitations and discrepancies below remain open.

**Validation status:** Evidence inspected or located, not executed. No numerical qualification is implied.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Estimate annualized variance either from demeaned sample changes or a normalized finite-window zero-mean EWMA. Even when betas are fitted on Levels, volatility is estimated on changes of the resulting factor and residual series.

## Input domain and conventions

Statistical observations are bp changes, producing bp² annualized variance. Public calibration supplies complete panels; private helpers filter None observations. Annualization A is 252/12/4, selected by panel frequency. Sample and EWMA both return zero with fewer than two valid observations.

## Algorithm stages

1. In Returns mode, use peeled working series directly. In Levels mode, first-difference both each factor series and each issuer residual.
2. Sample variance is A*sum((r-mean)²)/(n-1), floored at zero.
3. EWMA starts acc=0 and folds oldest to newest: acc=lambda*acc+(1-lambda)*r². Normalize by 1-lambda^n and multiply by A, then floor at zero. This is a zero-mean second-moment estimate, unlike the demeaned Sample method.
4. Store selected factor variances as Sample or Ewma{lambda,variance}; store idiosyncratic state from the assigned annualized residual volatility squared.
5. Stored factor histories retain the original peel space: Returns dates drop the first panel date; Levels histories retain levels/dates even though variance used changes. Missing factor-history values fail assembly rather than being zero-filled.

## Selection and controls

The default is Sample; EWMA requires finite lambda strictly in (0,1). Missing observations inside private EWMA are skipped with no extra calendar-time decay. No GARCH/EGARCH fitting or forward re-estimation is implemented. Covariance strategy may later replace systematic covariance (LedoitWolf) while retaining these variance states as diagnostics.

## Limitations, failures, and fallbacks

Sample and EWMA estimate different mean conventions. Returning zero with insufficient history is a numerical fallback, not established absence of risk. The inspected evidence does not qualify near-one lambda normalization or large values. An artifact history field called values is not uniformly returns; its panel-space metadata is essential.

## Alternatives and tradeoffs

Weighted-demeaned EWMA, an initialized infinite-history recursion, elapsed-time decay and model-based volatility forecasts answer different questions. A minimum-history error would be stricter than a zero fallback. Their adoption requires explicit changes rather than assuming all EWMA variants coincide.

## Implementation and evidence

- [src/credit/calibration/statistics.rs](../../../../finstack-quant/factor/src/credit/calibration/statistics.rs) — inspected implementation.
- [src/credit/calibration/calibrator.rs](../../../../finstack-quant/factor/src/credit/calibration/calibrator.rs) — inspected implementation.
- [src/credit/calibration/assemble.rs](../../../../finstack-quant/factor/src/credit/calibration/assemble.rs) — inspected implementation.
- [src/credit/hierarchy.rs](../../../../finstack-quant/factor/src/credit/hierarchy.rs) — inspected implementation.

[src/credit/calibration/tests.rs](../../../../finstack-quant/factor/src/credit/calibration/tests.rs) includes inspected hand calculations: sample [0,2] gives variance 2; EWMA lambda 0.5 over [2,1] gives 2; inserting None and multiplying by 12 gives 24; [1,1,3] gives 39/7. [RiskMetrics](../../../REFERENCES.md#jpmorgan1996RiskMetrics) is a general EWMA reference, not evidence that every normalization/mean convention here is identical to a cited production model.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models). This record does not
authorize a runtime change or certify the observed method.
