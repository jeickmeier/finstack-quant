# VALUATIONS-STRUCTURED-CREDIT-07: Card master-trust allocation

- **Decision status:** Accepted 2026-11-15 — complete card trust collections, investor and seller allocation, funding, amortization and metric package.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent monthly collection, allocation, funding and metric calculations checked. Production pricing, calibration and host qualification remain pending.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Accepted decision for card master-trust allocation

1. **Calculate gross trust flows on the monthly collection grid.** Apply
   the annual simple portfolio yield to opening eligible trust principal
   with the stated accrual convention. Eligible finance charges,
   interchange and fees enter yield. Keep principal and recovery
   separate. Convert annual effective gross charge-off to monthly
   conditional mortality and apply it to opening eligible par. Apply
   the separately supplied monthly principal payment rate to
   post-charge-off par. Post any dated replenishment before the next
   monthly calculation, including for deals distributing quarterly.
   Require a qualified stub basis or explicit cash amounts.
   [Chase collection definitions](https://www.sec.gov/Archives/edgar/data/1174821/000000000015046959/filename1.pdf).

2. **Allocate each cash and loss component on its own basis.** During
   revolving use live investor reference divided by qualified trust
   receivables. After revolving, freeze the principal numerator once at
   the authorized cut and use the then-applicable trust denominator
   for each calculation. Apply a denominator floor only under the
   governing terms. Finance-charge and gross-default shares continue
   to use the live investor reference. Attribute the checked residual
   to the seller and route dated recoveries separately. Investor
   default attribution feeds the waterfall under the applicable note
   loss rules.
   [SEC-filed allocation terms](https://www.sec.gov/Archives/edgar/data/72333/000095012309063728/v54115exv4w1.htm).

3. **Reconcile funding, purchases and opening state.** Keep trust
   receivables, investor and seller interests, note par and funding
   cash as separate balances. Require explicit opening seller interest,
   with qualified zero allowed. Recognize authorized funded purchases
   or additions and require sufficient actual replenishment for a
   level trust balance assumption. A seasoned run uses the restored
   phase, frozen principal numerator, funded balances and trigger
   history at the valuation cut.

4. **Distinguish revolving, controlled accumulation and amortization.**
   During revolving recycle eligible principal into authorized
   purchases. For controlled accumulation deposit eligible investor
   principal up to the contractual cap and route excess under the
   deal terms. Release held funds as principal once on the bullet or
   effective acceleration. Accelerated amortization stops further
   recycling and accumulation. A deposit and a note principal payment
   are separate dated events.
   [SEC-filed accumulation example](https://www.sec.gov/Archives/edgar/data/1332961/000095015208003083/l31255afwp.htm).

5. **Evaluate early amortization against dated history.** For the
   three-month excess-spread profile use the mean of three consecutive
   completed monthly observations on the qualified cash, net credit
   cost, balance and annualization basis. A value strictly below the
   floor triggers. A selected cumulative-loss threshold triggers at
   or above its level on the authorized fixed original investor base.
   Require sourced prior observations when due. Apply the trigger at
   its contractual phase before eligible reinvestment and retain the
   triggered state.
   [SEC-filed monthly test](https://www.sec.gov/Archives/edgar/data/1139552/000119312516653679/d34377d424b5.htm),
   [Accepted principal regimes](13-principal-regimes-and-amortization.md#accepted-decision-for-principal-regimes-and-early-amortization).

6. **Report distinct metrics with explicit units.** Static excess
   spread is an annual proxy with matched investor finance income,
   qualified note coupon cost, servicing and modeled gross credit
   cost, separate from the realized trigger series.
   `abs_payment_rate` reports configured monthly principal payment
   percentage. Report the annual gross charge-off assumption
   separately from cumulative gross charge-offs as a percentage of
   fixed original investor par. Report dated delinquent par over
   qualified current par. Missing or zero denominators make ratios
   unavailable. Align metric metadata and host documentation with
   percentage-point outputs.

7. **Use a statistical monthly expected-flow card profile.** Exclude
   ordinary installment-loan coupons and amortization and generic
   prepayment or default overrides. Require separate qualification
   for stochastic payment, purchase, default or credit-dependent
   OAS behavior. Keep the same Rust term and state validation and
   cash accounting in the existing Python and WASM interfaces.

## Independent calculations

- Frozen principal reference 600, trust par 800 and live investor
  reference 480 give principal allocation 75% and finance and default
  allocations 60%. Gross principal/finance/default of 120/20/16
  give investor amounts 90/12/9.60 and seller amounts 30/8/6.40.
  Without replenishment receivable par closes at 380.40 and 283.60.
- Opening trust par 1,000, modeled monthly gross loss 2%, monthly
  principal payment 10% and annual simple yield 18% on monthly
  fraction 1/12 give gross default 20, principal 98 and finance
  charges 15. Reinvesting 98 of principal leaves 980 for the next
  month and principal collections of 96.04 in that month. Two
  nodes produce 194.04 versus 182.476 in a block with no interim
  replenishment.
- Annualized dated excess-spread observations of +12%, 0% and
  −12% average to zero and tie a zero floor. Gross charge-offs of 6
  remain 6% of fixed original investor par 100 after current par
  declines to 40.

## Observed implementation and scope

Card portfolios use portfolio-level yield, charge-off and payment
rates. The investor and seller allocation changes at revolving end.
The following sections describe observed behavior. The accepted
contract above governs reconciliation.

## Algorithm, domain and conventions

Finance charges use annual decimal portfolio yield. Annual charge-off rate is converted to the applicable monthly/period default fraction, while the monthly payment rate is a separate fraction of eligible receivables. During revolving, investor share follows current investor balance relative to the receivables base. When amortization begins, the principal-allocation share is frozen so principal collection allocation does not shrink automatically with investor paydown. Seller cash and investor available collections remain separately accounted for in the pool-flow calculation.

Card-specific inputs validate finite ranges and pool eligibility; instrument-backed collateral is rejected. Revolving end, loss/excess-spread events and controlled accumulation alter downstream reinvestment and principal allocation. They do not change annual charge-off into a monthly payment input.

The registered excess-spread metric is portfolio yield minus weighted note coupon divided by pool balance, minus servicing rate, minus annual charge-off rate, expressed as percent. Floating note coupons use current market projection. The payment-rate metric reports monthly payment fraction ×100. The charge-off metric instead reports cumulative defaults/current pool balance ×100; it is not the same annual input rate. Delinquency is opening delinquent bucket balance/current balance ×100.

## Limitations and alternatives

There is no account-level limit migration, purchase generation or stochastic payment-rate calibration in this branch. Annualized excess-spread metrics and the realized trailing-period trigger are distinct quantities. Ordinary statistical installment-loan terms are a supported separate mode, not an equivalent alternative for card economics. A richer account-cohort master trust would require calibrated purchase/payment/default dependencies.

## Implementation and validation evidence

- [structured_credit/types/card.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/card.rs)
- [structured_credit/pricing/simulation_engine/pool_flows.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/pool_flows.rs)
- [structured_credit/pricing/simulation_engine/simulate_period/cash.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/simulate_period/cash.rs)
- [structured_credit/metrics/deal_specific/abs.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/deal_specific/abs.rs)

Existing evidence: [structured_credit/unit/card_master_trust_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/card_master_trust_tests.rs); [structured_credit/unit/metrics/deal_specific_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/metrics/deal_specific_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

Independent master-trust cashflow references and correlated payment/default behavior remain unvalidated.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
