# FEATURES-009: Rolling linear trend against row positions

**Decision status:** Draft — observed implementation; adoption pending for this scoped choice. Existing accepted decisions retain their own scope.

**Implementation status:** Observed in the exact source paths and symbols below; discrepancies and limitations are stated explicitly.

**Validation status:** Source and the named test bodies inspected, not executed. This record does not certify numerical correctness or host parity.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See [scope and source ledger](README.md).

## Scope and selection

`RollingSlope` applies ordinary unweighted linear slope with an implicit intercept to finite values in an inclusive trailing row window. Defaults window=1/min_periods=window; effective finite count is at least two. The time regressor is the integer position in that window, not the order-key text or elapsed time.

## Ordered algorithm

1. Enumerate all window rows with positions 0 through window_length-1, retaining `(position,value)` only for finite values. Missing rows retain their gaps in the retained positions.
2. Compute the arithmetic mean of retained positions. Scale and center retained response values with the shared helper; let its scale be `a` and deviations `d_i`.
3. Accumulate `C=sum((position_i-position_mean)*d_i)` and `V=sum((position_i-position_mean)²)` with plain additions. Return `(C/V)*a`, or 0 if V<=0. The normal caller supplies at least two distinct row positions, so V<=0 is a defensive/degenerate branch.

Output is response units per entity-row step. It can emit at a missing current row because only the fitting window must be sufficiently populated. Within-window position reset does not change an intercept slope; irregular actual dates nevertheless have no effect on its slope clock. No trend confidence, forecast or residual variance is returned.

## Rationale, alternatives and evidence

Public documentation expressly preserves row gaps. Proposed rationale is a consistent panel-grid trend; calendar-time regression or finite-observation positions would answer different questions. There is no recorded choice that one row represents one trading day, so annualized trend claims would be unsupported.

- Owning [advanced slope](../../../../finstack-quant/features/src/timeseries/advanced.rs) `rolling_slope`, [rolling selection](../../../../finstack-quant/features/src/timeseries.rs), and [window indexing](../../../../finstack-quant/features/src/index.rs).
- Inspected `rolling_windows_and_slope_preserve_row_gaps` in [audit regressions](../../../../finstack-quant/features/tests/audit_regressions.rs): [0,None,2] produces slope 1, preserving the missing row. Inspected [transform tests](../../../../finstack-quant/features/tests/transforms.rs) include slope 1 on [1,2,3]. These are hand-sized behavioral oracles.

The source has no date-distance regression option.

## Follow-up tracking

Pending choices, implementation reconciliation, and qualification work are tracked
in the [feature follow-ups](../FOLLOW_UPS.md#features).
