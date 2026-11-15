# STATEMENTS-ANALYTICS-VALUATION-13: Balance roll-forward and balance-sheet articulation checks

- **Decision status:** Accepted for the full balance roll-forward and articulation contract (2026-11-15).
- **Implementation status:** Reconciliation pending.
- **Validation status:** Existing evidence mapped. Independent qualification pending.
- **Audit date/source revision:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Scope:** [Inventory](README.md); [source coverage](coverage.json).

## Accepted decision for account roll-forwards and articulation

1. Verify the model, evaluation run, input snapshot and period plan. For each identity, require compatible typed balances and movements in the same reporting scope, with one currency for monetary values. Reject duplicate or overlapping inputs within the identity. Apply signed movements as entered. For magnitude buckets, require nonnegative amounts, add increases and subtract decreases.
2. Check the first period when a dated opening is supplied. A designated first closing balance used as the baseline has no roll-forward verdict. In later periods, use the preceding close as opening or reconcile an explicit opening to the preceding close plus separately identified boundary adjustments. Check opening continuity and the closing roll-forward separately.
3. Select articulation explicitly and reuse the accepted balance-sheet articulation contract at every period close. Require complete and disjoint asset, liability and equity groups within the same reporting scope. Include each account once using its subtotal or components. [Balance-sheet articulation](../statements-contracts/10-balance-sheet-articulation.md#accepted-decision-for-balance-sheet-articulation).
4. Use exact checked `Money` arithmetic or checked scalar arithmetic. Report actual, expected, signed difference and tolerance by account or identity and period. Default to zero tolerance. An explicit tolerance is the greater of the absolute allowance and the relative allowance times the magnitude of the identity's reference. Use the expected close for roll-forward, the prior close for opening continuity and assets for articulation. An absolute-only override replaces both terms, and a difference exactly at the tolerance passes.
5. Show missing or masked required inputs and mark the affected check incomplete. Invalid configuration, units or arithmetic produce errors. Determine pass, fail, incomplete or unevaluated from the full results. Advisory findings can allow dependent work to continue. A required gate needs a complete pass. [Numerical comparisons and check suites](../statements-contracts/14-comparison-and-suite-policy.md#accepted-decision-for-numerical-comparisons-and-check-suites).

For example, USD 100 of opening balance plus USD 20 of increases less USD 5 of disposals gives an expected close of USD 115. An actual close of USD 114.98 has a signed difference of USD −0.02 and fails with a USD 0.01 allowance in either advisory or required mode. A missing required disposal makes the check incomplete.

## Audited choice, domain and controls

[The corkscrew extension](../../../../finstack-quant/statements-analytics/src/extensions/corkscrew/mod.rs) validates configured account balances and movement nodes. Tolerance defaults to `.01` **absolute numeric units**, must be finite and nonnegative, and is not currency-precision or balance-scaled. The configuration does not ensure a common currency/value type or unique account-node membership.

## Audited algorithm and failure semantics

1. For each configured account, start at the model's second period. Require previous ending balance and current ending/movement values. Opening balance is current-period explicit beginning-node value when configured, otherwise the previous ending balance. The previous balance is still looked up even when an explicit beginning node is used.
2. Compute `expected_end = opening + sum(increases) − sum(decreases)` and `difference=actual_end−expected_end`. A finite absolute difference within tolerance passes. Missing/nonfinite data errors; comparisons do not make invalid values pass.
3. For every model period, aggregate accounts classified as assets, liabilities or equity and compare `assets−liabilities−equity` with the same absolute tolerance. Account type affects this articulation classification; it does not change movement arithmetic or infer debit/credit signs. Report the maximum absolute articulation error.
4. A one-period model warns that no roll-forward transition can be checked, but still performs articulation. Empty configured accounts have no underlying balance-sheet coverage to validate.
5. A finite identity failure becomes a warning with successful extension status when `fail_on_error=false`; in strict mode it contributes errors/failed report. Missing/invalid input causes failed status even in lenient mode, and can return an immediate error in strict mode. Returned status and diagnostics must be read together.

The explicit beginning-node path preserves small movements that could be lost by subtracting large ending balances; it is still a supplied opening-balance reconciliation, not independently reconstructed ledger history.

## Audited alternatives and evidence

Absolute tolerance is appropriate only after selecting display/monetary units. Relative/materiality thresholds, per-currency tolerances, accounting sign rules and automatic complete-account coverage are not provided. Duplicate accounts can double count articulation. Lenient success means diagnostics remain advisory, not that identities reconciled.

Colocated tests cover strict versus lenient breaks, small movements with explicit opening, missing opening, single-period warning, every-period articulation, absolute tolerance and subtraction of decreases. These are arithmetic/invariant evidence; no accounting-standard compliance is inferred.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-valuation-comparisons-and-templates).
