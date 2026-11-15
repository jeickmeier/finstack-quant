# VALUATIONS-STRUCTURED-CREDIT-09: Collateral call and put exercise assumptions

- **Decision status:** Accepted 2026-11-15 — complete collateral exercise, payoff, workout yield and incentive scenario package.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent exercise chronology, payoff, yield inversion and incentive-boundary calculations checked. Production pricing, contract, risk and host qualification remain pending.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Accepted decision for collateral call and put exercise

1. **Select the policy per side and honor binding elections.** Explicitly
   select no new exercise, first eligible opportunity or an admitted quote
   or incentive rule. Validate overrides against unique collateral
   identities. Honor binding prior notices and elections under every
   policy. Resolve actual rights, notices and decision dates from the
   bond or term-loan owner. Keep notice, exercise, economic retirement,
   receipt and deal distribution dates distinct.
   [Filed notice and redemption terms](https://www.sec.gov/Archives/edgar/data/1219601/000119312513010869/d465899dex42.htm).

2. **Resolve competing rights in contractual chronology.** Evaluate at
   exact authorized dates and phases. A later call cannot displace a
   binding earlier put merely because both receipts belong to one
   collection period. Use qualified contractual priority for simultaneous
   opportunities. Apply holder-put priority where the accepted bond
   profile qualifies and reject unresolved priority or incompatible
   rights.
   [Accepted bond exercise rules](../valuations-bonds/04-rates-tree-exercise.md).

3. **Prepare actual payoff cash and preserve surviving rights.** Apply
   the strike to eligible outstanding par after contractually prior
   events. A clean percent-of-par strike pays separately eligible
   exercise accrual. A dirty or make-whole strike follows its own
   admitted terms. Include independently vested cash once. Cancel only
   rights and obligations extinguished by exercise. Keep actual redemption
   cash, retired par and receipt date distinct. Classify below-par
   exercise as a redemption discount, separate from gross credit default.
   Coordinate prepayment and default with the same dated ledger.
   [Accepted instrument collateral ledger](08-instrument-collateral.md#accepted-decision-for-instrument-collateral-and-liquidity).

4. **Reuse the bond and term-loan workout yield laws.** Use one
   authorized dirty buyer purchase amount, actual buyer settlement and
   the instrument's selected yield law. Include scheduled maturity
   when reachable and every still-reachable qualified full issuer or
   borrower call. Prospective holder puts stay outside the minimum.
   Preserve actual dates of entitled receipts and funding obligations.
   An unresolved or failed required inversion makes full YTW unavailable.
   Compare finite yields before display rounding and report every exact
   tie. Require explicit selection among tied exits before projecting
   cashflows. A maturity minimum leaves the no-new-call schedule in force.
   [Bond workout yield](../valuations-bonds/11-workout-yield.md),
   [Term-loan workout yield](../valuations-other-fixed-income/09-term-loan-risk-yields.md).

5. **Use explicit eligible rate comparisons for incentive scenarios.**
   At the selected decision a call requires continuation coupon minus
   qualified refinancing rate to exceed its stated threshold. A put
   requires qualified reinvestment rate minus continuation coupon to
   exceed its threshold. At equality continue. Use the effective
   contractual step-up coupon in the tested accrual slice. A step-up
   alone creates no exercise instruction. Bind the comparison rate to
   a named qualified source, dates and clock. A path comparison requires
   a matched conditional rate at the decision. Missing or unsupported
   active inputs error.
   [Filed step-up and call terms](https://www.sec.gov/Archives/edgar/data/19617/000089109213003757/e53390_424b2.pdf).

6. **Publish the resolved policy and dated payoff.** Preserve the policy,
   right, decision and phase, active rate source or quote and resulting
   cash. This layer supplies rule-based scenario cashflows to the
   downstream waterfall. Use the same Rust checks, dependencies and
   results in the existing Python and WASM interfaces.

## Independent calculations

- Opening par 100 less prior payment 20 leaves 80. A June 15
  put at 101% pays 80.80 of redemption, plus 1.20 exercise
  accrual and 3 independently vested coupon, for total 85.00 and
  retired par 80. A June 20 call at 103% would pay 86.60 on the
  same cash assumptions. Both lie in one collection period.
  A 95% exit has redemption cash 76 before accrued and retired
  par 80, with discount 4.
- A call paying 108 after 365 days and a maturity path paying
  5 after 365 days and 109 after 730 days yield 8.0000% and
  6.9330% when clean purchase 100 is used directly. On the common
  dirty purchase 105 the respective yields are 2.8571% and
  4.2957%, selecting the call. The annual cash vector
  `[-100,230,-132]` has two XIRRs, 10% and 20%, and cannot
  enter a complete minimum under the admitted term-loan law.
- A one-year simple comparison with factor 20/21 gives 5%.
  Against initial coupon 4% and applicable stepped coupon 7%,
  the incentive moves from −100 bp to +200 bp. The stepped
  coupon triggers a 100 bp threshold. Under exact admitted
  rates, 6.5% less 6% ties a 50 bp threshold.

## Observed implementation and scope

The observed implementation selects early exercise by policy and
processes calls before puts. The following sections describe the audited
paths. The accepted contract above governs reconciliation.

## Algorithm and conventions

Contractual-only preserves the no-early-exercise schedule. First-opportunity uses the first matching window in a deal period. Yield-to-worst builds candidate redemption schedules, computes XIRR and chooses the minimum successfully solved yield. Failed candidate solves are skipped. Refinancing compares a coupon with a simple forward rate over an Act/365F horizon plus the configured threshold. The current coupon helper takes the initial step-up rate and the policy enum has no separate step-up trigger. Selected exercise changes projected collateral termination cash and the period to which it is assigned.

Prices expressed as percent of par are converted to cash on outstanding balance. Candidate dates, call/put schedules and override identifiers come from collateral terms. The exercise pass runs before cashflow bucketing and subsequent credit/prepayment overlays; it does not revise a policy in response to every downstream coverage test.

## Selection, failure and limitations

No-exercise and policy overrides are supported controls. Missing/unresolvable required market inputs error where fetched; yield candidate failure is a distinct silent exclusion. The minimum-yield procedure need not identify the lowest investor value under arbitrary nonconventional cashflows, and call-before-put ordering can resolve competing opportunities asymmetrically. Threshold refinancing is not a calibrated borrower option model.

## Alternatives and tradeoffs

The explicit deterministic policies are supported alternatives. Lattice/PDE or least-squares continuation-value optimization requires a separate qualified option model. The accepted profile selects documented scenario rules.

## Implementation and validation evidence

- [structured_credit/types/collateral.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/collateral.rs)
- [structured_credit/pricing/simulation_engine/exercise.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/exercise.rs)
- [structured_credit/pricing/simulation_engine/instrument_flows.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/instrument_flows.rs)

Existing evidence: [structured_credit/unit/instrument_collateral_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/instrument_collateral_tests.rs); [structured_credit/unit/instrument_pool_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/instrument_pool_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

Reference context: [fixed-income option context](../../../REFERENCES.md#fabozzi-fixed-income-handbook). These references identify the method family; they do not certify the preset parameters or every implementation branch.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
