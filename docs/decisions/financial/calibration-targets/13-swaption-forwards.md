# CALIBRATION-TARGETS-013 — Contractual swaption coordinates, schedules and calibration forwards

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed for the stated source scope; behavior and scope limitations are described below.
- **Validation status:** Source and selected evidence inspected or located; not executed.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Choice and conventions

Quoted swaption smiles are indexed by exact ACT/365F option expiry and contractual swap tenor derived from settlement-adjusted underlying start and explicit maturity. Calibration forwards are schedule-weighted par rates; fixed-leg day count is independent from the option-variance clock. These helpers also supply schedules to Hull–White adapters, whose payoff integration and rejection policies are separately owned.

## Algorithm

1. Resolve quote SwaptionConventionId, then floating rate-index conventions. Currency comes from floating index; fixed frequency/day count/BDC/calendar/settlement from swaption convention; floating frequency/day count/BDC/reset lag/compounding and both legs' payment lags come from index defaults. Parameter fixed-day-count/calendar overrides replace only those two fields. Quote currency must equal step currency.
2. Add settlement business days and fixed BDC to option expiry to obtain start; maturity must be after it. Use exact quote expiry in ACT/365F and `contractual_swap_tenor_years(start,maturity)` for tenor. These are model coordinates; rounded keys are only a bucket lookup mechanism [014](14-swaption-cube.md).
3. Build fixed and floating schedules with ShortBack stub, no EOM, raw accrual endpoints, no roll rule, leg BDC/day count/frequency/payment lag; floating reset lag is stored on periods. The annuity is `A=sum(alpha_fixed*D(payment))`, with D measured from discount-curve origin, and must be finite above `1e-16`.
4. Floating coupon growth is either `forward.rate_between_dates(start,end,alpha_float)*alpha_float`, or, without projection curve, `1/D(start,end)-1`. Discount each growth at its actual payment date. Return `Fswap=sum(growth_i*D(payment_i))/A`. Common discount-origin scale cancels. Single-curve telescoping to `D(start)-D(end)` applies only when coupon accrual-end and payment dates align; the implementation keeps individual payment discounting.
5. For **interpolated target** cube nodes without an exact calibrated bucket, require `params.swap_index`; use its default fixed/floating conventions plus overrides. Convert numeric expiry/tenor to rounded whole months, reject values outside integer month range, add expiry months to base, settle, then add tenor months to start to calculate a synthetic forward. Published node coordinates still retain requested numeric values.

## Observed limitations and alternatives

`float_compounding` is resolved but not consulted by this target's forward calculation. Constructed reset dates likewise do not select fixings; all coupon projection uses accrual endpoints/DF-implied growth. Consequently a registry lookback/observation-shift/cutoff convention is not fully represented by this par-forward helper. Hull–White adapter explicitly rejects nonzero lookback/shift/cutoff; this SABR cube target has no equivalent check. This is a static convention-application gap; numerical effect requires a configured convention/contract comparison.

Quoted nodes use quote-specific swap conventions, while interpolated nodes use index defaults and month rounding. Those can differ materially near stubs/settlement/holiday boundaries. Alternatives are reusing the full IRS cashflow projector, requiring explicit contracts at every node, or publishing both grid model coordinates and exact reference dates. Explicit projected forwards need not include a stochastic delayed-payment convexity correction; that is a separate pricing model decision.

## Evidence

- Owning [swaption.rs](../../../../finstack-quant/calibration/src/targets/swaption.rs), especially convention/date/schedule and par-rate helpers, inspected beyond the early test-only import.
- Inspected single-curve telescoping test and multicurve test compare DF-implied coupon growth against the rejected integral-average alternative. Located settlement-date, payment-lag and fixed-day-count-independent-expiry cases in [swaption tests](../../../../finstack-quant/calibration/tests/calibration/swaption_vol.rs). Same-library schedule generation limits independence.
- [European swaption methodology](../valuations-rates/07-european-swaptions.md) and [framework Hull–White adapter](../calibration-framework/18-hull-white-market-adapter.md) document downstream meanings.

No test run was performed.

## Follow-up tracking

Adoption, reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
