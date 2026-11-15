# STATEMENTS-CONTRACTS-011: Cash roll-forward and independent cash-flow components

**Decision status:** Accepted for the full cash roll-forward and component reconciliation contract (2026-11-15).

**Implementation status:** Reconciliation pending. The audited roll-forward starts in the second period, and partial configuration or missing observations can cause skips.

**Validation status:** Existing source and regression evidence mapped. Independent qualification of period boundaries, adjustments, arithmetic and coverage remains pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Scope inventory](README.md).

## Accepted decision for cash roll-forward and component reconciliation

1. Reconcile closing cash to opening cash plus classified cash flows and separately identified FX and cash-scope adjustments. Require an explicit dated opening cash balance at the first period boundary. Each subsequent opening uses the immediately adjacent prior close at the same boundary. Use explicit sources for signed FX and cash-scope adjustments and include each adjustment once. All inputs share the same cash scope, compatible units and one currency where monetary. Cash inflows are positive and outflows are negative.
2. Independently reconcile total classified cash flows to CFO + CFI + CFF. Configure all three component roles or omit all three. Reject partial configuration and duplicate component IDs. Evaluate each enabled identity for every model period, including the first.
3. Use checked exact `Money` or Decimal arithmetic for monetary comparisons and checked compensated arithmetic for scalars. Retain signed and absolute residuals. Default to zero tolerance. Rounded or estimated inputs can use an explicit allowance of `max(absolute tolerance, relative tolerance × reference)`, with an absolute override replacing both terms and equality within tolerance. Use `|opening cash|` as the roll-forward reference and `|total classified cash flows|` as the component reference. These references have no floor. Report the relative residual percentage when the reference is nonzero and the diagnostic is representable. [Balance-sheet arithmetic and tolerance](10-balance-sheet-articulation.md#accepted-decision-for-balance-sheet-articulation).
4. Mark missing required comparisons incomplete and identify the missing inputs. Invalid inputs or required arithmetic produce an error. The check passes only when every enabled comparison is complete and within tolerance.

## Audited input domain

CashReconciliation checks two distinct identities: Cash_t = Cash_(t−1) + TotalCF_t and, when all components are configured, TotalCF_t = CFO_t + CFI_t + CFF_t. Inputs are signed amounts with cash inflows positive; compatible units and one currency are required.

## Audited algorithm and controls

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

In the audited path, gaps and overlaps are visible as Info skips. The audited check can pass with only skips or warnings. The accepted contract requires complete enabled comparisons.

## Evidence

[Cash implementation](../../../../finstack-quant/statements/src/checks/builtins/cash_reconciliation.rs), [cash tests](../../../../finstack-quant/statements/tests/checks/cash_reconciliation_tests.rs), [accounting regressions](../../../../finstack-quant/statements/tests/finance_accounting_regressions.rs), [audit regressions](../../../../finstack-quant/statements/tests/audit_regressions.rs) and [shared tolerance](../../../../finstack-quant/statements/src/checks/types.rs) are the evidence. Tests use explicit hand-built balance/flow identities and inspect partial component configuration, non-finite data and overflow. They do not establish a universal classification of CFO/CFI/CFF.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-model-formula-and-check-contracts).
