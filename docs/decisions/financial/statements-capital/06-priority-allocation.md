# STATEMENTS-CAPITAL-006: Priority cash allocation, seniority and conservation controls

**Decision status:** Accepted for the full waterfall allocation and conservation contract (2026-11-15).
**Implementation status:** Reconciliation pending. The audited path uses a direct dependency guard, floating-point allocation, fixed shortfall and balance thresholds, and unconditional residual distributions.
**Validation status:** Existing source and regression evidence mapped. Independent contractual waterfall and exact-conservation qualification remain pending.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; [scope and file ledger](README.md).

## Accepted decision for waterfall cash allocation and conservation

1. Use one finite pre-debt-service cash pool in the waterfall's native currency, with `cash` as the default expression. Check the expression and its transitive dependencies for any same-period dependence on outputs of that waterfall. A negative pool produces a funding shortfall and zero allocatable cash. Evaluate the configured waterfall even when no debt claims are present.
2. Require Fees, Interest and Amortization once each in the configured payment order. Each optional prepayment priority requires its corresponding sizing input and each sizing input requires its priority. Equity, when selected, must be last. Validate unique categories, class IDs, class ranks, claim currencies and instrument membership. When classes are omitted, use one class. Pay senior classes first within each category, allocate pro rata across eligible instrument claims within a class, and apply each instrument's allocation to its oldest eligible due claims first. [Prepayment opt-in](01-instrument-construction.md#accepted-decision-for-opt-in-waterfall-prepayments) · [Dated principal allocation](04-dated-principal.md#scheduled-principal-allocation).
3. Pay exact `Money` quotas when permitted and representable. Otherwise use an authorized payment quantum. Floor quotas to that quantum, then distribute remaining whole quanta to eligible instruments by largest fractional remainder, breaking ties by stable ID and respecting claim and cash caps. Keep cash that cannot be allocated in the pool for terminal treatment. With five cents and three equal eligible claims each able to receive two cents, allocate two cents each to the first two stable IDs and one cent to the third. [Monetary precision and rounding](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity).
4. Carry every unpaid contractual amount, including subcent balances. Reject negative outflow claims and allocations above claim capacity or available cash. Reconcile allocatable cash exactly to fees, cash interest, principal including prepayments, distributions and retained cash. Commit financial state, schedules and derived totals together after validation.
5. Distribute residual cash when the configured terminal category is Equity. Otherwise report it as retained cash. For example, a \$15 residual is a \$15 equity distribution with terminal Equity and \$15 of retained cash when Equity is absent.

## Observed selection and input validation

The default order is Fees, Interest, Amortization and Equity. The default cash expression is `cash`. Fees, Interest and Amortization are mandatory, and priorities must be unique. Equity, if present, must be last. Each prepayment priority requires its matching sizing input, and each sizing input requires its matching priority. Payment classes require unique nonempty IDs, unique ranks, nonempty member lists and no duplicate membership. All contractual rows must resolve to a class. [Accepted prepayment selection and instrument scope](01-instrument-construction.md#accepted-decision-for-opt-in-waterfall-prepayments).

A nonempty cash expression is required. A substring guard rejects `cs.interest_expense`, `cs.interest_expense_cash`, `cs.principal_payment` or `cs.fees` in it and in a directly named node's formula to avoid funding cash already net of allocated debt service. It is not transitive dependency analysis and does not prove every supplied pool is pre-debt-service. A referenced node value is preferred; otherwise compile/evaluate an inline expression against a cloned context, preserving newly raised warnings. Nonfinite results and unrepresentable Money amounts error.

## Observed ordered allocation

1. Validate the spec. An empty contractual map returns no flows, cash pool or equity result; target/currency checks then do not run. Otherwise validate targets and require one currency across all rows and fields. Cross-currency allocation is unsupported.
2. Evaluate PIK state before allocation. Evaluate available cash; negative cash emits a warning and is floored to zero. Stage prior interest/fee arrears, principal capacity and class ranks; clear principal result components to zero.
3. Move enabled PIK coupons before cash caps. Walk the configured stack. Fees and interest neutralize negative planned claims to zero with a warning. Amortization uses dated remaining capacity; prepayment rules are in [07](07-prepayments-and-ecf.md).
4. For each class, let `Q=Σ q_i` for nonnegative claims. Pay all if cash suffices; otherwise use `cash·q_i/Q`, capped by each claim. The final array entry receives remaining floating-point residual capped to its own claim; available cash is then zero. Class vectors retain zero entries for other classes, so this is the final staged row, not necessarily the final positive member of that class.
5. Carry material fee/interest/principal shortfalls with structured warnings; `1e−6` currency units distinguishes reported shortfalls from noise. Allocate all remaining cash to equity after the stack **even if no Equity rung is present**.
6. Finish principal accounts, reject post-payment balances below `−0.005`, collapse absolute balances below `0.005` to zero, then add toggle PIK. Preserve earned accrued interest and add new unpaid interest. Validate every output currency invariant. Commit staged state only after the entire calculation succeeds.

The intended identity is `paid fees + paid cash interest + paid principal + equity = floored available cash`; income and PIK are separate, and income must already be reflected in the supplied operating cash pool if it should fund service.

## Limitations, alternatives and evidence

Absolute tolerances and the 0.005 dust floor are currency-agnostic; this is not a currency minor-unit rounding policy. Pro-rata arithmetic is f64 before Money conversion. The current source's “exact” conservation language is stronger than an unqualified floating-point proof, particularly when the last staged row is outside the paying class. No material failure was reproduced.

The audited path distributes residual cash to equity even when the Equity rung is absent. Separate retained-cash reporting remains pending.

- **Implementation:** [orchestrator](../../../../finstack-quant/statements/src/capital_structure/waterfall/mod.rs), `eval_value_or_formula`, `execute_waterfall`; [distribution](../../../../finstack-quant/statements/src/capital_structure/waterfall/cash_distribution.rs); [currency validation](../../../../finstack-quant/statements/src/capital_structure/waterfall/payment_stack.rs); [spec validation](../../../../finstack-quant/statements/src/capital_structure/waterfall_spec.rs).
- **Regression evidence:** inline `capped_waterfall_conserves_available_cash_identity`, `payment_classes_pay_senior_interest_before_junior`, `waterfall_errors_preserve_state_and_retry_matches_a_clean_run`, nonfinite/currency/overflow/warning cases; [priority regressions](../../../../finstack-quant/statements/src/capital_structure/tests/finance_waterfall_regressions.rs). Internal controls/invariants, not an external contractual waterfall benchmark; not executed.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-capital-structure).
