# STATEMENTS-ANALYTICS-VALUATION-06: Comparable-company signals, regression residual scores and report exclusions

- **Decision status:** Accepted for the full comparable-company scoring contract (2026-11-15).
- **Implementation status:** Reconciliation pending.
- **Validation status:** Existing evidence mapped. Independent qualification pending.
- **Audit date/source revision:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Scope:** [Inventory](README.md); [source coverage](coverage.json).

## Accepted decision for comparable-company scoring

1. Use the validated comparable-company population. Require an explicit response metric, direction and finite nonnegative weight for each dimension, with an optional predictor for regression. Orient each score so a positive value denotes relative cheapness. A zero weight disables the dimension. Reject repeated response-and-predictor combinations. [Comparable-company populations and statistics](05-comps-population-statistics.md#accepted-decision-for-comparable-company-populations-and-statistics).
2. For a univariate signal, use the peer sample z-score. For a regression signal, divide the subject's actual-minus-fitted residual by the sample standard deviation of fitted peer residuals, using denominator `n−1`, then apply the configured direction. Raw z-scores and percentiles are independent diagnostics. Zero residual dispersion makes the regression score unavailable. Outside the peer predictor range, report the extrapolated fit and residual with an unavailable score.
3. By default, require every positive-weight dimension for the composite. A caller may explicitly elect partial coverage with a minimum share of configured weight and at least one valid signal. Use a checked weighted mean and report configured weights, effective weights and the retained share of configured weight. If the composite is unavailable, still return dimension results and reasons.
4. Report the peers used for each metric and each fit, with exclusions, directions and diagnostics. Label the weighted mean regression R² over contributing fits with defined R². That value is unavailable if none qualify. A multiples-only report is valid.

For example, a +20 basis point regression residual over a peer residual standard deviation of 15.49 basis points gives +1.29. Combining it with a −1.00 multiple signal at weights of two and one gives +0.53. If the multiple signal is missing, two thirds of configured weight is covered and the default composite is unavailable. An explicitly elected partial-coverage threshold of at most two thirds yields +1.29.

## Audited choice and domain

[Scoring](../../../../finstack-quant/statements-analytics/src/analysis/comps/scoring.rs) combines explicitly selected metric dimensions into a relative-value signal. Each dimension specifies response metric, optional regression predictor, nonnegative finite weight and direction. Default direction is higher-is-cheap; users must choose lower-is-cheap for metrics where that interpretation applies. [Reports](../../../../finstack-quant/statements-analytics/src/analysis/comps/report.rs) retain missing values and exclusion reasons rather than substituting neutral zero signals.

## Audited algorithm and controls

1. Build finite usable peer responses, or pairwise-complete predictor/response observations for regression. Calculate the subject response and raw peer z-score. Missing subject data, inadequate peers, zero dispersion or zero weight exclude a dimension.
2. A non-regression dimension uses the signed raw z-score. A regression dimension additionally requires subject predictor, successful OLS and finite positive residual dispersion; it does not fall back to univariate z-score if regression fails.
3. For regression, compute every fitted peer residual, then divide the subject residual by the **sample standard deviation of peer residuals**. This uses the shared `n−1` statistic, not the OLS residual standard error based on `n−2`, a studentized residual or predictive interval. Raw z-score eligibility is still required before regression signal construction.
4. Reverse sign for lower-is-cheap. Combine usable signals by a weighted mean, dynamically rescaling weights to avoid overflow. Direct checked scoring errors if no positive usable weight or if the composite is nonfinite.
5. Confidence is the weighted mean R² over **regression dimensions only**, zero if none; it is not a probability or an interval on the composite. Returned peer count is the full peer-set length, not each regression fit sample size. Dimension details expose their own evidence and exclusions.
6. Report generation can produce multiple statistics without a scoring configuration or with unavailable composite. It carries per-peer/subject values, dimension diagnostics and excluded reasons; no missing signal is silently set to neutral.

## Audited limits, alternatives and evidence

The sample-residual score changes magnitude with peer count relative to a regression-standard-error score. Correlated dimensions can be counted repeatedly; no covariance adjustment or statistical significance test is applied. Confidence R² does not measure forecast skill, valuation accuracy or sample coverage. Extreme total weights are rescaled, but model choice and direction remain caller policy.

Colocated tests `unusable_regression_does_not_fall_back_to_an_univariate_signal`, `no_data_or_zero_dispersion_is_unavailable_instead_of_neutral`, `missing_subject_predictor_excludes_regression`, invalid-weight and confidence-underflow cases document failure and stability contracts. They are not out-of-sample financial validation. [Matrix computations](../../../REFERENCES.md#golub-van-loan-matrix-computations) is numerical context; no calibration standard for the composite is established. Robust regression, shrinkage and predictive intervals are unimplemented alternatives, not hidden defaults.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-valuation-comparisons-and-templates).
