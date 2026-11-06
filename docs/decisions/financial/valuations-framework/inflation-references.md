# VALUATIONS-FRAMEWORK-17 — Inflation lag, publication availability, and reference-index interpolation

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in the source paths cited below.
- Validation status: Evidence inspected/located, not executed; no runtime checks or external accuracy comparison performed.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope, and rationale

The shared CPI/RPI helper treats observation lag and publication availability separately. Historical contractual anchors must come from supplied index history; unpublished anchors come from the inflation curve. This avoids silently projecting a missing historical fixing and applies month lag/daily interpolation once. Inflation instrument pricing and option distribution remain in their product/model records.

## Algorithm and controls

Effective lag priority is explicit contract lag, inflation-index lag, projection-curve `indexation_lag_months`, then none. Calendar months or days are subtracted; unsupported lag variants and date underflow fail. A curve lag that cannot fit `u8` becomes none, as does zero. This observed fallback is not an adopted economic convention.

Monthly-anchor mode is selected for a month lag or an index carrying explicit monthly publication dates. A reference anchor is known when its publication date (or reference date if unspecified) is on/before as-of. Known anchors require history; unknown anchors require a same-ID inflation curve. Month lags move the reference month but retain the original contractual month's daily interpolation weight. Day/zero lags first move the reference date, then use that date's month/weight when publication metadata selects monthly mode.

Let `I0` be lagged month's first-day anchor and `I1` the next month's. Step interpolation or day one returns `I0`; linear interpolation returns `I0+w(I1−I0)`, `w=(day−1)/days_in_weight_month`. The reference is known only if both required anchors are known. Its outstanding publication date is the latest unknown-anchor publication when all required dates are supplied, otherwise unspecified. Known anchors are never replaced by projected values because history is missing.

Outside monthly mode, apply the effective lag directly and resolve the resulting date. For historical Step series, the last supplied observation may carry through the remainder of its calendar month; Linear requires right-anchor coverage through the effective date. The helper errors before extrapolating stale history. `BlackScholesInputsDf::r_eff` separately derives `−ln(DF)/T` for a model's volatility clock, returning zero for nonpositive time or discount factor; this convenience accessor does not enforce the stricter positive-DF validator used elsewhere.

The helper's `interpolation` argument controls only the monthly-anchor branch. Outside it, historical resolution uses the index's own interpolation after clearing its lag, and future resolution delegates to the projection curve's `cpi_on_date`. A requested Step therefore does not override a Linear historical index in this branch, and vice versa.

## Limitations, alternatives

Publication schedules must be supplied to represent not-yet-published past reference months. Without them, reference date is the availability boundary, so missing history errors. This is intentionally different from an automatic publication-lag estimate.

## Evidence

- [helpers.rs](../../../../finstack-quant/valuations/core/src/instruments/common_impl/helpers.rs): production inflation helpers after the first inline test module; `realized_inflation_index_tests` exercise known/future anchors and stale history. Those tests are behavioral, not vendor calibration evidence.
- [Deacon–Derry–Mirfendereski](../../../REFERENCES.md#deacon-derry-mirfendereski-2004) and [Kerkhof](../../../REFERENCES.md#kerkhof-2005) are canonical inflation-model context. The precise publication/default policy remains a repository choice awaiting approval.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
