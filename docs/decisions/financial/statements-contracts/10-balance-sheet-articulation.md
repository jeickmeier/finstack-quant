# STATEMENTS-CONTRACTS-010: Balance-sheet summation, materiality and missing-input behavior

**Decision status:** Draft (observed implementation; adoption pending).

**Implementation status:** Observed for the source responsibilities stated below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Source baseline and audit date: [slice README](README.md).

## Choice and input domain

BalanceSheetArticulation checks the identity Assets = Liabilities + Equity for every model period using caller-selected groups of node IDs. All participating known result operands must have compatible scalar/monetary types and, for monetary values, one currency. It performs no FX conversion or account classification.

## Algorithm and controls

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

Compensated sums, explicit account-set validation or a separate completeness verdict would address different risks. Their adoption is not implied here.

## Evidence

[Balance-sheet implementation](../../../../finstack-quant/statements/src/checks/builtins/balance_sheet.rs), [input helpers](../../../../finstack-quant/statements/src/checks/helpers.rs), [balance tests](../../../../finstack-quant/statements/tests/checks/balance_sheet_tests.rs), [property tests](../../../../finstack-quant/statements/tests/checks/proptest_checks.rs) and [overflow regressions](../../../../finstack-quant/statements/tests/finance_accounting_regressions.rs) provide direct identity, edge and invariant evidence. Tests cover empty groups, misnamed/missing/non-finite operands, relative tolerance at scale and arithmetic overflow. They do not validate an external chart of accounts.

A passing result does not prove that every period was checked or that the statements comply with an accounting standard.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-model-formula-and-check-contracts).
