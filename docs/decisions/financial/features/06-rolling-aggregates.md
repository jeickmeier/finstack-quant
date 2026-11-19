# FEATURES-006: Trailing-row aggregates and current-value sample z-scores

**Decision status:** Accepted 2026-11-17.

**Implementation status:** Inclusive windows and observation eligibility are implemented. Checked scaled accumulation and z-score evaluation require reconciliation.

**Validation status:** Independent examples checked missing-current behavior and avoidable sum overflow. Rust fixtures were inspected. Revised arithmetic still requires qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See [scope and source ledger](README.md).

## Accepted decision

1. Use inclusive trailing row windows and the shared window controls. Mean, sum, minimum, maximum, and sample standard deviation may return an estimate at a missing current row if enough finite observations remain. Sample standard deviation requires at least max(min_periods, 2) finite observations. Return missing when the applicable count is insufficient.
2. Rolling z-score requires a finite current observation and at least max(min_periods, 2) finite observations. Score it against the inclusive window mean and sample standard deviation. Use zero for a genuinely constant eligible window. Retain the dimensionless result. With [2, 4, missing], window 3, and minimum count 2, the last row has mean 3, sum 6, sample standard deviation √2, and a missing z-score. [Shared statistics](04-scaled-statistics-and-quantiles.md).
3. Use checked, scaled, compensated summation. Preserve representable sums such as f64::MAX + f64::MAX − f64::MAX, and fail the whole call when the final statistic is unrepresentable. [Failure policy](02-parameters-missing-and-failure.md).

## Observed scope and selection

`RollingMean`, `RollingSum`, `RollingStd`, `RollingMin`, `RollingMax` and `RollingZscore` use `window=1` and `min_periods=window` by default. The window includes the current row and spans entity rows. Finite counts gate output; standard deviation and z-score raise the effective count to at least two. A window smaller than that floor returns None throughout rather than rejecting the request.

## Algorithm, units and missing data

For each trailing row window, collect finite values in time order. If fewer than the effective minimum remain, emit None. Otherwise mean uses [scaled Kahan mean](04-scaled-statistics-and-quantiles.md); sum uses plain iterator addition; minimum/maximum reduce finite observations; standard deviation uses sample denominator `n-1`. Each retains input units. These aggregates can emit at a missing current row when earlier finite observations still satisfy the count.

Rolling z-score additionally requires a finite current value. Scale and center the finite window, compute its sample deviation in normalized coordinates, and return `(current/scale - mean(raw)/scale)/std(centered)`. Return 0 for zero deviation. The current observation participates in its own mean and deviation; this is an inclusive standardized observation, not an out-of-sample innovation. Output is dimensionless and missing at a missing current row.

The window is rebuilt for every row, with no remove/add recurrence or cached estimator. Empty series pass parameter validation and return empty output. Final nonfinite results fail the entire public call; there is no per-window error object.

## Rationale, alternatives and limitations

Documented rationale for scaled statistics is unit stability. The code does not state a numerical rationale for the plain rolling sum. Unlike the mean, it is neither scaled nor compensated: summation order, cancellation and intermediate overflow therefore need separate qualification even when the mathematically final sum is representable. An error for an actually unrepresentable sum is intentional and tested; avoidable intermediate overflow is a distinct investigation, not a reproduced failure here.

Alternatives include compensated/rebased sums, population rather than sample z-scores, or excluding the current observation. They change numerical or research interpretation and have not been adopted. Constant-window zero is a substitution for an unestimable standardized deviation and needs an explicit feature contract.

## Evidence

- Owning [rolling dispatch/kernel](../../../../finstack-quant/features/src/timeseries.rs): `rolling`, `RollingOp`; [shared statistics](../../../../finstack-quant/features/src/types.rs).
- Inspected `transform_timeseries_aligns_unsorted_returns_lag_and_rolling_std` and `transform_timeseries_supports_mvp_rolling_and_ewma_ops` in [transform tests](../../../../finstack-quant/features/tests/transforms.rs): hand sample SD and inclusive z-score values.
- Inspected [audit regressions](../../../../finstack-quant/features/tests/audit_regressions.rs): a missing current row still emits mean; scaled z-score comparisons; 1e308 mean remains finite while a truly excessive sum errors. These do not independently qualify all mixed-sign intermediate sums.

## Follow-up tracking

Implementation reconciliation and qualification work are tracked
in the [feature follow-ups](../FOLLOW_UPS.md#features).
