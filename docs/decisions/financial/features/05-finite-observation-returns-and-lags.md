# FEATURES-005: Finite-observation returns, log returns, differences and lags

**Decision status:** Accepted 2026-11-17.

**Implementation status:** The finite-observation clock and signed domains are implemented. Stable evaluation near equal levels and at extreme ratios requires reconciliation.

**Validation status:** An independent signed-input example checked the clock and formulas. Rust fixtures were inspected. Extreme-ratio and near-equal cases require qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See [scope and source ledger](README.md).

## Accepted decision

1. Count finite observations within each entity, with a positive periods control defaulting to 1. A missing current row yields missing and does not advance history. Lag returns the selected earlier observation and Diff subtracts it from the current value. Both preserve input units and yield missing when history is insufficient.
2. Simple return is current divided by prior minus one, with nonzero prior. Log return is the log ratio of two nonzero observations with the same sign, including two negative observations. A pair outside the respective domain yields missing. Returns are dimensionless, and observation lags may span irregular time gaps.
3. Use stable arithmetic near equal values and at extreme ratios. Preserve small representable moves and evaluate an eligible log return when direct division overflows or underflows. Fail the whole call on an unrepresentable final result under [FEATURES-002](02-parameters-missing-and-failure.md).

For [-4, missing, -2], the one-observation lag at -2 is -4, the difference is +2, the simple return is −50%, and the log return is ln(0.5).

## Observed scope and selection

`Returns`, `LogReturns`, `Diff` and `Lag` use positive integer `periods`, default 1. Inputs are numeric observations per lexically ordered entity. No price adjustment, calendar day count, annualization, cash flow or currency handling occurs. Prices need not be positive: the implemented signed-input domain is wider than a conventional positive-price interpretation.

## Ordered algorithm

1. Keep a history containing only finite observations. At a finite current row, find the value `periods` finite observations earlier; visit that pair, then append the current observation. A missing/nonfinite row emits None and does not advance history.
2. `Lag` emits that earlier value only when the current row is finite. It is not a row-shift that fills a missing current row. `Diff` emits current minus earlier; insufficient history gives None.
3. Simple return emits `current/previous - 1` when previous is nonzero, otherwise None. A zero current against nonzero previous gives -1. Opposite-signed values are allowed, and nonfinite computed ratios cause the public call to fail.
4. Log return first uses `ln(current/previous)` when that ratio is finite and positive. If division underflows to zero or overflows but both observations are nonzero with the same sign, it uses `ln(abs(current))-ln(abs(previous))`. Zero previous/current or opposite signs yield None. Thus two negative observations can give a finite log return.

Differences and lags retain input units; both return forms are dimensionless. The log fallback changes the evaluation route, not the mathematical same-sign ratio. Return intervals can cover many calendar days or missing rows, with no duration metadata. Every output goes through final finiteness validation.

## Rationale and alternatives

The finite-observation clock is expressly documented. Proposed rationale is comparing observed data without inventing values, but this also removes the elapsed missing intervals from `periods`. Row/calendar shifts and strictly positive-price returns would be different choices. The implementation does not infer which interpretation is appropriate for a signal versus a price series; adopting a level-price domain would require explicit reconciliation rather than silently changing this record.

## Evidence

- Owning [time-series implementation](../../../../finstack-quant/features/src/timeseries.rs): `PreparedOp::parse`, `for_each_finite_lag`, `shifted_ratio`, `lag`, `diff`; [ordering contract](01-ordering-keys-and-partitions.md).
- Inspected [transform tests](../../../../finstack-quant/features/tests/transforms.rs): unsorted per-entity returns/lags and `periods=2` differences across a None row. Expected returns are hand-sized examples with 1e-12 absolute tolerance.
- Inspected `rolling_windows_and_slope_preserve_row_gaps` in [audit regressions](../../../../finstack-quant/features/tests/audit_regressions.rs) contrasts row windows with a finite-observation lag.

Located tests do not establish those numerical boundaries.

## Follow-up tracking

Implementation reconciliation and qualification work are tracked
in the [feature follow-ups](../FOLLOW_UPS.md#features).
