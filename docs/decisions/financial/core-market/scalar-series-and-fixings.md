# CORE-MARKET-12: Scalar observations, time-series interpolation, and exact fixings

- Decision status: **Draft — observed implementation, adoption pending.**
- Implementation status: **Observed** in the owning paths identified below; discrepancies are stated explicitly.
- Validation status: **Evidence inspected or located, not executed.** Formula checks, invariants, and round trips are distinguished below; no production accuracy certification is implied.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Choice and rationale

Generic market observations permit finite signed values and either last-observation carry or linear interpolation. Historical fixing helpers deliberately use an exact-date lookup rather than those generic interpolation modes. The missing-data policy is therefore selected by the caller/API, not by the stored series alone.

## Algorithm, units, and selection

`MarketScalar` is Unitless(f64) or Price(Money). Validating constructors and deserialization reject nonfinite unitless values; Money construction checks its own monetary representation. The public enum can still be constructed directly, and context insertion does not revalidate it, so the constructor invariant is not universal for Rust callers.

A scalar time series stores sorted calendar days since 1970-01-01 and finite numeric values in parallel arrays. Empty input and duplicate dates are rejected; one point is accepted. Default Step binary-searches the query and returns its exact value or the most recent prior value; dates before the first observation error. Linear uses adjacent day coordinates and `v0+w*(v1-v0)`; it also errors before the first observation and holds the last value after the last date. There is no finite-result check after interpolation of extreme finite values. `value_on_exact` requires an observed query date. `value_in_month` requires exactly one observation in the requested calendar month.

Fixing IDs are `FIXING:<forward_curve_id>`. CMS IDs are `FIXING:CMS-<tenor>:<curve_id>`; integral month counts within 1e-9 are rendered in months or whole years, otherwise raw year representation is used. The formatting helper does not independently validate positive finite tenor. Missing series or missing exact observation returns a contextual validation error naming date/index; `as_of` is diagnostic context rather than a historical/future eligibility gate.

Scalar/series generic bumps allow parallel standard additive bp/percentage-point/fraction or multiplicative factor, with finite-result validation through constructors. Unitless additive Percent adds value/100. Money additive Fraction adds native currency units, additive Percent scales by `1+value/100`, and Factor multiplies checked Decimal arithmetic; rate-bp money bumps are rejected. Time series apply the same arithmetic to every observation while preserving dates, currency and interpolation. No time-bucket shock is accepted.

## Alternatives, limitations, evidence

Carrying fixings forward is not an alternative in the exact fixing API; consumers needing stale-data tolerance must choose a separate policy explicitly. Generic interpolation is suitable for sparse indicators but does not encode market publication calendars, permissible fixing gaps, vintages or data ages. A general finite-result check and validating opaque scalar type would tighten the current direct-construction boundary.

Owning code: [scalars and series](../../../../finstack-quant/core/src/market_data/scalars/primitives.rs), [fixing helpers](../../../../finstack-quant/core/src/market_data/fixings.rs). Located tests `series_step_and_linear`, `linear_interpolation_after_last_date_uses_last_value`, constructor/serde domain tests, checked scalar overflow and exact fixing cases provide elementary formula and error-policy evidence. [Missing-data ADR](../conventions/0009-missing-and-invalid-data.md) is relevant policy context; [ISDA 2021](../../../REFERENCES.md#isda-2021-definitions) and [ARRC SOFR guide](../../../REFERENCES.md#arrc-sofr-users-guide) describe downstream rate conventions rather than approving generic observation carry.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
