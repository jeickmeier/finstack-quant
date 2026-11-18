# FEATURES-020: Scaled full-rank SVD neutralization and standardized residuals

**Decision status:** Accepted 2026-11-17.

**Implementation status:** Scaled full-rank OLS is implemented. Raw residual preservation and whole-vector roundoff handling before standardization require reconciliation.

**Validation status:** Exact arithmetic checked fitted residuals and their orthogonality. Rust fixtures were inspected. Rank boundaries and roundoff handling still require qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See [scope and source ledger](README.md).

## Accepted decision

1. Fit equal-weighted OLS on complete rows in each exact time partition, with an intercept by default. Keep scaled SVD and the rank cutoff ε × max(n, p) × σmax, where p counts fitted coefficients. Allow exactly determined full-rank fits. An underdetermined or rank-deficient partition fails the call with partition context.
2. Return checked observed minus fitted residuals in response units. Exactly determined full-rank fits yield zero residuals in exact arithmetic. Preserve small representable residuals from overdetermined fits.
3. In neutralize_and_zscore, always fit an intercept and standardize residuals using population standard deviation. Return zero scores if the entire residual vector is within a qualified solver-roundoff bound. Otherwise standardize the full vector. [Population standardization](14-cross-sectional-standardization.md).

For exposures [1, 2, 3] and responses [1, 1, 2], the residuals are [1/6, −1/3, 1/6]. Their sum and exposure dot product are both zero.

## Observed scope and actual selection

`neutralize` performs equal-weighted OLS separately by exact time key, with `fit_intercept=true` by default (the only parameter). Exposure input is column-major: each numeric optional column must match the response length. Missing/nonfinite response or any exposure excludes that row from both fitting and residual output. The same solver supplies [rolling residuals](21-rolling-regression-residuals.md), whose failure/output policy differs.

## Ordered fit and evaluation

1. Width p is number of exposure columns plus the intercept flag. Collect complete rows n. Return no fit if p=0 or n<max(p,min_rows). Cross-sectional neutralization passes min_rows=0, so exactly determined n=p fits are allowed; residual inference or degrees-of-freedom estimates are not calculated.
2. Scale response by maximum absolute complete response (use 1 for all-zero y). For each exposure, scale by its maximum absolute complete value; zero scale gives no fit. With intercept, center normalized exposure by its scaled mean; without intercept use center 0. Divide the centered normalized exposure by its maximum absolute deviation; zero spread gives no fit. Prepend an all-ones column when fitting an intercept.
3. Compute `nalgebra::linalg::SVD::try_new(design,true,true,f64::EPSILON,1000)`. Failure to converge is Validation. Let `cutoff=EPSILON*max(n,p)*max_singular`; require `svd.rank(cutoff)>=p`. Rank deficiency returns no fit; there is no ridge, pseudoinverse rank truncation or fallback method.
4. Solve the scaled response using the same cutoff and reject solver errors or nonfinite coefficients. Store coefficients in normalized coordinates plus scales/centers/spreads, rather than unscaling them to raw exposure units.
5. Evaluate each complete row using those same normalized coordinates. Accumulate fitted value and `M=abs(y_scaled)+abs(intercept_term)+sum(abs(exposure_term))`. Let `r=y_scaled-fitted`, `tau=8*EPSILON*max(n,p)`. Set residual to 0 when `abs(r)<=tau*M`; otherwise emit `r*response_scale`. This threshold intentionally suppresses solver roundoff before subsequent standardization.
6. Cross-sectional no-fit is a whole-call Validation naming the time key as singular or underdetermined. SVD/solve/nonfinite failures have their own messages and need not name the partition. Successful residuals are checked for finiteness. `neutralize_and_zscore` always uses the default intercept fit, then population cross-sectional z-scores its residuals; there is no no-intercept override for that helper.

Residuals retain response units; standardized residuals are dimensionless. The intercept supports zero-mean residuals in exact OLS, and subsequent common scaling preserves exact exposure orthogonality. Numerical thresholding, centering and arithmetic can perturb that identity, so it is not an exact floating-point guarantee.

## Rationale and alternatives

Comments explicitly choose scaled SVD to avoid squaring the design condition number through X'X and choose near-zero residual suppression to avoid amplifying roundoff into unit-variance signals. [Matrix Computations](../../../REFERENCES.md#golub-van-loan-matrix-computations) is the repository numerical foundation. No independent provenance establishes the exact rank multiplier, 1,000-iteration limit or residual threshold as accepted calibration constants. Proposed alternatives include QR, a declared condition-number exclusion, robust/weighted regression or ridge; each changes the fitted method and none is selected.

## Evidence

- Owning [multi-input OLS](../../../../finstack-quant/features/src/multi.rs): `neutralize`, `residualize_partition`, `fit_ols`, `residual_for_idx`, `neutralize_and_zscore`.
- Inspected [transform tests](../../../../finstack-quant/features/tests/transforms.rs): hand exposure residuals, singular constant exposure with partition in error, and residual z-scores, tolerance 1e-12.
- Inspected [audit regressions](../../../../finstack-quant/features/tests/audit_regressions.rs): exposures scaled 1e-150 through 1e150 preserve hand residuals [1/6,-1/3,1/6]; duplicate columns fail; standardized residuals have near-zero sum/exposure dot product; an exact line yields zeros. Audit tolerance is 1e-11*max(1,abs(expected)); exact-fit zeros use exact assertions.

No solve or threshold experiment was executed.

## Follow-up tracking

Implementation reconciliation and qualification work are tracked
in the [feature follow-ups](../FOLLOW_UPS.md#features).
