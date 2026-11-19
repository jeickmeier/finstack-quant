# FEATURES-012: Current-row exponential-decay weight in a trailing row window

**Decision status:** Accepted 2026-11-17.

**Implementation status:** Window membership, finite-observation decay, and current-row output are implemented. Stable normalization and checked count arithmetic require reconciliation.

**Validation status:** Exact arithmetic checked a gapped window. The Rust fixture was inspected. Extreme half-lives and large counts still require qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See [scope and source ledger](README.md).

## Accepted decision

1. Use an inclusive trailing row window with window defaulting to 1. Require a finite positive half_life in finite-observation steps. Missing rows occupy window slots, contribute no weight, and do not advance decay age. A missing current row yields missing.
2. Return the current observation's normalized weight within its own window. Its raw weight is 1, divided by the sum of decayed weights over finite observations in that window. With half-life 1 and window 3, [10, missing, 20, 30] gives [1, missing, 2/3, 2/3].
3. Use stable geometric normalization with limits of 1 as half-life approaches zero and 1/n as it grows, for finite count n. Avoid unchecked exponent conversions. [Parameter and failure policy](02-parameters-missing-and-failure.md).

## Observed scope and selection

`ExponentialDecayWeights` requires finite `half_life>0`; `window` is a positive integer defaulting to 1. It does not accept `min_periods`. Half-life is in finite-observation steps, while membership is bounded by trailing entity rows. Only input finiteness matters; numeric input magnitudes are not used as weights or returns.

## Ordered computation

1. Compute `d=exp(-ln(2)/half_life)` once during parameter preparation. Very small half-life can underflow d to zero; very large half-life can round it to one. Both are accepted finite decay limits.
2. For each finite current row, locate the inclusive trailing row window and count finite observations n within it. Missing current rows remain None. The current finite row ensures n>=1; no minimum warm-up is imposed.
3. Treat the retained finite observations as ages 0 through n-1 and calculate `D=sum(d^age)` with `powi(age as i32)` and plain addition. Emit `1/D`, the current observation's normalized weight. Missing rows are not additional decay ages, but they still consume window membership.

The output is one dimensionless current-row weight per date. Its entire output column is not a weight vector that sums to one: each row corresponds to a different trailing normalization. With window=1 the finite output is always 1, and with d=1 it is 1/n. Historical weights and decay-adjusted data values are not returned.

## Rationale, alternatives and limitations

The function documentation names the current-row weight; proposed rationale is exposing a rolling influence feature. Calendar-time decay, windows over the last n finite observations, or returning all window weights would be different methods. The current algorithm has no closed-form geometric-sum optimization or stable `expm1` route. It casts age to i32 without a guard, so extremely large finite counts exceed the exponent's supported integer range even though window is a usize.

## Evidence

- Owning [time-series preparation](../../../../finstack-quant/features/src/timeseries.rs) and [advanced decay](../../../../finstack-quant/features/src/timeseries/advanced.rs) `exponential_decay_weights`.
- Inspected [transform test](../../../../finstack-quant/features/tests/transforms.rs), `transform_timeseries_supports_advanced_rolling_signal_ops`: half_life=1/window=3 on three finite rows expects 1, 2/3 and 4/7, at 1e-12 tolerance. This checks the finite-count normalization, not long-window or limiting-half-life accuracy.

Large-count consequences were not executed or allocated here.

## Follow-up tracking

Implementation reconciliation and qualification work are tracked
in the [feature follow-ups](../FOLLOW_UPS.md#features).
