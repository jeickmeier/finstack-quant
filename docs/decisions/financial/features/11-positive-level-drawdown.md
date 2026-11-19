# FEATURES-011: Running-peak drawdown on positive levels

**Decision status:** Accepted 2026-11-17.

**Implementation status:** The running peak and gap semantics are implemented. Stable relative-difference evaluation requires reconciliation.

**Validation status:** Independent examples checked an interrupted path and a decline near the peak. The Rust fixture was inspected. Boundary and entity isolation checks remain pending.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See [scope and source ledger](README.md).

## Accepted decision

1. Maintain a running peak per entity with no supplied initial level. The first finite positive level and each subsequent high yield zero. For a level below the peak, return the dimensionless relative difference (level − peak) / peak.
2. Missing, nonfinite, zero, and negative levels yield missing and leave the peak unchanged. For [100, 120, 0, missing, 90, 132], the output is [0, 0, missing, missing, −25%, 0]. [Entity ordering](01-ordering-keys-and-partitions.md).
3. Evaluate the relative difference directly to preserve small declines near the peak. An extreme decline from a positive level may round to −1.

## Observed scope and selection

`Drawdown` accepts no parameters and runs independently per lexically ordered entity. The documented input is a positive level series, such as cumulative value, not a return series. No initial wealth, externally supplied high-water mark, rolling horizon or cash-flow adjustment is available.

## Ordered algorithm and domains

Start with no peak. At each finite strictly positive observation, set peak to max(previous peak,current), then emit `current/peak - 1`. The first positive observation emits 0; subsequent peaks also emit 0. Positive levels below the peak give negative dimensionless drawdown. The method has no currency or calendar-duration calculation.

Missing, NaN, infinite, zero and negative values emit None and leave the prior peak unchanged. A zero level is consequently not treated as a total loss of -1. Later positive observations resume against the peak retained before the invalid/nonpositive row. The public final-output check still applies; underflow of a very small positive current/peak ratio can produce the finite boundary value -1.

The high-water mark is based only on supplied positive observations. There is no prior valuation date or implied starting level of one, and no drawdown-episode aggregation, recovery calculation or duration. Sorting and duplicate-order behavior follow [FEATURES-001](01-ordering-keys-and-partitions.md).

## Rationale, alternatives and evidence

The source explicitly calls this a level-series transform. Proposed rationale is a lightweight feature with a persistent peak across missing rows; rejecting nonpositive levels or representing a zero level as a wipeout would be different contracts. Analytics wealth-path conventions apply to their own scope and are not silently selected here.

- Owning [advanced drawdown scan](../../../../finstack-quant/features/src/timeseries/advanced.rs) `drawdown`; [time-series selector](../../../../finstack-quant/features/src/timeseries.rs) `PreparedOp::Drawdown`.
- Inspected `transform_timeseries_supports_advanced_rolling_signal_ops` in [transform tests](../../../../finstack-quant/features/tests/transforms.rs): [100,120,90] gives [0,0,-.25] with 1e-12 absolute tolerance. It is a simple formula example, not evidence covering interrupted or nonpositive paths.

## Follow-up tracking

Implementation reconciliation and qualification work are tracked
in the [feature follow-ups](../FOLLOW_UPS.md#features).
