# FEATURES-011: Running-peak drawdown on positive levels

**Decision status:** Draft — observed implementation; adoption pending for this scoped choice. Existing accepted decisions retain their own scope.

**Implementation status:** Observed in the exact source paths and symbols below; discrepancies and limitations are stated explicitly.

**Validation status:** Source and the named test bodies inspected, not executed. This record does not certify numerical correctness or host parity.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See [scope and source ledger](README.md).

## Scope and selection

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

Pending choices, implementation reconciliation, and qualification work are tracked
in the [feature follow-ups](../FOLLOW_UPS.md#features).
