# STATEMENTS-CAPITAL-002: Debt and hedge cashflow classification and period boundaries

**Decision status:** Draft — observed implementation; adoption pending.
**Implementation status:** Observed for the source scope and discrepancies stated below.
**Validation status:** Evidence inspected or located, not executed; numerical qualification remains open.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; [scope and file ledger](README.md).

## Choice and rationale

Cashflow kinds and the declared instrument kind jointly determine statement debt service. Coupon legs are netted within one instrument and period before separating positive expense and positive income. This keeps hedge receipts and negative-rate debt coupons out of positive waterfall expense claims. The rationale follows the explicit source contract; historical adoption is not inferred.

## Inputs and ordered calculation

`calculate_period_flows` takes an instrument, actual reporting interval, stateful opening balance, market context/as-of date and optional residual schedule. The residual schedule wins when supplied; otherwise it requests the full raw schedule, retaining history and PIK events. Placeholder schedules and unsupported instrument categories fail.

Cash belongs to the half-open interval `[start,end)`. Balances/accrual use the inclusive snapshot `end−1 day`; the snapshot helper returns `start` for an empty/reversed interval, whereas the dated-principal constructor rejects such intervals. Opening scheduled principal is the latest balance strictly before `start`. The scaling and funding rules are in [03](03-funding-and-balances.md).

For each in-period flow, reject a currency differing from the schedule notional. With scale `s`, accumulate signed `Fixed`, `Stub`, and `FloatReset` cash into `I=Σ amount·s`. Borrowing debt uses net expense `E=I`; hedges use `E=−I`. Report expense `max(E,0)` and income `max(−E,0)`. Two hedge legs can offset across different payment dates inside the same reporting period; this is period netting, not daily collateral settlement.

Principal-repayment kinds include amortization, prepayment, revolver repayment and positive Notional redemption; record their absolute cash magnitudes times `s`. Notional/RevolvingDraw funding is not a principal payment. PIK is an absolute amount times `s` in a separate noncash expense field. Commitment, facility, letter-of-credit and fronting fees pass through at absolute scheduled amounts, because their base need not be drawn principal. Generic and usage fees use `abs(amount)·s`.

DefaultedNotional, Recovery and unrecognized kinds produce structured ignored-cashflow warnings and no debt-service bucket. The outstanding schedule may still contain their principal effects; ignoring their cash classification is not credit-risk valuation. Hedging instruments use scale one and zero debt balance, funding and debt accrued interest. Hedge-leg mark-to-market/accrued valuation is outside this API.

## Controls, limitations and alternatives

A zero opening balance may carry a different currency without failure; a nonzero mismatched opening fails. `Money` conversions and checked additions propagate representation/currency failures. Fee/principal/PIK magnitudes discard original direction; custom schedules must respect the documented holder/debt signs. Netting all coupon cash in a period loses the timing of gross hedge receipts/payments. A gross-leg or daily-liquidity report would answer a different question and need separate fields.

Ignored credit events are visible warnings, not a complete distressed-debt accounting policy. Unknown kinds are ignored with warning rather than guessed. Public callers must supply a meaningful `Period`; uniform invalid-period rejection would require a separate change.

## Evidence

- **Implementation:** [period extraction](../../../../finstack-quant/statements/src/capital_structure/period_flows.rs), `is_cash_interest_kind`, `is_debt_kind`, `period_snapshot_date`, `calculate_period_flows` and `period_principal_claims`.
- **Regression evidence:** [declared-kind tests](../../../../finstack-quant/statements/src/capital_structure/tests/senior_review_capital.rs), `declared_swap_kind_nets_negative_rates_and_zero_fixed_legs` and `canonical_negative_debt_coupons_are_income`; inline period tests exercise boundaries, currency, fees and ignored kinds. These are implementation/invariant fixtures, not independent reference reconciliation; not executed.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-capital-structure).
