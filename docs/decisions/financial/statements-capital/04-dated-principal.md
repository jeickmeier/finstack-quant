# STATEMENTS-CAPITAL-004: Dated principal claims, settlement arrears and advance reserves

**Decision status:** Draft — observed implementation; adoption pending.
**Implementation status:** Observed for the source scope and discrepancies stated below.
**Validation status:** Evidence inspected or located, not executed; numerical qualification remains open.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; [scope and file ledger](README.md).

## Choice and rationale

Principal allocation separates economic capacity, settlement-only claims and principal cash paid before its economic date. This prevents a late settlement reducing economic debt twice and prevents an early payment leaving the same principal available to a second sweep. Dated claims are mandatory for market-aware evaluation; aggregate-only callers use a narrower fallback.

## Inputs and algorithm

A `PrincipalClaim` carries nonnegative native-currency amount, cash payment date and economic balance date. Current cash dates must lie in a valid half-open period. The sum of current dated claims must match contractual principal cash within `1e−6` currency units. Original dates survive carried arrears and advances.

1. Start economic capacity `E` from contractual economic closing. For carried paid advances whose balance date is at/after period end, subtract their amount from `E` and put it in future-paid reserve `F`. Advances that become economically effective this period are no longer reserved.
2. Add back carried unpaid principal becoming economically effective inside the period; older effective arrears already sit in opening/residual debt. Carried claims are sorted by `(payment_date,balance_date)` and precede the separately sorted current claims.
3. For current claims economically effective before period start, put amount into settlement-only capacity `S`; for those effective inside the period add it back to `E`; later effective claims already sit in economic principal. Reject currency, negative amount and materially negative `E` errors; clip `E` in `[-1e−6,0)` to zero.
4. Prepayments can use `E` only. Scheduled capacity walks claims in order, limiting each by the remaining associated `E` or `S` so claims cannot reserve more capacity than exists. Payment reduces the corresponding bucket. Paying a claim with economic date at/after period end reduces `E` now but adds it to `F` and stores an advance lot.
5. Finish with `closing=E+S+F`, unpaid dated claims and outstanding advance lots. Adding `F` back preserves still-effective economic notional; its reserve prevents paying it again. Remaining settlement-only cash liability is carried in closing, without reapplying the prior contractual principal reduction.
6. Without dated input, initialize `E=opening+funding+contractual PIK`, put aggregate scheduled principal in a synthetic same-period claim and reject any carried advance lots. This fallback has no means to model separated dates.

## Controls and tradeoffs

Payments must match currency, be nonnegative and not exceed capacity by more than `1e−6`. Scheduled claims truncated by earlier prepayment capacity do not become phantom arrears; economically prior settlement-only claims remain payable even if the live economic principal is swept. All state is staged transactionally by the enclosing waterfall.

Compared with a single `opening+draws−cash principal` balance, the ledger preserves economic and settlement ordering at the cost of dated state. It is not a legal default/acceleration ledger, and no default interest or penalty is generated. The returned closing can contain unpaid settlement claims, so a debt report should not equate it automatically to contractual economic face alone.

## Evidence

- **Implementation:** [principal ledger](../../../../finstack-quant/statements/src/capital_structure/principal.rs), `PeriodPrincipalFlows::new`, `PrincipalAllocation::{new,pay_prepayment,pay_scheduled,finish}`; [state integration](../../../../finstack-quant/statements/src/capital_structure/state.rs); [claim extraction](../../../../finstack-quant/statements/src/capital_structure/period_flows.rs).
- **Regression evidence:** [finance waterfall cases](../../../../finstack-quant/statements/src/capital_structure/tests/finance_waterfall_regressions.rs), `delayed_principal_settlement_does_not_reduce_economic_balance_twice`, `delayed_settlement_remains_payable_after_economic_principal_is_swept`, `early_unpaid_principal_is_carried_without_creating_debt_before_its_economic_date`, and `advance_principal_cannot_be_repaid_again_before_its_economic_date`. These are dated ledger/invariant examples, not independently audited debt statements; not run.

The zero-opening upstream concern is in [03](03-funding-and-balances.md).

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-capital-structure).
