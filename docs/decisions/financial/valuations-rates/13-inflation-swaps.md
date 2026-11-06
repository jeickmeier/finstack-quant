# VALUATIONS-RATES-13-INFLATION-SWAPS: Inflation swap CPI ratios and accrual conventions

- ID: VALUATIONS-RATES-13-INFLATION-SWAPS
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed in the owning paths below.
- Validation status: Evidence located/inspected, not executed; no universal accuracy qualification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope and rationale

Zero-coupon inflation swaps exchange compounded fixed growth against a terminal CPI ratio; YoY swaps exchange each period's CPI change against simple fixed interest. These are deterministic projected-CPI valuations. This formulation is observed, not an approval of curve/fixing conventions or stochastic inflation modeling. The local product does not estimate inflation convexity from a stochastic nominal/real-rate model.

## Algorithm and controls

1. Resolve lag through the shared inflation resolver. Interpolation is instrument override, then index setting, then the scalar interpolation default. Resolve each CPI reference using the valuation date and publication-aware shared helper; an explicit positive `base_cpi` replaces only the start reference.
2. Zero-coupon fixed amount is `N*((1+k)^tau-1)`; inflation amount is `N*(I_end/I_start-1)`. Require `k > -1`. The normal year fraction uses the instrument day count. `OneOne` instead creates unadjusted annual periods with a short front stub, counts full years, and prorates the first year by actual days against its annual reference period.
3. Adjust maturity only when a calendar is supplied; without one the zero-coupon payment remains unadjusted. Discount with `D(as_of,payment)`. Payments on or before valuation have zero PV. Pay means pay fixed/receive inflation; Receive reverses it.
4. Zero-coupon par rate is `(I_end/I_start)^(1/tau)-1`, returning zero for nonpositive tau. Discounting cancels because both legs share payment date.
5. YoY schedules use the specified frequency, `StubKind::None`, unadjusted accrual boundaries, zero payment lag and weekends-only calendar if omitted. For each unpaid period, inflation cashflow is `N*(I_end/I_start-1)` and fixed cashflow `N*k*alpha`; the inflation ratio is not multiplied by alpha again. Par rate is discounted CPI changes divided by discounted accrual annuity, with zero for an annuity below `1e-15` in absolute value.

## Risk, conventions and failures

Positive CPI is the economic input domain. The zero-coupon start reference is checked finite and positive; YoY period starts are checked positive. Nonpositive finite observed ending CPI can pass the shared resolver and is not consistently rejected before price/flow ratios. Zero-coupon par-rate inversion separately rejects a nonpositive final ratio, so pricing and inverse-rate domain enforcement differ. See [observed CPI inputs](../core-market/inflation-observations.md) and [inflation projection](../core-market/inflation-projection.md). Discount and inflation curves have distinct roles. Rates are decimal; the inflation risk helper passes ±0.01 percentage points to `inflation_shift_pct`, representing ±1 bp in decimal inflation rates. Inflation01 is half the up/down PV difference. Inflation convexity divides the second difference by `0.0001^2`. Zero-coupon inflation01 calls the instrument value path; convexity uses the metric repricer. DV01 and triangular bucketed DV01 use the shared rate sensitivity engine. Missing published fixings are governed by the common resolver, not replaced locally by current CPI.

## Alternatives and tradeoffs

Stochastic inflation/nominal-rate convexity and explicit YoY market curves would model risks not present in deterministic ratio projection; they are not alternative branches here. Alternative stub and payment conventions change economics. Whether the no-calendar difference between zero-coupon and YoY products is intended remains an adoption question.

## Evidence

- [Inflation swap implementations](../../../../finstack-quant/valuations/rates/src/inflation_swap/types.rs): ratio resolution, zero-coupon/YoY pricing, cashflow export and par rates.
- [Inflation finite differences](../../../../finstack-quant/valuations/rates/src/inflation_fd.rs) and [metric registration](../../../../finstack-quant/valuations/rates/src/inflation_swap/metrics/mod.rs): bump units, repricing and selected metrics.
- [Lag tests](../../../../finstack-quant/valuations/rates/tests/inflation_swap/integration/test_lag_policies.rs), [YoY tests](../../../../finstack-quant/valuations/rates/tests/inflation_swap/integration/test_yoy.rs), [par-rate tests](../../../../finstack-quant/valuations/rates/tests/inflation_swap/metrics/test_par_rate.rs): convention, formula and roundtrip evidence; not a stochastic-model benchmark.
- [Deacon, Derry and Mirfendereski](../../../REFERENCES.md#deacon-derry-mirfendereski-2004) and [Kerkhof](../../../REFERENCES.md#kerkhof-2005): canonical reference entries, not proof that every implemented convention matches a particular market contract.

## Evidence limits

YoY raw cashflow construction includes valuation-date flows (`pay < as_of` exclusion), while valuation excludes them and the instrument declares valuation-date exclusion. No independent stochastic inflation accuracy claim is established.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
