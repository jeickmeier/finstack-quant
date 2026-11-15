# STATEMENTS-CAPITAL-002: Debt and hedge cashflow classification and period boundaries

**Decision status:** Accepted for the full debt and hedge cashflow classification contract (2026-11-15).
**Implementation status:** Period netting, separate expense and income totals, and the Interest claim are implemented. Reconciliation of the remaining policy is pending, including source-perspective handling for custom schedules and separate distress cash reporting.
**Validation status:** Source and regression evidence mapped. Independent qualification remains pending, including borrower and hedge cash-ledger reconciliation.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; [scope and file ledger](README.md).

## Accepted decision for per-instrument coupon netting

Convert each signed coupon cash amount from the declared schedule perspective to the company's cash perspective, where a receipt is positive and a payment is negative. Use the schedule producer's stated perspective at this boundary. Sum the signed coupon amounts for each instrument and reporting period over settlement dates in `[start,end)`. For net company cash `C`, report expense `max(−C,0)` and income `max(C,0)`. Sum expense and income separately across instruments. This keeps a hedge receipt on one instrument from reducing another instrument's scheduled interest claim. [ADR 0044 financial perspectives](../conventions/0044-financial-perspectives-and-boundary-conversions.md#decision).

Use each instrument's positive net expense as its scheduled claim in the waterfall's Interest category. The configured pre-waterfall cash pool determines the cash available to pay claims, including any hedge receipts the cash-pool definition includes. [Waterfall cash allocation](06-priority-allocation.md).

Use the accepted settlement, balance and accrual clocks for their respective quantities and the accepted period-close FX boundary for reporting totals. Netting can combine coupon settlements from multiple payment dates within one reporting period. [ADR 0045 date boundaries](../conventions/0045-financial-clocks-and-date-boundaries.md#preserve-date-boundaries) · [Cash settlement and principal effects](../core-conventions/015-cashflow-semantics.md#accepted-decision-cash-settlement-principal-effects-and-accrual-metadata) · [Reporting FX](09-reporting-and-fx.md).

## Accepted decision for separate distress reporting

Keep identified distress cash, such as a recovery or separate cash settlement on default, outside scheduled debt service. Report each amount in the company's cash perspective, with receipts positive and payments negative, and retain its instrument, currency and actual payment date.

Apply an explicit dated principal change only when it changes the reporting company's liability, on its economic date. A lender impairment or a distress cash amount alone supplies no principal change. A liability of 100 and an explicit reduction of 40 leave principal of 60. If the company also pays a separately identified distress settlement of 15 and has no scheduled principal payment, report scheduled principal of zero and a separate distress cash flow of −15 on the settlement date. [Cash settlement and principal effects](../core-conventions/015-cashflow-semantics.md#accepted-decision-cash-settlement-principal-effects-and-accrual-metadata).

## Accepted decision for period admission and cashflow classification

1. Validate the reporting period before reading a schedule. Assign cash to the half-open period `[start,end)` by settlement date. Convert each signed cash amount once from the producer's declared perspective to the company's perspective. Custom and rebuilt schedules must establish the producer perspective and instrument native currency, including for zero amounts.
2. Apply the accepted coupon netting and distress reporting sections above. Report each instrument's net coupon expense or income, sum them separately and use each positive net expense as the scheduled Interest claim. Hedge receipts are available for allocation only if the configured cash pool includes them. Report signed distress cash separately and apply only dated explicit principal changes to the reporting company's liability.
3. Record signed funding proceeds, repayment cash and economic principal separately on their governing dates. Use explicit principal effects for face changes. PIK has zero cash and a signed principal effect on its capitalization date, including any authorized correction. [Dated principal claims](04-dated-principal.md#accepted-decision-for-dated-principal-claims).
4. Classify fees under the canonical contract. Payable fees supply waterfall fee claims, and fee rebates remain receipts. Classify supported hedge settlements in their declared categories. Apply borrowing balances, funding and debt accrual only to debt. Reject unclassified cash or principal effects.

For example, a USD 100 liability extinguished on March 31 has a USD 100 face reduction in March. A USD 102 payment on April 1 records company cash of USD −102 in April and settles the payment obligation.

## Observed inputs and ordered calculation

`calculate_period_flows` takes an instrument, actual reporting interval, stateful opening balance, market context/as-of date and optional residual schedule. The residual schedule wins when supplied; otherwise it requests the full raw schedule, retaining history and PIK events. Placeholder schedules and unsupported instrument categories fail.

Cash belongs to the half-open interval `[start,end)`. Balances/accrual use the inclusive snapshot `end−1 day`; the snapshot helper returns `start` for an empty/reversed interval, whereas the dated-principal constructor rejects such intervals. Opening scheduled principal is the latest balance strictly before `start`. The scaling and funding rules are in [03](03-funding-and-balances.md).

For each in-period flow, reject a currency differing from the schedule notional. With scale `s`, accumulate signed `Fixed`, `Stub` and `Floating` cash into `I=Σ amount·s`. The current extraction interprets borrowing schedule coupons as issuer expense when positive and rate-hedge coupons as company receipts when positive. It uses the declared instrument kind to apply that source-specific sign and then splits the result into expense and income. This routing assumes custom or residual schedules supplied at this boundary use the corresponding producer's perspective.

Principal-repayment kinds include amortization, prepayment, revolver repayment and positive Notional redemption. Their absolute cash magnitudes are multiplied by `s`. Notional and RevolvingDraw funding are excluded from principal payments. PIK uses the absolute principal delta times `s` in a separate noncash expense field. Commitment, facility, letter-of-credit and fronting fees pass through at absolute scheduled amounts because their base can differ from drawn principal. Generic and usage fees use `abs(amount)·s`.

`DefaultedNotional` and `Recovery` currently produce structured ignored-cashflow warnings and have no separate cash reporting. The outstanding schedule may still carry explicit principal effects. Unrecognized kinds also produce warnings. Hedging instruments use scale one and zero debt balance, funding and debt accrued interest. Hedge-leg mark-to-market/accrued valuation is outside this API.

## Audited controls, limitations and alternatives

A zero opening balance may carry a different currency without failure; a nonzero mismatched opening fails. `Money` conversions and checked additions propagate representation/currency failures. Fee/principal/PIK magnitudes discard original direction; custom schedules must respect the documented holder/debt signs. Netting all coupon cash in a period loses the timing of gross hedge receipts/payments. A gross-leg or daily-liquidity report would answer a different question and need separate fields.

Unknown kinds are ignored with a warning. Public callers must supply a meaningful `Period`. The audited implementation lacks uniform invalid-period rejection.

## Evidence

- **Implementation:** [period extraction](../../../../finstack-quant/statements/src/capital_structure/period_flows.rs), `is_cash_interest_kind`, `is_debt_kind`, `period_snapshot_date`, `calculate_period_flows` and `period_principal_claims`.
- **Totals and claims** use distinct expense and income buckets at reporting aggregation and use the expense bucket for scheduled waterfall Interest. [Reporting totals](../../../../finstack-quant/statements/src/evaluator/capital_structure_runtime.rs) · [Waterfall Interest allocation](../../../../finstack-quant/statements/src/capital_structure/waterfall/mod.rs).
- **Regression evidence:** [declared-kind tests](../../../../finstack-quant/statements/src/capital_structure/tests/senior_review_capital.rs), `declared_swap_kind_nets_negative_rates_and_zero_fixed_legs` and `canonical_negative_debt_coupons_are_income`; inline period tests exercise boundaries, currency, fees and ignored kinds. These are implementation/invariant fixtures, not independent reference reconciliation; not executed.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-capital-structure).
