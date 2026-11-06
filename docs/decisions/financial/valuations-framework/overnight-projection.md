# VALUATIONS-FRAMEWORK-04 — Overnight realized/projected splice and coupon derivatives

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in the source paths cited below.
- Validation status: Evidence inspected/located, not executed; no runtime checks or external accuracy comparison performed.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope, and rationale

The shared projector builds daily business-calendar observations for OIS, swap, TRS and related coupon consumers. It distinguishes arithmetic averaging, compounded arrears with lookback, observation-shift compounding, and rate cutoff. Each published observation is deterministic and each future observation is curve projected. The separation is necessary for seasoned coupons and forward risk; contractual selection of these conventions still requires agreement.

## Algorithm and conventions

1. Resolve the supplied fixing calendar strictly, or a currency default (USD usny, EUR target2, GBP gblo, JPY jpto, AUD auce, CAD cato, CHF chzh). Unsupported currencies need an explicit calendar. Accrual boundaries can be business-day adjusted independently by the caller.
2. Step from accrual start to the next business day, capped at end. For lookback, move observation endpoints backward by the specified business days but retain unshifted accrual weights. Observation shift moves both observation and weight endpoints. Rate cutoff reuses the observation immediately before the cutoff window for its remaining factors. The final fixing date is the last observation start, with the relevant shift/cutoff applied.
3. An observation strictly before `as_of` requires an exact fixing; one on `as_of` is projected in this routine. A forward curve uses `rate_period(t0,t1) × (t1−t0)/index_accrual`, so the forward curve's day-count axis and index accrual can differ. Projection before the forward curve's base date fails. A discount curve uses `(DF(start)/DF(end)−1)/index_accrual` through its date-based ratio.
4. With factor `f_i=1+(r_i+s_compounded)δ_i`, compute `P=∏f_i` and coupon rate `(P−1)/τ`. Every factor/product must remain positive and finite. The separate arithmetic spread is added as `N(P−1)+N s_arithmetic τ`; it is not compounded by this helper.
5. Let `e_i` be one for projected observations and zero for historical observations. Initialize `(P,D,D2)=(1,0,0)` and update simultaneously: `D2←D2 f_i+2D δ_i e_i`, `D←D f_i+P δ_i e_i`, `P←P f_i`. Output derivatives are `D/τ` and `D2/τ`. Individual forward derivatives use prefix/suffix products, `δ_i e_i ∏_{j≠i}f_j/τ`, avoiding division by a small factor.
6. Arithmetic averaging weights each rate by `δ_i/τ`; sensitivity is the sum of future-observation weights. A nonbusiness start uses the prior business observation for its partial first interval. This separate routine supports arithmetic futures; `SimpleAverage` is rejected by the shared compounded swap-coupon resolver.

## Fast path and controls

For a discount-curve projection with zero compounded spread, zero-lookback arrears and no requested observation exposures, projected products telescope to `DF(start)/DF(end)`. A seasoned coupon first multiplies exact historical factors, then appends the remaining discount ratio. This fast path returns zero derivative fields and an empty observation list; consumers needing derivatives must request exposures and use the explicit observation path. No stochastic-rate convexity adjustment is added here.

The explicit convention takes precedence over registry inference. Unknown registered index IDs yield no convention; known overnight indices reject simple term compounding when the validation helper is invoked. Oversized lookback/shift values fail conversion to signed day counts; the cutoff conversion uses `ok()` and consequently an unrepresentable positive cutoff is treated as absent. That asymmetry is observed, not an endorsed fallback.

A representable cutoff can span the entire coupon: the adapter subtracts cutoff business days from the end and references the preceding business observation without requiring that reference to lie inside accrual. This can freeze every factor to a pre-accrual fixing. The cashflows engine instead rejects cutoff at least as large as its compiled observation count. Valuations also projects over the complete observation-start to next-business-day interval, including weekend/holiday span; cashflows projects one calendar day then carries that rate across its weight. On a nonflat curve those raw observations need not agree even with identical accrual endpoints and no lookback. TRS exports such projected rates by observation date, so combining cross-engine schedules may produce conflicting same-series/date projections; that consequence is a source-based risk, not a reproduced failure.

## Alternatives, evidence

- [overnight.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing/overnight.rs): formulas, splice, fast path, derivative recurrence, validation and inline tests.
- [overnight_conventions.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing/overnight_conventions.rs): explicit/registry selection and unsupported arithmetic swap coupons.
- [ARRC SOFR guide](../../../REFERENCES.md#arrc-sofr-users-guide), [ISDA 2021](../../../REFERENCES.md#isda-2021-definitions), and [Ametrano–Bianchetti](../../../REFERENCES.md#ametrano-bianchetti-2013) are canonical convention/multicurve context, not evidence that every configured index uses the correct settings.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
