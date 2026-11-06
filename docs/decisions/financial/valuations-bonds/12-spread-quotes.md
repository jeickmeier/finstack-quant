# VALUATIONS-BONDS-12: Z-spread, discount margin, I/G spreads and OAS inversions

**Decision status:** Draft — observed implementation; adoption pending.
**Implementation status:** Observed in the linked bond production scope; discrepancies are identified explicitly.
**Validation status:** Evidence inspected or located, not executed; no accuracy certification.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and conventions

Z-spread/DM discount contractual or selected workout cashflows; OAS reprices the selected model with exercise/default; I/G spread subtract a horizon par benchmark from quoted yield. All public spread quotes are decimal rates; tree OAS solver operates internally in bp. Prices are settlement dirty currency. The alternatives have different economic content and are not interchangeable labels.

## Algorithms and selection

**Z-spread:** cache positive-time settlement-entitled flows, base discount factors and times on the discount-curve day count (ACT/365L uses contractual periods). Use core periodic Z-spread DF: convert base DF to a coupon-frequency nominal rate, add Z and compound back; frequency comes from `compounding_frequency`. Option-bearing bonds require an explicit clean quote selecting a workout path, otherwise error. Solve `Σ CF·DF_z−P=0` by fallible Brent from zero; tolerance `1e−10`; initial half-bracket is 1000 bp multiplied by `1+min(years_ACT365F/30,1)`, capped at 3000 bp. The cap is on the initial bracket, not the ultimate root domain.

**DM:** floating-only; clear price overrides, project coupons once and solve the same Z kernel against target. Initial bracket is 500 bp with the same maturity scale, capped 1500 bp; tolerance `1e−10`. DM changes discounting, not the contractual floating spread or projection curve. Option-bearing DM after clearing clean-price driver cannot select a workout and can error.

**I/G spread:** `selected_yield−benchmark_par_rate`; use workout yield/horizon when an explicit clean optional quote supplies it, otherwise YTM/maturity. I-spread uses the bond discount curve; G-spread requires an explicit government curve. If curve calibration contains at least two swap quotes, restate each quote from its index fixed-leg compounding to bond yield frequency, linearly interpolate by remaining tenor, and clamp outside end tenors. Otherwise construct a ShortFront fixed-leg schedule and use `(D_start−D_end)/ΣαD`. Fixed specs use bond day count/frequency; other specs use ACT/ACT annual. Even G-spread calls this same par-rate/swap-quote helper; it does not interpolate a sovereign cash-bond yield directly.

**OAS:** an explicit OAS is returned as-is; otherwise a price-driving quote is required. Convert quote to settlement dirty, remove price drivers, prepare the selected model at settlement and solve price minus target in bp. Default tolerance `1e−6`, max 50 iterations, initial bracket 1000 bp. Standard tree/joint dispatch reuses calibrated preparation; custom registries preserve their dispatch. Trial pricing errors are captured while the numeric objective returns `1e12`; if solve fails the first underlying error is reported. An optional MC CI gate is suppressed during trial solves and checked on the final candidate, without a separate price-residual acceptance check. Hazard and stochastic joint OAS require continuous quotes.

## Evidence and limitations

Owning code: [spread kernel/inverses](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/spread_price.rs), [benchmark par logic](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/annuity.rs), [Z](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/z_spread.rs), [DM](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/dm.rs), [I](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/i_spread.rs), [G](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/g_spread.rs), [OAS](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/oas.rs). Tests here and [quote-engine tests](../../../../finstack-quant/valuations/fixed-income/tests/bond/metrics/quote_engine.rs) are chiefly roundtrip/selection evidence. Reference: [Tuckman–Serrat](../../../REFERENCES.md#tuckman-serrat-fixed-income). Finite-difference risk should not assume all these spreads share units of compounding merely because all are decimal.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
