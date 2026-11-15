# VALUATIONS-STRUCTURED-CREDIT-11: Waterfall claims, fee priority, PIK and integer rounding

- **Decision status:** Accepted 2026-11-15 — complete waterfall priority, dated claims, shortfall and PIK, exact capped allocation, settlement and incentive package.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent claim, monetary quota, and dated hurdle calculations checked. Production pricing, transaction terms and host qualification remain pending.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Accepted decision for waterfall claims and rounding

1. **Elect a waterfall explicitly.** Use a versioned standard template or
   complete custom tiers. The standard profile orders leading fees, ranked
   debt interest, junior fees and reserve replenishment, sequential
   principal, elected carryover and incentive claims, then the named
   residual. Require valid recipients, unique priorities and explicit
   funding sources. Principal support and nondeferrable interest require
   contractual authority. Reject duplicate or conflicting claims and
   fee sources.
   [Filed CLO priority of payments](https://www.sec.gov/Archives/edgar/data/1379785/000137978519000025/exhbiti101-cloindenture.htm).

2. **Prepare one dated claim ledger.** Reuse the same claims for payment,
   reserve support, coverage at the applicable cut and reporting.
   Fixed charges and annual fees require their own dates, currency,
   base and accrual or installment convention. Invalid fees error.
   Calculate note coupon on the contractually effective balance
   using the note's day count. A writedown after coupon entitlement
   leaves the earned claim intact. Quote accrued has its own basis.

3. **Separate unpaid coupon, PIK and cap carryover.** Record the
   contractual shortfall disposition for every claim. Unpaid
   nondeferrable interest remains a claim under its terms. Cash
   deferral retains arrears. PIK creates a zero-cash principal
   change at its effective phase. An elected net-WAC cap uses
   qualified performing all-in WAC less the authorized fee load,
   floored at zero. A separate non-interest-bearing carryover
   for cap-withheld coupon requires election and its own eligible tier.
   [Filed carryover example](https://www.sec.gov/Archives/edgar/data/1365771/000088237706002353/d532401.htm).

4. **Use an explicit payment quantum and exact capped quotas.** Any
   claim rounding requires an elected scale and mode. Floor available
   funding and authorized caps to executable units. Preserve sub-unit
   balances and all hard caps. For capped pro rata, solve weighted
   final quotas with caps applied, floor the quotas once, then assign
   remaining units by largest fractional remainder. Break ties by
   declared recipient order. Require explicit equal weights or positive
   participating weights. An expressly excluded zero weight stays
   excluded.
   [Accepted Money rounding](../core-conventions/012-money-decimal-precision.md).

5. **Track every claim through its disposition.** Use the same cash
   and cap rules in sequential and pro-rata tiers. Track principal
   already paid so later tiers cannot retire the same par again.
   Record due, paid and exact shortfall for every claim, including
   after funding is exhausted. Reconcile funding, debits and credits
   through the account ledger. Expose the same Rust validation and
   results in the existing Python and WASM interfaces.
   [Accepted account conservation](10-reinvestment-reserves-and-hedges.md#accepted-decision-for-reinvestment-reserves-and-hedge-cash).

6. **Calculate incentive fees directly at the contractual hurdle.**
   Carry authorized dated equity contributions and eligible distributions
   to the current payment date at the stated ACT/365F hurdle. Require
   complete history and a conventional contribution-then-distribution
   pattern. Count earlier co-dated equity cash once. Pay equity the
   first executable amount that clears any deficit, limited by remaining
   eligible cash, and apply the manager's share to the remainder.
   Use the same rule at ordinary and terminal distributions.
   Missing history, unsupported sign patterns and numerical failures
   error.
   [Filed CLO incentive terms](https://www.sec.gov/Archives/edgar/data/1930087/000110465926005334/tm263361d1_ex10-1.htm),
   [Accepted dated yield rules](../core-conventions/017-irr.md).

## Independent calculations

- Five payment units with weights 1/2/4 and caps 1/1/4 have exact
  quotas 0.8/1/3.2 and executable amounts 1/1/3 under the accepted
  rule. Rounding while redistributing gives 0/1/4. At a 0.01
  quantum, 0.006 can fund no whole unit and remains in its account.
- Face 1,000, quarter accrual and coupon 8% give uncapped interest
  20. A 5% net-WAC cap gives current due 12.50 and withheld
  carryover 7.50. With opening arrears 3, cash 10 and arrears-first
  priority, pay 3 arrears and 7 current. The elected PIK is 5.50,
  separate from 7.50 cap carryover.
- Equity investment 100, receipt 30 after 365 days and an annual
  hurdle of 10% leave a deficit of 88 after 730 days
  because `100*1.10^2 - 30*1.10 = 88`. Of residual cash
  125, a 20% manager share above the hurdle produces fee 7.40.

## Observed implementation and scope

The waterfall computes claims and distributes finite interest and
principal resources by priority. The following sections describe the
observed paths. The accepted contract above governs reconciliation.

## Algorithm and conventions

The standard template pays leading fees, debt interest in seniority/input order, junior fees/reserve replenishment, sequential principal, optional net-WAC carryover and incentive fees, then residual. Principal defaults to principal funding; other tiers default to interest. CLO/CBO defaults permit principal support for leading fees and nondeferrable note interest. Senior notes are nondeferrable by default; callers can override.

Claims include fixed cash, annualized percentage of collateral/special-serviced balance, tranche coupon plus deferred interest, principal to target balance, reserve top-up, net-WAC carryover and incentive fee. Fee annualization defaults to Act/360. Standard trustee fees are annual amounts divided by payment frequency; nonpositive/nonfinite standard bp fees are silently omitted. The payment coupon claim uses the balance after the period's realized-loss allocation, before waterfall principal payment, and the note's day count. Separate accrued-interest metadata preserves the opening pre-writedown balance. Interest shortfall can defer or capitalize as PIK; net-WAC cap uses live collateral WAC less selected fees and tracks cap carryover separately.

Sequential tiers track principal already paid so repeated claims cannot pay the same balance twice. Pro-rata tiers convert available cash and caps to integer currency units, water-fill weighted recipients up to caps, then distribute remainders deterministically. If all weights are zero the allocator uses equal weights. Nearest/floor/ceiling claim rounding are supported; recorded shortfall uses a small numerical threshold.

Incentive fees test the equity XIRR against a hurdle. Hurdle shortfall is found by up to 64 bisections with 0.005 currency-unit width; failed XIRR candidates count as not reaching the hurdle, and a failed final money conversion returns the original candidate. Remaining eligible cash above the shortfall is shared with the manager.

## Failures, alternatives and limitations

Custom waterfalls reject duplicate priorities, invalid references, duplicate interest owners, inconsistent coverage tiers and simultaneous deal-level fees. Equity must use the residual recipient rather than a debt tranche ID. Sequential/pro-rata and funding sources are explicit supported alternatives. Exact fractional allocation without currency rounding would differ economically at small notionals. Nonconventional equity cashflows can break monotonic IRR assumptions; standard-fee omission and equal-zero-weight allocation are observed fallbacks, not agreed permissions.

## Implementation and validation evidence

- [structured_credit/pricing/waterfall.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/waterfall.rs)
- [structured_credit/types/waterfall/mod.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/waterfall/mod.rs)
- [structured_credit/types/waterfall/builder.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/waterfall/builder.rs)
- [structured_credit/types/structured_credit_impl.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/structured_credit_impl.rs)
- [structured_credit/types/setup.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/setup.rs)
- [structured_credit/pricing/simulation_engine/simulate_period/period_waterfall.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/simulate_period/period_waterfall.rs)
- [structured_credit/utils/validation.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/utils/validation.rs)

Existing evidence: [structured_credit/waterfall_golden.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/waterfall_golden.rs); [structured_credit/waterfall_properties.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/waterfall_properties.rs); [structured_credit/unit/components/rounding_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/components/rounding_tests.rs); [structured_credit/unit/coupon_claim_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/coupon_claim_tests.rs); [structured_credit/unit/fee_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/fee_tests.rs); [structured_credit/unit/afc_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/afc_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

Hand-built golden waterfalls are not external legal-document validation.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
