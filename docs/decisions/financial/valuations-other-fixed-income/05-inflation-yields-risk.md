# VALUATIONS-OTHER-FIXED-INCOME-05: Real yields, breakeven inflation and inflation sensitivities

- **Decision status:** Draft — observed implementation; adoption and rationale require agreement.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the entry points and kernels linked below.
- **Validation status:** Source and named evidence inspected or located; tests and numerical experiments were not executed. Evidence limits are stated below.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Choice and rationale

The [real-yield path](../../../../finstack-quant/valuations/fixed-income/src/inflation_linked_bond/types.rs) uses unindexed cashflows and clean price per 100 **real face**; model PV separately uses indexed nominal cashflows. [Metrics](../../../../finstack-quant/valuations/fixed-income/src/inflation_linked_bond/metrics/mod.rs) keep real-yield risk and projected-inflation risk distinct. The compounding reconciliation is documented, but the nominal reference chosen for metric breakeven remains a methodological choice.

## Algorithm

1. Construct a real schedule with coupon `face × real_coupon × period_fraction` and unindexed principal at adjusted maturity. Retain coupon-period metadata for accrual and Actual/365L denominator handling. Real-yield remaining flows include payment dates on/after `as_of`; nominal PV excludes same-day payments.
2. Add real accrued to `clean_percent × face/100`. Solve the bond Street-compounding YTM using contractual frequency and dated flow times. Missing/nonpositive quote, no remaining cashflows, invalid timing or solver failure propagates as error.
3. Real duration solves base real yield, reprices real flows at ±yield bump (default 1 bp, caller-positive override), and returns `−(P_up−P_down)/(2h P_dirty)`. Denominator is model dirty price per 100, floored at `1e-6`, not clean quote.
4. Breakeven converts real Street yield to effective-annual yield and computes `(1+nominal_annual)/(1+real_annual)−1`. Direct callers supply a comparable nominal annual yield; the metric instead infers the **zero yield to legal maturity** from the nominal curve `DF^(−1/T)−1`, not a matched coupon-bond yield. The conversion horizon falls back to one year on day-count failure and is floored at `1e-6`; unsupported noninteger frequency is only debug-asserted before rounding.
5. [Inflation01](../../../../finstack-quant/valuations/fixed-income/src/inflation_linked_bond/metrics/inflation01.rs) shifts the inflation curve ±.01 percent (±1 bp annual inflation-rate shock) through the shared bump API and returns half the currency PV difference. When only a published index exists, validate base pricing and leave market unchanged, giving zero projected-inflation risk.
6. InflationConvexity uses the same ±1 bp shock and reports `(P_up+P_down−2P0)/h²` in currency per squared decimal rate, without dividing by PV. Near-zero base PV yields zero. Parallel/bucketed nominal DV01 delegates to shared curve repricing.

## Scope, limitations and alternatives

Real yield is a quote convention, not a stochastic real-rate model. The metric breakeven blends a nominal zero yield with a coupon-bond real yield; it need not equal a market matched-maturity bond breakeven or inflation-swap rate. The inflation bump affects projected anchors; already published fixings are held fixed. Floor optionality may make inflation risk nonlinear. Zero-bond, matched-cashflow and liquidity/seasonality-adjusted breakevens are unimplemented alternatives. Silent conversion-time fallback and frequency rounding require an explicit acceptance decision.

## Evidence

Colocated `breakeven_inflation_uses_consistent_annual_compounding_c8`, `real_duration_uses_dirty_model_price_denominator`, `act_act_isma_real_schedule_and_accrual_need_no_day_count_override`, and `act365l_real_yield_and_duration_retain_coupon_periods` test identities and conventions. [Yield integration cases](../../../../finstack-quant/valuations/fixed-income/tests/inflation_linked_bond/test_yields.rs) and [duration cases](../../../../finstack-quant/valuations/fixed-income/tests/inflation_linked_bond/test_duration.rs) were located, not run. [Inflation reference](../../../REFERENCES.md#deacon-derry-mirfendereski-2004) supplies context.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
