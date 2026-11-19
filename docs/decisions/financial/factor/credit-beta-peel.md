# FACTOR-013: Sequential generic and leave-one-out hierarchy beta fitting

**Decision status:** Accepted 2026-11-17.

**Implementation status:** The sequential peel is implemented. Fit admission, explicit proxy policy, and fit and application diagnostics require reconciliation.

**Validation status:** The targeted leave-one-out test and both peel tests passed. Fit admission, fallback provenance, and held-out performance still require qualification.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision

1. Fit the generic slope by centered OLS and remove only `beta × factor`. Then process active buckets from broadest to narrowest. At each step, fit the issuer slope against the weighted leave-one-out mean of pre-step residuals and apply the loading to the published inclusive factor. Folded levels retain zero loadings.
2. Require at least three aligned observations and an identifiable regressor for an estimated loading. Apply shrinkage toward one only when explicitly selected. `BucketOnly` uses elected unit loadings. An unavailable fit requires an explicitly authorized unit proxy with a recorded reason, or calibration fails. Nonfinite arithmetic produces an error.
3. Distinguish fitted, elected, and proxy loadings. Report the raw fitted slope when available, the applied loading after any shrinkage, and the residual diagnostics measured on application to the inclusive factor. Preserve negative residual R² when it occurs.

## Observed choice and rationale

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

[src/credit/calibration/tests.rs](../../../../finstack-quant/factor/src/credit/calibration/tests.rs) contains a passing synthetic six-name case (five identical plus one doubled series). Exact arithmetic gives a leave-one-out slope of 2 and an inclusive slope of 12/7. Both targeted peel tests passed and cover observed behavior. [Analytics beta](../../../../finstack-quant/analytics/src/benchmark.rs) supplies the centered slope and three-observation minimum.

## Follow-up tracking

Implementation reconciliation and independent qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models).
