# CASHFLOWS-007 — Overnight observation grids, compounding and balance allocation

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed**, scoped to the cashflows overnight engine and builder emission. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: [slice audit](README.md).

## Choice and scope

The engine compiles business-day observations once and replays raw one-day fixings with calendar-day carry weights. It serves the cashflow builder and reusable bond replay. The source explicitly identifies a separate valuations overnight engine for swap-style products and states that it snaps accrual endpoints to business days, whereas this engine preserves a leading non-business-day slice. The [shared valuations record](../valuations-framework/overnight-projection.md) also identifies a projection-interval difference: valuations projects over the full observation interval, including weekend/holiday spans, while cashflows projects a one-calendar-day fixing and carries it over the weight. These rates can differ on a nonflat curve even with identical accrual endpoints. This record does not assert the engines are economically interchangeable.

## Observation algorithm and variants

1. Require ordered accrual bounds and an overnight method; `Simple` is the term-rate path and is rejected here. Zero-length periods can compile, but positive periods must yield a positive observation interval.
2. SimpleAverage and CompoundedInArrears use accrual endpoints as weight endpoints. In-arrears lookback moves each observation backward by the specified number of business days while retaining original weights.
3. ObservationShift moves both observation-window endpoints and the mapping of every partial accrual date backward by `shift_days` business days. Both observations and weights therefore move. Reject positive accrual/partial windows whose shifted observation interval is empty.
4. Compile business-day observations. Each fixing receives a simple-interest weight covering that business day and following nonbusiness days. A nonbusiness-day start uses the preceding business-day fixing (plus any lookback) for the leading slice. It is retained even when the whole coupon is a weekend stub.
5. RateCutoff leaves weights unchanged and replaces the last `cutoff_days` observation-slice dates with the preceding slice's observation date. The count is of compiled slices, not calendar days. Cutoff zero is unchanged; cutoff at least the number of slices is rejected because no preceding fixing remains.

Lookback/shift counts must fit the business-day arithmetic's signed integer range. JSON Schema marks these controls with a maximum of 31, but the runtime compiler does not enforce 31; it enforces representability/calendar viability. The valuations engine has a different short-coupon cutoff domain: a representable cutoff can freeze the whole coupon to a preceding pre-accrual fixing, while cashflows requires at least one uncut compiled observation. The shared enum does not imply identical accepted cutoff windows.

## Rate aggregation and numerical rules

With raw fixing `ri`, weight `di` actual days and basis `D`, compounded growth is `G=∏(1+ri di/D)` and annualized index is `(G-1)D/Σdi`. SimpleAverage uses `Σri di/Σdi`. Builder index basis must be ACT/360 or ACT/365F, selected by `overnight_basis` or inherited coupon day count. The public accumulator accepts any finite positive numeric basis. Published fixings stay in index convention. Projected observations use one-calendar-day curve rate `rate_period(t,t+1day) × curve_yf/(1/D)`; a weekend carry does not project a multi-day fixing.

Raw and constrained accumulators are maintained separately. Daily index constraints floor/cap each observation before compounding; Period constraints bound the final annualized raw rate. Spread, gearing and all-in constraints are applied after index aggregation. Require finite raw observations and positive finite simple-interest factors for compounded methods; reject nonfinite aggregated rates. Negative rates are allowed within the positive-factor domain.

## Partial replay and changing principal

An accumulator belongs to the exact compiled schedule instance. Advancement is monotone and bounded by contractual end, and clones state before work so errors do not partially mutate it. If a checkpoint splits a weekend carry, the next advance removes the prior partial factor and replaces it with a longer factor using the **same cached fixing**. It does not compound the pieces separately or refetch a changed fixing. A zero-weight prefix returns rate zero.

Builder emission preserves one cumulative clock across balance segments. Let `αj` be contractual accrual from coupon start through segment end and `Rj` its replayed annualized rate. Segment index increment is `(Rj αj − Rj−1 αj−1)/segment_yf`; zero-year-fraction segments produce `(0,0)` while still advancing the observation clock. This weights the compound increment by the segment's actual outstanding balance.

For period-level index or all-in constraints, replay prefixes without those period bounds. At full coupon end compute the constrained all-in rate minus its unbounded affine counterpart, divide that adjustment by gearing, and add the resulting index adjustment to every segment. Settlement then uses affine spread/gearing with bounds cleared. Thus the final period-bound adjustment is distributed uniformly over contractual accrual time, while raw compounded increments retain their timing. Daily index bounds remain inside the cumulative products. This is a substantive allocation choice for changing principal, not merely a cap/floor implementation detail.

## Alternatives and evidence

Supported alternatives are weighted simple average, lookback, observation shift, cutoff and daily/period index constraints. Restarting compounding at each balance change, capping every prefix or dropping weekend-start accrual are not this engine's behavior.

- [overnight.rs](../../../../finstack-quant/cashflows/src/builder/overnight.rs), [projection helpers](../../../../finstack-quant/cashflows/src/builder/rate_helpers.rs), [segment-rate allocation](../../../../finstack-quant/cashflows/src/builder/emission/coupons.rs), [compounding controls](../../../../finstack-quant/cashflows/src/builder/specs/coupon.rs).
- Inspected independent arithmetic examples in [overnight_replay.rs](../../../../finstack-quant/cashflows/tests/cashflows/builder/overnight_replay.rs): explicit weekend product weights and leading carry with one fixing invocation. Located partial-shift, cutoff, daily/period bounds and error-atomicity tests there. Inspected `overnight_period_bounds_use_final_rate_and_uniform_accrual_adjustment` and daily-bound formulas in [compiler_regressions.rs](../../../../finstack-quant/cashflows/tests/cashflows/builder/compiler_regressions.rs); these validate the chosen allocation, not an external market standard.
- Reference pointers: [ARRC SOFR guide](../../../REFERENCES.md#arrc-sofr-users-guide), [BoE SONIA](../../../REFERENCES.md#boe-sonia-key-features). Clause-level reconciliation, external published-rate fixtures and cross-engine parity across holiday starts were not established by this audit.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
