# VALUATIONS-BONDS-10: Street, Moosmüller and Japanese yield pricing and inversion

**Decision status:** Draft — observed implementation; adoption pending.
**Implementation status:** Observed in the linked bond production scope; discrepancies are identified explicitly.
**Validation status:** Evidence inspected or located, not executed; no accuracy certification.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, domain and selection

Default YTM is Street nominal yield compounded at coupon frequency, using settlement-entitled contractual cashflows and the time rules in record 02. A separate Moosmüller metric and public generic `Rate(Compounding)` kernel are supported. These are quote-convention calculations, not stochastic discounting. Coupon rate only seeds the inverse solve; arbitrary projected/signed holder cashflows can be supplied, without a uniqueness guarantee for signed IRRs.

## Algorithms

Street DF is `(1+y/m)^(−m t)` with `m=max(payments_per_year,1)`, requiring positive compounding base. For Moosmüller full-path pricing, first positive cashflow time `w` supplies simple-interest `1/(1+yw)`, then each distinct positive payment date increments a coupon index and contributes `(1+y/m)^(−(k−1))`; cash and principal on one date share the index. Standalone `df_from_yield` infers `w` from fractional periods instead, so irregular-date calls should use the complete schedule kernel. Nonpositive time returns DF one in that helper, while price summation omits nonpositive-time flows. Generic Rate compounding delegates the core compounding DF and rejects nonfinite results. Yield below −50% warns but is not rejected solely for that threshold.

YTM inverse requires positive target and nonempty flow list. A single positive dated flow uses closed-form Rate inverse, simple inverse for Moosmüller, or Street periodic inverse. Otherwise Brent solves `price(y)−target` with yield-axis tolerance `1e−12` and 50 iterations, initialized by current yield plus half annualized pull-to-par, clamped to [−1,10] only as a starting guess. Pricing-domain failures and nonconvergence propagate; no false residual is substituted.

Japanese simple yield is fixed-spec-only and uses ACT/365F remaining life `n`, positive clean price percentage `P`: `y=(100c+(100−P)/n)/P`; inversion is `P=100(1+c n)/(1+y n)`, requiring a positive denominator. It ignores the detailed timing of remaining coupons, and no model-based option adjustment is included.

## Failure, alternatives and validation

Zero-price yield is undefined; no positive future time or mixed-sign flows can undermine uniqueness. The one-flow shortcut counts supplied rows, so an extra zero or expired row changes algorithm selection. ACT/365L has explicit coupon-period constraints rather than generic date-only fallback. Numerical tolerance is not independent quote accuracy.

Owning code: [yield/price kernels](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/yield_price.rs), [solver](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/ytm_solver.rs), [YTM metric](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/ytm.rs), [Japanese metric](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/mod.rs), [quote type](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/types.rs). Inline tests include scalar formulas and round trips; [market benchmarks](../../../../finstack-quant/valuations/fixed-income/tests/bond/validation/market_benchmarks.rs) include broad expected ranges and self-repricing, which must not be described as vendor certification. [Fabozzi](../../../REFERENCES.md#fabozzi-fixed-income-handbook) and [ICMA](../../../REFERENCES.md#icma-rule-book) are general references; exact market/security applicability and Moosmüller irregular-schedule provenance remain open. Continuous, periodic and simple alternatives are implemented explicitly; selecting one changes the quote meaning.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
