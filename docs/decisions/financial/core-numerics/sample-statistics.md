# CORE-NUMERICS-09: Compensated aggregation, online moments and R7 quantiles

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, within the path scope below.
- Validation status: **Evidence inspected/located, not executed**. Test roles and limits are stated below; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/stats.rs` and `math/summation.rs`: ordered real-valued observations, paired observations, sample means/variances/covariances, empirical quantiles, and independent-replicate confidence intervals. These helpers do not infer trading dates, weights, autocorrelation, missing-observation treatment, or annualization.

## Choice and rationale

Kahan/Neumaier accumulation reduces rounding loss, and Welford recurrences avoid subtracting two large raw second moments. Parallel-merge formulas allow partial samples to be combined. R7 quantiles choose a particular finite-sample interpolation convention. These are observed choices; accepting their empty/constant/missing-data policies for financial reporting remains separate.

## Algorithm

1. `kahan_sum` subtracts accumulated rounding correction from each input before adding and updates that correction. Neumaier chooses the correction formula by comparing magnitudes of old sum and new input and returns sum plus correction; neither rejects nonfinite data.
2. Online statistics update `n←n+1`, `δ=x−mean`, `mean←mean+δ/n`, `M2←M2+δ(x−mean)`. Sample variance is `M2/(n−1)`; population variance is `M2/n`. Merging groups adds `δ² n_a n_b/(n_a+n_b)` to the two M2 values and combines weighted means.
3. Online covariance tracks two means/M2 values and `C←C+δx(y−new_mean_y)`; merge adds `δx δy n_a n_b/(n_a+n_b)`. Sample covariance divides by n−1. Correlation is covariance divided by `sqrt(var_x var_y)`, clamped to [-1,1], with exact zero denominator returning 0. Control-variate beta is covariance/var_y, or 0 for exact zero var_y.
4. Batch `mean` uses Kahan; `mean_var` uses online mean. These can differ in rounding. R7 uses `h=(n−1)p`, lower index floor(h), and linear interpolation to the next order statistic. The mutable helper partitions to select only needed order statistics; the sorted helper trusts sorting and finiteness.
5. Standard error is sample standard deviation/sqrt(n), NaN for n≤1. A two-sided `(1−alpha)` interval uses `−t_inv(alpha/2,n−1)` times the standard error; `ci_half_width` fixes alpha=0.05.

## Selection, controls and conventions

Callers select batch, online, population/sample, or sorted quantile APIs explicitly. Probabilities and alpha are decimal fractions; moments retain observation units. No finite observations are dropped by moment estimators. Only `finite_min_or_nan`, `finite_max_or_nan`, and `finite_count` filter nonfinite entries.

## Limitations, failure and fallback

Default mean/variance/covariance helpers return 0 for insufficient samples; `_or_nan` helpers deliberately differ. Unequal paired lengths return NaN. Invalid quantile p or nonfinite unsorted data returns NaN. The sorted helper validates only length and p. Online methods accept nonfinite data and arithmetic overflow can propagate. A Student-t interval is not a justified interval for correlated path draws merely because it is calculable; the calling estimator must establish independent replicates.

## Alternatives and tradeoffs

Sample versus population normalization and NaN versus zero insufficient-data behavior are implemented alternatives. Weighted/robust estimators, autocorrelation-adjusted standard errors and alternative quantile types are not provided by these functions. Compensated folds improve roundoff but do not guarantee identical answers across different merge orders.

## Evidence

- [Statistics implementation and inline tests](../../../../finstack-quant/core/src/math/stats.rs), [summation implementation](../../../../finstack-quant/core/src/math/summation.rs), [summation tests](../../../../finstack-quant/core/tests/math/summation.rs).
- `confidence_intervals_account_for_estimated_small_sample_variance` uses the df=1 Cauchy closed form (1e-10 absolute) and a tabulated df=15 value; table provenance is not documented there. `quantile_matches_r7_interpolation` is a hand-worked convention check. Merge/batch agreement checks consistency, not independent coverage of every scale.
- [Welford](../../../REFERENCES.md#welford-1962), [floating-point analysis](../../../REFERENCES.md#higham-accuracy-and-stability). The inspected evidence does not include R7-specific reference provenance.

Correlation can return NaN if both variances overflow or their product underflows; no scale-normalized correlation kernel is implemented here.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
