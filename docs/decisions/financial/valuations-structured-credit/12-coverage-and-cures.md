# VALUATIONS-STRUCTURED-CREDIT-12: Coverage collateral values, ratios and cure diversion

- **Decision status:** Accepted 2026-11-15 — complete dated coverage, collateral and claim calculations, breach state and cure allocation package.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent OC, IC, borrowing-base, hysteresis and executable cure calculations checked. Production pricing, transaction terms and host qualification remain pending.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Accepted decision for coverage tests and cure diversion

1. **Bind assessment and action separately.** Each named test specifies
   tested debt, its dated determination cut and its waterfall action tier.
   Keep pre-purchase tests distinct from later actual or pro forma tests.
   Settle from cash, balances and claims remaining after prior actions
   at the selected tier. Report assessment, action and effect separately.
   [Accepted waterfall claims](11-waterfall-claims-and-rounding.md#accepted-decision-for-waterfall-claims-and-rounding).

2. **Build OC from expressly eligible assets and accounts.** Include
   adjusted current eligible par, qualified principal cash and restricted
   accounts, and pending default claims under an elected valuation law.
   On recovery receipt remove the corresponding pending carrying value
   and admit eligible cash once. Divide by the named tested and senior
   debt on the same basis. Borrowing-base tests use the facility's
   eligible concentration-adjusted advance value and its covered
   exposure.
   [Accepted borrowing-base facility](19-borrowing-base-facility.md#accepted-decision-for-borrowing-base-facilities-and-lender-cashflows).

3. **Apply overlapping collateral adjustments once.** In the selected
   profile carry each slice at the lowest applicable rating,
   discount-obligation or excess-CCC value. Size CCC excess on the
   declared par basis and apply it starting from the lowest qualified
   current prices, with stable ties and a fractional final slice.
   Require explicit thresholds and price and rating sources. Distinguish
   missing from `NR`. Required missing data makes a test unavailable
   or errors unless a conservative treatment is expressly authorized.
   [Filed CLO overlap and CCC terms](https://www.sec.gov/Archives/edgar/data/1379785/000137978519000025/exhbiti101-cloindenture.htm).

4. **Use the prepared ledger for IC.** The net-fee profile divides
   eligible period interest less designated higher-ranking expenses
   by current coupon and expressly eligible carried claims for the
   tested and senior classes. Bind whether each deduction is due
   or paid, and use the accepted dated claim ledger with qualified
   floating coupons. Spendable interest at the diversion tier is
   separate. A known zero denominator is inactive with no numeric
   ratio. Missing required inputs error.
   [Filed IC definition](https://www.sec.gov/Archives/edgar/data/1930087/000110465926005334/tm263361d1_ex10-1.htm).

5. **Persist test state and qualified consequences.** Compare unrounded
   results by test ID. Breach below entry and clear at or above cure,
   with the cure threshold at least the entry threshold. Restore
   authorized opening state, retain breach dates and elect the effect
   of inactivity on a stored breach. Specify authorized diversion,
   trapping, reinvestment restriction or latched acceleration.
   [Accepted principal regimes](13-principal-regimes-and-amortization.md#accepted-decision-for-principal-regimes-and-early-amortization).

6. **Size and settle an actual authorized cure.** Use designated
   funding, payment rank and allocation, remaining par and the
   executable cash quantum. Recompute both sides of the ratio
   affected by the action. Use the largest required cure at a shared
   tier only for tests sharing one proven monotone action path.
   Incompatible actions need distinct authorized order. Apply actual
   caps and authorized partial cures, and report the amount diverted
   and any remaining breach.
   [Filed CLO cure sequence](https://www.sec.gov/Archives/edgar/data/1379785/000137978519000025/exhbiti101-cloindenture.htm).

7. **Keep current results and prospective improvement distinct.**
   Principal paydown leaves earned coupon and arrears intact. A failed
   current IC can authorize deleveraging. A prospective IC cure needs
   its own qualified pro forma claims and cut. A reinvestment diversion
   becomes tagged principal cash. It adds to OC only when the account
   or subsequent purchase qualifies at the relevant cut. Expose the
   same checked Rust ratios, actions and outcomes in the existing
   Python and WASM interfaces.

## Independent calculations

- With performing par 1,000 and 7.5% CCC allowance, CCC par 250
  creates excess 175. Regular par 750 qualifies at par. The two CCC
  positions are 100 and 150, each with a 5% haircut. The first
  was bought at 60% and is marked at 50%. The second was bought
  at par and is marked at 90%. Assign the excess to the first 100
  and 75 of the second. The combined values are 50 and 138.75.
  With principal cash 20, qualified restricted cash 10 and pending
  claim carrying value 15, the numerator is 983.75 and OC on tested
  debt 850 is about 1.15735294 times. Receipt 20 replaces the
  corresponding pending claim and raises the numerator to 988.75.
- At numerator 105, debt 100 and target 1.20, paydown from outside
  the numerator needs 12.50. Paying with principal counted in the
  numerator at par needs 75. Retaining 15 as newly eligible value
  meets the same target without paydown. Two cures of 10 and 15 on
  a common monotone path need one diversion of 15.
- Gross eligible interest 22 less designated fees 3 gives 19.
  Senior coupon 10, tested coupon 7 and eligible arrears 3 give
  a denominator of 20 and IC of 0.95. Paying senior coupon leaves
  9 at the action tier. Later principal paydown leaves that earned
  IC unchanged. Entry 1.20 and cure 1.25 applied to
  1.19, 1.20, 1.24, 1.25 and 1.20 give breach, breach,
  breach, cure and clear.

## Observed implementation and scope

The observed engine uses a period snapshot and diverts eligible interest
at its waterfall position. The following sections describe audited
behavior. The accepted contract above governs reconciliation.

## Algorithm and conventions

OC numerator combines performing collateral adjusted for rating/discount/CCC rules, principal cash, restricted qualifying accounts and modeled value of pending default claims. Default claims carry recovery value by default or configured market-value percent of defaulted par. The CLO helper uses a 7.5% CCC threshold and an 80% discount-obligation price threshold; it is not automatically a universally applicable covenant. Additional CCC excess reduction is applied beyond ordinary row adjustments. Unrated instrument/new-cohort handling differs from rated rows; eligibility is not established by those differences.

OC divides by the tested note plus all notes senior in payment priority. IC uses interest collections less senior fees divided by current coupon/deferred claims of tested and senior classes. The no-market helper can use floating spread alone; the simulation has market-aware claims. Zero denominator produces an absent/degenerate result rather than an ordinary finite ratio.

A failed test sizes the principal paydown needed to cure, traversing actual principal recipients. Tests sharing a tier divert once based on the maximum required cure, not the sum. Diversion is limited by available interest and optional percent cap. `PayDownSenior` immediately pays senior principal; `Reinvest` transfers interest to the principal account for permitted use. Placement defaults after the tested tranche's interest, with explicit alternatives after another tranche or junior fees.

Legacy tranche triggers retain breach state and cure hysteresis: once breached they remain so until `cure_level` (or the trigger level) is reached. Consequences can divert, trap, accelerate or stop reinvestment. Explicit test tiers must follow relevant interest and share action/diversion cap within a tier.

## Limitations and alternatives

Ratios are multiples (1.2 = 120%); haircut fractions differ from pct thresholds. Coverage math is contractual and cannot be approved solely by comparing it with a generic CLO example. Market-value and recovery valuation, test placement/action and custom thresholds are supported alternatives. The expected collateral valuation and forward coupon estimates do not establish stress calibration; snapshot ratios do not represent intraperiod breaches.

## Implementation and validation evidence

- [structured_credit/pricing/coverage.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/coverage.rs)
- [structured_credit/pricing/waterfall.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/waterfall.rs)
- [structured_credit/pricing/simulation_engine/triggers.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/triggers.rs)
- [structured_credit/pricing/simulation_engine/simulate_period/losses.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/simulate_period/losses.rs)
- [structured_credit/types/waterfall/coverage.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/waterfall/coverage.rs)
- [structured_credit/types/tranches.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/tranches.rs)

Existing evidence: [structured_credit/unit/coverage_rules_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/coverage_rules_tests.rs); [structured_credit/unit/coverage_position_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/coverage_position_tests.rs); [structured_credit/unit/trigger_consistency_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/trigger_consistency_tests.rs); [structured_credit/unit/components/coverage_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/components/coverage_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
