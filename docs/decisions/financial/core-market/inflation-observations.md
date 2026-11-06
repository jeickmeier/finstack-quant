# CORE-MARKET-11: Observed reference CPI, calendar lag, and seasonality

- Decision status: **Draft — observed implementation, adoption pending.**
- Implementation status: **Observed** in the owning paths identified below; discrepancies are stated explicitly.
- Validation status: **Evidence inspected or located, not executed.** Formula checks, invariants, and round trips are distinguished below; no production accuracy certification is implied.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Choice and rationale

`InflationIndex` combines a scalar observation series with currency, lag, interpolation policy, optional monthly multipliers and publication-date metadata. Its monthly-lag branch deliberately requires actual observations in the reference months. It does not silently substitute projected values.

## Algorithm and selection

The underlying series sorts observations, rejects duplicates/nonfinite values and defaults to Step. No lag and no seasonality are default. Lag is None, calendar Months, or calendar Days; month counts parsed through the public lag enum fit u8, while direct `ref_cpi_months_lag` accepts u32 and rejects conversion beyond i32.

For Days, subtract the calendar-day count, use the scalar series' Step/Linear lookup, then multiply by the effective query month's seasonal factor. For None, use the query date directly. For Months:

1. Under Step mode, replace the contract query date by the first of its month; under Linear retain its day.
2. Anchor at the first of the contract month minus the requested calendar months.
3. Require **exactly one observation anywhere in that reference month**; no observation errors and multiple observations in the same month error. Multiply that observed level by that reference month's seasonality factor.
4. If contract day is one, return that first level. Otherwise obtain the next reference month's unique observation and seasonality-adjust it.
5. Interpolate `C0 + ((contract_day-1)/days_in_contract_month)*(C1-C0)`. The denominator is days in the settlement/contract month, not days in the lagged reference month.

Calling `ref_cpi_months_lag` directly uses its date/day formula regardless of stored interpolation metadata. `ratio(base_date,settle_date)` divides independently resolved values and rejects only an exactly zero denominator.

## Domain and limitations

Observation values need only be finite through the scalar-series constructor; positive CPI is not enforced here. Thus negative or zero observed CPI is not uniformly rejected, unlike the projected CPI curve. Seasonal multipliers must be finite and strictly positive, but are not normalized to a unit geometric mean or annual product. Their multiplication can also overflow without a dedicated result check.

Publication dates must map unique first-of-month reference anchors to dates on or after those anchors. They are stored and exposed, but `value_on` does not take an as-of date or consult publication metadata. These accessors therefore do not enforce point-in-time knowledge or prevent look-ahead use; callers must perform that resolution. The no-lag/day-lag generic series path can carry the final observation forward indefinitely. The monthly reference path rejects missing monthly anchors.

## Alternatives, evidence

Supported Step versus Linear monthly policy changes daily indexation. A normalized seasonality model, positivity validation, vintage-aware published-only lookup, and hybrid fixing/projection lookup are possible choices, not added here.

Owning code: [inflation_index.rs](../../../../finstack-quant/core/src/market_data/scalars/inflation_index.rs), [series monthly lookup](../../../../finstack-quant/core/src/market_data/scalars/primitives.rs). Inspected tests `ref_cpi_months_lag_requires_exact_monthly_observations`, `ref_cpi_months_lag_uses_settlement_month_day_count`, and `seasonality_adjusts_values_and_survives_serde` check missing data, hand-computed daily interpolation and multiplication. Publication tests are round-trip/domain evidence. References: [Deacon–Derry–Mirfendereski](../../../REFERENCES.md#deacon-derry-mirfendereski-2004), [Kerkhof](../../../REFERENCES.md#kerkhof-2005). No security-specific official reference-index fixture is asserted by these tests.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
