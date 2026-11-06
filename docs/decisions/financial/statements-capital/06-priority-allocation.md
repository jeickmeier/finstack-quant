# STATEMENTS-CAPITAL-006: Priority cash allocation, seniority and conservation controls

**Decision status:** Draft — observed implementation; adoption pending.
**Implementation status:** Observed for the source scope and discrepancies stated below.
**Validation status:** Evidence inspected or located, not executed; numerical qualification remains open.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; [scope and file ledger](README.md).

## Choice and rationale

One finite pre-waterfall cash pool funds each payment category in configured order. Within a category, ascending class rank is senior; rows in a class share cash pro rata. Computation mutates cloned state and commits only on success, preserving a retryable period if a later validation or amount conversion fails.

## Selection and input validation

Default order is Fees, Interest, Amortization, Sweep, Equity and default cash expression is `cash`. Fees/Interest/Amortization are mandatory; priorities must be unique. Equity, if present, must be last. Named mandatory/voluntary nodes and their priorities require each other. Payment classes require unique nonempty IDs, unique ranks, nonempty member lists and no duplicate membership; all contractual rows must resolve to a class. Positive ECF requires Sweep. Enclosing instrument restrictions are in [01](01-instrument-construction.md).

A nonempty cash expression is required. A substring guard rejects `cs.interest_expense`, `cs.interest_expense_cash`, `cs.principal_payment` or `cs.fees` in it and in a directly named node's formula to avoid funding cash already net of allocated debt service. It is not transitive dependency analysis and does not prove every supplied pool is pre-debt-service. A referenced node value is preferred; otherwise compile/evaluate an inline expression against a cloned context, preserving newly raised warnings. Nonfinite results and unrepresentable Money amounts error.

## Ordered allocation

1. Validate the spec. An empty contractual map returns no flows, cash pool or equity result; target/currency checks then do not run. Otherwise validate targets and require one currency across all rows and fields. Cross-currency allocation is unsupported.
2. Evaluate PIK state before allocation. Evaluate available cash; negative cash emits a warning and is floored to zero. Stage prior interest/fee arrears, principal capacity and class ranks; clear principal result components to zero.
3. Move enabled PIK coupons before cash caps. Walk the configured stack. Fees and interest neutralize negative planned claims to zero with a warning. Amortization uses dated remaining capacity; prepayment rules are in [07](07-prepayments-and-ecf.md).
4. For each class, let `Q=Σ q_i` for nonnegative claims. Pay all if cash suffices; otherwise use `cash·q_i/Q`, capped by each claim. The final array entry receives remaining floating-point residual capped to its own claim; available cash is then zero. Class vectors retain zero entries for other classes, so this is the final staged row, not necessarily the final positive member of that class.
5. Carry material fee/interest/principal shortfalls with structured warnings; `1e−6` currency units distinguishes reported shortfalls from noise. Allocate all remaining cash to equity after the stack **even if no Equity rung is present**.
6. Finish principal accounts, reject post-payment balances below `−0.005`, collapse absolute balances below `0.005` to zero, then add toggle PIK. Preserve earned accrued interest and add new unpaid interest. Validate every output currency invariant. Commit staged state only after the entire calculation succeeds.

The intended identity is `paid fees + paid cash interest + paid principal + equity = floored available cash`; income and PIK are separate, and income must already be reflected in the supplied operating cash pool if it should fund service.

## Limitations, alternatives and evidence

Absolute tolerances and the 0.005 dust floor are currency-agnostic; this is not a currency minor-unit rounding policy. Pro-rata arithmetic is f64 before Money conversion. The current source's “exact” conservation language is stronger than an unqualified floating-point proof, particularly when the last staged row is outside the paying class. No material failure was reproduced.

An omitted Equity rung does not retain cash on balance sheet. Modelers needing retained cash require a different explicit output contract. Sequential exact/decimal allocation or residual-to-last-eligible-member would have different rounding behavior; no change is made.

- **Implementation:** [orchestrator](../../../../finstack-quant/statements/src/capital_structure/waterfall/mod.rs), `eval_value_or_formula`, `execute_waterfall`; [distribution](../../../../finstack-quant/statements/src/capital_structure/waterfall/cash_distribution.rs); [currency validation](../../../../finstack-quant/statements/src/capital_structure/waterfall/payment_stack.rs); [spec validation](../../../../finstack-quant/statements/src/capital_structure/waterfall_spec.rs).
- **Regression evidence:** inline `capped_waterfall_conserves_available_cash_identity`, `payment_classes_pay_senior_interest_before_junior`, `waterfall_errors_preserve_state_and_retry_matches_a_clean_run`, nonfinite/currency/overflow/warning cases; [priority regressions](../../../../finstack-quant/statements/src/capital_structure/tests/finance_waterfall_regressions.rs). Internal controls/invariants, not an external contractual waterfall benchmark; not executed.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-capital-structure).
