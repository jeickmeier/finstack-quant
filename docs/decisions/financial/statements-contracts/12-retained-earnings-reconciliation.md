# STATEMENTS-CONTRACTS-012: Retained earnings, dividends and signed adjustments

**Decision status:** Accepted for the full retained-earnings reconciliation contract (2026-11-15).

**Implementation status:** Reconciled (2026-11-17). Every model period is reconciled: `closing = opening + net income + dividend effect + direct entries`, the first from the required explicit dated opening (`opening_balance`) and each later one from the adjacent prior close. Under `MagnitudePositive` a non-negative dividend is subtracted and a negative magnitude is rejected; under the explicitly selected `InflowPositive` the signed dividend is added. Direct retained-earnings entries are distinct signed nodes, and a node that is also the dividend series is rejected. A configured series without a value makes the period incomplete; a typed zero is a known zero. Arithmetic, tolerance and the unfloored `|opening retained earnings|` reference follow the balance-sheet contract. The sections after the accepted decision describe the implementation audited before reconciliation.

**Validation status:** Qualified by first-period reconciliation, a wrong opening, a negative dividend magnitude, a signed dividend under the elected convention, a typed zero, distinct signed adjustments, duplicate and overlapping movement nodes, a missing configured series and an unknown node.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Scope inventory](README.md).

## Accepted decision for retained-earnings reconciliation

1. Reconcile every model period. Use an explicit dated first opening at the first period boundary and the immediately adjacent prior close at later shared boundaries. Expected closing retained earnings equal opening retained earnings plus net income, the signed dividend effect and signed direct retained-earnings adjustments. Use the same ownership scope, compatible units and one currency where monetary.
2. Recognize dividends in the period they affect retained earnings. Under the default `MagnitudePositive` policy, accept a nonnegative dividend and subtract it. Reject a negative magnitude. Under the explicitly selected `InflowPositive` policy, add the signed dividend. Omit a dividend or adjustment series only when it is inapplicable. Use a typed zero for known zero activity and mark a configured missing amount incomplete. [Dividend perspectives](../conventions/0044-financial-perspectives-and-boundary-conversions.md#decision).
3. Include each distinctly sourced direct retained-earnings entry once and reject duplicate movement IDs. Reflect a restatement once, either in an already-restated opening balance or as an explicit movement. Other comprehensive income and buybacks contribute only the part recorded directly in retained earnings.
4. Use the checked exact monetary and compensated scalar arithmetic from the balance-sheet contract. Retain signed and absolute residuals. Apply zero default tolerance, an explicit allowance for rounded or estimated balances, and the absolute override and inclusive comparison rules. The relative reference is `|opening retained earnings|` without a floor. Report a relative percentage only when that reference is nonzero and the diagnostic is representable. Missing required inputs make a comparison incomplete. Invalid inputs or required arithmetic produce an error. The check passes only when all required comparisons are complete and within tolerance. [Balance-sheet arithmetic and tolerance](10-balance-sheet-articulation.md#accepted-decision-for-balance-sheet-articulation).

## Audited input domain

RetainedEarningsReconciliation uses closing retained earnings, net income, optional dividends and signed adjustments. Net income and adjustments use inflow-positive semantics. Dividends default to MagnitudePositive; an explicit InflowPositive policy changes how they enter the identity.

[ADR 0044](../conventions/0044-financial-perspectives-and-boundary-conversions.md) establishes the dividend magnitude default and explicitly selected signed alternative. The audited warning-only treatment of negative magnitudes is an implementation gap.

## Audited algorithm and controls

1. Validate compatible operand units, config and optional absolute tolerance.
2. Begin with the second model period and require date adjacency (prior.end == current.start). A gap or overlap emits Info and skips the pair.
3. Require finite prior/current retained earnings and current net income. Also require every configured dividend or adjustment node to be present and finite; these configured missing inputs do not become zero. Missing/non-finite inputs emit Warning and skip.
4. An unconfigured dividends node contributes zero. For MagnitudePositive, a negative finite dividend emits a sign Warning, and dividend_change = −dividends. No absolute-value repair is applied. For InflowPositive, dividend_change = dividends and both signs are accepted.
5. Sum signed adjustments in supplied order. Compute expected = prior_RE + NI + dividend_change + adjustments, then difference = current_RE − expected.
6. Use max(abs(prior_RE),1) as reference for shared absolute/relative tolerance and percentage materiality. Check adjustment sum and each cumulative arithmetic stage for finite output; overflow emits Error without publishing non-finite comparisons.
7. Record a finite comparison and emit Error when abs(difference) exceeds tolerance. Any sign/data Warning alone leaves passed true.

## Limitations and alternatives

The first period's retained earnings is an opening anchor, not reconciled. The check cannot establish that the selected adjustment nodes are legitimate changes to retained earnings. A broad label such as buybacks or AOCI in input documentation does not automatically make that account an appropriate RE adjustment.

A wrong-sign magnitude dividend is warned about but still used numerically, so it can offset an unrelated difference. Repeated adjustment IDs are summed repeatedly. First-period opening and complete comparisons remain pending implementation.

Separate retained-earnings, total-equity and other-comprehensive-income roll-forwards are meaningful alternatives with different account membership. They should not be conflated by silently broadening this identity.

## Evidence

[Retained-earnings implementation](../../../../finstack-quant/statements/src/checks/builtins/retained_earnings.rs), [sign policy](../../../../finstack-quant/statements/src/checks/types.rs), [RE tests](../../../../finstack-quant/statements/tests/checks/retained_earnings_tests.rs) and [overflow/unit regressions](../../../../finstack-quant/statements/tests/finance_accounting_regressions.rs) provide formula and input-handling evidence. Tests inspect a correct roll-forward, mismatches, first-period skipping, adjustments and configured missing/non-finite optional inputs. These are accounting arithmetic fixtures, not independent entity-specific accounting-policy validation.

## Follow-up tracking

This scope has no open follow-ups; the [follow-up register](../FOLLOW_UPS.md) tracks the other scopes.
