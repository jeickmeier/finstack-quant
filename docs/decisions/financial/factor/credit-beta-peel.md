# FACTOR-013: Sequential generic and leave-one-out hierarchy beta fitting

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the stated source scope; limitations and discrepancies below remain open.

**Validation status:** Evidence inspected or located, not executed. No numerical qualification is implied.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Fit issuer loadings sequentially: remove the external generic factor, then weighted hierarchy means of successive residuals. For issuer-specific level betas, regress on a leave-one-out bucket mean to reduce mechanical self-inclusion; residual peeling still uses the all-member factor.

## Input domain and conventions

The working panel is bp levels or bp differences, with time-aligned issuer/generic series and positive weights. BucketOnly mode uses unit active loadings; folded levels stay zero. Dynamic issuer mode does not guarantee a successful regression: the delegated analytics beta needs at least three paired observations and a nondegenerate regressor.

## Algorithm stages

1. Store the generic factor series. For each issuer, fit the centered OLS slope on generic (or use 1 for BucketOnly), then apply shrinkage beta=(1-alpha)beta_fit+alpha. A missing/NaN fit falls back to 1.
2. Subtract beta_generic*generic without subtracting a fitted intercept. Initialize all hierarchy betas to zero.
3. At level k, exclude folded names and build weighted residual means F_g,t=sum(w*r)/sum(w) from the remaining members. Missing/nonpositive weights are skipped internally.
4. BucketOnly takes beta_k=1. IssuerBeta first checks the inclusive factor for degeneracy and whether the child has exactly the same members as its parent. Non-refining/degenerate cases fall back to 1. Otherwise subtract that issuer contribution/weight from the bucket to form a leave-one-out regressor, fit a centered slope and shrink it.
5. Update each residual with beta_k times the inclusive factor, preserving missingness. Store the inclusive factor, not its issuer-specific leave-one-out series.
6. Fit quality for >=3 points uses RSS=sum(y-beta*x)², R²=1-RSS/TSS (0 when TSS=0), and residual_std=sqrt(RSS/n). Generic quality includes fallback fits; level quality is present only for a successful leave-one-out fit and is evaluated against the inclusive factor.

## Selection and controls

Degenerate means centered regressor sum of squares <=1e-12 times centered dependent sum of squares, before delegating to `analytics::beta`. Shrinkage is None or TowardOne(alpha), bounded at calibration entry. Levels and hierarchy order are explicit inputs and alter later residual regressors. No constrained beta range or simultaneous multivariate regression is used.

## Limitations, failures, and fallbacks

The centered regression slope and zero-intercept peel are deliberate distinct operations; fit quality is not the conventional intercept-adjusted OLS R² and can be negative. A successful leave-one-out coefficient is applied to a different inclusive regressor, so in-sample residuals are not the fitted LOO residuals. Fallback beta 1 and missing fit quality must not be reported as a successful estimated loading. The helper treats NaN beta as missing but does not separately reject infinity.

## Alternatives and tradeoffs

Simultaneous constrained regression can reduce order dependence but loses the simple nested attribution. Through-origin fitting would align the coefficient objective with the peel while changing estimates. Cross-fitting entire bucket time series or shrinkage toward parent/loadings offers other bias-variance tradeoffs.

## Implementation and evidence

- [src/credit/calibration/peel_fit.rs](../../../../finstack-quant/factor/src/credit/calibration/peel_fit.rs) — inspected implementation.
- [src/credit/calibration/panel.rs](../../../../finstack-quant/factor/src/credit/calibration/panel.rs) — inspected implementation.
- [src/credit/calibration/calibrator.rs](../../../../finstack-quant/factor/src/credit/calibration/calibrator.rs) — inspected implementation.

[src/credit/calibration/tests.rs](../../../../finstack-quant/factor/src/credit/calibration/tests.rs) contains an inspected synthetic six-name case (five identical plus one doubled series) checking LOO beta near 2 and distinct from inclusive beta 12/7; also non-refining/unit fallback and per-level quality cases. [Analytics beta](../../../../finstack-quant/analytics/src/benchmark.rs) supplies the centered slope and >=3-observation rule. These are constructed examples, not out-of-sample validation.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models). This record does not
authorize a runtime change or certify the observed method.
