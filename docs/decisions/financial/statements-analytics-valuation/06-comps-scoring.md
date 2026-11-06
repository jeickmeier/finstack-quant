# STATEMENTS-ANALYTICS-VALUATION-06: Comparable-company signals, regression residual scores and report exclusions

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed in the defined source scope linked below; discrepancies and limits are explicit.
- **Validation status:** Evidence inspected or located, not executed. No numerical qualification or external reconciliation is inferred.
- **Audit date/source revision:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Scope:** [Inventory](README.md); [source coverage](coverage.json).

## Choice and domain

[Scoring](../../../../finstack-quant/statements-analytics/src/analysis/comps/scoring.rs) combines explicitly selected metric dimensions into a relative-value signal. Each dimension specifies response metric, optional regression predictor, nonnegative finite weight and direction. Default direction is higher-is-cheap; users must choose lower-is-cheap for metrics where that interpretation applies. [Reports](../../../../finstack-quant/statements-analytics/src/analysis/comps/report.rs) retain missing values and exclusion reasons rather than substituting neutral zero signals.

## Algorithm and controls

1. Build finite usable peer responses, or pairwise-complete predictor/response observations for regression. Calculate the subject response and raw peer z-score. Missing subject data, inadequate peers, zero dispersion or zero weight exclude a dimension.
2. A non-regression dimension uses the signed raw z-score. A regression dimension additionally requires subject predictor, successful OLS and finite positive residual dispersion; it does not fall back to univariate z-score if regression fails.
3. For regression, compute every fitted peer residual, then divide the subject residual by the **sample standard deviation of peer residuals**. This uses the shared `n−1` statistic, not the OLS residual standard error based on `n−2`, a studentized residual or predictive interval. Raw z-score eligibility is still required before regression signal construction.
4. Reverse sign for lower-is-cheap. Combine usable signals by a weighted mean, dynamically rescaling weights to avoid overflow. Direct checked scoring errors if no positive usable weight or if the composite is nonfinite.
5. Confidence is the weighted mean R² over **regression dimensions only**, zero if none; it is not a probability or an interval on the composite. Returned peer count is the full peer-set length, not each regression fit sample size. Dimension details expose their own evidence and exclusions.
6. Report generation can produce multiple statistics without a scoring configuration or with unavailable composite. It carries per-peer/subject values, dimension diagnostics and excluded reasons; no missing signal is silently set to neutral.

## Limits, alternatives and evidence

The sample-residual score changes magnitude with peer count relative to a regression-standard-error score. Correlated dimensions can be counted repeatedly; no covariance adjustment or statistical significance test is applied. Confidence R² does not measure forecast skill, valuation accuracy or sample coverage. Extreme total weights are rescaled, but model choice and direction remain caller policy.

Colocated tests `unusable_regression_does_not_fall_back_to_an_univariate_signal`, `no_data_or_zero_dispersion_is_unavailable_instead_of_neutral`, `missing_subject_predictor_excludes_regression`, invalid-weight and confidence-underflow cases document failure and stability contracts. They are not out-of-sample financial validation. [Matrix computations](../../../REFERENCES.md#golub-van-loan-matrix-computations) is numerical context; no calibration standard for the composite is established. Robust regression, shrinkage and predictive intervals are unimplemented alternatives, not hidden defaults.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-valuation-comparisons-and-templates).
