# VALUATIONS-RATES-14-INFLATION-OPTIONS: Inflation option forward proxy and information expiry

- ID: VALUATIONS-RATES-14-INFLATION-OPTIONS
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed in the owning paths below.
- Validation status: Evidence located/inspected, not executed; no universal accuracy qualification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope and rationale

Inflation cap/floor pricing treats the deterministic CPI forward ratio as the mean of a simple annualized inflation rate, then applies a Normal or Black caplet kernel. This avoids a joint CPI/rates simulation but does not calculate a stochastic ratio convexity correction. Black76 is the default; Normal is explicitly registered. Selection follows the requested model and volatility convention, not an automatic switch for negative inflation.

## Algorithm and controls

1. Build one period for a caplet/floorlet or a frequency schedule for a cap/floor. Defaults are short-front stub and ModifiedFollowing; absent calendar resolves to weekends-only. Accrual boundaries remain unadjusted.
2. Resolve start/end CPI with lag, interpolation and publication state. Require positive CPI and positive accrual alpha. Compute `F=(I_end/I_start-1)/alpha`.
3. If both references are known, set time to zero and use deterministic intrinsic payoff, requiring no vol quote. Otherwise ReferenceDate (default) uses lagged end date; PublicationDate uses the latest publication date among all unknown anchors and requires each unknown publication date explicitly.
4. Use ACT/365F to this expiry. An unknown CPI with nonpositive selected expiry errors and instructs the caller to supply publication-expiry quotes. Resolve vol at `(expiry, tenor=0, strike)` with surface clamping enabled and required Normal/Lognormal convention.
5. Price `N*alpha*D(as_of,pay)*option(F,K,sigma,T)` through the shared caplet convention dispatch and sum only payments strictly after valuation.

## Risk, limitations and fallbacks

Inflation01 is a central ±1 bp inflation-curve bump through the active metric repricer. Gamma is the second difference divided by the squared decimal bump; vega centrally bumps the stored surface and reports currency per volatility point. Parallel/bucketed rates risk delegates to the shared DV01 implementation. CPI below 50 emits a warning even though a positive rebased CPI can be economically valid; it is not rejected. Volatility surface clamping can flatten extrapolation. A volatility override may make a surface-only vega bump ineffective.

## Alternatives and tradeoffs

Publication-expiry quoting is supported as an explicit alternative to reference-date expiry. Stochastic CPI ratio expectation/convexity and market-calibrated YoY forwards are unimplemented alternatives. Bachelier supports negative forward inflation without lognormal positivity, but is a separate quote/model choice.

## Evidence

- [Inflation option implementation](../../../../finstack-quant/valuations/rates/src/inflation_cap_floor/types.rs) and [model adapter](../../../../finstack-quant/valuations/rates/src/inflation_cap_floor/pricer.rs): information expiry, deterministic proxy and kernel dispatch.
- [Inflation option metrics](../../../../finstack-quant/valuations/rates/src/inflation_cap_floor/metrics/mod.rs) and [inflation01](../../../../finstack-quant/valuations/rates/src/inflation_cap_floor/metrics/inflation01.rs): selected bump/reprice channels.
- [Pricing tests](../../../../finstack-quant/valuations/rates/tests/inflation_cap_floor/pricing.rs): formula and information-state behavior. [Production inflation audit](../../../../finstack-quant/valuations/tests/production_inflation_audit.rs): cross-product convention evidence, not independent model certification.
- [Inflation references](../../../REFERENCES.md#deacon-derry-mirfendereski-2004), [Black](../../../REFERENCES.md#black-1976), [Bachelier](../../../REFERENCES.md#bachelier-1900).

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
