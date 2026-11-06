# STATEMENTS-CAPITAL-008: PIK toggles, hysteresis, carried claims and period state

**Decision status:** Draft — observed implementation; adoption pending.
**Implementation status:** Observed for the source scope and discrepancies stated below.
**Validation status:** Evidence inspected or located, not executed; numerical qualification remains open.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; [scope and file ledger](README.md).

## Choice and rationale

A liquidity threshold can move selected borrowing coupons from cash expense into PIK before cash allocation. A minimum number of consecutive PIK periods prevents immediate deactivation. Contractual PIK follows its schedule date; toggle-generated PIK capitalizes at period close. Separate carried maps preserve fee, interest and dated principal claim priority.

## Inputs and transitions

`PikToggleSpec` names a liquidity node/formula, threshold in that metric's units, explicit target IDs and a `min_periods_in_pik` count (default zero). The metric must evaluate to a finite number. Initial mode is false with zero active periods. The request is `metric < threshold`, so equality requests cash mode.

For each targeted current instrument: if currently PIK, requested off, and prior consecutive count is below the minimum, remain PIK; otherwise use the request. Increment count each active period and reset to zero when off. Minimum duration measures evaluated reporting periods, not elapsed days or coupons; changing reporting frequency changes its duration.

Prior unpaid cash interest is added to current cash expense before the PIK transfer. If the state's mode is active, move the **entire staged cash-interest expense, including carried arrears**, into PIK and zero cash expense. This precedes all cash caps and ECF deductions. Earned accrued interest is not erased. Fee arrears remain fees and are not toggled. Explicit targets are required; enclosing model validation rejects hedge/option targets, but contractual instrument-level PIK permission/rate step-up is not otherwise modeled.

At close add only the amount moved by the toggle to principal after the principal ledger finishes/dust-floors. Contractual PIK already belongs to dated economic principal and must not be capitalized again. Toggle PIK can create closing principal even after the opening principal has been fully repaid that period. Unpaid interest above `1e−6` is added to accrued and carried to next period; fee arrears remain a separate fee-rung claim; dated principal arrears/advances retain dates.

`advance_period` promotes closing balances into opening using a move, clears closing and current-period principal-flow inputs, and retains mode/counters/arrears/residual schedules. Funding is overwritten per evaluated period. Residual schedules are rebuilt from closing at the inclusive period end as in [05](05-residual-coupons.md).

## Confirmed input gap and other limitations

`PikToggleSpec::validate` checks nonempty targets only; it does not require finite threshold or a nonempty metric string. The metric evaluation path rejects nonfinite metric output, but direct typed `threshold=NaN` is not rejected and `metric < NaN` is false. Positive/negative infinity likewise choose always-on/off requests subject to hysteresis. This is a confirmed static input-contract gap, not a measured downstream loss; wire-format acceptance can differ from direct Rust construction.

Arrears capitalization and full-paydown-period toggle capitalization require explicit contractual permission; the engine does not distinguish coupon entitlement from an unpaid prior claim for this decision. Supplying a different toggle spec midstream does not globally reset all stored modes. These are stateful scenario semantics, not automatic legal rights.

## Alternatives and evidence

Alternatives include coupon-date decisions, elapsed-time hysteresis, current-coupon-only PIK, and explicit instrument capability/rate terms. Each changes principal and cash liquidity; none is adopted here.

- **Implementation:** [toggle state](../../../../finstack-quant/statements/src/capital_structure/waterfall/payment_in_kind.rs); [staging and close](../../../../finstack-quant/statements/src/capital_structure/waterfall/mod.rs); [validation](../../../../finstack-quant/statements/src/capital_structure/waterfall_spec.rs), `PikToggleSpec::validate`; [persistent state](../../../../finstack-quant/statements/src/capital_structure/state.rs).
- **Regression evidence:** inline `test_pik_hysteresis_holds_pik_active_for_min_periods`, `pik_liquidity_metric_rejects_non_finite_value`, `test_pik_coupon_never_consumes_cash_without_prepayment_priority`; [earned-accrual and dated PIK cases](../../../../finstack-quant/statements/src/capital_structure/tests/finance_waterfall_regressions.rs). Internal state/timing checks, not executed.

Period-close timing remains in force unless a separately authorized change is made.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-capital-structure).
