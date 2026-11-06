# VALUATIONS-STRUCTURED-CREDIT-06: CMBS balloon extension, penalties and special servicing

- **Decision status:** Draft — observed implementation; adoption pending agreement.
- **Implementation status:** Observed in the owning paths below; scope: statistical commercial-mortgage collateral and DSCR metrics.
- **Validation status:** Evidence inspected/located, not executed. Source and selected assertions establish implemented behavior, not general numerical accuracy or market approval.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Choice and rationale

The engine applies explicit mortgage-servicing scenario terms inside the statistical collateral recursion. Maturity resolution is split by probabilities, while penalties and servicing costs directly change available collections. These are contract/scenario choices requiring deal-specific agreement.

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

Adoption questions, implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
