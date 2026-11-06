# CASHFLOWS-009 — Fixed, periodic and utilization-tier fees

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed**, scoped below. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: [slice audit](README.md).

## Choice and domain

The builder supports fixed dated amounts and periodic annual-basis-point fees on drawn or undrawn principal. PointInTime is the default accrual basis; TimeWeightedAverage integrates economic balance changes using contractual day count. Tier evaluation is a standalone rate selector, not an automatically connected piece of periodic emission.

## Algorithms and controls

- Fixed fees emit once at the exact supplied date, with no BDC or lag. Zero amounts are suppressed. Dates after raw maturity fail; pre-issue dates are accepted. The fixed amount's currency is retained.
- Periodic fee specifications supply tenor, day count, BDC, calendar, stub and annual bp. Compiler uses one full-horizon schedule with unadjusted accrual, no EOM, no roll rule and no payment lag. Default stub is short-front. Empty schedules or unknown calendars fail.
- PointInTime chooses the balance at the **start of the fee period**, then uses it for the full accrual interval. TimeWeightedAverage partitions at economic balance changes and emits one fee per nonzero segment using that segment's contractual day-count fraction.
- Drawn base is outstanding `B`. Undrawn base is `max(commitment-B,0)` and requires commitment currency to match the builder currency. No negative-commitment rejection is imposed by this branch; a negative commitment ordinarily produces zero undrawn fee.
- Fee amount is `base × bp × 10^-4 × year_fraction`. Checked Decimal multiplication pairs smallest and largest magnitudes at each step to reduce intermediate overflow/underflow. Nonrepresentable final/intermediate arithmetic errors. Negative bp fees are permitted and emit negative amounts. Flow rate records `bp×10^-4`; each flow carries its own accrual metadata.
- `evaluate_fee_tiers` requires strictly ascending thresholds, then selects the last tier with `utilization >= threshold`. It returns zero below the first threshold or for an empty table. The selected rate applies to the whole utilization input; this is not marginal-band integration. Decimal utilization, thresholds and quotes are not constrained to `[0,1]` or nonnegative values by this function.

## Ordering, alternatives and limitations

Emission runs after same-day coupon capitalization, principal events and amortization, but period calculations use history on `[start,end)`, so closing-date movements do not retrospectively change the preceding fee base. Issue-date amortization updates the initial history before later fee accrual.

Supported alternatives are opening-balance and integrated-balance fees. Ending-balance fees, marginal tier bands, dynamic commitments and fee-specific payment lags are not implemented in these specs. No currency minor-unit rounding per fee is configured here. Caller-side checks must establish product-specific utilization/commitment domains.

## Evidence

- [fees spec/tier selector](../../../../finstack-quant/cashflows/src/builder/specs/fees.rs), [fee compiler](../../../../finstack-quant/cashflows/src/builder/compiler.rs), [fee emission](../../../../finstack-quant/cashflows/src/builder/emission/fees.rs).
- Inspected hand formula `b14_fee_uses_contract_daycount_on_each_balance_segment` in emission/fees uses ACT/ACT and an extinguished balance to expect `1,000,000×0.005×30/365`. Located PointInTime/TWA constant-balance equivalence and changing-balance cases there. [principal_credit_regressions.rs](../../../../finstack-quant/cashflows/tests/principal_credit_regressions.rs) locates large-notional/tiny-quote precision tests. Tier source tests check ordering and zero-below-threshold behavior.
- No universal fee-calculation standard or contract fixture establishes the default basis. Existing tests substantiate formulas/branches but do not supply product adoption authority.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
