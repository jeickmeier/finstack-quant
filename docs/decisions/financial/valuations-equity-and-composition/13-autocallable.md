# VALUATIONS-EQUITY-AND-COMPOSITION-13: Autocallable seasoning, coupon memory and redemption

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-13`.
- **Decision status:** Draft — observed implementation; adoption pending.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths listed below; shared kernels are identified explicitly.
- **Validation status:** Source and linked evidence inspected or located; no numerical tests executed in this audit. Located tests establish the stated local behavior only, not independent accuracy across the domain.

## Choice and rationale

The adapter replays past discrete observations before simulating the remaining note. Coupon barriers are independent from autocall barriers, and payment-date DF ratios preserve coupon/redemption settlement lags.

## Scope and input domain

Nonempty increasing observations, matching barrier/coupon/payment arrays, nondecreasing payments between each observation and expiry. Initial levels are positive when supplied; any past or same-day observation requires an explicit initial level and exact fixing. Barrier ratios are nonnegative; coupons are finite signed return amounts.

## Algorithm

1. After expiry return zero. Replay all observations ≤ as-of. Update the discrete minimum; when the coupon barrier is met, release that coupon plus accumulated memory, retaining only payments after as-of. Otherwise accrue the coupon only when memory is enabled.
2. At first satisfied autocall barrier, return zero if paid, otherwise discount principal and all fixed unpaid coupons to their actual payment dates.
3. If every observation is past without autocall, compute final redemption from the last fixing and past observed minimum, payable at expiry. CapitalProtection is `max(floor, participation×min(Sfinal/S0,cap))`; Participation is `1+participation×max(min(Sfinal/S0,cap)−1,0)`; KnockInPut pays `max(1−max(strike_ratio−Sfinal/S0,0),0)` when observed minimum ≤ S0×barrier, otherwise one.
4. Otherwise simulate remaining observations under record 12, seeding historical minimum, memory and fixed coupons. Encode each payment as `Dpayment/Dexpiry` in the payoff and discount the resulting path total by Dexpiry. Final fixing freezes at the last observation even when expiry/payment is later.

## Selection, controls and conventions

`MonteCarloGBM` is the product default. Final barrier monitoring is discrete at contractual observation dates, not every simulation step. Spot risk pins omitted initial level; vega/volga and rate risks delegate to shared bump calculators. Theta-state construction fills newly crossed observations with the scenario spot and sorts them; that is scenario state, not historical data repair.

## Limitations, failures and alternatives

Unreleased memory coupons are not automatically paid at maturity unless their coupon barrier is met. Negative coupons/participation settings have variant-specific acceptance. Continuous knock-in monitoring and a different capital-protection payoff would be distinct methods. The current final-payoff names alone do not establish a particular issuer term sheet.

## Evidence

[structured-product reference](../../../REFERENCES.md#overhaus-2007-equity-derivatives); [test_seasoned.rs](../../../../finstack-quant/valuations/equity/tests/autocallable/test_seasoned.rs), [test_settlement_gap.rs](../../../../finstack-quant/valuations/equity/tests/autocallable/test_settlement_gap.rs) and [test_day_count_basis.rs](../../../../finstack-quant/valuations/equity/tests/autocallable/test_day_count_basis.rs) locate seasoning, fixed payment and clock regressions. Shared payoff tests independently assert simple deterministic paths but do not establish calibrated-market accuracy.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [types.rs](../../../../finstack-quant/valuations/equity/src/autocallable/types.rs)
- [pricer.rs](../../../../finstack-quant/valuations/equity/src/autocallable/pricer.rs)
- [metrics.rs](../../../../finstack-quant/valuations/equity/src/autocallable/metrics.rs)
