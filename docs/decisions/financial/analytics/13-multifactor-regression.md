# ANALYTICS-013: Multifactor OLS, return-kind conversion and residual risk

**Decision status:** Draft — observed implementation; adoption pending for choices not already settled. Existing accepted directions remain reconciliation requirements.

**Implementation status:** Observed in the source paths below, including the stated discrepancies and fallback behavior.

**Validation status:** Source inspected; test evidence inspected or located as identified below, not executed. No financial accuracy certification or runtime change is implied.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope and exact source manifest](README.md).

## Choice, scope and domain

Multifactor exposure is equal-weight OLS with an intercept and full column rank. `Performance.multi_factor_greeks` supplies the ticker's active returns, but factor slices have no dates: callers must already align every factor exactly to that ticker span. At least one factor and N>=k+2 observations are required for positive residual degrees of freedom. Nonfinite factors or mismatched lengths error. No risk-model factor construction or time alignment occurs in this crate.

## Algorithm and selection

`ReturnKind::Excess` is the default and uses supplied response returns unchanged. `Total {risk_free_rate}` converts annual effective cash to q and subtracts it from the dependent response only; factor columns remain unchanged and must have the intended interpretation supplied by the caller. A nonfinite converted q errors. `from_label` accepts omitted/"excess" only when its separate cash argument is zero, rejects ignored nonzero cash, accepts "total", and rejects unknown labels.

Build X with an all-one intercept and k factor columns, then solve using the normalized full-rank SVD kernel in [record 14](14-svd-and-constrained-regression.md). No ridge, pseudoinverse rank truncation or dropped-factor fallback is allowed. Returned factor loadings preserve supplied factor ordering; alpha is the intercept multiplied by annualization A.

To avoid squaring original-scale extreme returns, normalize the response and fitted values by the largest absolute response value (or one for all-zero response). Compute normalized residual SSE and an online response variance giving SST. `R²=1−SSE/SST`; zero SST yields NaN rather than a perfect fit. Adjusted R² is `1−(1−R²)*(N−1)/(N−k−1)`. Residual volatility is `sqrt(SSE/(N−k−1))*response_scale*sqrt(A)`. Fitted values, annual alpha/residual volatility and finite-domain fit statistics are checked for representability; overflow errors rather than returning an infinite coefficient. Constant-response undefined fit fields have explicit nonfinite wire support.

## Conventions, limitations and alternatives

Returns are decimal fractions per observation, A is the declared frequency, beta units depend on supplied factor units, and residual volatility is annualized sample residual risk. This measures exposure conditional on chosen factors, not causal attribution or an automatically tradable hedge. Alpha/R² can change under factor transformations; column normalization is numerical scaling and is reversed before outputs. It does not standardize factor economic definitions or identify correlated factors by convention.

Factors must be full rank at the internal tolerance; collinear/near-collinear factors fail instead of receiving a minimum-norm solution. Robust regression, ridge/Bayesian estimates, PCA factor extraction, rolling multifactor inference and heteroskedastic coefficient standard errors are unimplemented alternatives. No confidence intervals are returned.

## Implementation and validation evidence

- [Return-kind and multifactor algorithm](../../../../finstack-quant/analytics/src/multi_factor.rs), [public factor caller](../../../../finstack-quant/analytics/src/performance/benchmark.rs), and [SVD implementation](../../../../finstack-quant/analytics/src/regression.rs).
- Located inline tests include synthetic single/two-factor recovery, manual cash subtraction, singular/hidden-collinearity rejection, scaled full-rank inputs and undefined fit-field contracts. [Correctness regressions](../../../../finstack-quant/analytics/tests/correctness_regressions.rs) cover coefficient overflow and representable large residual volatility. These are synthetic/algebraic/numerical checks; no empirical factor data was validated or test run executed.
- [Fama–French](../../../REFERENCES.md#fama-french-1993) is factor-model context; the crate does not prescribe that particular factor set. [Matrix computations](../../../REFERENCES.md#golub-van-loan-matrix-computations) gives numerical context.

The separate factor crate owns its own factor model policies.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[analytics follow-ups](../FOLLOW_UPS.md#analytics).
