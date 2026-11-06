# STATEMENTS-CONTRACTS-011: Cash roll-forward and independent cash-flow components

**Decision status:** Draft (observed implementation; adoption pending).

**Implementation status:** Observed for the source responsibilities stated below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice README](README.md)).

## Choice and input domain

CashReconciliation checks two distinct identities: Cash_t = Cash_(t−1) + TotalCF_t and, when all components are configured, TotalCF_t = CFO_t + CFI_t + CFF_t. Inputs are signed amounts with cash inflows positive; compatible units and one currency are required.

## Algorithm and controls

Validate config, optional absolute tolerance and all configured input units. If exactly one or two of CFO/CFI/CFF are configured, emit a global Warning and do not run that component identity.

For the roll-forward:

1. Begin at the second model period; the first has no opening balance test.
2. Require prior.end == current.start. Otherwise emit Info and skip the pair; adjacency is determined by actual half-open date intervals, not merely neighboring vector positions.
3. Require finite prior cash, current cash and current TotalCF. Otherwise emit Warning and skip.
4. Set expected = prior_cash + TotalCF and difference = current_cash − expected. Reference scale is max(abs(prior_cash),1).
5. Apply the shared max(absolute,relative×reference) tolerance unless an explicit absolute override is given. Relative materiality is abs(difference/reference) × 100.
6. Reject non-finite intermediate arithmetic with an Error; otherwise record the comparison and emit an Error when abs(difference) exceeds tolerance.

For component reconciliation, separately visit every model period, including the first and periods skipped by the roll-forward. If all three component IDs are configured, require finite TotalCF/CFO/CFI/CFF, sum them in CFO then CFI then CFF order, and compare the sum to TotalCF. Use max(abs(TotalCF),1) as reference. Check both the partial and final sums for finite arithmetic. Missing components emit Warning and skip only that identity.

## Limitations and alternatives

No opening-cash or residual-FX/restricted-cash adjustment is inferred. Any such reconciling item must already be included in TotalCF or explicitly represented by a different formula check. Repeated component IDs can double-count. A single period can fully validate components while having no roll-forward comparison.

Gaps and overlaps are visible Info skips, not validation failures. The check can pass with only skips or warnings. A stronger publication gate could require a minimum number of comparisons or treat missing adjacent periods as an error; no such coverage policy is implemented here.

## Evidence

[Cash implementation](../../../../finstack-quant/statements/src/checks/builtins/cash_reconciliation.rs), [cash tests](../../../../finstack-quant/statements/tests/checks/cash_reconciliation_tests.rs), [accounting regressions](../../../../finstack-quant/statements/tests/finance_accounting_regressions.rs), [audit regressions](../../../../finstack-quant/statements/tests/audit_regressions.rs) and [shared tolerance](../../../../finstack-quant/statements/src/checks/types.rs) are the evidence. Tests use explicit hand-built balance/flow identities and inspect partial component configuration, non-finite data and overflow. They do not establish a universal classification of CFO/CFI/CFF.

A passing check does not establish a fully reconciled cash history.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-model-formula-and-check-contracts).
