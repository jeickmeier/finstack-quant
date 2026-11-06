# FEATURES-006: Trailing-row aggregates and current-value sample z-scores

**Decision status:** Draft — observed implementation; adoption pending for this scoped choice. Existing accepted decisions retain their own scope.

**Implementation status:** Observed in the exact source paths and symbols below; discrepancies and limitations are stated explicitly.

**Validation status:** Source and the named test bodies inspected, not executed. This record does not certify numerical correctness or host parity.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See [scope and source ledger](README.md).

## Scope and selection

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

Pending choices, implementation reconciliation, and qualification work are tracked
in the [feature follow-ups](../FOLLOW_UPS.md#features).
