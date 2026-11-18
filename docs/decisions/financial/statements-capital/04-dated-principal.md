# STATEMENTS-CAPITAL-004: Dated principal claims, settlement arrears and advance reserves

**Decision status:** Accepted for the full dated principal claims, settlement arrears and advance reserves contract (2026-11-15).
**Implementation status:** Reconciled (2026-11-17). Outstanding economic principal is the dated schedule's close and falls only by prepayments: a scheduled reduction takes effect on its contractual date whether or not its cash was paid, and that balance is the one the opening reconciliation reads. Each scheduled payment is a dated claim with a stable identity (due date, effective date and ordinal), its cash, the face that cash settles and its payment history, including partial payments. Claims due and unpaid are reported as `principal_arrears` and carried with their identity and dates; cash owed for a reduction already effective and not yet due is `principal_pending`; face paid before its reduction is `principal_advance_reserve`, which stays in outstanding principal until effective and is excluded from prepayment capacity. Claims are paid oldest due first, carried before current. The aggregate path that synthesized a same-period claim is removed: a waterfall fails for an instrument without dated claims, and current claims must equal the contractual principal cash exactly in native `Money`. A scheduled amount beyond the balance is rejected. The waterfall commits balances, claims, reserves and schedules together on success. Payment sizing inside the waterfall is still computed in floating point and validated to `1e-6`; the exact-allocation contract owns that. The sections below the accepted decision describe the audited implementation.
**Validation status:** Qualified by the record's 31 March and 15 April example on a dated schedule and in a waterfall (pending, then a partial payment on the due date, arrears, and a late payment that leaves the balance unchanged), a payment ahead of its effective date that is reserved, excluded from a sweep and released on that date, missing dated claims, claims a cent short of the contractual cash and scheduled principal beyond the balance.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; [scope and file ledger](README.md).

## Accepted decision for dated principal claims

### Claim identity and dated inputs

Require stable claim identity, the instrument and its native currency for each scheduled principal claim. Keep signed cash and the face effect distinct. Retain the contractual due date and economic effective date, plus the actual or modeled payment date and amount for each payment, including partial payments. Require dated claims for scheduled principal and remove the aggregate path that synthesizes a same-period claim.

### Accepted decision for separate principal and settlement reporting

When a principal reduction takes effect under the contract independently of cash settlement, reduce outstanding economic principal on the effective date. Use that outstanding amount for opening principal reconciliation. Report the outstanding cash obligation separately with its payment due date and status. Mark it pending before the due date, classify the unpaid amount as arrears when payment is missed, and discharge the cash obligation as payments are made. [Dated principal effects](../core-conventions/015-cashflow-semantics.md#accepted-decision-cash-settlement-principal-effects-and-accrual-metadata) · [Opening principal reconciliation](03-funding-and-balances.md#accepted-decision-for-opening-principal-reconciliation).

For example, a \$20 principal reduction effective March 31 leaves \$80 outstanding on a \$100 instrument. The \$20 due April 15 is pending until its due date, becomes arrears if missed, and is discharged by later payment. The outstanding economic principal stays at \$80 when that payment settles the existing obligation.

### Principal paid before its economic effective date

Record cash on its actual or modeled payment date. Reserve the face already paid from further allocation until the contractual reduction takes effect. Report the reserve separately and reduce outstanding economic principal on the effective date.

### Scheduled principal allocation

Follow the configured waterfall category order. Within scheduled principal, fund senior payment classes first. Within each class, allocate pro rata by each instrument's total eligible principal claims, including arrears. Apply each instrument's allocation to its oldest due claims first.

### Exact reconciliation and atomic state

Reconcile cash, face and claim amounts exactly in native-currency `Money` after any authorized rounding. Detailed rounding remains a separate policy. Commit balances, dated claims, advance reserves and residual schedule state together after successful reconciliation. [Monetary precision and rounding](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity) · [Numerical representation and rounding](../conventions/0020-numerical-representation-and-rounding.md).

## Observed inputs and algorithm

The allocation engine tracks economic capacity, settlement-only claims and principal cash paid before its economic date. Dated claims are mandatory for market-aware evaluation. Aggregate-only callers use a narrower fallback.

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
