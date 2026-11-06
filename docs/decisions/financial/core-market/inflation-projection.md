# CORE-MARKET-10: Projected CPI, continuous-time lag, CAGR, and inflation shocks

- Decision status: **Draft — observed implementation, adoption pending.**
- Implementation status: **Observed** in the owning paths identified below; discrepancies are stated explicitly.
- Validation status: **Evidence inspected or located, not executed.** Formula checks, invariants, and round trips are distinguished below; no production accuracy certification is implied.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Choice and rationale

`InflationCurve` stores positive projected CPI levels. Its growth measure is annually compounded CAGR, and its numeric lag approximates calendar months by months/12 years. This is distinct from the calendar-month observed-index method in [inflation observations](inflation-observations.md).

## Algorithm and conventions

Defaults are base CPI 100, Act/365F, three-month indexation lag, LogLinear interpolation, and the shared default FlatZero extrapolation. The builder requires base date, ordered finite knots and finite positive levels/base CPI, inserts an exact `(0,base_cpi)` when absent, and rejects a conflicting exact-zero point. MonotoneConvex is not supported. Other interpolation choices use strict positive source validation.

`cpi(t<=0)` returns base CPI. For positive time it evaluates the interpolator (or saved rolled source). `cpi_with_lag(t)=cpi(t-lag_months/12)`. `index_ratio(t)` divides lagged projected CPI by base CPI. `inflation_rate(t1,t2)=expm1((ln(CPI2)-ln(CPI1))/(t2-t1))` requires finite ordered times, finite interval and positive finite CPI endpoints. Date CPI lookup uses the stored day-count clock but does not apply indexation lag automatically.

For a knot at positive time `t`, form annual zero inflation `r=(CPI(t)/base_cpi)^(1/t)-1`. An additive bump gives `r_b=r+b*w(t)`. A multiplicative factor gives `1+r_b=(1+r)*(1+(factor-1)*w(t))`, scaling the **gross annual inflation factor**, not multiplying `r` alone. Reconstruct `CPI_b=base_cpi*(1+r_b)^t`; origin/base CPI is pinned. Standard additive bp, percentage-point and fraction units are supported. Invalid resulting levels fail builder validation. These are knot/source-knot shocks, not exact continuous zero-inflation shifts between arbitrary interpolation knots.

Rolling uses signed day count, takes old CPI at the offset as new base CPI, shifts future knots, inserts origin and preserves source interpolation for nonnegative cumulative offset. If all future knots expire it errors. A backward offset crossing below the original source origin discards source roll state and rebuilds from rolled pillars; rolled-source shock mapping can insert an anchor to preserve the current origin.

## Limitations and alternatives

FlatZero means flat **CPI level** beyond the final knot and thus zero incremental tail inflation, not flat annualized inflation. Negative historical query times return base CPI, not historical prints. This data type does not merge fixings, publication schedules, or seasonality. A month/12 lag is not a substitute for calendar-month reference CPI on actual settlement dates.

LogLinear with FlatForward would continue an endpoint log-CPI growth rate and is explicitly selectable. Date-aware lagging/hybrid historical-forward resolution is a different consumer-level choice.

## Evidence

Owning code and tests: [inflation.rs](../../../../finstack-quant/core/src/market_data/term_structures/inflation.rs), [shared roll implementation](../../../../finstack-quant/core/src/market_data/term_structures/common/roll.rs). Inspected `inflation_rate_uses_cagr` checks 300 to 306 gives 2%, while lag/index-ratio tests check local formulas; retained-source and serde tests are invariants. No external inflation market repricing validation is claimed. References: [Deacon–Derry–Mirfendereski](../../../REFERENCES.md#deacon-derry-mirfendereski-2004), [Kerkhof](../../../REFERENCES.md#kerkhof-2005).

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
