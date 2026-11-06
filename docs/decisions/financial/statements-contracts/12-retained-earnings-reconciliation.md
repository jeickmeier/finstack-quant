# STATEMENTS-CONTRACTS-012: Retained earnings, dividends and signed adjustments

**Decision status:** Draft (observed implementation; adoption pending).

**Implementation status:** Observed for the source responsibilities stated below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Source baseline and audit date: [slice README](README.md).

## Choice and input domain

RetainedEarningsReconciliation uses closing retained earnings, net income, optional dividends and signed adjustments. Net income and adjustments use inflow-positive semantics. Dividends default to MagnitudePositive; an explicit InflowPositive policy changes how they enter the identity.

[Accepted ADR 0044](../conventions/0044-financial-perspectives-and-boundary-conversions.md) already settles the dividend magnitude default and explicitly selected signed alternative. This Draft owns the reconciliation algorithm and remaining account/input choices; its warning-only treatment of negative magnitudes is an implementation gap under that accepted contract.

## Algorithm and controls

1. Validate compatible operand units, config and optional absolute tolerance.
2. Begin with the second model period and require date adjacency (prior.end == current.start). A gap or overlap emits Info and skips the pair.
3. Require finite prior/current retained earnings and current net income. Also require every configured dividend or adjustment node to be present and finite; these configured missing inputs do not become zero. Missing/non-finite inputs emit Warning and skip.
4. An unconfigured dividends node contributes zero. For MagnitudePositive, a negative finite dividend emits a sign Warning, and dividend_change = −dividends. No absolute-value repair is applied. For InflowPositive, dividend_change = dividends and both signs are accepted.
5. Sum signed adjustments in supplied order. Compute expected = prior_RE + NI + dividend_change + adjustments, then difference = current_RE − expected.
6. Use max(abs(prior_RE),1) as reference for shared absolute/relative tolerance and percentage materiality. Check adjustment sum and each cumulative arithmetic stage for finite output; overflow emits Error without publishing non-finite comparisons.
7. Record a finite comparison and emit Error when abs(difference) exceeds tolerance. Any sign/data Warning alone leaves passed true.

## Limitations and alternatives

The first period's retained earnings is an opening anchor, not reconciled. The check cannot establish that the selected adjustment nodes are legitimate changes to retained earnings. A broad label such as buybacks or AOCI in input documentation does not automatically make that account an appropriate RE adjustment.

A wrong-sign magnitude dividend is warned about but still used numerically, so it can offset an unrelated difference. Warning-only handling does not enforce the accepted non-negative magnitude contract. Repeated adjustment IDs are summed repeatedly. A requirement for configured opening data and publication treatment of other advisory findings remain separate open choices.

Separate retained-earnings, total-equity and other-comprehensive-income roll-forwards are meaningful alternatives with different account membership. They should not be conflated by silently broadening this identity.

## Evidence

[Retained-earnings implementation](../../../../finstack-quant/statements/src/checks/builtins/retained_earnings.rs), [sign policy](../../../../finstack-quant/statements/src/checks/types.rs), [RE tests](../../../../finstack-quant/statements/tests/checks/retained_earnings_tests.rs) and [overflow/unit regressions](../../../../finstack-quant/statements/tests/finance_accounting_regressions.rs) provide formula and input-handling evidence. Tests inspect a correct roll-forward, mismatches, first-period skipping, adjustments and configured missing/non-finite optional inputs. These are accounting arithmetic fixtures, not independent entity-specific accounting-policy validation.

ADR 0044 already settles the non-negative magnitude default and explicit signed alternative; those choices are not reopened.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-model-formula-and-check-contracts).
