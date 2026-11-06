# VALUATIONS-RATES-05-CAP-FLOOR-PRICING: Cap/floor projection, volatility conventions and Hull–White approximation

- ID: VALUATIONS-RATES-05-CAP-FLOOR-PRICING
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed in the owning paths below.
- Validation status: Evidence located/inspected, not executed; no universal accuracy qualification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope and rationale

European cash-settled caps/floors are sums of optionlets on either term rates or explicitly configured compounded overnight coupons. The default registry key is Black76, but Auto volatility convention follows source metadata and can dispatch Bachelier or shifted Black. HullWhite1F is a separate explicit model. [ADR 0029](0029-rate-option-exercise-and-volatility.md) records the broader selection question; this record states the actual numerical paths.

## Algorithm and controls

1. Build one actual period for Caplet/Floorlet or a frequency schedule for Cap/Floor. Defaults are short-front/ModifiedFollowing, unadjusted accruals and no EOM. Calendar priority is explicit, forward-index registry, supported currency default, else error. Term reset/payment lags come from registry or no reset/zero payment lag. If constructor frequency is absent, a day-span heuristic selects monthly/quarterly/semiannual/annual; the product enum determines single versus multiple periods.
2. Term rates require exact past fixing (strictly before valuation) or period forward otherwise. Add bp spread to coupon; for simple additive spread, remove it from both forward and strike before option pricing. Overnight settings require a registered overnight index and explicitly select compounding, spread inclusion/exclusion, fixing/payment calendars and lag (0–31 business days). Project daily observations, retain first/second parallel-forward sensitivities and observation exposures, and use the final fixing as option expiry. Same-ID overnight projection prefers an existing forward object, otherwise the discount curve.
3. For unpaid periods use ACT/365F expiry, floored to `1e-6` only for a still-future volatility lookup. Fixed optionlets use zero-vol Normal intrinsic without a vol lookup. Resolve Auto/Normal/Lognormal/ShiftedLognormal quote with clamping; shifted quotes transform F and K together. Price `N*alpha*D*Black_or_Bachelier(F,K,sigma,T)`. Normal vol below `1e-8` becomes intrinsic. Black requires its model domain after quote transformation; there is no automatic negative-rate model conversion.
4. Hull–White term optionlets call the model's term-caplet formula with forward-curve pseudo-DFs rebased at valuation, actual reset/start/end/payment times, discount-payment DF and spread-adjusted strike. The model kernel is owned by models.
5. Hull–White overnight optionlets instead linearize the compounded coupon against each unfixed daily rate. Loading is `coupon_forward_derivative*(1+f_i*alpha_i)*B(0,delta_t_i)/alpha_i`; coupon variance sums every pair of loadings times OU state covariance, floored at zero. Match `sigma_normal=sqrt(variance/T_final_fixing)` and apply Bachelier to the projected coupon mean. This is a first-order moment match, not exact compounded-option pricing.
6. Hull–White params accept explicit sigma schedule plus positive mean reversion, or a persisted cap/floor-family sigma series anchored at `max(as_of,discount_base)` with its active value at time zero. Explicit scalar sigma suppresses persisted schedule selection; simultaneous explicit scalar/schedule errors. Otherwise use shared family parameter resolution. All models subtract the discounted nonnegative same-currency holder premium only while its date is strictly future.

## Limitations and alternatives

Non-European exercise and physical cap/floor requests error. Rates/strikes are decimals; normal volatility is absolute annualized rate volatility (`0.005` is 50 bp), while Black volatility is relative (`0.20` is 20%). Displacement is a decimal rate applied to both forward and strike. Model expiry uses ACT/365F independently of contractual coupon accrual day count. The normal moment match omits higher-order distribution effects; direct simulation of compounded Gaussian bond ratios is an unimplemented alternative. Surface quotes on compounded coupons are used as aggregate coupon-rate quotes, with no local stripping/calibration. Same-day fixing uses the live projected value at zero option time. Cashflow export is Placeholder, not a realized exercise ledger.

## Evidence

[Types/schedules](../../../../finstack-quant/valuations/rates/src/cap_floor/types.rs), [projection](../../../../finstack-quant/valuations/rates/src/cap_floor/pricing/projection.rs), [convention dispatch](../../../../finstack-quant/valuations/rates/src/cap_floor/pricing/pricer.rs), [Black kernel](../../../../finstack-quant/valuations/rates/src/cap_floor/pricing/black.rs), [Normal kernel](../../../../finstack-quant/valuations/rates/src/cap_floor/pricing/normal.rs), [HW adapter/moment match](../../../../finstack-quant/valuations/rates/src/cap_floor/hw_pricer.rs) establish these branches. [QuantLib parity](../../../../finstack-quant/valuations/rates/tests/cap_floor/integration/quantlib_parity.rs), [convention tests](../../../../finstack-quant/valuations/rates/tests/cap_floor/validation/auto_vol_type.rs) and inline HW tests provide scoped formula/invariant evidence, not executed. The cap/floor file labelled QuantLib parity includes broad range checks; its name alone does not establish independent vendor agreement. Canonical entries are [Black](../../../REFERENCES.md#black-1976), [Bachelier](../../../REFERENCES.md#bachelier-1900), [Hull–White](../../../REFERENCES.md#hull-white-1990-pricing-interest-rate-derivatives).

The Auto variant prose and test introduction describe negative-rate conversion, while executable convention resolution preserves quote units and rejects invalid Black coordinates. This is a documentation discrepancy, not an adopted conversion fallback.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
