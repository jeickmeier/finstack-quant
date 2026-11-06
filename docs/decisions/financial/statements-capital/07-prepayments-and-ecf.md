# STATEMENTS-CAPITAL-007: Independent prepayment buckets and excess-cash-flow sweep

**Decision status:** Draft — observed implementation; adoption pending.
**Implementation status:** Observed for the source scope and discrepancies stated below.
**Validation status:** Evidence inspected or located, not executed; numerical qualification remains open.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; [scope and file ledger](README.md).

## Choice and rationale

Mandatory, voluntary and ECF prepayments are independently sized and execute at their configured position in the shared cash/principal stack. Earlier rungs consume capacity; later contractual or targeted payments reserve none. This preserves the configured category/class priority rather than silently privileging future scheduled amortization.

## ECF sizing and allocation

For the current period evaluate finite node/formula values in the waterfall currency. Omitted tax, capital expenditure and change-in-working-capital nodes contribute zero. They are subtracted with their supplied sign; negative taxes/capex/working-capital use increases ECF. EBITDA is required. Cash interest uses an explicit override if present, else interest **actually paid by earlier priorities**, including arrears, and is floored at zero.

`ECF = EBITDA − taxes − capex − ΔWC − max(cash_interest,0) − earlier_paid_scheduled_principal − earlier_paid_fees`.

`requested_sweep = max(ECF · sweep_percentage,0)` where percentage is finite and in `[0,1]`. Only scheduled principal in earlier Amortization payments is in this principal deduction; the independent mandatory/voluntary prepayment buckets are not automatically deducted as scheduled amortization. PIK and unpaid service consume no cash. If Interest/Amortization follows Sweep, its automatic deduction is zero and the later category has no reserved cash. This is a stack-dependent formula, not a universal lender definition of ECF.

At Sweep, apply only the configured ECF amount. An optional target must exactly match a contractual row; without one allocate against all staged principal capacities by class and pro rata. There is no runtime instrument-type filter in the allocation helper; model validation restricts eligible capital structures. The “all term loans” wording on the target field should not be read as filtering out an admitted revolver.

Mandatory/voluntary rungs separately evaluate their sizing expressions, floor negative requests to zero, and use the same generic allocation without an ECF target. For any prepayment, budget is `max(min(requested,remaining_cash),0)`, additionally bounded by each row's remaining economic principal. Record only actual allocated cash, reduce both available cash and capacity, and increment the corresponding audit component. Requests exceeding capacity leave unused cash for later rungs/equity.

## Controls, alternatives and limitations

Specs reject missing/mismatched sizing-node priorities; positive ECF without Sweep fails even when another prepayment priority exists. Missing target IDs and nonfinite values fail. All prepayments are at par: premiums, penalties, legal restricted-payment tests and OID accretion are absent. Earlier principal cannot be repaid again, and no later payment can manufacture arrears against extinguished capacity.

Real credit agreements often define addbacks, thresholds, leverage step-downs, exclusions, mandatory-prepayment credits and permitted carryforwards. Those are not inferred here. They can be represented partly in supplied formulas, but the fixed deduction/order contract still applies. Alternative reserving of future debt service would change seniority and cash timing.

## Evidence

- **Implementation:** [ECF equation](../../../../finstack-quant/statements/src/capital_structure/waterfall/excess_cash_flow.rs); [prepayment allocation](../../../../finstack-quant/statements/src/capital_structure/waterfall/cash_distribution.rs), `apply_prepayment`; [priority dispatch](../../../../finstack-quant/statements/src/capital_structure/waterfall/mod.rs), `apply_named_prepay`; [spec/target semantics](../../../../finstack-quant/statements/src/capital_structure/waterfall_spec.rs).
- **Regression evidence:** [finance waterfall tests](../../../../finstack-quant/statements/src/capital_structure/tests/finance_waterfall_regressions.rs), `prepayment_rungs_follow_configured_order_and_class_rank`, `ecf_deducts_cash_interest_fees_and_scheduled_arrears_actually_paid`, `ecf_does_not_reserve_cash_for_a_later_interest_priority`; [named-priority and sweep cases](../../../../finstack-quant/statements/src/capital_structure/tests/senior_review_capital.rs). Formula/ordering fixtures only, not executed.

No external compliance claim is made.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-capital-structure).
