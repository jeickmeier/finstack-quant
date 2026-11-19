# STATEMENTS-CONTRACTS-010: Balance-sheet summation, materiality and missing-input behavior

**Decision status:** Accepted for the full balance-sheet articulation contract (2026-11-15).

**Implementation status:** Reconciled (2026-11-17). The check runs at every model period close on three non-empty groups of known nodes, unique within a group and disjoint across groups; an unknown, repeated or shared account is a configuration error. Monetary balances are summed exactly and scalar balances with compensated sums, and the signed difference `assets − (liabilities + equity)` is one exact residual over all terms. The default tolerance is zero and a stated allowance uses `|assets|` as its reference. A period with a missing balance is incomplete and names the nodes without a value; an unrepresentable sum or tolerance is an error; the check passes only when every period is complete and within tolerance. The sections after the accepted decision describe the implementation audited before reconciliation.

**Validation status:** Qualified by a hand-built three-statement model, one unit of unexplained cash, cancelling balances of 1e16 with a difference of one, unknown, repeated and shared accounts, a missing balance, and comparisons exactly at the allowance.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Scope inventory](README.md).

## Accepted decision for balance-sheet articulation

1. Check assets = liabilities + equity at every model period close. Require three nonempty groups of known node IDs, with unique IDs within each group and disjoint membership across groups. All operands use compatible units, one reporting scope and one currency where monetary. The caller includes each account once using either the applicable subtotal or its components. Preserve signed balances and explicit typed zeros. [Formula units and currency](05-currency-dimensions.md#accepted-decision-for-formula-units-and-currency).
2. Sum monetary balances with checked exact `Money` or Decimal arithmetic and scalar balances with checked compensated sums. Retain the signed difference `assets − (liabilities + equity)` and its absolute amount. When assets are nonzero and the diagnostic is representable, report the relative percentage as `|difference| / |assets| × 100`. [Monetary arithmetic](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity) · [Checked accumulation](../../standards/0042-numerical-accumulation-and-comparisons.md).
3. Default to zero tolerance. Rounded or estimated balances can use an explicitly configured allowance of `max(absolute tolerance, relative tolerance × |assets|)`, with relative tolerance expressed as a decimal fraction. An explicit absolute override replaces both terms. A difference exactly at the allowance is within tolerance.
4. Mark a period with any missing balance incomplete and identify the missing node IDs. Invalid inputs or invalid required arithmetic produce an error. The check passes only when every required period is complete and every absolute difference is within tolerance.

## Audited input domain

BalanceSheetArticulation checks the identity Assets = Liabilities + Equity for every model period using caller-selected groups of node IDs. All participating known result operands must have compatible scalar/monetary types and, for monetary values, one currency. It performs no FX conversion or account classification.

## Audited algorithm and controls

1. Validate numeric check configuration and an optional absolute tolerance override. Compare available result type metadata with model declarations or inferred explicit-series types; reject contradictions or mixed operand units.
2. Require each of assets_nodes, liabilities_nodes and equity_nodes to be nonempty.
3. For each model period, look up every configured operand and require a finite value. A missing or non-finite cell emits a Warning and skips that period's identity.
4. Sum each group with ordinary f64 iteration in supplied node order. Compute imbalance = assets − (liabilities + equity).
5. Without an override, tolerance = max(default_tolerance, default_relative_tolerance × abs(assets)); defaults are 0.01 base units and 1e-9 relative. An explicit override replaces both terms and is absolute.
6. Compute relative materiality as abs(imbalance/assets) × 100 when abs(assets) > machine epsilon, else zero. Require finite group sums, liabilities+equity, imbalance, tolerance and relative materiality. Unrepresentable intermediate arithmetic emits an Error with no invalid numeric comparison.
7. Record each finite actual/expected/tolerance comparison. Emit an Error only when abs(imbalance) > tolerance; equality is accepted. Attach absolute imbalance, percentage materiality and assets as the reference. passed is true if there is no Error finding.

## Limitations and alternatives

Repeated or overlapping node IDs are counted repeatedly; the check does not prove that each account was included once or that the groups represent complete financial statements. A Scalar series can carry currency-sized amounts without a currency tag. If neither result nor model supplies type metadata, the helper assumes Scalar.

A skipped period is not a demonstrated identity, even though its Warning does not fail the check. Near-zero assets report zero relative percentage, so absolute materiality and the comparison remain essential. Ordinary summation can be order-sensitive with large opposing balances; finite-intermediate checks prevent overflow from silently passing but are not an accuracy bound.

The accepted contract requires checked accumulation, valid account sets and explicit completeness.

## Evidence

[Balance-sheet implementation](../../../../finstack-quant/statements/src/checks/builtins/balance_sheet.rs), [input helpers](../../../../finstack-quant/statements/src/checks/helpers.rs), [balance tests](../../../../finstack-quant/statements/tests/checks/balance_sheet_tests.rs), [property tests](../../../../finstack-quant/statements/tests/checks/proptest_checks.rs) and [overflow regressions](../../../../finstack-quant/statements/tests/finance_accounting_regressions.rs) provide direct identity, edge and invariant evidence. Tests cover empty groups, misnamed/missing/non-finite operands, relative tolerance at scale and arithmetic overflow. They do not validate an external chart of accounts.

The audited passing status can include periods skipped for missing inputs.

## Follow-up tracking

This scope has no open follow-ups; the [follow-up register](../FOLLOW_UPS.md) tracks the other scopes.
