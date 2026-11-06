# STATEMENTS-EVALUATION-012 — Contractual cashflows, waterfall feedback and period snapshots

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed for the stated source scope**, with discrepancies identified below. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice and owned boundary

The evaluator integrates contractual instrument cashflows, statement formulas and an optional funding waterfall in a staged period loop. This record owns orchestration and aggregate observation semantics. Instrument adapters, debt accrual, residual schedule mutation and waterfall allocation are separately audited in [statements-capital](../statements-capital/README.md); those algorithms are not redefined here. Model-declared units and previously agreed reported-cost treatment remain in force: reported COGS/OPEX include their reported D&A; this audit does not introduce a second add-back in statement evaluation.

## Ordered algorithm

1. Build a capital integration with unique instrument IDs. Initialize debt opening balances from absolute raw-schedule outstanding strictly before period start. A positive funding event exactly at issue can make the opening balance zero; future issuance is zero, otherwise initial notional is the fallback. Derivatives start with zero debt notional.
2. For each period, rebuild/init residual debt state as required and delegate contractual period-flow calculation. Record instrument cashflow buckets, economic principal claims, closing balances and new funding. Aggregate contractual capital snapshots, then evaluate all statement nodes against them.
3. If a waterfall is configured, inspect the available-cash formula for direct debt-service feedback through the delegated guard, then execute the allocation using the evaluated available cash and current debt state. Replace affected instrument buckets and totals; set available/equity outputs and rebuild residual schedules at the final included day `period.end-1`.
4. Find formulas or masks that read capital references, then include their DAG dependents and reevaluate that set **once** against the post-waterfall snapshot. This is not a simultaneous fixed-point solve: the waterfall is not rerun, and the forecast cache remains intact. Period warnings are deduplicated before contractual warnings are appended.
5. Store the final snapshot as historical capital data, push statement results into history and advance instrument state for the next period. Lagged capital formulas subsequently read those recorded snapshots.

## Currency, signs and date conventions

Aggregate Money amounts first within native currency. With only one currency and no explicit reporting currency, aggregate in that native currency; with multiple currencies and no reporting currency, preserve currency-specific totals instead of inventing a common unit. Reporting-currency conversion uses the model's selected currency and configured FX policy, default PeriodEnd.

The aggregate converter is supplied the same inclusive period-end snapshot date for each total. Even when policy is named `CashflowDate`, this caller does not supply individual cash dates. The agreed **period-close FX for waterfall allocations** is preserved. Builder prose promising per-cashflow conversion is a documentation/implementation-contract reconciliation item, not permission to change the allocation convention.

Ordinary/prepared/Monte Carlo evaluations use the default empty-market path when no explicit market API is invoked; fixed obligations may work but required curve/FX data fail. The public market evaluator supplies the actual context and as-of cutoff.

## Limitations and alternatives

One post-waterfall pass can change dependent ratios without re-solving the cash allocation that produced them. A fixed-point solver is a distinct financial model. The direct-feedback guard should not be described as proving the absence of every indirect cyclic economic dependency. Debt initialization takes magnitudes at this boundary, with instrument-specific sign/perspective treatment delegated to the capital scope.

No underlying cashflow or FX changes are authorized by this record.

## Evidence

[Capital runtime](../../../../finstack-quant/statements/src/evaluator/capital_structure_runtime.rs), [evaluation loop](../../../../finstack-quant/statements/src/evaluator/engine.rs), [historical context](../../../../finstack-quant/statements/src/evaluator/context.rs). Located [capital integration tests](../../../../finstack-quant/statements/tests/capital_structure_integration.rs) and [accrual audit tests](../../../../finstack-quant/statements/tests/audit_accrual.rs). Inspected [Monte Carlo capital replay](../../../../finstack-quant/statements/tests/monte_carlo_capital_structure.rs) confirms use of the same period engine; it is not an independent debt accounting oracle. No tests were executed.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-evaluation-forecasts-and-reporting).
