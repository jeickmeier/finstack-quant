# STATEMENTS-ANALYTICS-VALUATION-13: Balance roll-forward and balance-sheet articulation checks

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed in the defined source scope linked below; discrepancies and limits are explicit.
- **Validation status:** Evidence inspected or located, not executed. No numerical qualification or external reconciliation is inferred.
- **Audit date/source revision:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Scope:** [Inventory](README.md); [source coverage](coverage.json).

## Choice, domain and controls

[The corkscrew extension](../../../../finstack-quant/statements-analytics/src/extensions/corkscrew/mod.rs) validates configured account balances and movement nodes. Tolerance defaults to `.01` **absolute numeric units**, must be finite and nonnegative, and is not currency-precision or balance-scaled. The configuration does not ensure a common currency/value type or unique account-node membership.

## Algorithm and failure semantics

1. For each configured account, start at the model's second period. Require previous ending balance and current ending/movement values. Opening balance is current-period explicit beginning-node value when configured, otherwise the previous ending balance. The previous balance is still looked up even when an explicit beginning node is used.
2. Compute `expected_end = opening + sum(increases) − sum(decreases)` and `difference=actual_end−expected_end`. A finite absolute difference within tolerance passes. Missing/nonfinite data errors; comparisons do not make invalid values pass.
3. For every model period, aggregate accounts classified as assets, liabilities or equity and compare `assets−liabilities−equity` with the same absolute tolerance. Account type affects this articulation classification; it does not change movement arithmetic or infer debit/credit signs. Report the maximum absolute articulation error.
4. A one-period model warns that no roll-forward transition can be checked, but still performs articulation. Empty configured accounts have no underlying balance-sheet coverage to validate.
5. A finite identity failure becomes a warning with successful extension status when `fail_on_error=false`; in strict mode it contributes errors/failed report. Missing/invalid input causes failed status even in lenient mode, and can return an immediate error in strict mode. Returned status and diagnostics must be read together.

The explicit beginning-node path preserves small movements that could be lost by subtracting large ending balances; it is still a supplied opening-balance reconciliation, not independently reconstructed ledger history.

## Alternatives and evidence

Absolute tolerance is appropriate only after selecting display/monetary units. Relative/materiality thresholds, per-currency tolerances, accounting sign rules and automatic complete-account coverage are not provided. Duplicate accounts can double count articulation. Lenient success means diagnostics remain advisory, not that identities reconciled.

Colocated tests cover strict versus lenient breaks, small movements with explicit opening, missing opening, single-period warning, every-period articulation, absolute tolerance and subtraction of decreases. These are arithmetic/invariant evidence; no accounting-standard compliance is inferred.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-valuation-comparisons-and-templates).
