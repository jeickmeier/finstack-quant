# VALUATIONS-STRUCTURED-CREDIT-06: CMBS balloon extension, penalties and special servicing

- **Decision status:** Accepted 2026-11-15 — complete CMBS balloon, workout, servicing, prepayment protection, DSCR and dispatch package.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent balloon, fee, dated premium and full-population DSCR calculations checked. Production pricing, reporting and host qualification remain pending.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Accepted decision for CMBS balloon and servicing

1. **Split remaining performing balloon par once at contractual maturity.**
   Require explicit extension and workout fractions in `[0,1]` whose
   sum is at most one. Pay the residual as scheduled principal and keep
   extended par in a separate cohort. Require its maturity, retained or
   revised coupon terms and a scaled or recast installment under the
   applicable terms. Preserve the floating index unless explicitly
   replaced. Ordinary contractual payoff applies when no split is elected.

2. **Date workout recovery from the maturity event.** Retire gross
   workout par once and apply explicit severity with either a qualified
   receipt date or a checked calendar-month lag from contractual maturity.
   Zero lag means same-day eligibility under the stated phase. Require
   authorized treatment of remaining delinquency, carrying costs, advances
   and cash after legal final. When the maturity event is historical,
   carry its actual resolved balances and claims forward.
   [Accepted NPL workout](05-npl-resolution.md#accepted-decision-for-npl-resolution-and-reperformance),
   [Terminal cash](14-loss-recognition-and-terminal-cash.md#accepted-decision-for-loss-recognition-and-terminal-cash).

3. **Apply appraisal reductions and servicing fees to their own bases.**
   Apply the appraisal reduction to eligible interest advances in this
   profile. Borrower contractual dues and gross par are separate.
   Accrue recurring special-servicing fees on qualified opening serviced
   par using the fee day count. Newly serviced balances enter at the
   next qualified accrual slice and servicing status applies only to
   the affected cohort. Apply workout fees to eligible workout receipts
   and liquidation fees to eligible recovery receipts. Require explicit
   accounts, priority and offsets so each cost or reimbursement enters
   once.
   [SEC-filed CMBS servicing agreement](https://www.sec.gov/Archives/edgar/data/2148771/000153949726002320/exh4_2-5yr23psa.htm).

4. **Apply prepayment protection at the contractual voluntary event.**
   Support an explicit unprotected profile, lockout, fixed premium,
   step-down or yield maintenance. A lockout applies through its end
   date and blocks voluntary prepayment and the related installment
   reduction. Fixed and step-down premiums apply to actual prepaid par.
   Step dates strictly increase, percentages are non-increasing, and
   the first step covering the event applies. Record the premium
   as separate fee cash on actual settlement and route under the
   waterfall terms.

5. **Keep two explicit yield-maintenance profiles.** The undiscounted
   approximation multiplies prepaid par by the positive coupon less
   comparison-yield spread and the contractual remaining year fraction.
   The discounted profile weights the same lost coupon by actual dated
   accrual fractions and relative discount factors, including any genuine
   stub. Apply any contractual minimum to prepaid par. Require explicit
   protection horizon, day count, comparison-yield authority and the
   discount source where applicable. Query market inputs only when an
   eligible calculation consumes them.
   [Fannie Mae prepayment premium example](https://multifamily.fanniemae.com/media/document/pdf/43500).

6. **Define operating DSCR on the eligible performing population.**
   Divide qualified annual property NOI by annualized regular
   contractual borrower principal and interest for the same positive-
   par performing loans. Require comparable dated NOI and explicit
   allocation for shared properties. Use each loan's own regular
   installment and frequency or a qualified annual-service amount for
   irregular terms. Include only interest for interest-only loans and
   scheduled principal for zero-coupon amortizers. Resolve active
   floating coupons under record 03. Exclude balloons, premiums,
   servicing fees, advances and recoveries. Incomplete required data
   makes full-pool DSCR unavailable and requires coverage disclosure.
   Require positive aggregate debt service.
   [Accepted statistical collateral projection](03-statistical-collateral-projection.md#accepted-decision-for-statistical-collateral-projection),
   [FHFA underwriting guidance](https://www.fhfa.gov/sites/default/files/2023-05/Multifamily%20Mortgage%20Underwriting%20and%20Acquisitions%20-%20Final%20Version%201%200.pdf).

7. **Use the expected balloon split for deterministic pricing.** Run
   one waterfall on the expected flows. Nonlinear tranche values can
   differ from averages over discrete payoff, extension and workout
   paths. An unresolved modeled split requires separate qualification
   for stochastic pricing or stochastic OAS. Use the same Rust terms,
   dates, cash classification and metric availability through the
   existing Python and WASM routes.

## Independent calculations

- Remaining maturity par 1,000, extension fraction 30% and workout
  fraction 20% give 500 payoff, 300 extension and 200 gross workout.
  At 40% severity and a 2% fee on recovery, gross recovery is 120,
  the fee is 2.40 and net receipt is 117.60. An opening serviced
  balance of 200 at 25 bp for a quarter accrues 0.125 even if it pays
  off at period end. Applying 40% appraisal reduction to an eligible
  interest advance of 10 leaves 6.
- Prepaid par 1,000, coupon 8%, comparison yield 5%, contractual
  accruals 1, 1 and 0.25 and relative factors 0.95, 0.92 and 0.89 give
  67.50 undiscounted and 62.775 discounted. A 7% floor yields 70
  on either route.
- A 120-par zero-coupon amortizer with 2 monthly principal and 30
  annual NOI, together with a 200-par 6% interest-only loan with 18
  annual NOI, gives 48/36 = 1.333333 times. Missing NOI for the
  interest-only loan leaves full-pool DSCR unavailable. The subset
  would show 1.25 times on 37.5% of outstanding par.

## Observed implementation and scope

The engine applies mortgage-servicing scenario terms inside the
statistical collateral recursion. Maturity resolution is split by
probabilities, while penalties and servicing costs affect collections.
The following sections describe observed behavior. The accepted
contract above governs reconciliation.

## Algorithm and conventions

At maturity a balloon divides into payoff, extension and workout fractions. Extended balance receives a later maturity and optional replacement fixed rate; its amortization schedule is scaled to the extended fraction. Workout/default recoveries move to a lagged queue. Appraisal reduction reduces available interest; special-servicing fees accrue on the specially serviced balance, with newly defaulted collateral joining that base in the next period. Workout and liquidation fees reduce corresponding collections/recoveries.

Prepayment penalties support lockout, fixed/stepped percentages, and yield-maintenance terms. The simple yield-maintenance branch approximates lost coupon less reinvestment over remaining horizon. A curve-based branch discounts annual coupon differences plus a stub and applies the contractual premium floor. It is not a generic market-priced defeasance model. A loan-specific lockout suppresses deal-level CPR for that asset.

DSCR sums annual NOI and annual debt service only for performing loans having NOI. Debt service uses explicit contractual payment when available, interest-only balance × coupon for IO/nonamortizing loans, or the level-payment annuity with amortization term. Floating debt service projects live forward index with floor plus spread, using weekends-only reset lag. Missing all performing NOI or nonpositive debt service errors; partially missing NOI silently restricts the sample.

## Controls and limitations

Probabilities and fee shares are fractions; penalty/price fields explicitly marked pct are percentages. NOI and carrying cash use pool currency. IO length cannot exceed contractual term, amortization term must cover maturity, and rate/probability inputs validate. Expected fractional extension/workout may understate nonlinear downside compared with discrete loan events. The DSCR sample-selection policy and simple penalty branch require explicit intended uses.

## Alternatives and tradeoffs

Contractual payoff without extension is supported by omitting balloon terms. Curve-discounted versus simple yield-maintenance branches trade market sensitivity for required inputs. Independent stochastic borrower refinancing and market-property valuations are possible future alternatives, not calculations done here.

## Implementation and validation evidence

- [structured_credit/types/cmbs.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/cmbs.rs)
- [structured_credit/pricing/simulation_engine/pool_flows.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/pool_flows.rs)
- [structured_credit/metrics/deal_specific/cmbs.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/deal_specific/cmbs.rs)

Existing evidence: [structured_credit/unit/cmbs_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/cmbs_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

Reference context: [fixed-income context](../../../REFERENCES.md#fabozzi-fixed-income-handbook). These references identify the method family; they do not certify the preset parameters or every implementation branch.

Existing tests locate 30%/24-month extension, fixed-rate replacement, lost-coupon premiums, curve-discounting, fee timing and floors; external deal/vendor accuracy evidence remains open.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
